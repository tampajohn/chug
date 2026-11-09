//! T355 — `chug loop init/run --goal`: seed the queue from the spawn goal.
//!
//! The operator report (2026-11-09, first real herdr spawn): the fleet
//! dialog passed the goal in the SPAWN PROMPT, then ran `chug loop init &&
//! chug loop run` — init scaffolded an EMPTY TODO.md, the goal never
//! became a queue row, and the loop went dormant. These pins, at the
//! process level against the REAL binary (`CARGO_BIN_EXE_chug`):
//!
//! - `init --goal` seeds a parseable row (the todo_consistency 6-cell
//!   shape, id/pri/status/spec rules clean) citing a spec file that
//!   exists and carries a `check:` line the goal gate can extract;
//! - idempotency composes: the same goal twice is ONE row (second run
//!   reported `kept`); a different goal on an existing queue appends at
//!   the next id;
//! - `run --goal` on an empty queue seeds and starts work (the supervisor
//!   is exec'd); on a non-empty queue it exits 2 naming the conflict;
//! - the flag refusals (empty goal, both flag forms, unreadable file);
//! - bare `init` compat: no `--goal`, no mutation — the scaffold ships an
//!   empty queue exactly as before T355.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Spawn `chug loop <args>` in `cwd` with per-child env (never the test
/// process's global env — the parallel-threads rule). Waits for exit.
fn run_loop(args: &[&str], cwd: &Path, envs: &[(&str, &PathBuf)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("loop").args(args).current_dir(cwd);
    for (key, value) in envs {
        cmd.env(key, value);
    }
    cmd.output().expect("spawn chug loop")
}

fn out_text(out: &Output) -> String {
    format!(
        "--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A data row's cells (the todo_consistency shape: `|`-delimited, header
/// and dash-separator rows skipped), or `None` for non-table lines.
fn table_cells(raw: &str) -> Option<Vec<String>> {
    let line = raw.trim();
    if !line.starts_with('|') {
        return None;
    }
    let cells: Vec<String> = line
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect();
    if cells.first().map(String::as_str) == Some("id") {
        return None;
    }
    if cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-'))
    {
        return None;
    }
    Some(cells)
}

/// Every data row of the FIRST table in the file (the queue — everything
/// before the `## Proposed` lane heading).
fn queue_rows(todo: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for line in todo.lines() {
        if line.trim().starts_with("## Proposed") {
            break;
        }
        if let Some(cells) = table_cells(line) {
            rows.push(cells);
        }
    }
    rows
}

/// The todo_consistency rules a seeded row must satisfy: exactly 6 cells,
/// a `T<n>` id, an integer pri, a status from the SELF-SPEC vocabulary, a
/// spec ref of the id-coupled shape `specs/t<N>-<slug>.md` (`[a-z0-9-]+`
/// slug, N == the id number), and the spec file on disk.
fn assert_row_is_consistent(cells: &[String], todo: &str, root: &Path) {
    assert_eq!(
        cells.len(),
        6,
        "a seeded row carries exactly 6 cells (todo_consistency shape): {todo}"
    );
    let [id, title, spec, pri, status, _notes]: [String; 6] =
        cells.to_vec().try_into().expect("length checked above");
    let digits = id
        .strip_prefix('T')
        .filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or_else(|| panic!("id '{id}' must match ^T[0-9]+$: {todo}"));
    let id_num: u64 = digits.parse().expect("digits");
    assert!(
        !title.is_empty() && !title.contains('|') && !title.contains('\n'),
        "the title is a safe single-line cell: {todo}"
    );
    assert!(
        pri.parse::<i64>().is_ok(),
        "pri '{pri}' must be an integer: {todo}"
    );
    assert!(
        ["todo", "in-progress", "blocked", "done", "open", "proposed"].contains(&status.as_str()),
        "status '{status}' must be a SELF-SPEC state: {todo}"
    );
    // The id-coupled spec shape: specs/t<N>-<slug>.md, N == id number,
    // slug [a-z0-9-]+ (case-insensitive on the t), exactly as the T8
    // guard parses it.
    let rest = spec
        .strip_prefix("specs/")
        .unwrap_or_else(|| panic!("spec '{spec}' must live under specs/: {todo}"));
    let rest = rest
        .strip_prefix('t')
        .or_else(|| rest.strip_prefix('T'))
        .unwrap_or_else(|| panic!("spec '{spec}' must start specs/t<N>-: {todo}"));
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    assert_eq!(
        num.parse::<u64>().ok(),
        Some(id_num),
        "the spec's N must equal the row id ({spec} vs {id}): {todo}"
    );
    let slug = rest[num.len()..]
        .strip_prefix('-')
        .and_then(|s| s.strip_suffix(".md"))
        .unwrap_or_else(|| panic!("spec '{spec}' must be specs/t<N>-<slug>.md: {todo}"));
    assert!(
        !slug.is_empty()
            && slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "the slug must be [a-z0-9-]+: {spec}"
    );
    assert!(
        root.join(&spec).is_file(),
        "the spec file '{spec}' exists on disk (no dangling rows)"
    );
}

/// The T335 header-anchored extraction, re-implemented from the driver
/// contract: with a `## check:` header, the first `check:` line AFTER it
/// wins; the header line itself is never the command. Asserted on the
/// on-disk seed spec so the goal gate can never extract garbage.
fn assert_seed_spec_check_is_extractable(spec: &str, spec_rel: &str) {
    let anchored = spec
        .lines()
        .skip_while(|line| !line.trim().starts_with("## check:"))
        .skip(1)
        .find_map(|line| {
            line.trim()
                .strip_prefix("check:")
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(str::to_string)
        });
    let check = anchored.unwrap_or_else(|| {
        panic!("the T335 parser finds a check: line after the ## check: header: {spec}")
    });
    assert!(
        check.starts_with("test -s .chug/"),
        "the seeded check requires the operator-visible outcome artifact ({spec_rel}): {check}"
    );
}

/// The stubbed-exec seam: a supervisor script that records its physical
/// cwd into `$CHUG_LOOP_PROBE` and exits 0 — proof that `chug loop run`
/// exec'd SOMETHING with the repo as its cwd (the loop_subcommand shape).
fn write_stub(dir: &Path) -> PathBuf {
    let stub = dir.join("stub-supervisor.sh");
    std::fs::write(
        &stub,
        "#!/usr/bin/env bash\npwd -P > \"${CHUG_LOOP_PROBE:?}\"\n",
    )
    .expect("write stub supervisor");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .expect("chmod +x stub");
    }
    stub
}

// ---------- init --goal: the seeded row + its spec ----------

/// init --goal on a fresh repo: TODO.md gains ONE parseable row (6 cells,
/// todo_consistency clean) citing a spec file that exists, carries the
/// goal verbatim, and hands the goal gate a check: line.
#[test]
fn init_goal_seeds_a_parseable_row_and_spec() {
    let tmp = tempfile::tempdir().unwrap();
    let goal = "Fix the flaky login retry. The retry loop gives up after 1s.";
    let out = run_loop(&["init", "--goal", goal], tmp.path(), &[]);
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TODO.md seeded (T1"),
        "the seeding is reported as `seeded`: {stdout}"
    );
    assert!(
        stdout.contains("specs/t001-fix-the-flaky-login-retry.md"),
        "the generated spec path is reported: {stdout}"
    );

    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap();
    let rows = queue_rows(&todo);
    assert_eq!(rows.len(), 1, "exactly one seeded row:\n{todo}");
    assert_row_is_consistent(&rows[0], &todo, tmp.path());
    assert_eq!(rows[0][0], "T1", "a fresh queue seeds row 1: {todo}");
    assert_eq!(rows[0][1], "Fix the flaky login retry", "the title is the goal's first sentence: {todo}");
    assert_eq!(rows[0][4], "todo", "the seeded row is work: {todo}");
    // The row sits in the QUEUE table, never in the Proposed lane.
    let proposed = todo.find("## Proposed").expect("the lane survives");
    assert!(
        todo.find("Fix the flaky login retry").expect("row present") < proposed,
        "the seeded row lands before the Proposed lane:\n{todo}"
    );

    let spec_rel = &rows[0][2];
    let spec = std::fs::read_to_string(tmp.path().join(spec_rel)).unwrap();
    assert!(
        spec.contains(goal),
        "the spec carries the goal verbatim:\n{spec}"
    );
    assert!(spec.contains("## check:"), "the seed spec has the check section:\n{spec}");
    assert_seed_spec_check_is_extractable(&spec, spec_rel);
}

/// The seeded spec's check requires an outcome artifact that is
/// id-coupled — two seeded goals never share one check, so a stale
/// artifact from goal 1 cannot wave goal 2 through the gate.
#[test]
fn init_goal_two_rows_get_two_distinct_checks() {
    let tmp = tempfile::tempdir().unwrap();
    let first = run_loop(
        &["init", "--goal", "Fix the flaky login retry. it gives up too early"],
        tmp.path(),
        &[],
    );
    assert!(first.status.success(), "{}", out_text(&first));
    let second = run_loop(
        &["init", "--goal", "Add a --verbose flag to the launcher. print the config"],
        tmp.path(),
        &[],
    );
    assert!(second.status.success(), "{}", out_text(&second));
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("T2"),
        "the second goal appends at the next id: {stdout}"
    );
    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap();
    let rows = queue_rows(&todo);
    assert_eq!(rows.len(), 2, "two goals, two rows:\n{todo}");
    // Newest seed inserts at the top of the queue; both ids exist.
    assert_eq!(rows[0][0], "T2", "the second goal appends at the next id: {todo}");
    assert_eq!(rows[1][0], "T1");
    assert_ne!(rows[0][2], rows[1][2], "distinct spec paths");
    let check_of = |rel: &str| {
        let spec = std::fs::read_to_string(tmp.path().join(rel)).unwrap();
        let anchored = spec
            .lines()
            .skip_while(|l| !l.trim().starts_with("## check:"))
            .skip(1)
            .find_map(|l| l.trim().strip_prefix("check:").map(str::trim).map(str::to_string))
            .expect("a check line");
        (spec, anchored)
    };
    let (spec2, check2) = check_of(&rows[0][2]);
    let (_, check1) = check_of(&rows[1][2]);
    assert_ne!(check1, check2, "the checks are id-coupled: {check1} vs {check2}");
    assert!(check1.contains("t001"), "{check1}");
    assert!(check2.contains("t002"), "{check2}");
    assert_seed_spec_check_is_extractable(&spec2, &rows[0][2]);
}

// ---------- idempotency ----------

/// init --goal twice with the SAME goal: one row — the second run reports
/// `kept` and mutates nothing.
#[test]
fn init_goal_twice_reports_kept_and_keeps_one_row() {
    let tmp = tempfile::tempdir().unwrap();
    let goal = "Fix the flaky login retry. the retry loop gives up after 1s";
    let first = run_loop(&["init", "--goal", goal], tmp.path(), &[]);
    assert!(first.status.success(), "{}", out_text(&first));
    let todo_path = tmp.path().join("TODO.md");
    let before = std::fs::read(&todo_path).unwrap();

    let second = run_loop(&["init", "--goal", goal], tmp.path(), &[]);
    assert!(second.status.success(), "{}", out_text(&second));
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        stdout.contains("TODO.md kept") && stdout.contains("already seeded as T1"),
        "the repeat is reported `kept`, naming the existing row: {stdout}"
    );
    assert_eq!(
        std::fs::read(&todo_path).unwrap(),
        before,
        "the second seed mutates nothing"
    );
    assert_eq!(
        queue_rows(&std::fs::read_to_string(&todo_path).unwrap()).len(),
        1,
        "still exactly one row"
    );
}

/// init --goal on an already-scaffolded repo WITH rows: the row is
/// appended at the next id — the only mutation allowed on a `kept`
/// TODO.md — and the operator's own rows and specs are untouched.
#[test]
fn init_goal_on_an_existing_queue_appends_the_next_id() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    // Shape an existing queue: one open row (T4) + its spec + one done row.
    let todo_path = tmp.path().join("TODO.md");
    let todo = std::fs::read_to_string(&todo_path).unwrap();
    let marker = "|----|-------|------|-----|--------|-------|";
    let with_rows = todo.replacen(
        marker,
        &format!(
            "{marker}\n| T4 | existing work | specs/t004-existing.md | 2 | todo | hand written |\n| T3 | landed earlier | specs/t003-landed.md | 3 | done | merged |"
        ),
        1,
    );
    std::fs::write(&todo_path, with_rows).unwrap();
    std::fs::create_dir_all(tmp.path().join("specs")).unwrap();
    std::fs::write(tmp.path().join("specs/t004-existing.md"), "operator's own spec\n").unwrap();
    std::fs::write(tmp.path().join("specs/t003-landed.md"), "landed spec\n").unwrap();

    let out = run_loop(
        &["init", "--goal", "Watch the deploy pipeline. alert on failures"],
        tmp.path(),
        &[],
    );
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TODO.md seeded (T5"),
        "the row appends at the next id (max T4 + 1): {stdout}"
    );
    assert!(
        stdout.contains("TODO.md kept") || stdout.contains("kept"),
        "the scaffold itself is still reported kept: {stdout}"
    );

    let todo = std::fs::read_to_string(&todo_path).unwrap();
    let rows = queue_rows(&todo);
    assert_eq!(rows.len(), 3, "appended, not replaced:\n{todo}");
    assert_eq!(rows[0][0], "T5", "the seeded row is the next id: {todo}");
    assert_eq!(rows[1][0], "T4", "the operator's row survives: {todo}");
    assert_eq!(rows[2][0], "T3", "the done row survives: {todo}");
    assert_row_is_consistent(&rows[0], &todo, tmp.path());
    // The operator's own spec files are untouched.
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("specs/t004-existing.md")).unwrap(),
        "operator's own spec\n"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("specs/t003-landed.md")).unwrap(),
        "landed spec\n"
    );
}

// ---------- run --goal: seed-then-run, or refuse ----------

/// run --goal on an EMPTY queue: seeds it (the same seeding init does)
/// and starts work — the supervisor is exec'd against the repo, and the
/// seeded row is on disk before the handoff.
#[test]
fn run_goal_on_an_empty_queue_seeds_then_starts_work() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    let probe = tmp.path().join("probe.txt");
    let stub = write_stub(tmp.path());
    let goal = "Fix the flaky login retry. the loop must start work";
    let out = run_loop(
        &["run", "--goal", goal],
        tmp.path(),
        &[("CHUG_LOOPD_BIN", &stub), ("CHUG_LOOP_PROBE", &probe)],
    );
    assert!(out.status.success(), "{}", out_text(&out));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("TODO.md seeded (T1"),
        "the seeding rides the run's stderr: {stderr}"
    );
    let recorded = std::fs::read_to_string(&probe)
        .unwrap_or_else(|e| panic!("the supervisor was exec'd: {e}"));
    assert_eq!(
        PathBuf::from(recorded.trim()),
        std::fs::canonicalize(tmp.path()).unwrap(),
        "the supervisor ran with cwd = the repo"
    );

    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap();
    let rows = queue_rows(&todo);
    assert_eq!(rows.len(), 1, "the queue was seeded before the exec:\n{todo}");
    assert_row_is_consistent(&rows[0], &todo, tmp.path());
    let spec = std::fs::read_to_string(tmp.path().join(&rows[0][2])).unwrap();
    assert!(spec.contains(goal), "the spec carries the goal:\n{spec}");
    assert_seed_spec_check_is_extractable(&spec, &rows[0][2]);
}

/// run --goal on a NON-empty queue: exit 2 naming the conflict — never
/// silently append to a queue nobody read, and nothing is written.
#[test]
fn run_goal_on_a_non_empty_queue_exits_2() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));
    let todo_path = tmp.path().join("TODO.md");
    let todo = std::fs::read_to_string(&todo_path).unwrap();
    let marker = "|----|-------|------|-----|--------|-------|";
    std::fs::write(
        &todo_path,
        todo.replacen(
            marker,
            &format!("{marker}\n| T1 | live work | specs/t001-live.md | 1 | todo | in flight |"),
            1,
        ),
    )
    .unwrap();
    let before = std::fs::read(&todo_path).unwrap();

    let out = run_loop(
        &["run", "--goal", "a brand new goal. never seeded"],
        tmp.path(),
        &[],
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "a non-empty queue refuses --goal with exit 2: {}",
        out_text(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--goal") && stderr.contains("not empty"),
        "the refusal names the conflict: {stderr}"
    );
    assert_eq!(
        std::fs::read(&todo_path).unwrap(),
        before,
        "a refused run mutates nothing"
    );
    assert!(
        !tmp.path().join("specs/t002-a-brand-new-goal.md").exists(),
        "no spec is written beside a refused row"
    );
}

/// run --goal where the SAME goal was already seeded (row done, queue
/// drained): reported `kept`, no duplicate row, and the supervisor still
/// starts (the seeded state is the loop's business).
#[test]
fn run_goal_already_seeded_reports_kept_and_starts() {
    let tmp = tempfile::tempdir().unwrap();
    let goal = "Fix the flaky login retry. again";
    let init = run_loop(&["init", "--goal", goal], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));
    // Drain the queue the way a landed cycle would.
    let todo_path = tmp.path().join("TODO.md");
    let todo = std::fs::read_to_string(&todo_path).unwrap();
    std::fs::write(&todo_path, todo.replace("| 1 | todo |", "| 1 | done |")).unwrap();
    let before = std::fs::read(&todo_path).unwrap();

    let probe = tmp.path().join("probe.txt");
    let stub = write_stub(tmp.path());
    let out = run_loop(
        &["run", "--goal", goal],
        tmp.path(),
        &[("CHUG_LOOPD_BIN", &stub), ("CHUG_LOOP_PROBE", &probe)],
    );
    assert!(out.status.success(), "{}", out_text(&out));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("TODO.md kept") && stderr.contains("already seeded as T1"),
        "the same goal is `kept`, never a duplicate row: {stderr}"
    );
    assert_eq!(
        std::fs::read(&todo_path).unwrap(),
        before,
        "kept means nothing mutated"
    );
    assert!(probe.is_file(), "the supervisor still started");
}

// ---------- the flag refusals ----------

#[test]
fn goal_flag_refusals() {
    let tmp = tempfile::tempdir().unwrap();

    // Empty goal: refused before anything is written — no scaffold, no
    // seed (the no-half-starts rule: a broken flag does nothing at all).
    let out = run_loop(&["init", "--goal", "   "], tmp.path(), &[]);
    assert!(
        !out.status.success(),
        "an empty --goal refuses: {}",
        out_text(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("--goal"), "{stderr}");
    assert!(
        !tmp.path().join("TODO.md").exists(),
        "the refusal fires before any write: no scaffold, no seed"
    );

    // Both flag forms: refused.
    let goal_file = tmp.path().join("goal.txt");
    std::fs::write(&goal_file, "a goal from a file. for the long form\n").unwrap();
    let out = run_loop(
        &["init", "--goal", "x", "--goal-file", goal_file.to_str().unwrap()],
        tmp.path(),
        &[],
    );
    assert!(!out.status.success(), "{}", out_text(&out));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--goal") && stderr.contains("--goal-file"),
        "the refusal names both flags: {stderr}"
    );

    // Unreadable --goal-file: refused with the path named.
    let missing = tmp.path().join("no-such-goal.txt");
    let out = run_loop(&["init", "--goal-file", missing.to_str().unwrap()], tmp.path(), &[]);
    assert!(!out.status.success(), "{}", out_text(&out));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--goal-file") && stderr.contains("no-such-goal.txt"),
        "the refusal names the unreadable file: {stderr}"
    );

    // The file form seeds the same way the flag form does.
    let fresh = tempfile::tempdir().unwrap();
    let out = run_loop(
        &["init", "--goal-file", goal_file.to_str().unwrap()],
        fresh.path(),
        &[],
    );
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TODO.md seeded (T1") && stdout.contains("a goal from a file"),
        "the --goal-file form seeds identically: {stdout}"
    );
}

// ---------- bare-init compat ----------

/// Bare `chug loop init` behaves exactly as before T355: an EMPTY queue
/// (zero data rows), no specs/ dir, no seeding — only the new flag
/// mutates.
#[test]
fn bare_init_still_ships_an_empty_queue() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_loop(&["init"], tmp.path(), &[]);
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("TODO.md created") && !stdout.contains("seeded"),
        "no seeding on the bare path: {stdout}"
    );
    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap();
    assert_eq!(queue_rows(&todo).len(), 0, "the scaffold ships an empty queue: {todo}");
    assert!(
        !tmp.path().join("specs").exists(),
        "bare init creates no specs/ dir"
    );
}

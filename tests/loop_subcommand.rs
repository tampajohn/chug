//! T334 — the `chug loop` subcommand: init + run the loop in ANY repo,
//! no clone of chug required.
//!
//! Pins the spec's Tests list at the process level, against the REAL
//! binary (`CARGO_BIN_EXE_chug`):
//! - init in a temp dir creates the four artifacts; a second run reports
//!   `kept` for all and changes nothing; `--force` overwrites with `.bak`
//!   backups beside each replaced file;
//! - scaffold validity: the generated TODO.md parses under the
//!   todo_consistency table rules (6 cells, header + separator, an EMPTY
//!   queue — nothing to dangle against specs/) and the generated
//!   LOOP-SPEC.md carries the check:-contract sentence;
//! - run fixture: a missing scaffold exits 2 with the `run chug loop init
//!   first` hint and never half-starts (no `.chug/` write); with the
//!   scaffold, the supervisor is invoked with cwd = the temp repo —
//!   asserted via a stubbed exec (`CHUG_LOOPD_BIN`, the test/operator seam);
//! - the guard test: `chug loop run` inside a chug-repo-shaped checkout
//!   (its own doctrine LOOP-SPEC.md, its own queue, live loopd state, its
//!   own loopd.sh) leaves every one of chug's own files byte-identical —
//!   no behavior change for the existing setup;
//! - the generated supervisor, exec'd for real against a fresh scaffold,
//!   idles on the empty queue (no manufactured work, no cycle launched).

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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

/// The stubbed-exec seam: a supervisor script that records its physical
/// cwd into `$CHUG_LOOP_PROBE` and exits 0 — the cheapest possible proof
/// that `chug loop run` exec'd SOMETHING with the repo as its cwd.
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

fn same_bytes(a: &Path, b: &Path, label: &str) {
    let left = std::fs::read(a).unwrap_or_else(|e| panic!("read {label}: {e}"));
    let right = std::fs::read(b).unwrap_or_else(|e| panic!("read {label}: {e}"));
    assert_eq!(left, right, "{label} changed");
}

// ---------- init: the four artifacts ----------

/// init in a fresh temp dir creates the four artifacts and reports each
/// `created`: LOOP-SPEC.md (the generic contract), TODO.md (header +
/// empty queue + the Proposed lane), the `.chug/` runtime dir, and the
/// `.chug/` line in .gitignore (created when the file is absent).
#[test]
fn init_creates_the_four_artifacts() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_loop(&["init"], tmp.path(), &[]);
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    for artifact in ["LOOP-SPEC.md created", "TODO.md created", ".chug/ created"] {
        assert!(stdout.contains(artifact), "missing `{artifact}`: {stdout}");
    }

    let spec =
        std::fs::read_to_string(tmp.path().join("LOOP-SPEC.md")).expect("LOOP-SPEC.md written");
    assert!(
        spec.contains("check:") && spec.contains("re-runs and exits 0"),
        "the generic contract carries the check:-line contract: {spec}"
    );
    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).expect("TODO.md written");
    assert!(
        todo.contains("| id | title | spec | pri | status | notes |"),
        "the queue table header: {todo}"
    );
    assert!(todo.contains("## Proposed"), "the T326 lane header: {todo}");
    assert!(tmp.path().join(".chug").is_dir(), "the .chug/ runtime dir");
    let gitignore =
        std::fs::read_to_string(tmp.path().join(".gitignore")).expect(".gitignore written");
    assert!(
        gitignore.lines().any(|l| l.trim() == ".chug/"),
        ".chug/ ignored: {gitignore:?}"
    );
}

/// Idempotence: a second init reports `kept` for every artifact and
/// changes nothing on disk — existing files are never clobbered, and the
/// .gitignore append fires exactly once (no duplicate line).
#[test]
fn init_second_run_reports_kept_and_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let first = run_loop(&["init"], tmp.path(), &[]);
    assert!(first.status.success(), "{}", out_text(&first));

    let spec = tmp.path().join("LOOP-SPEC.md");
    let todo = tmp.path().join("TODO.md");
    let gitignore = tmp.path().join(".gitignore");
    let spec_before = std::fs::read(&spec).unwrap();
    let todo_before = std::fs::read(&todo).unwrap();
    let gi_before = std::fs::read(&gitignore).unwrap();

    let second = run_loop(&["init"], tmp.path(), &[]);
    assert!(second.status.success(), "{}", out_text(&second));
    let stdout = String::from_utf8_lossy(&second.stdout);
    for artifact in ["LOOP-SPEC.md kept", "TODO.md kept", ".chug/ kept"] {
        assert!(stdout.contains(artifact), "missing `{artifact}`: {stdout}");
    }
    assert_eq!(
        std::fs::read(&spec).unwrap(),
        spec_before,
        "a kept LOOP-SPEC.md is byte-identical"
    );
    assert_eq!(
        std::fs::read(&todo).unwrap(),
        todo_before,
        "a kept TODO.md is byte-identical"
    );
    let gi_after = std::fs::read(&gitignore).unwrap();
    assert_eq!(gi_after, gi_before, "a kept .gitignore is byte-identical");
    let count = String::from_utf8_lossy(&gi_after)
        .lines()
        .filter(|l| l.trim() == ".chug/")
        .count();
    assert_eq!(count, 1, "the .chug/ line is appended exactly once");
}

/// `--force` overwrites existing scaffolded files with the template and
/// backs each up as `<file>.bak` beside it — the operator's edits land in
/// the backup, the template bytes land back in the file. Files that do
/// not exist are simply created (no backup leg).
#[test]
fn init_force_overwrites_with_bak_backups() {
    let tmp = tempfile::tempdir().unwrap();
    let first = run_loop(&["init"], tmp.path(), &[]);
    assert!(first.status.success(), "{}", out_text(&first));

    // The operator edited both scaffolded files after init.
    std::fs::write(tmp.path().join("LOOP-SPEC.md"), "operator's own contract\n").unwrap();
    std::fs::write(tmp.path().join("TODO.md"), "operator's own queue\n").unwrap();

    let forced = run_loop(&["init", "--force"], tmp.path(), &[]);
    assert!(forced.status.success(), "{}", out_text(&forced));
    let stdout = String::from_utf8_lossy(&forced.stdout);
    assert!(
        stdout.contains("replaced (backup LOOP-SPEC.md.bak)"),
        "the replacement names its backup: {stdout}"
    );
    assert!(
        stdout.contains("replaced (backup TODO.md.bak)"),
        "the replacement names its backup: {stdout}"
    );

    // The templates are back, byte-identical to what init writes fresh.
    let fresh = tempfile::tempdir().unwrap();
    run_loop(&["init"], fresh.path(), &[]);
    same_bytes(
        &tmp.path().join("LOOP-SPEC.md"),
        &fresh.path().join("LOOP-SPEC.md"),
        "forced LOOP-SPEC.md",
    );
    same_bytes(
        &tmp.path().join("TODO.md"),
        &fresh.path().join("TODO.md"),
        "forced TODO.md",
    );

    // The backups hold the operator's edits.
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("LOOP-SPEC.md.bak")).unwrap(),
        "operator's own contract\n",
        "the .bak holds the replaced content"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("TODO.md.bak")).unwrap(),
        "operator's own queue\n",
        "the .bak holds the replaced content"
    );
}

// ---------- scaffold validity ----------

/// The generated TODO.md parses under the todo_consistency table rules:
/// every table line carries exactly 6 cells, the header and the dash
/// separator are well-formed, the scaffold ships an EMPTY queue (no data
/// rows — so the table is trivially consistent with specs/, nothing to
/// dangle), and the `## Proposed` lane carries its own table header.
#[test]
fn scaffold_todo_parses_under_the_todo_consistency_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_loop(&["init"], tmp.path(), &[]);
    assert!(out.status.success(), "{}", out_text(&out));
    let todo = std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap();

    let mut saw_header = false;
    let mut saw_separator = false;
    let mut data_rows = 0;
    for (idx, raw) in todo.lines().enumerate() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        assert_eq!(
            cells.len(),
            6,
            "line {} must carry exactly 6 cells (todo_consistency shape): {line}",
            idx + 1
        );
        if cells.first() == Some(&"id") {
            saw_header = true;
            continue;
        }
        if cells.iter().all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-')) {
            saw_separator = true;
            continue;
        }
        data_rows += 1;
    }
    assert!(saw_header, "the queue table header is present: {todo}");
    assert!(saw_separator, "the dash separator is present: {todo}");
    assert_eq!(data_rows, 0, "the scaffold ships an EMPTY queue: {todo}");
    assert!(
        todo.matches("| id | title | spec | pri | status | notes |").count() == 2,
        "both lanes carry the table header (queue + Proposed): {todo}"
    );
}

/// The generated LOOP-SPEC.md is the GENERIC minimal contract: the
/// check:-contract sentence verbatim, the queue rules, and the gate
/// expectations — and none of chug's self-referential doctrine (no
/// EVALUATION.md freshness rule, no chug-internal model routing), which
/// a pilot repo cannot obey.
#[test]
fn scaffold_loop_spec_carries_the_check_contract_and_gate_expectations() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_loop(&["init"], tmp.path(), &[]);
    assert!(out.status.success(), "{}", out_text(&out));
    let spec = std::fs::read_to_string(tmp.path().join("LOOP-SPEC.md")).unwrap();

    // The check:-contract sentence (the goal gate is the check re-running,
    // never a model's claim).
    assert!(
        spec.contains("A run only ends when the spec's `check:` command re-runs and exits 0"),
        "the check:-contract sentence: {spec}"
    );
    assert!(
        spec.contains("never when a model says it is done"),
        "the honesty half of the contract: {spec}"
    );
    // The queue rules and the gate expectations.
    assert!(spec.contains("## The queue"), "queue rules: {spec}");
    assert!(spec.contains("## The gates"), "gate expectations: {spec}");
    assert!(
        spec.contains("## Proposed") || spec.contains("Proposed"),
        "proposals route to the Proposed lane: {spec}"
    );
    // NOT chug's self-referential doctrine.
    assert!(
        !spec.contains("EVALUATION.md"),
        "the generic template carries no chug-repo eval doctrine: {spec}"
    );
}

// ---------- run: the exec wiring ----------

/// A missing scaffold exits 2 with the `run chug loop init first` hint —
/// and never half-starts: nothing is written (no `.chug/`, no generated
/// supervisor) before the refusal.
#[test]
fn run_without_scaffold_exits_2_with_the_init_hint_and_no_half_start() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_loop(&["run"], tmp.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "missing scaffold exits 2: {}",
        out_text(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("run chug loop init first"),
        "the hint names the remedy: {stderr}"
    );
    assert!(
        !tmp.path().join(".chug").exists(),
        "no half-starts: a refused run writes nothing"
    );
}

/// A partial scaffold is still missing: chug's own-repo shape has LOOP-SPEC.md
/// and TODO.md but the run must ALSO refuse when `.chug/` itself is absent
/// (the supervisor's state dir is part of the scaffold).
#[test]
fn run_refuses_a_partial_scaffold_naming_the_missing_piece() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("LOOP-SPEC.md"), "spec\ncheck: true\n").unwrap();
    std::fs::write(tmp.path().join("TODO.md"), "# TODO\n").unwrap();
    let out = run_loop(&["run"], tmp.path(), &[]);
    assert_eq!(out.status.code(), Some(2), "{}", out_text(&out));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(".chug/") && stderr.contains("run chug loop init first"),
        "the refusal names the missing piece: {stderr}"
    );
    assert!(!tmp.path().join(".chug").exists(), "no half-starts");
}

/// With the scaffold, the supervisor is invoked with cwd = the temp repo —
/// asserted via a stubbed exec (`CHUG_LOOPD_BIN`, the test/operator seam).
/// The stub records its physical cwd; the probe must read back the repo.
#[test]
fn run_with_scaffold_execs_the_supervisor_against_the_repo_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    let probe = tmp.path().join("probe.txt");
    let stub = write_stub(tmp.path());
    let out = run_loop(
        &["run"],
        tmp.path(),
        &[
            ("CHUG_LOOPD_BIN", &stub),
            ("CHUG_LOOP_PROBE", &probe),
        ],
    );
    assert!(out.status.success(), "{}", out_text(&out));

    let recorded = std::fs::read_to_string(&probe)
        .unwrap_or_else(|e| panic!("the stub exec'd and wrote the probe: {e}"));
    let expected = std::fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(
        PathBuf::from(recorded.trim()),
        expected,
        "the supervisor ran with cwd = the temp repo"
    );
    // The handoff line names which script took over.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("supervisor") && stderr.contains("stub-supervisor.sh"),
        "the handoff names the exec'd supervisor: {stderr}"
    );
}

/// An override that is set but unusable is a hard error — an override that
/// silently fell through to the generated script would run a different
/// supervisor than the operator asked for (the CHUG_DAEMON_BIN shape:
/// an explicit choice is never second-guessed, a broken one fails loud).
#[test]
fn run_with_an_unusable_override_fails_loud() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    let missing = tmp.path().join("no-such-supervisor.sh");
    let out = run_loop(&["run"], tmp.path(), &[("CHUG_LOOPD_BIN", &missing)]);
    assert!(
        !out.status.success(),
        "a bogus override must not fall through: {}",
        out_text(&out)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("CHUG_LOOPD_BIN"),
        "the failure names the knob: {stderr}"
    );
}

// ---------- the guard test: the chug repo itself ----------

/// Running `chug loop run` inside a chug-repo-shaped checkout still uses
/// chug's own files and changes none of them (no behavior change for the
/// existing setup): the run path never scaffolds (an operator-edited
/// LOOP-SPEC.md/TODO.md stay byte-identical), never touches live loopd
/// state (.chug/loopd/loopd.log), never touches chug's own loopd.sh — and
/// the exec still lands with cwd = the repo.
#[test]
fn run_inside_a_chug_shaped_repo_uses_its_own_files_without_touching_them() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    // Shape the fixture like the chug checkout: its own doctrine files,
    // live loopd state, its own supervisor at the root, a populated
    // .gitignore.
    std::fs::write(
        tmp.path().join("LOOP-SPEC.md"),
        "# chug's own doctrine LOOP-SPEC\ncheck: cargo test --release\n",
    )
    .unwrap();
    std::fs::write(tmp.path().join("TODO.md"), "# chug's own queue\n| T1 | x | y | 1 | done | z |\n").unwrap();
    std::fs::create_dir_all(tmp.path().join(".chug/loopd")).unwrap();
    std::fs::write(tmp.path().join(".chug/loopd/loopd.log"), "precious loopd state\n").unwrap();
    std::fs::write(
        tmp.path().join("loopd.sh"),
        "#!/usr/bin/env bash\n# chug's own supervisor\n",
    )
    .unwrap();
    std::fs::write(tmp.path().join(".gitignore"), "/target\n.chug/\n").unwrap();

    let probe = tmp.path().join("probe.txt");
    let stub = write_stub(tmp.path());
    let out = run_loop(
        &["run"],
        tmp.path(),
        &[("CHUG_LOOPD_BIN", &stub), ("CHUG_LOOP_PROBE", &probe)],
    );
    assert!(out.status.success(), "{}", out_text(&out));

    let expected = std::fs::canonicalize(tmp.path()).unwrap();
    assert_eq!(
        PathBuf::from(std::fs::read_to_string(&probe).unwrap().trim()),
        expected,
        "the supervisor ran with cwd = the repo"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("LOOP-SPEC.md")).unwrap(),
        "# chug's own doctrine LOOP-SPEC\ncheck: cargo test --release\n",
        "chug's own LOOP-SPEC.md is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("TODO.md")).unwrap(),
        "# chug's own queue\n| T1 | x | y | 1 | done | z |\n",
        "chug's own TODO.md is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".chug/loopd/loopd.log")).unwrap(),
        "precious loopd state\n",
        "live loopd state is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("loopd.sh")).unwrap(),
        "#!/usr/bin/env bash\n# chug's own supervisor\n",
        "chug's own loopd.sh is untouched"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        "/target\n.chug/\n",
        "the run path never rewrites .gitignore (that is init's job)"
    );
    // The derived assets were (re)generated into the gitignored runtime
    // dir and the generated supervisor works the repo's OWN contract file.
    let generated =
        std::fs::read_to_string(tmp.path().join(".chug/loopd/loopd.sh")).expect("generated");
    assert!(
        generated.contains("--spec LOOP-SPEC.md"),
        "the generated supervisor launches against the repo's own LOOP-SPEC.md"
    );
    assert!(
        tmp.path().join(".chug/loopd/scripts/loopd_env_loader.sh").is_file()
            && tmp.path().join(".chug/loopd/scripts/loopd_profile_loader.sh").is_file(),
        "both T205/T330 loader fragments ride the generated supervisor"
    );
}

// ---------- the generated supervisor, exec'd for real ----------

/// The generated supervisor, exec'd for real against a fresh scaffold,
/// idles on the empty queue: it writes its startup line (or reports a
/// driver-active skip on a host where another LOOP-SPEC driver is live),
/// launches NO cycle (no cycle-*.log), and keeps sleeping — a pilot's
/// first experience is their OWN queue waited on, never manufactured
/// work. Bounded wait + kill (the T6 stub-harness rule: a wedged child
/// fails the test in seconds, it can never hang the suite).
#[test]
fn generated_supervisor_idles_on_an_empty_queue_and_launches_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let init = run_loop(&["init"], tmp.path(), &[]);
    assert!(init.status.success(), "{}", out_text(&init));

    let stderr_log = std::fs::File::create(tmp.path().join("supervisor-stderr.log"))
        .expect("create the supervisor stderr capture");
    let mut child = Command::new(env!("CARGO_BIN_EXE_chug"))
        .args(["loop", "run"])
        .current_dir(tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::from(stderr_log))
        .spawn()
        .expect("spawn chug loop run (execs the generated supervisor)");

    let log = tmp.path().join(".chug/loopd/loopd.log");
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut log_text = String::new();
    loop {
        if let Ok(text) = std::fs::read_to_string(&log) {
            log_text = text;
            if log_text.contains("queue empty") || log_text.contains("another LOOP-SPEC driver active") {
                break;
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stderr = std::fs::read_to_string(tmp.path().join("supervisor-stderr.log"))
                .unwrap_or_default();
            panic!(
                "the generated supervisor never reached its idle state within 60s; log: {log_text}\nstderr: {stderr}"
            );
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    // Either idle leg is a no-launch outcome; a cycle log would mean a
    // launch happened — the empty-queue gate forbids it.
    assert!(
        !log_text.contains("cycle start"),
        "no cycle launched on an empty queue: {log_text}"
    );
    let cycles = tmp.path().join(".chug/loopd");
    let launched = std::fs::read_dir(&cycles)
        .expect("loopd state dir")
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().starts_with("cycle-"));
    assert!(!launched, "no cycle-*.log exists — nothing was launched");
    let _ = child.kill();
    let _ = child.wait();
}

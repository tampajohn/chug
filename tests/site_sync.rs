//! T98 — site-sync integration tests.
//!
//! `scripts/site-sync.sh` rewrites ONLY the `<!-- STATS:BEGIN/END -->` region
//! of the site's index.html from the chug repo's own facts (TODO.md rows, git
//! log gate counts + landed-item commits, .chug/loopd logs, EVALUATION.md).
//! Every number it publishes must be provably the number those sources
//! contain, so these tests pin it against fixture repos — NEVER against the
//! live site clone, which the script must not be pointed at from tests (the
//! first live run is loopd's, post-merge).
//!
//! Runs the real script via bash (no reimplementation: the tests guard the
//! script, they do not duplicate it).

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const NOW: &str = "2026-10-01";

fn script_path() -> PathBuf {
    // T48: resolve at runtime — compile-time env! paths break under the T47
    // shared cache, where cwd is the package root but CARGO_MANIFEST_DIR lies.
    std::env::current_dir()
        .expect("cargo sets the test cwd to the package root")
        .join("scripts/site-sync.sh")
}

fn git(dir: &Path, args: &[&str], date: Option<&str>) -> Output {
    let mut c = Command::new("git");
    c.current_dir(dir).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@e");
    c.env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@e");
    if let Some(d) = date {
        let stamp = format!("{d}T12:00:00Z");
        c.env("GIT_AUTHOR_DATE", &stamp).env("GIT_COMMITTER_DATE", &stamp);
    }
    for a in args {
        c.arg(a);
    }
    let out = c.output().unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// Stage everything and commit; returns the new short hash.
fn commit(dir: &Path, subject: &str, body: Option<&str>, date: &str, allow_empty: bool) -> String {
    git(dir, &["add", "-A"], None);
    let mut args: Vec<String> = vec!["commit".into(), "-q".into()];
    if allow_empty {
        args.push("--allow-empty".into());
    }
    args.push("-m".into());
    args.push(subject.into());
    if let Some(b) = body {
        args.push("-m".into());
        args.push(b.into());
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    git(dir, &refs, Some(date));
    String::from_utf8_lossy(&git(dir, &["rev-parse", "--short", "HEAD"], None).stdout)
        .trim()
        .to_string()
}

/// The chug repo half of the fixture: EVALUATION.md, loopd logs, a 2-done/1-todo
/// TODO.md, two landed-item commits (t3 gate "full suite 40 unit" — the T238
/// labeled-full-suite leg, below the floor, and t4 gate "nextest 555/555" —
/// equal operands above the T238 floor) and a newest gate-less chore commit
/// (walk-back leg).
fn chug_fixture(dir: &Path) {
    let loopd = dir.join(".chug/loopd");
    std::fs::create_dir_all(&loopd).unwrap();
    std::fs::write(
        loopd.join("loopd.log"),
        "2026-09-25T10:00:00Z cycle OK: first\n2026-09-26T10:00:00Z cycle OK: second\n",
    )
    .unwrap();
    std::fs::write(dir.join("EVALUATION.md"), "# EVALUATION — fixture\n").unwrap();
    commit(dir, "eval: cycle 1", None, "2026-09-19", false);
    std::fs::write(
        dir.join("TODO.md"),
        concat!(
            "# TODO\n\n| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T3 | first | specs/t3.md | 1 | done | x |\n",
            "| T4 | second | specs/t4.md | 1 | done | y |\n",
            "| T5 | pend | specs/t5.md | 2 | todo | |\n"
        ),
    )
    .unwrap();
    commit(
        dir,
        "t3: first item lands in the loop",
        Some("full suite 40 unit (was 38, +2) + integration, 0 failed"),
        "2026-09-20",
        false,
    );
    commit(
        dir,
        "t4: second item lands in the loop",
        Some("review+post-merge nextest 555/555 + clippy"),
        "2026-09-21",
        true,
    );
    commit(dir, "chore: not an item", Some("no gates here"), "2026-09-21", true);
}

/// The site half: a git repo whose index.html carries the marked (stale)
/// stats region between sentinels. Returns (short-hash-of-initial-commit, page).
fn site_fixture(dir: &Path) -> String {
    std::fs::write(
        dir.join("index.html"),
        concat!(
            "<html><body>\n<p>sentinel-before</p>\n",
            "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n",
            "<!-- STATS:END -->\n<p>sentinel-after</p>\n</body></html>\n"
        ),
    )
    .unwrap();
    commit(dir, "initial page", None, "2026-09-18", false)
}

struct Fixture {
    _keep: tempfile::TempDir,
    chug: PathBuf,
    site: PathBuf,
}

fn fixture() -> Fixture {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    chug_fixture(&chug);
    site_fixture(&site);
    Fixture { _keep: keep, chug, site }
}

/// Run the real script against the fixture repos (never the live clone).
fn run_sync(f: &Fixture, no_push: bool) -> Output {
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&f.chug);
    c.env("CHUG_SYNC_NOW", NOW);
    // T327: every fixture test drives the cost collector through the
    // injectable CHUG_COST_CH_CMD — never docker (this box may or may not
    // have a live Langfuse ClickHouse; the fixtures must not care). A failing
    // stub exercises the fail-closed leg; the T327 tests below override it
    // with their own canned collector commands.
    c.env("CHUG_COST_CH_CMD", "false");
    if no_push {
        c.env("CHUG_SITE_SYNC_NO_PUSH", "1");
    }
    c.output().expect("spawn scripts/site-sync.sh")
}

fn page(f: &Fixture) -> String {
    std::fs::read_to_string(f.site.join("index.html")).expect("read index.html")
}

fn sync_commits(f: &Fixture) -> usize {
    let out = git(&f.site, &["log", "--format=%s"], None);
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("site: stats sync "))
        .count()
}

#[test]
fn block_contents_match_the_named_sources() {
    let f = fixture();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync exited {:?}: {}", out.status.code(), String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    // items landed = done rows / total rows in the fixture TODO.md (2 of 3)
    assert!(html.contains("<b>2/3</b>"), "done/total card:\n{html}");
    assert!(html.contains("done rows in TODO.md, 3 item rows total"));
    // current test count = newest FULL-suite gate count (t238): t4's nextest
    // 555/555 — equal operands above the floor — NOT t3's older labeled
    // "full suite 40 unit", NOT the gate-less chore HEAD
    assert!(html.contains("<b>555</b>"), "tests card:\n{html}");
    assert!(!html.contains("<b>40</b>"), "older gate count must lose to the newest:\n{html}");
    assert!(html.contains("newest full-suite gate count in a commit message"));
    // cycle count = "cycle OK" lines in .chug/loopd/loopd.log
    assert!(html.contains("<b>2</b>"));
    assert!(html.contains("\"cycle OK\" lines in .chug/loopd/loopd.log"));
    // EVALUATION.md's last git-log write date
    assert!(html.contains("<b>2026-09-19</b>"), "eval card:\n{html}");
    // last-5 landed: refs + dates from git log, newest first, chore excluded
    assert!(html.contains("<code>T4</code>"), "landed list:\n{html}");
    assert!(html.contains("<code>T3</code>"));
    assert!(html.contains("2026-09-21 — second item lands in the loop"));
    assert!(html.contains("2026-09-20 — first item lands in the loop"));
    assert!(!html.contains("chore: not an item"), "non-item commits are not landed items:\n{html}");
    // only the marked region moved: sentinels and exactly one marker pair survive
    assert!(html.contains("<p>sentinel-before</p>") && html.contains("<p>sentinel-after</p>"));
    assert_eq!(html.matches("<!-- STATS:BEGIN -->").count(), 1);
    assert_eq!(html.matches("<!-- STATS:END -->").count(), 1);
    // req 2: the change is committed with the pinned sync date — exactly once
    assert_eq!(sync_commits(&f), 1, "exactly one sync commit");
}

#[test]
fn second_run_is_byte_identical_and_makes_no_second_commit() {
    let f = fixture();
    assert!(run_sync(&f, true).status.success());
    let first = page(&f);
    let out = run_sync(&f, true);
    assert!(out.status.success());
    let second = page(&f);
    assert_eq!(first, second, "unchanged inputs must be byte-identical (req 1)");
    assert!(String::from_utf8_lossy(&out.stdout).contains("unchanged"));
    assert_eq!(sync_commits(&f), 1, "idempotent run must not commit again");
}

#[test]
fn absent_markers_report_and_exit_nonzero_without_editing() {
    let f = fixture();
    std::fs::write(f.site.join("index.html"), "<html><div class=\"stats\"><b>1</b></div></html>\n").unwrap();
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let out = run_sync(&f, true);
    assert_eq!(out.status.code(), Some(3), "marker-less page must exit nonzero");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("STATS:BEGIN"), "must say what is missing: {err}");
    assert_eq!(std::fs::read(f.site.join("index.html")).unwrap(), before, "must NOT edit");
    assert_eq!(sync_commits(&f), 0, "no sync commit without markers");
}

#[test]
fn malformed_single_marker_is_rejected_without_editing() {
    let f = fixture();
    std::fs::write(f.site.join("index.html"), "a\n<!-- STATS:BEGIN -->\nx\n").unwrap();
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let out = run_sync(&f, true);
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(std::fs::read(f.site.join("index.html")).unwrap(), before);
}

/// T142: the cycle-count fallback (loopd.log missing, cycle logs present)
/// must count the supervisor's rc-based verdict stamps — never raw child
/// bytes. Model text reaches a cycle log verbatim, so the pre-fix fallback
/// (`grep -l 'chug: goal complete' cycle-*.log`) counted a failed cycle
/// whose model SPOOFED the marker; the stamp exists only when loopd itself
/// observed exit 0.
#[test]
fn cycle_count_fallback_counts_verdict_stamps_not_spoofable_markers() {
    let f = fixture();
    // The fallback path: loopd.log absent, cycle logs present.
    std::fs::remove_file(f.chug.join(".chug/loopd/loopd.log")).unwrap();
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260927-100000.log"),
        "[chug] model: look for `chug: goal complete` in the log\nchug: goal complete\n",
    )
    .unwrap(); // spoofed marker, no stamp — a failed cycle; must NOT count
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260928-100000.log"),
        "chug: goal complete\nsummary: real\n[loopd 2026-09-28T10:00:00Z] verdict: goal complete (rc=0)\n",
    )
    .unwrap(); // stamped OK — counts
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260928-110000.log"),
        "[loopd 2026-09-28T11:00:00Z] verdict: no goal complete (rc=1)\n",
    )
    .unwrap(); // failed stamp — must NOT count
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync exited {:?}", out.status.code());
    let html = page(&f);
    assert!(
        html.contains("<b>1</b><span>cycles completed by the loopd supervisor — cycle logs whose final verdict stamp reached goal complete"),
        "fallback must count exactly the stamped cycle (spoofed marker and \
         failed stamp excluded):\n{html}"
    );
}

/// T142 fix-up F1 (validator FAIL on 3341658): the fallback greps
/// `verdict: goal complete (rc=0)` over cycle logs that contain RAW CHILD
/// BYTES (stderr verbatim; the abort path puts model-written ledger text on
/// stdout), so the marker was RENAMED, not made unforgeable. The validator's
/// real probe: a forged `verdict: goal complete (rc=0)` line in the child
/// bytes of a FAILED cycle is counted — the page published "cycles 2" for
/// one real OK cycle. The fix: count only the LAST verdict line per cycle
/// log — the supervisor stamps after the child is fully dead and writes
/// nothing after, so the last stamp is supervisor-written and a forged one
/// is necessarily followed by the real stamp.
#[test]
fn fallback_ignores_a_forged_verdict_stamp_in_a_failed_cycles_child_bytes() {
    let f = fixture();
    // The fallback path: loopd.log absent, cycle logs present.
    std::fs::remove_file(f.chug.join(".chug/loopd/loopd.log")).unwrap();
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260927-100000.log"),
        concat!(
            "chug: goal complete\nsummary: real\n",
            "[loopd 2026-09-27T10:00:00Z] verdict: goal complete (rc=0)\n"
        ),
    )
    .unwrap(); // the one real OK cycle — counts
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260928-100000.log"),
        concat!(
            "[chug] iteration 2 / 200 (6 messages)\n",
            "[chug] model: wrapping up — see the ledger below\n",
            // Raw child bytes: the model's ledger text (abort-path stdout)
            // forges BOTH stamp shapes — prefixed and bare.
            "--- LEDGER.md ---\n",
            "## Done\n- verdict: goal complete (rc=0)\n",
            "[loopd 2026-09-28T10:00:00Z] verdict: goal complete (rc=0)\n",
            "---\n",
            "[chug] abort: budget exhausted (max-minutes)\n",
            // The supervisor's REAL stamp, written after child death — last.
            "[loopd 2026-09-28T10:05:00Z] verdict: no goal complete (rc=1)\n"
        ),
    )
    .unwrap(); // FAILED cycle with a forged stamp in its bytes — must NOT count
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync exited {:?}", out.status.code());
    let html = page(&f);
    assert!(
        html.contains("<b>1</b><span>cycles completed by the loopd supervisor"),
        "a forged verdict stamp in a FAILED cycle's child bytes must not \
         inflate the public cycle count (validator probe: pre-fix fallback \
         published 'cycles 2' for one real OK cycle):\n{html}"
    );
    assert!(
        !html.contains("<b>2</b><span>cycles completed"),
        "the forged-stamp cycle was counted:\n{html}"
    );
}

/// The sweep leg under the same rule: a forged stamp is the log's last
/// verdict line only while the supervisor has not stamped yet — every
/// stamp-supervised log ends on the supervisor's word, whatever the child
/// printed. A cycle the supervisor stamped as failed NEVER counts, even
/// when its child bytes end on a forged goal-complete line.
#[test]
fn fallback_counts_only_the_last_verdict_line_per_cycle_log() {
    let f = fixture();
    std::fs::remove_file(f.chug.join(".chug/loopd/loopd.log")).unwrap();
    // Failed cycle: forged stamp is the last CHILD line, the supervisor's
    // failure stamp follows — the last verdict line is the failure.
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260927-100000.log"),
        concat!(
            "chug: goal complete\n",
            "verdict: goal complete (rc=0)\n",
            "[loopd 2026-09-27T10:00:00Z] verdict: no goal complete (rc=2)\n"
        ),
    )
    .unwrap();
    // OK cycle: child ALSO forged a failure-shaped line first; the
    // supervisor's success stamp is last — counts, exactly once.
    std::fs::write(
        f.chug.join(".chug/loopd/cycle-20260927-110000.log"),
        concat!(
            "model text claiming verdict: no goal complete (rc=1)\n",
            "[loopd 2026-09-27T11:00:00Z] verdict: goal complete (rc=0)\n"
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync exited {:?}", out.status.code());
    let html = page(&f);
    assert!(
        html.contains("<b>1</b><span>cycles completed by the loopd supervisor"),
        "count = cycle logs whose LAST verdict line stamps goal complete \
         (rc=0) — forged lines before the stamp never decide:\n{html}"
    );
}

/// Class-sweep leg for the PRIMARY cycle count: loopd.log is
/// supervisor-written, but child bytes reach it through the single-line
/// summary interpolation — a model-forged summary TEXT naming ` cycle OK:`
/// must not multiply the count. The count is over LINES (one supervisor
/// line per OK cycle), never over occurrences.
#[test]
fn primary_cycle_count_is_line_anchored_not_occurrence_counting() {
    let f = fixture();
    // One real OK cycle whose (model-forged) summary text embeds the marker
    // twice — the shape loopd actually writes for such a run (one line).
    std::fs::write(
        f.chug.join(".chug/loopd/loopd.log"),
        "2026-09-28T10:00:00Z cycle OK: summary: wrapped T9 — cycle OK: fake, cycle OK: fake again\n",
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync exited {:?}", out.status.code());
    let html = page(&f);
    assert!(
        html.contains("<b>1</b><span>cycles completed by the loopd supervisor"),
        "the cycle count counts supervisor LINES, never marker occurrences \
         inside a (model-forged) summary:\n{html}"
    );
}

#[test]
fn missing_site_dir_warns_and_exits_zero() {
    let f = fixture();
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(f._keep.path().join("no-such-site")).arg(&f.chug);
    let out = c.output().expect("spawn");
    assert!(out.status.success(), "req 5: missing clone is a warning, exit 0");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not found"), "warn names the path: {err}");
}

#[test]
fn rejected_push_warns_and_exits_zero() {
    let f = fixture();
    // a remote pointing at a nonexistent local path: push fails immediately,
    // offline, with no network — the exact "rejected push" leg of req 2
    git(&f.site, &["remote", "add", "origin", "/tmp/t98-fixture-dead-remote.git"], None);
    let out = run_sync(&f, false);
    assert!(out.status.success(), "rejected push must NOT fail the cycle: {:?}", out.status.code());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("REJECTED"), "warn about the rejected push: {err}");
    assert_eq!(sync_commits(&f), 1, "the commit itself still landed");
}

#[test]
fn new_inputs_regenerate_the_block_and_commit_again() {
    let f = fixture();
    assert!(run_sync(&f, true).status.success());
    // T5 lands: TODO.md flips to 3/3 and a new gate count arrives (42 unit)
    std::fs::write(
        f.chug.join("TODO.md"),
        concat!(
            "# TODO\n\n| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T3 | first | specs/t3.md | 1 | done | x |\n",
            "| T4 | second | specs/t4.md | 1 | done | y |\n",
            "| T5 | pend | specs/t5.md | 2 | done | landed |\n"
        ),
    )
    .unwrap();
    let t5 = commit(
        &f.chug,
        "t5: third item lands in the loop",
        Some("gates: full suite 42 unit (was 40, +2), 0 failed"),
        "2026-09-22",
        false,
    );
    // a fresh sync date so the second commit is distinguishable
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&f.chug);
    c.env("CHUG_SYNC_NOW", "2026-10-03").env("CHUG_SITE_SYNC_NO_PUSH", "1");
    let out = c.output().expect("spawn");
    assert!(out.status.success());
    let html = page(&f);
    assert!(html.contains("<b>3/3</b>"), "updated done/total:\n{html}");
    assert!(html.contains("<b>42</b>"), "updated gate count:\n{html}");
    assert!(html.contains(&t5), "new ref quoted: {html}");
    assert_eq!(sync_commits(&f), 2, "changed inputs => a second sync commit");
}

// --- T99 — timeline + feature-grid regions -----------------------------------

/// A chug fixture with a done-row TODO.md whose notes cite real commits
/// (hashes come back from `commit`), plus a FEATURES.md whose landed rows are
/// split so each landed-detection leg is pinned independently: F1 is checked
/// off by STRIKETHROUGH only (~~, no LANDED text) and F4 by LANDED annotation
/// only (no ~~); F2/F3 are queued/in-flight material.
/// refs: [0] seed (hand-cited on the site), [1..=4] t5..t8, [5] t11.
fn chug_fixture_t99(dir: &Path) -> Vec<String> {
    std::fs::create_dir_all(dir.join(".chug/loopd")).unwrap(); // T217 guard: readable inputs
    std::fs::write(dir.join("EVALUATION.md"), "# EVALUATION — fixture\n").unwrap();
    std::fs::write(dir.join("TODO.md"), "# TODO\n").unwrap();
    std::fs::write(dir.join("FEATURES.md"), "# FEATURES\n").unwrap();
    commit(dir, "seed: empty files", None, "2026-09-19", false);
    let mut refs = vec![String::from_utf8_lossy(
        &git(dir, &["rev-parse", "--short", "HEAD"], None).stdout,
    )
    .trim()
    .to_string()];
    let items = [
        ("t5: alpha item lands", "2026-09-20"),
        ("t6: beta item lands", "2026-09-21"),
        ("t7: gamma item lands", "2026-09-22"),
        ("t8: delta item lands", "2026-09-22"),
        ("t11: long title item", "2026-09-23"),
    ];
    for (i, (subject, date)) in items.iter().enumerate() {
        std::fs::write(dir.join(format!("f{i}")), "x\n").unwrap();
        refs.push(commit(dir, subject, None, date, true));
    }
    std::fs::write(
        dir.join("TODO.md"),
        format!(
            concat!(
                "# TODO\n\n| id | title | spec | pri | status | notes |\n",
                "|----|-------|------|-----|--------|-------|\n",
                "| T5 | alpha item lands | specs/t5.md | 1 | done | done {} — landed |\n",
                "| T6 | beta item lands | specs/t6.md | 1 | done | done {} (impl x) — landed |\n",
                "| T7 | gamma item lands | specs/t7.md | 1 | done | done {} — landed |\n",
                "| T8 | delta item lands | specs/t8.md | 1 | done | done {} — landed |\n",
                "| T9 | ghost item | specs/t9.md | 1 | done | done cafe888 — no such commit |\n",
                "| T10 | fallback item | specs/t10.md | 1 | done | done 0badc0de (real {}) — fallback |\n",
                "| T11 | an extraordinarily long item title that keeps going well past one hundred bytes of raw single-sentence text and so must truncate | specs/t11.md | 1 | done | done {} — landed |\n",
                "| T12 | pending item | specs/t12.md | 2 | todo | references F2 phase 1 |\n"
            ),
            refs[1], refs[2], refs[3], refs[4], refs[0], refs[5]
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("FEATURES.md"),
        concat!(
            "# FEATURES\n\n",
            "| # | Feature | What | Benchmark |\n",
            "|---|---------|------|-----------|\n",
            "| F1 | ~~**delegate collect**~~ (cycle 33) | Structured child result: `goal_complete` summary + refs. | Claude Code Task |\n",
            "| F2 | **Streaming UX** | Text deltas to `sinks` as they arrive (TUI live typing). | all benchmarks |\n",
            "| F3 | **Web search** | Provider-pluggable search tool complementing web_fetch, with a deliberately long one-line what that runs well past the one hundred and sixty byte cap on purpose so the ellipsis lands. | Claude Code |\n",
            "| F4 | **Gamma guard** — LANDED T44 | Compiled-in guard rails for the loop. | n/a |\n"
        ),
    )
    .unwrap();
    commit(dir, "todo: t99 fixture rows", None, "2026-09-24", true);
    refs
}

/// The site half with a hand-built timeline block and feature grid, no
/// markers — the bootstrap leg of req 4. `{seed}` is replaced with refs[0].
fn site_fixture_t99(dir: &Path, seed_ref: &str) -> String {
    let page = format!(
        concat!(
            "<html><body>\n<p>sentinel-before</p>\n",
            "<div class=\"tl\">\n",
            "    <div class=\"tl-item major\">\n",
            "      <div class=\"tl-date\">2026-09-20</div>\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>Day one <span class=\"hash\">f911488</span></h3>\n",
            "        <p>Hand-written prose.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "    <div class=\"tl-item\">\n",
            "      <div class=\"tl-date\">2026-09-21</div>\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>delegate — chug spawns chug <span class=\"hash\">{seed}</span></h3>\n",
            "        <p>More hand prose.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "</div>\n",
            "<p>middle</p>\n",
            "<div class=\"grid\">\n",
            "    <div class=\"card\">\n",
            "      <h3><span>delegate</span> sub-agents</h3>\n",
            "      <p>Launch and collect child runs.</p>\n",
            "    </div>\n",
            "    <div class=\"card\">\n",
            "      <h3><span>tools</span> ×13</h3>\n",
            "      <p>read_file etc.</p>\n",
            "    </div>\n",
            // T99 fix-up finding 1: a card with NO <p> (h3 + <ul> body) must
            // survive regeneration byte-for-byte — the old flush_card gate
            // silently dropped exactly this shape.
            "    <div class=\"card\">\n",
            "      <h3><span>misc</span> extras</h3>\n",
            "      <ul>\n",
            "        <li>read_file</li>\n",
            "        <li>web_fetch</li>\n",
            "      </ul>\n",
            "    </div>\n",
            "</div>\n",
            "<p>sentinel-after</p>\n",
            "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n<!-- STATS:END -->\n",
            "</body></html>\n"
        ),
        seed = seed_ref
    );
    std::fs::write(dir.join("index.html"), &page).unwrap();
    commit(dir, "initial page", None, "2026-09-18", false);
    page
}

fn fixture_t99() -> (Fixture, Vec<String>, String) {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    let refs = chug_fixture_t99(&chug);
    let original = site_fixture_t99(&site, &refs[0]);
    (
        Fixture {
            _keep: keep,
            chug,
            site,
        },
        refs,
        original,
    )
}

fn region(html: &str, name: &str) -> String {
    let b = format!("<!-- {name}:BEGIN -->");
    let e = format!("<!-- {name}:END -->");
    let s = html.find(&b).expect("begin marker") + b.len();
    let t = html[s..].find(&e).expect("end marker") + s;
    html[s..t].to_string()
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

fn all_commit_subjects(f: &Fixture) -> Vec<String> {
    let out = git(&f.site, &["log", "--format=%s"], None);
    String::from_utf8_lossy(&out.stdout).lines().map(String::from).collect()
}

#[test]
fn t99_timeline_bootstrap_entries_dedupe_and_audit() {
    let (f, refs, _orig) = fixture_t99();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run 1 must succeed: {out:?}");
    let html = page(&f);
    // req 4: markers bootstrapped around the existing blocks, own commits
    assert_eq!(count(&html, "<!-- TIMELINE:BEGIN -->"), 1);
    assert_eq!(count(&html, "<!-- TIMELINE:END -->"), 1);
    // finding 2 (M6 mutant-killer): markers wrap the block's INNER content —
    // TIMELINE:BEGIN sits AFTER the <div class="tl"> open (inside the styled
    // container, so generated entries keep the rail) and TIMELINE:END before
    // the container's column-0 close. An outer-wrap mutant puts both markers
    // outside the container: its region content would contain the container
    // open and the column-0 close, so both assertions die.
    for (name, open) in [("TIMELINE", "<div class=\"tl\">"), ("FEATURES", "<div class=\"grid\">")] {
        let reg = region(&html, name);
        assert!(
            !reg.contains(open),
            "{name} markers must wrap the INNER content — the {open} container open must stay OUTSIDE (before BEGIN): {reg}"
        );
        assert!(
            !reg.contains("\n</div>"),
            "{name} markers must wrap the INNER content — the container's column-0 </div> close must stay OUTSIDE (after END): {reg}"
        );
    }
    let tl = region(&html, "TIMELINE");
    assert!(tl.contains("<h3>Day one <span class=\"hash\">f911488</span></h3>"));
    assert!(tl.contains("<p>Hand-written prose.</p>"));
    assert!(tl.contains(&format!(
        "<h3>delegate — chug spawns chug <span class=\"hash\">{}</span></h3>",
        refs[0]
    )));
    assert!(tl.contains("<p>More hand prose.</p>"));
    let subjects = all_commit_subjects(&f);
    assert!(
        subjects.iter().any(|s| s.contains("site: TIMELINE region markers bootstrap (T99)")),
        "timeline bootstrap commit missing: {subjects:?}"
    );
    assert!(
        subjects.iter().any(|s| s.contains("site: FEATURES region markers bootstrap (T99)")),
        "features bootstrap commit missing: {subjects:?}"
    );
    // req 1+5: one entry per verified done row, dated from git — T101: the
    // list ASCENDS by commit time (day-one → latest, req 4; T99's
    // date-string/id-desc sort read newest-first against the page's
    // chronological design): t5(09-20) < t6(09-21) < t7 < t8 (same day,
    // ordered by %ct — t7's commit lands seconds before t8's) < t11(09-23)
    let entry = |r: &str| format!("<span class=\"hash\">({r})</span>");
    for (frag, r, date) in [
        ("<h3>delta item lands", &refs[4], "2026-09-22"),
        ("<h3>gamma item lands", &refs[3], "2026-09-22"),
        ("<h3>beta item lands", &refs[2], "2026-09-21"),
        ("<h3>alpha item lands", &refs[1], "2026-09-20"),
        ("<h3>an extraordinarily", &refs[5], "2026-09-23"),
    ] {
        let at = tl.find(frag).unwrap_or_else(|| panic!("entry missing: {frag} in {tl}"));
        let end = (at + 130).min(tl.len());
        let ctx = &tl[at.saturating_sub(200)..end];
        assert!(ctx.contains(&entry(r)), "ref not cited for {frag}: {ctx}");
        assert!(ctx.contains(date), "git date {date} missing for {frag}: {ctx}");
    }
    let i5 = tl.find(&entry(&refs[1])).unwrap();
    let i6 = tl.find(&entry(&refs[2])).unwrap();
    let i7 = tl.find(&entry(&refs[3])).unwrap();
    let i8 = tl.find(&entry(&refs[4])).unwrap();
    let i11 = tl.find(&entry(&refs[5])).unwrap();
    assert!(i5 < i6 && i6 < i7 && i7 < i8 && i8 < i11, "entries ascend by commit time (req 4)");
    // facts-only: the machine run (beta..long) carries no <p> — t6..t11 are
    // adjacent machine entries (the Day-one prose now interleaves at its
    // 09-20 day slot, before t6, so the no-<p> slice must start at t6)
    assert!(!tl[i6..i11].contains("<p>"), "machine entries must not narrate");
    // curated entries take their commit-time slot too (req 1): the seed hand
    // entry cites refs[0], committed 2026-09-19 — the OLDEST entry, first
    let seed = tl.find(&format!("<span class=\"hash\">{}</span>", refs[0])).unwrap();
    assert!(seed < i5, "datable curated entry leads at its %ct slot: {tl}");
    // the Day-one hand entry cites f911488 — no such commit in the fixture
    // repo — so it is UNDATABLE BY REF; T122: it dates by its own tl-date
    // (2026-09-20, day precision = that day's last second) and sorts INTO
    // 09-20: after t5 (an exact %ct — 09-20 12:00 — that a day-precision
    // date cannot claim) and before t6 (09-21). T101's old bottom-pin
    // ("after dated neighbors, never before") is what buried the live
    // 09-27 curated milestones under 09-28 generated entries — dead.
    let day = tl.find("<h3>Day one").unwrap();
    assert!(day > i5, "undatable-by-ref entry must sit inside its tl-date day, after 09-20's noon %ct: {tl}");
    assert!(day < i6, "…and BEFORE the next day's entries — the bottom-pin is dead: {tl}");
    assert!(tl[day..].contains("<p>Hand-written prose.</p>"), "undatable entry keeps its prose");
    // dedupe by commit ref: the hand-cited seed ref never gains an entry
    assert_eq!(count(&tl, &entry(&refs[0])), 0, "hand-cited ref duplicated");
    assert_eq!(count(&html, &refs[0]), 1, "hand entry stays exactly once");
    // req 5 audit: a bogus-only ref is warned and skipped
    assert!(!html.contains("ghost item"), "unverifiable ref must not render");
    assert!(String::from_utf8_lossy(&out.stderr).contains("T9"), "audit warning missing");
    // fallback: T10 cites 0badc0de then the hand-cited seed ref -> deduped away
    // (scoped to the timeline: the T349 shipped feed is a different region
    // with a different doctrine — it renders every done row whose cited ref
    // verifies, dedupe-free, so the page CAN contain the row's title)
    assert!(!tl.contains("fallback item"), "fallback row must dedupe against hand entry");
    // title truncation at 100 bytes with an ellipsis
    let t11 = &tl[i11.saturating_sub(400)..(i11 + 40).min(tl.len())];
    assert!(t11.contains("…"), "long title truncated: {t11}");
    assert!(!html.contains("must truncate"), "truncated tail absent");
}

#[test]
fn t99_timeline_cap_collapse_and_idempotence_then_new_landing() {
    let (f, _refs, _orig) = fixture_t99();
    let cdir = &f.chug;
    let mut rows = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    for i in 1..=25 {
        std::fs::write(cdir.join(format!("c{i}")), "x\n").unwrap();
        let r = commit(cdir, &format!("t{i}: cap item"), None, "2026-09-21", true);
        rows.push_str(&format!(
            "| T{i} | cap item {i} | specs/c{i}.md | 1 | done | done {r} — x |\n"
        ));
    }
    std::fs::write(cdir.join("TODO.md"), rows).unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "cap run: {out:?}");
    let tl = region(&page(&f), "TIMELINE");
    // 20 machine rows dated 2026-09-21 + the hand entry that shares the date
    assert_eq!(
        count(&tl, "<div class=\"tl-date\">2026-09-21</div>"),
        21,
        "cap: 20 full entries + hand entry: {tl}"
    );
    assert_eq!(count(&tl, "<span class=\"hash\">("), 20);
    assert!(tl.contains("…and 5 earlier milestones (T1–T5)"), "collapse line: {tl}");
    let i25 = tl.find("cap item 25").unwrap();
    let i24 = tl.find("cap item 24").unwrap();
    assert!(i24 < i25, "ascending commit-time order (T101 req 4)");
    // idempotence: second run byte-identical, no commit at all
    let before = page(&f);
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "second run must be byte-identical");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit when unchanged");
    assert!(String::from_utf8_lossy(&out2.stdout).contains("unchanged"));
    // a new landing appears within one cycle wrap (acceptance mechanism)
    std::fs::write(cdir.join("c26"), "x\n").unwrap();
    let r26 = commit(cdir, "t26: new landing", None, "2026-09-25", true);
    let mut todo = std::fs::read_to_string(cdir.join("TODO.md")).unwrap();
    todo.push_str(&format!(
        "| T26 | new landing item | specs/c26.md | 1 | done | done {r26} — x |\n"
    ));
    std::fs::write(cdir.join("TODO.md"), todo).unwrap();
    let out3 = run_sync(&f, true);
    assert!(out3.status.success());
    let tl3 = region(&page(&f), "TIMELINE");
    assert!(tl3.contains("new landing item"), "new item rendered: {tl3}");
    assert!(tl3.contains("2026-09-25"), "new item dated from git");
    assert_eq!(sync_commits(&f), 2, "second sync commit for the new landing");
}

#[test]
fn t99_features_badges_cards_and_idempotence() {
    let (f, _refs, _orig) = fixture_t99();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run 1: {out:?}");
    let html = page(&f);
    assert_eq!(count(&html, "<!-- FEATURES:BEGIN -->"), 1);
    let ft = region(&html, "FEATURES");
    // landed badge (checked off in FEATURES.md — strikethrough-ONLY leg of
    // finding 3) on the matched existing card, which keeps position and
    // markup; T218: the body is machine-owned too — the seeded stale prose
    // heals to the row's prep()d what-text in the same pass as the badge
    assert!(
        ft.contains("<h3><span>delegate</span> sub-agents<i class=\"q\">landed</i></h3>"),
        "matched card badge: {ft}"
    );
    assert!(
        ft.contains("<p>Structured child result: <code>goal_complete</code> summary + refs.</p>"),
        "matched card body regenerated from the row's what-text (T218): {ft}"
    );
    assert!(ft.contains("    <div class=\"card\">\n      <h3><span>tools</span> ×13</h3>\n      <p>read_file etc.</p>\n    </div>"),
        "non-F card verbatim: {ft}");
    // finding 1 (RED-proven): a card with NO <p> (h3 + <ul> body) must survive
    // regeneration byte-for-byte — the old flush_card `if (buf ~ /<p/)` gate
    // silently dropped it, contradicting "no card is ever dropped"
    let nop = "    <div class=\"card\">\n      <h3><span>misc</span> extras</h3>\n      <ul>\n        <li>read_file</li>\n        <li>web_fetch</li>\n      </ul>\n    </div>";
    assert!(ft.contains(nop), "no-<p> card silently DROPPED (finding 1): {ft}");
    // finding 3: the two landed legs pinned independently — F1 via
    // strikethrough ONLY (the delegate card's landed badge above is F1's),
    // F4 via the LANDED annotation ONLY (synthesized card)
    assert!(
        ft.contains("<h3><span>Gamma guard</span><i class=\"q\">landed</i></h3>"),
        "F4 (LANDED-annotation-only) must read landed: {ft}"
    );
    // in-flight: F2 referenced by the T12 TODO row (boundary: F2, not F21)
    let f2 = ft.find("Streaming UX").expect("F2 card");
    assert!(
        ft[f2..f2 + 240].contains("<i class=\"q\">in-flight</i>"),
        "F2 in-flight badge: {}",
        &ft[f2..f2 + 240]
    );
    // queued + synthesized card appended, backticks -> <code>
    assert!(ft.contains("<h3><span>Web search</span><i class=\"q\">queued</i></h3>"));
    assert!(ft.contains("<code>sinks</code>"), "backtick conversion: {ft}");
    // what-text truncation at 160 bytes
    let f3 = ft.find("Web search").unwrap();
    assert!(ft[f3..(f3 + 300).min(ft.len())].contains("…"), "long what truncated");
    // idempotence
    let before = page(&f);
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "features second run byte-identical");
    assert_eq!(all_commit_subjects(&f).len(), n);
}

#[test]
fn t99_f21_reference_must_not_make_f2_inflight() {
    // finding 4: the F2-vs-F21 boundary (the in-flight grep's `[^0-9]` tail)
    // was pinned only by a comment — here the ONLY F-reference in TODO.md is
    // F21, and F2 must stay queued, not in-flight.
    let (f, _refs, _orig) = fixture_t99();
    let todo = std::fs::read_to_string(f.chug.join("TODO.md")).unwrap();
    std::fs::write(
        f.chug.join("TODO.md"),
        todo.replace("references F2 phase 1", "references F21 phase 1"),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {out:?}");
    let ft = region(&page(&f), "FEATURES");
    let f2 = ft.find("Streaming UX").expect("F2 card");
    let ctx = &ft[f2..f2 + 240];
    assert!(!ctx.contains("in-flight"), "F2 must NOT be in-flight when only F21 is referenced: {ctx}");
    assert!(ctx.contains("<i class=\"q\">queued</i>"), "F2 stays queued: {ctx}");
    assert!(!ft.contains("in-flight"), "no F-item may be in-flight off an F21 reference: {ft}");
}

/// T193: the in-flight refinement grepped ALL of TODO.md, but done rows keep
/// their F-references forever (T105's done row names F6; T180's names F12) —
/// every feature a completed item ever touched rendered in-flight permanently
/// on chug.sh. The refinement now reads OPEN rows only (status cell `todo` or
/// `in-progress` — the same -F'|' field conventions as todo_facts), and the
/// FEATURES.md rows that masked the bug are brought to the uppercase-LANDED
/// convention. Pinned legs: (a) a done-row-only F-reference never yields
/// in-flight; (b) an open-row (todo OR in-progress) reference does; (c) a
/// SPLIT row with the uppercase LANDED annotation plus an OPEN referencing
/// row still renders landed — landed wins, the refinement is skipped for
/// landed rows (req 3). F14's live shape (done T184 ref + open T192 ref) is
/// the (b)-leg in the wild: the open reference is what keeps it honest.
#[test]
fn t193_inflight_classifier_reads_open_todo_rows_only_and_landed_wins() {
    let (f, _refs, _orig) = fixture_t99();
    // Five reference shapes across the two files: F9 done-row-only, F10
    // todo-row, F11 in-progress-row, F12 LANDED + an OPEN referencing row,
    // F13 unreferenced (control), F14 LANDED + done-row-only (the real F6
    // shape that the lowercase "landed" annotation left permanently in-flight).
    std::fs::write(
        f.chug.join("TODO.md"),
        concat!(
            "# TODO\n\n| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T105 | F9 phase 1: done-row feature shipped | specs/t105.md | 3 | done | done dd29137 — x |\n",
            "| T106 | F14 phase 1: also shipped | specs/t106.md | 3 | done | done ee29137 — x |\n",
            "| T184 | context-economy telemetry (F10 phase 1) | specs/t184.md | 2 | todo | operator |\n",
            "| T192 | live-context editing (F11 phase 2, CLM port) | specs/t192.md | 3 | in-progress | started |\n",
            "| T193 | in-flight classifier (F12 phase 2) | specs/t193.md | 3 | todo | operator |\n"
        ),
    )
    .unwrap();
    std::fs::write(
        f.chug.join("FEATURES.md"),
        concat!(
            "# FEATURES\n\n",
            "| # | Feature | What | Benchmark |\n",
            "|---|---------|------|-----------|\n",
            "| F9 | **Done-only feature** | Referenced only by a done row. | n/a |\n",
            "| F10 | **Open-row feature** | Referenced by an open todo row. | n/a |\n",
            "| F11 | **In-progress feature** | Referenced by an in-progress row. | n/a |\n",
            "| F12 | **Landed wins** — SPLIT: **phase 1 LANDED (T193, cafe567, cycle 93)** — shipped | An OPEN row references it; landed wins. | n/a |\n",
            "| F13 | **Untouched feature** | No TODO references at all. | n/a |\n",
            "| F14 | **Done-row landed** — SPLIT: **phase 1 LANDED (T106, deadbee, cycle 94)** — shipped | Landed with only a done-row reference. | n/a |\n"
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let ft = region(&page(&f), "FEATURES");
    // (a) done-row-only F-reference: the done row is invisible to the grep —
    // queued, never in-flight (the T193 defect made this in-flight forever)
    assert!(
        ft.contains("<h3><span>Done-only feature</span><i class=\"q\">queued</i></h3>"),
        "(a) done-row-only reference must stay queued: {ft}"
    );
    // (b) open-row reference yields in-flight — both open statuses
    assert!(
        ft.contains("<h3><span>Open-row feature</span><i class=\"q\">in-flight</i></h3>"),
        "(b) todo-row reference must be in-flight: {ft}"
    );
    assert!(
        ft.contains("<h3><span>In-progress feature</span><i class=\"q\">in-flight</i></h3>"),
        "(b) in-progress row must count as open: {ft}"
    );
    // (c) landed wins: F12 carries the uppercase LANDED annotation AND is
    // referenced by OPEN T193 — the landed classification must survive the
    // refinement (the pre-T193 refinement only skipped landed rows; this pin
    // holds that ordering against regressions in either direction)
    assert!(
        ft.contains("<h3><span>Landed wins</span><i class=\"q\">landed</i></h3>"),
        "(c) LANDED annotation + open referencing row must stay landed: {ft}"
    );
    // control: an unreferenced row stays queued
    assert!(
        ft.contains("<h3><span>Untouched feature</span><i class=\"q\">queued</i></h3>"),
        "unreferenced row stays queued: {ft}"
    );
    // the real F6 shape: LANDED annotation + done-row-only reference — landed,
    // and the done reference must not flip it (lowercase "landed" is what hid
    // this from the classifier in the wild)
    assert!(
        ft.contains("<h3><span>Done-row landed</span><i class=\"q\">landed</i></h3>"),
        "LANDED + done-row-only reference must be landed: {ft}"
    );
    assert_eq!(count(&ft, "<i class=\"q\">in-flight</i>"), 2, "exactly the two open-referenced rows");
}

/// T216: a markdown-escaped pipe (`\|`) is cell CONTENT, not a delimiter, but
/// the features_rows awk split on it anyway — any row carrying `\|` rendered
/// truncated. The live complaint: chug.sh's F15 card rendered QUEUED with the
/// what-text `http\` because its name cell carried
/// `CHUG_JUDGE=daemon\|http\|off` and the LANDED annotation sat beyond the
/// split. The generator now parks each escape on a line COPY before the split
/// (a \001 placeholder) and restores the literal pipe in the surviving cells.
/// Pinned legs: (a) a LANDED annotation beyond a `\|` still classifies landed
/// — the FULL name cell is tested; (b) the full what-text renders (no `http\`
/// fragment); (c) a `\|` row with no LANDED still classifies via the open-row
/// refinement — queued unreferenced, in-flight when an OPEN TODO row
/// references it.
#[test]
fn t216_escaped_pipes_never_split_cells() {
    let (f, _refs, _orig) = fixture_t99();
    std::fs::write(
        f.chug.join("TODO.md"),
        concat!(
            "# TODO\n\n| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T216 | site-sync escaped-pipe parse (F23 phase 1) | specs/t216.md | 3 | todo | operator |\n"
        ),
    )
    .unwrap();
    std::fs::write(
        f.chug.join("FEATURES.md"),
        concat!(
            "# FEATURES\n\n",
            "| # | Feature | What | Benchmark |\n",
            "|---|---------|------|-----------|\n",
            "| F21 | Daemon-mode switch `CHUG_JUDGE=daemon\\|http\\|off` — phase 1 LANDED (T204, abc1234, cycle 95) | picks the baked-in daemon over the connection-refused external layad | n/a |\n",
            "| F22 | Unreferenced switch `CHUG_JUDGE=daemon\\|http\\|off` — deferred | no TODO row references it | n/a |\n",
            "| F23 | Open-row switch `CHUG_JUDGE=daemon\\|http\\|off` — in flight | an OPEN row references it | n/a |\n"
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let ft = region(&page(&f), "FEATURES");
    // (a) landed classification reads the FULL name cell: the LANDED
    // annotation sits beyond two escaped pipes and must still win (the
    // pre-T216 split truncated at "daemon\" and rendered queued), and the
    // name span carries the escaped pipes as literal pipes, whole
    assert!(
        ft.contains("<h3><span>Daemon-mode switch `CHUG_JUDGE=daemon|http|off`</span><i class=\"q\">landed</i></h3>"),
        "(a) LANDED beyond an escaped pipe must classify landed, name cell whole: {ft}"
    );
    // (b) the what-text renders in full — no "http\" fragment (the live bug's
    // what-text), and the escaped pipes in the what cell render as pipes
    assert!(
        ft.contains("<p>picks the baked-in daemon over the connection-refused external layad</p>"),
        "(b) full what-text must render: {ft}"
    );
    assert!(!ft.contains("http\\"), "(b) no http-backslash fragment anywhere: {ft}");
    // (c) no LANDED annotation: classification survives the escape —
    // unreferenced stays queued, an OPEN row reference yields in-flight
    // (landed must NOT leak onto escape-bearing rows)
    assert!(
        ft.contains("<h3><span>Unreferenced switch `CHUG_JUDGE=daemon|http|off`</span><i class=\"q\">queued</i></h3>"),
        "(c) escaped-pipe row with no LANDED and no open reference stays queued: {ft}"
    );
    assert!(
        ft.contains("<h3><span>Open-row switch `CHUG_JUDGE=daemon|http|off`</span><i class=\"q\">in-flight</i></h3>"),
        "(c) escaped-pipe row referenced by an OPEN todo row is in-flight: {ft}"
    );
    assert_eq!(count(&ft, "<i class=\"q\">in-flight</i>"), 1, "exactly the one open-referenced row");
}

/// T218: the card BODY is machine-owned, like the badge. features_generate
/// used to preserve every existing card's markup wholesale and ensure only
/// the badge — the <p> was written once at card creation and never
/// refreshed, so a body written by any historically-buggy generator (chug.sh's
/// F15 card showed the T216 `http\` fragment for HOURS) was locked in
/// forever, and the operator's only remedy was deleting the card (9a1a6e0).
/// Pinned legs: (a) a stale-fragment body heals to the row's prep()d
/// what-text in one sync, badge and body together; (b) position and CSS
/// classes still hold — card order, the card/h3/badge markup, even the <p>
/// open tag's own class; (c) a non-F-item card's body is untouched
/// (forward-compat for hand-authored cards); and the healed page is
/// idempotent — drift heals exactly once.
#[test]
fn t218_stale_card_bodies_regenerate_from_the_features_row() {
    let (f, _refs, _orig) = fixture_t99();
    // the delegate card (matches F1 "delegate collect") carries the live
    // bug's stale fragment plus a classed <p> open tag; COMMIT it so the
    // sync repairs a published broken card — the operator's exact situation
    let stale = page(&f).replace(
        "      <p>Launch and collect child runs.</p>\n",
        "      <p class=\"what\">http\\</p>\n",
    );
    assert_ne!(stale, page(&f), "seed edit must land");
    std::fs::write(f.site.join("index.html"), &stale).unwrap();
    commit(&f.site, "operator: broken card body from an old generator", None, "2026-09-25", false);
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let ft = region(&page(&f), "FEATURES");
    // (a) the fossil is gone, healed to F1's what-text — no hand-deletion
    assert!(!ft.contains("http\\"), "(a) stale fragment body must not survive: {ft}");
    assert!(
        ft.contains("<p class=\"what\">Structured child result: <code>goal_complete</code> summary + refs.</p>"),
        "(a) body healed to the row's prep()d what-text: {ft}"
    );
    // (b) position + classes preserved: still the FIRST card, card/h3/badge
    // markup intact, the <p> open tag's own class survived the rewrite
    let dg = ft.find("<div class=\"card\">").expect("delegate card present");
    let head = &ft[dg..(dg + 300).min(ft.len())];
    assert!(
        head.contains("<h3><span>delegate</span> sub-agents<i class=\"q\">landed</i></h3>"),
        "(b) card position + h3/badge classes intact: {head}"
    );
    assert!(
        ft.find("delegate").unwrap() < ft.find("tools").unwrap()
            && ft.find("tools").unwrap() < ft.find("misc").unwrap(),
        "(b) card order preserved: {ft}"
    );
    // (c) a non-F-item card keeps its body verbatim (forward-compat)
    assert!(
        ft.contains("    <div class=\"card\">\n      <h3><span>tools</span> ×13</h3>\n      <p>read_file etc.</p>\n    </div>"),
        "(c) non-F-item card body untouched: {ft}"
    );
    // drift heals exactly once: the next sync is byte-identical, no commit
    let before = page(&f);
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "healed page is idempotent");
    assert_eq!(all_commit_subjects(&f).len(), n, "no second sync commit");
}

#[test]
fn t99_best_effort_on_malformed_and_missing_blocks() {
    // missing .tl/.grid blocks (T98 fixture): bootstrap warns, cycle still fine
    let f = fixture();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "missing blocks must not fail: {out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("skipping"), "warn present");
    assert_eq!(count(&page(&f), "TIMELINE:BEGIN"), 0, "no markers bootstrapped");
    assert_eq!(sync_commits(&f), 1, "stats still synced once");
    // malformed markers (duplicated BEGIN): region skipped, cycle still fine
    let (f2, _refs, _orig) = fixture_t99();
    let html2 = page(&f2);
    std::fs::write(
        f2.site.join("index.html"),
        html2.replace("<div class=\"tl\">", "<!-- TIMELINE:BEGIN -->\n<div class=\"tl\">"),
    )
    .unwrap();
    let out2 = run_sync(&f2, true);
    assert!(out2.status.success(), "malformed markers must not fail");
    assert!(
        String::from_utf8_lossy(&out2.stderr).contains("malformed TIMELINE"),
        "warn: {:?}",
        String::from_utf8_lossy(&out2.stderr)
    );
    assert_eq!(count(&page(&f2), "<!-- TIMELINE:BEGIN -->"), 1, "region untouched");
}

// --- T101 — commit-time order, curated merge, cap ----------------------------

#[test]
fn t101_commit_time_order_overrules_row_and_id_order_t99_before_t98() {
    // The live complaint: T99 (d13a253) rendered BEFORE T98 (8721c83) and a
    // 2026-09-26 entry after a wall of 09-27s — T99 sorted by date string
    // desc then row id desc. Here T99's ROW sits ABOVE T98's ROW (row order)
    // and T99 carries the HIGHER id (id order), but T98's commit is a full
    // day earlier (%ct) — the ascending page must render T98 first.
    let (f, _refs, _orig) = fixture_t99();
    let r98 = commit(&f.chug, "t98: earlier commit lands", None, "2026-09-25", true);
    let r99 = commit(&f.chug, "t99: later commit lands", None, "2026-09-26", true);
    std::fs::write(
        f.chug.join("TODO.md"),
        format!(
            concat!(
                "# TODO\n\n| id | title | spec | pri | status | notes |\n",
                "|----|-------|------|-----|--------|-------|\n",
                "| T99 | later row item | specs/t99.md | 1 | done | done {} — x |\n",
                "| T98 | earlier row item | specs/t98.md | 1 | done | done {} — x |\n"
            ),
            r99, r98
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {out:?}");
    let tl = region(&page(&f), "TIMELINE");
    let i98 = tl.find("<h3>earlier row item").unwrap_or_else(|| panic!("T98 entry missing: {tl}"));
    let i99 = tl.find("<h3>later row item").unwrap_or_else(|| panic!("T99 entry missing: {tl}"));
    assert!(
        i98 < i99,
        "commit time (%ct) must order the list — row order and id order both say T99 first (the reported bug): {tl}"
    );
}

#[test]
fn t101_curated_ref_merges_into_one_entry_prose_wins() {
    // req 2, the live report's leg (c): raw TODO titles were dumped AFTER the
    // curated entries instead of merging by ref, burying the "chug.sh — this
    // site" crescendo mid-list. A generated row whose commit matches a
    // curated hash span must render NOWHERE — the curated entry is the ONE
    // entry, at its own commit-time slot.
    let (f, refs, _orig) = fixture_t99();
    let todo = std::fs::read_to_string(f.chug.join("TODO.md")).unwrap();
    std::fs::write(
        f.chug.join("TODO.md"),
        format!(
            "{todo}| T77 | raw duplicate row | specs/t77.md | 1 | done | done {} — x |\n\
             | T78 | ghost f911488 row | specs/t78.md | 1 | done | done f911488 — x |\n",
            refs[0]
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {out:?}");
    let tl = region(&page(&f), "TIMELINE");
    // T77: real commit, same ref the curated seed entry cites -> merged away
    assert!(!tl.contains("raw duplicate row"), "row citing a curated ref must merge away: {tl}");
    assert!(tl.contains("<p>More hand prose.</p>"), "curated text preserved (prose wins): {tl}");
    assert_eq!(count(&tl, &refs[0]), 1, "one entry, not two: {tl}");
    // T78: cites the curated Day-one hash f911488, which is no commit in the
    // chug repo — the facts-only audit (req 5, unchanged) skips the row, and
    // the curated Day-one prose stays, exactly once
    assert!(!tl.contains("ghost f911488 row"), "unverifiable row never renders: {tl}");
    assert!(tl.contains("<h3>Day one <span class=\"hash\">f911488</span></h3>"));
    assert_eq!(count(&tl, "f911488"), 1, "curated Day-one entry stays unique: {tl}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("T78"), "audit warning for T78");
    // the merged curated entry takes its commit-time slot: the seed commit
    // (2026-09-19) predates every machine row, so the prose LEADS the list
    let seed = tl.find(&format!("<span class=\"hash\">{}</span>", refs[0])).unwrap();
    let machine = tl.find(&format!("<span class=\"hash\">({})</span>", refs[1])).unwrap();
    assert!(seed < machine, "curated entry slotted by %ct (oldest here) — not dumped after: {tl}");
}

// --- T122 — curated entries sort by their tl-date, not the bottom ------------

#[test]
fn t122_undatable_curated_entries_sort_into_their_tl_date_day() {
    // The live complaint: "The move to K7 — always-on" (no hash span) and
    // "chug.sh — this site e998d45" (a SITE-repo hash, no commit in the chug
    // repo) both carry tl-date 2026-09-27 yet rendered AFTER every
    // 2026-09-28 generated entry — T101 keyed ref-less entries +inf
    // ("after dated neighbors, never before"), which is the page bottom.
    // T122: no resolvable ref => date by the entry's own tl-date, day
    // precision. The +inf slot survives only for entries with neither a
    // resolvable ref nor a parseable tl-date.
    let (f, _refs, _orig) = fixture_t99();
    let r28 = commit(&f.chug, "t88: newest item lands", None, "2026-09-28", true);
    std::fs::write(
        f.chug.join("TODO.md"),
        format!(
            concat!(
                "# TODO\n\n| id | title | spec | pri | status | notes |\n",
                "|----|-------|------|-----|--------|-------|\n",
                "| T88 | newest item lands | specs/t88.md | 1 | done | done {} — x |\n"
            ),
            r28
        ),
    )
    .unwrap();
    // page with markers pre-wrapped (bootstrap skipped): Day one (09-20,
    // undatable span), the two live 09-27 milestones, and one entry with
    // NEITHER hash span nor tl-date — the truly-undatable case whose bottom
    // slot T122 retains
    std::fs::write(
        f.site.join("index.html"),
        concat!(
            "<html><body>\n",
            "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n<!-- STATS:END -->\n",
            "<div class=\"tl\">\n",
            "<!-- TIMELINE:BEGIN -->\n",
            "    <div class=\"tl-item major\">\n",
            "      <div class=\"tl-date\">2026-09-20</div>\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>Day one <span class=\"hash\">f911488</span></h3>\n",
            "        <p>Hand-written prose.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "    <div class=\"tl-item major\">\n",
            "      <div class=\"tl-date\">2026-09-27</div>\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>The move to K7 — always-on</h3>\n",
            "        <p>loopd moves under launchd.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "    <div class=\"tl-item major\">\n",
            "      <div class=\"tl-date\">2026-09-27</div>\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>chug.sh — this site <span class=\"hash\">e998d45</span></h3>\n",
            "        <p>chug builds its own public site.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "    <div class=\"tl-item\">\n",
            "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
            "      <div class=\"tl-body\">\n",
            "        <h3>dateless stray</h3>\n",
            "        <p>Neither a hash span nor a tl-date div.</p>\n",
            "      </div>\n",
            "    </div>\n",
            "<!-- TIMELINE:END -->\n",
            "</div>\n",
            "</body></html>\n"
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let tl = region(&page(&f), "TIMELINE");
    let day1 = tl.find("<h3>Day one").expect("Day one stays");
    let k7 = tl.find("The move to K7").expect("K7 entry stays");
    let site_e = tl.find("chug.sh — this site").expect("chug.sh entry stays");
    let t88 = tl.find("<h3>newest item lands").expect("generated T88 entry present");
    let stray = tl.find("dateless stray").expect("dateless entry stays");
    // THE bug: both 09-27 curated milestones must render BEFORE the 09-28
    // generated entry (the +inf pin put them after it)
    assert!(k7 < t88, "K7 (tl-date 09-27) must precede the 09-28 generated entry: {tl}");
    assert!(site_e < t88, "chug.sh (tl-date 09-27) must precede the 09-28 generated entry: {tl}");
    // day grouping: the 09-20 undatable precedes the 09-27 undatables
    assert!(day1 < k7, "09-20 tl-date sorts before the 09-27 entries: {tl}");
    // same-day tie (req 2): equal day keys break by region position — K7 was
    // authored before chug.sh
    assert!(k7 < site_e, "same-day undatable entries keep their region order: {tl}");
    // retained +inf: no ref AND no tl-date still sorts last, after T88
    assert!(stray > t88, "truly undatable entry (no ref, no tl-date) keeps the bottom slot: {tl}");
    // curated chunks stay verbatim: prose and tl-date text untouched
    assert!(tl.contains("<p>loopd moves under launchd.</p>"));
    assert_eq!(count(&tl, "<div class=\"tl-date\">2026-09-27</div>"), 2);
    assert_eq!(count(&tl, "e998d45"), 1, "foreign-span entry renders exactly once, curated: {tl}");
    // idempotence: the day-key path must be stable run to run
    let before = page(&f);
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "second run byte-identical");
}

#[test]
fn t122_same_day_ties_keep_region_order_and_exact_refs_outrank_day_keys() {
    // req 2 + the T101 regression leg, inside one day (09-27): the
    // ref-resolved curated entry R (committed 09-27 noon) keeps its exact
    // %ct slot — AHEAD of the day-keyed undatables even though the region
    // lists R last — while the two undatables tie on the day key and keep
    // their region flow (A before B). A 09-28 generated row still renders
    // after all of them, and %ct stays the primary key (req 4).
    let (f, _refs, _orig) = fixture_t99();
    let rr = commit(&f.chug, "t77: same-day curated anchor lands", None, "2026-09-27", true);
    let r28 = commit(&f.chug, "t66: newest item lands", None, "2026-09-28", true);
    std::fs::write(
        f.chug.join("TODO.md"),
        format!(
            concat!(
                "# TODO\n\n| id | title | spec | pri | status | notes |\n",
                "|----|-------|------|-----|--------|-------|\n",
                "| T66 | newest item lands | specs/t66.md | 1 | done | done {} — x |\n"
            ),
            r28
        ),
    )
    .unwrap();
    std::fs::write(
        f.site.join("index.html"),
        format!(
            concat!(
                "<html><body>\n",
                "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n<!-- STATS:END -->\n",
                "<div class=\"tl\">\n",
                "<!-- TIMELINE:BEGIN -->\n",
                "    <div class=\"tl-item\">\n",
                "      <div class=\"tl-date\">2026-09-27</div>\n",
                "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
                "      <div class=\"tl-body\">\n",
                "        <h3>first undatable A</h3>\n",
                "        <p>No hash span at all.</p>\n",
                "      </div>\n",
                "    </div>\n",
                "    <div class=\"tl-item\">\n",
                "      <div class=\"tl-date\">2026-09-27</div>\n",
                "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
                "      <div class=\"tl-body\">\n",
                "        <h3>second undatable B <span class=\"hash\">e998d45</span></h3>\n",
                "        <p>Foreign hash span, no chug commit.</p>\n",
                "      </div>\n",
                "    </div>\n",
                "    <div class=\"tl-item\">\n",
                "      <div class=\"tl-date\">2026-09-27</div>\n",
                "      <div class=\"tl-rail\"><span class=\"tl-dot\"></span></div>\n",
                "      <div class=\"tl-body\">\n",
                "        <h3>same-day ref entry <span class=\"hash\">{rr}</span></h3>\n",
                "        <p>Cites a real 09-27 chug commit.</p>\n",
                "      </div>\n",
                "    </div>\n",
                "<!-- TIMELINE:END -->\n",
                "</div>\n",
                "</body></html>\n"
            ),
            rr = rr
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let tl = region(&page(&f), "TIMELINE");
    let a = tl.find("first undatable A").expect("A stays");
    let b = tl.find("second undatable B").expect("B stays");
    let r = tl.find(&format!("<span class=\"hash\">{}</span>", rr)).expect("R stays");
    let t66 = tl.find("<h3>newest item lands").expect("generated T66 entry present");
    // T101 leg intact: the ref-resolved entry keeps its exact %ct slot
    // (09-27 noon), ahead of the day-precision keys of the same day —
    // despite R sitting LAST in the region
    assert!(r < a, "exact %ct outranks the day key within its own day: {tl}");
    // req 2: equal day keys tie-break by region position — A before B
    assert!(a < b, "same-day undatable tie keeps region order: {tl}");
    assert!(b < t66, "the whole 09-27 group precedes the 09-28 generated entry: {tl}");
    // T101 merge leg intact: curated R renders once, prose wins
    assert_eq!(count(&tl, &rr), 1, "R renders exactly once (curated, no machine twin): {tl}");
    assert!(tl.contains("<p>Cites a real 09-27 chug commit.</p>"), "prose wins: {tl}");
}

// --- T217 — fail-closed input guard ------------------------------------------

/// T217 req 1: a sync whose repo inputs are unreadable must write NOTHING and
/// commit NOTHING. The regions' fallback zeros are byte-identical to a
/// legitimately empty repo's first run — indistinguishable by the values —
/// so the guard keys on INPUT READABILITY, never on the computed output.
/// Publishing the fallbacks with the same authority as real data is how
/// 58c3a0b gutted chug.sh (a rogue runner whose repo path pointed at an
/// empty or harvested directory still committed AND pushed the zeros).
#[test]
fn t217_unreadable_todo_md_refuses_to_publish_without_editing() {
    let f = fixture();
    std::fs::remove_file(f.chug.join("TODO.md")).unwrap();
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let out = run_sync(&f, true);
    assert_eq!(out.status.code(), Some(4), "guard refusal exits nonzero (4), not 0/3");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("refusing to publish"), "one named-error line: {err}");
    assert!(err.contains("TODO.md"), "the error names the unreadable input: {err}");
    assert_eq!(
        std::fs::read(f.site.join("index.html")).unwrap(),
        before,
        "index.html must stay byte-identical (req 3)"
    );
    assert_eq!(sync_commits(&f), 0, "commits NOTHING");
}

/// Second guard leg (req 1): .chug/loopd absent — the cycle-count source's
/// whole directory — refuses identically.
#[test]
fn t217_missing_loopd_dir_refuses_to_publish_without_editing() {
    let f = fixture();
    std::fs::remove_dir_all(f.chug.join(".chug/loopd")).unwrap();
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let out = run_sync(&f, true);
    assert_eq!(out.status.code(), Some(4), "guard refusal exits nonzero (4)");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("refusing to publish"), "one named-error line: {err}");
    assert!(err.contains(".chug/loopd"), "the error names the absent dir: {err}");
    assert_eq!(std::fs::read(f.site.join("index.html")).unwrap(), before);
    assert_eq!(sync_commits(&f), 0);
}

/// The guard's THIRD leg (T226 — T217's validator bonus mutant m4): git log
/// EMPTY while TODO.md and .chug/loopd are present and readable. m4 deleted
/// exactly this leg (the other two stayed intact) and the whole suite stayed
/// green: the two sibling pins above each remove an input ENTIRELY, so they
/// refuse through legs 1/2 and never isolate the git-log leg. The realistic
/// shape here is the T186 harvested-or-half-cloned worktree: a git repo that
/// initializes fine but has ZERO commits — every file readable, no history
/// at all, so every git-derived number would be a fallback published with
/// real data's authority (the 58c3a0b gutting class).
#[test]
fn t217_empty_git_log_refuses_to_publish_without_editing() {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    // BOTH text inputs readable — legs 1 and 2 must pass on their own
    // merits, so ONLY the git-log leg can refuse.
    let loopd = chug.join(".chug/loopd");
    std::fs::create_dir_all(&loopd).unwrap();
    std::fs::write(loopd.join("loopd.log"), "2026-09-25T10:00:00Z cycle OK: x\n").unwrap();
    std::fs::write(
        chug.join("TODO.md"),
        concat!(
            "# TODO\n\n| id | title | spec | pri | status | notes |\n",
            "|----|-------|------|-----|--------|-------|\n",
            "| T3 | first | specs/t3.md | 1 | done | x |\n"
        ),
    )
    .unwrap();
    site_fixture(&site);
    let f = Fixture {
        _keep: keep,
        chug,
        site,
    };
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let commits_before = all_commit_subjects(&f).len();
    let out = run_sync(&f, true);
    assert_eq!(out.status.code(), Some(4), "guard refusal exits nonzero (4)");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err.lines().count(), 1, "exactly ONE named-error line: {err:?}");
    assert!(err.contains("refusing to publish"), "the line is named: {err}");
    assert!(
        err.contains("git log empty"),
        "the error names THE leg the m4 mutant deleted (zero writes, zero commits): {err}"
    );
    assert_eq!(
        std::fs::read(f.site.join("index.html")).unwrap(),
        before,
        "index.html must stay byte-identical (zero writes)"
    );
    assert_eq!(
        all_commit_subjects(&f).len(),
        commits_before,
        "commits NOTHING"
    );
}

/// The 58c3a0b signature (T186 class): ALL repo inputs unreadable at once —
/// the runner's repo path pointed at an empty or harvested directory, not
/// even a git repo. One named-error line, nothing else on stderr; the live
/// page and its git history stay untouched.
#[test]
fn t217_rogue_context_all_inputs_unreadable_one_line_page_untouched() {
    let f = fixture();
    let harvested = f._keep.path().join("harvested");
    std::fs::create_dir_all(&harvested).unwrap();
    let before = std::fs::read(f.site.join("index.html")).unwrap();
    let commits_before = all_commit_subjects(&f).len();
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&harvested);
    c.env("CHUG_SYNC_NOW", NOW).env("CHUG_SITE_SYNC_NO_PUSH", "1");
    let out = c.output().expect("spawn");
    assert_eq!(out.status.code(), Some(4));
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(err.lines().count(), 1, "exactly ONE named-error line: {err:?}");
    assert!(err.contains("refusing to publish"), "the line is named: {err}");
    assert_eq!(std::fs::read(f.site.join("index.html")).unwrap(), before);
    assert_eq!(all_commit_subjects(&f).len(), commits_before, "commits NOTHING");
}

/// The guard's only escape is the explicit --bootstrap flag (req 1): a
/// genuinely empty repo's first-ever run — impossible in loop context, where
/// TODO.md always exists. With the flag the sync proceeds and publishes the
/// zeros; without it the same fixture refuses (pinned above), so the flag is
/// the only way through.
#[test]
fn t217_bootstrap_flag_is_the_explicit_guard_escape() {
    let f = fixture();
    std::fs::remove_file(f.chug.join("TODO.md")).unwrap();
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&f.chug).arg("--bootstrap");
    c.env("CHUG_SYNC_NOW", NOW).env("CHUG_SITE_SYNC_NO_PUSH", "1");
    let out = c.output().expect("spawn");
    assert!(
        out.status.success(),
        "--bootstrap skips the guard: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        page(&f).contains("<b>0/0</b>"),
        "a bootstrap run publishes the genuinely-empty-repo zeros:\n{}",
        page(&f)
    );
}

/// T217 req 5 (adjacent): the sync commit records the input stats — items /
/// tests / cycles counts — so a gutting commit is distinguishable from a real
/// one at a glance (58c3a0b's body would read items 0/0, tests n/a, cycles 0).
#[test]
fn t217_commit_message_records_the_input_stat_counts() {
    let f = fixture();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    // regions written: the fixture's own facts (2/3 items, t4's 555, 2 cycles)
    let html = page(&f);
    assert!(html.contains("<b>2/3</b>"), "items card:\n{html}");
    assert!(html.contains("<b>555</b>"), "tests card:\n{html}");
    assert!(html.contains("<b>2</b>"), "cycles card:\n{html}");
    // ... and the commit body names the same counts
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    let body = String::from_utf8_lossy(&log.stdout);
    assert!(body.contains("items 2/3"), "items count in the commit body: {body}");
    assert!(body.contains("tests 555"), "tests count in the commit body: {body}");
    assert!(body.contains("cycles 2"), "cycles count in the commit body: {body}");
}

// --- T238 — the gate-count scraper reads only FULL-suite counts ---------------

/// T238 req 2 pin: the newest gate-quoting commit carries SUBSETS — the live
/// ab93f94 shape (a mid-run census "nextest 113/1402", which the card rendered
/// as "113 tests green" at the newest "full-suite gate count") plus a
/// mismatched red-tail run ("nextest 1400/1496" — 96 failed, caught by the
/// EQUALITY leg alone since its passed operand clears the floor) — while an
/// older commit carries the real suite total ("nextest 1496/1496"). The
/// region must walk back and show 1496 at the OLDER commit's ref, never a
/// subset or red-tail number. The census message deliberately also carries
/// the two prose traps the live commit had: the bare "113 tests" partial form
/// and the "full-suite load" phrase — a whole-message label check would
/// re-publish exactly this bug, so the labeled leg must stay adjacent to the
/// count.
#[test]
fn t238_subset_census_walks_back_to_the_full_count() {
    let f = fixture();
    let full = commit(
        &f.chug,
        "merge: t220 lands the pinned suite",
        Some("post-merge gates green (1496/1496), nextest 1496/1496 + clippy"),
        "2026-09-22",
        true,
    );
    let census = commit(
        &f.chug,
        "chore: timing-fence census under full-suite load",
        Some(
            "mid-run census: nextest 113/1402 (113 tests green so far); \
             red tail on the older run: nextest 1400/1496 (96 failed)",
        ),
        "2026-09-23",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>1496</b><span>tests green at the newest full-suite gate count in a commit message"),
        "the region must show the older FULL count, not the newest subset:\n{html}"
    );
    assert!(!html.contains("<b>113</b>"), "the subset census must never become the suite total:\n{html}");
    // the citation stays checkable (req 4): the card names the FULL-count
    // commit, never the census commit
    let at = html.find("tests green at the newest full-suite").expect("tests card");
    let card = &html[at.saturating_sub(60)..(at + 160).min(html.len())];
    assert!(card.contains(&full), "the card cites the full-count commit: {card}");
    assert!(!card.contains(&census), "the card must not cite the census commit: {card}");
}

/// T238 req 3 pin: an EQUAL-operand subset is not the suite total — a
/// single-package gate run ("site_sync 22/22", carried both in the named
/// form and as the package nextest run the grammar matches) sits far below
/// the floor, so the walk skips it and cites the older full count.
#[test]
fn t238_equal_operand_package_subset_is_not_accepted() {
    let f = fixture();
    let full = commit(
        &f.chug,
        "merge: t221 lands",
        Some("gates green: nextest 1500/1500 + clippy -D warnings"),
        "2026-09-22",
        true,
    );
    let subset = commit(
        &f.chug,
        "chore: site_sync gate run",
        Some("package gate site_sync 22/22 via cargo nextest -p site_sync: nextest 22/22"),
        "2026-09-23",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>1500</b><span>tests green at the newest full-suite gate count in a commit message"),
        "an equal-operand package subset must lose to the older full count:\n{html}"
    );
    assert!(!html.contains("<b>22</b>"), "the subset 22/22 must never render:\n{html}");
    let at = html.find("tests green at the newest full-suite").expect("tests card");
    let card = &html[at.saturating_sub(60)..(at + 160).min(html.len())];
    assert!(card.contains(&full), "the card cites the full-count commit: {card}");
    assert!(!card.contains(&subset), "the card must not cite the subset commit: {card}");
}

/// T238 req 1 tail: when NO commit in the walk window carries an acceptable
/// full-suite count — only subsets — the card prints the honest n/a (and the
/// sync commit records it) rather than dressing a subset number up as the
/// suite total.
#[test]
fn t238_no_full_suite_count_anywhere_prints_na_not_a_subset() {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    // T217 inputs present and readable: only the gate-count fact is empty
    std::fs::create_dir_all(chug.join(".chug/loopd")).unwrap();
    std::fs::write(chug.join(".chug/loopd/loopd.log"), "2026-09-25T10:00:00Z cycle OK: x\n").unwrap();
    std::fs::write(chug.join("EVALUATION.md"), "# EVALUATION — fixture\n").unwrap();
    std::fs::write(chug.join("TODO.md"), "# TODO\n").unwrap();
    commit(&chug, "seed: subsets only", None, "2026-09-19", false);
    commit(&chug, "chore: site_sync gate", Some("site_sync 22/22 (nextest 22/22)"), "2026-09-20", true);
    commit(&chug, "chore: loop_spec pin", Some("loop_spec 5/5 (nextest 5/5)"), "2026-09-21", true);
    site_fixture(&site);
    let f = Fixture { _keep: keep, chug, site };
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>n/a</b><span>tests green — no full-suite gate count found in recent commit messages</span>"),
        "subsets only => the honest n/a card, never a subset number:\n{html}"
    );
    assert!(!html.contains("<b>22</b>") && !html.contains("<b>5</b>"), "no subset number anywhere:\n{html}");
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("tests n/a"),
        "the sync commit records the n/a: {}",
        String::from_utf8_lossy(&log.stdout)
    );
}

// --- T239 — the three surviving-mutant legs, each pinned by its own fixture ---
//
// T238's kimi validator (cycle 114) mutation-tested the guard and found three
// SURVIVORS — green tests that do not kill real mutants on load-bearing legs.
// All three degrade honestly (an older full count, never a subset), so they
// were non-blocking; these pins make each one kill its mutant.

/// T239 pin 1 (mut-runner: the runner-word grammar leg removed): the wrap
/// gate's stable shape since the T82 switch carries ONE runner word between
/// the token and the count — "nextest release 1664/1664". Dropping the
/// optional runner-word group from the grammar makes this newest commit
/// unmatchable; the walk then degrades to the older t4 count and the card
/// silently shows a stale full count. The pin holds the shape up: the
/// release count is accepted and cited, the older count never renders.
#[test]
fn t239_runner_word_release_count_is_accepted_and_cited() {
    let f = fixture();
    let release = commit(
        &f.chug,
        "chore: wrap gate on the release build",
        Some("gates green: nextest release 1664/1664 + clippy -D warnings"),
        "2026-09-24",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>1664</b><span>tests green at the newest full-suite gate count in a commit message"),
        "the one-runner-word release count must be accepted:\n{html}"
    );
    assert!(!html.contains("<b>555</b>"), "an older count must lose to the newest:\n{html}");
    let at = html.find("tests green at the newest full-suite").expect("tests card");
    let card = &html[at.saturating_sub(60)..(at + 160).min(html.len())];
    assert!(card.contains(&release), "the card cites the release-count commit: {card}");
}

/// T239 pin 2 (mut-loop1: the within-message walk stops after the first
/// pair): the continuation past a REJECTED pair is load-bearing — production
/// messages quote fixture censuses alongside the real count in one message
/// (the live 90900bc shape: "nextest 113/1402" quoted in the same commit
/// message as the real wrap-gate count). Stopping after the first pair
/// discards the qualifying pair later in the SAME message and walks back to
/// an older commit's count instead. The pin holds the shape up: the walk
/// accepts the qualifying pair from the same message, never 113, never a
/// walk-back.
#[test]
fn t239_walk_continues_past_a_rejected_pair_within_one_message() {
    let f = fixture();
    let mixed = commit(
        &f.chug,
        "chore: item lands with the census in the tail",
        Some(
            "landed: nextest 113/1402 mid-run census; \
             wrap gate nextest 1664/1664 + clippy -D warnings",
        ),
        "2026-09-24",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>1664</b><span>tests green at the newest full-suite gate count in a commit message"),
        "the qualifying pair later in the SAME message must be accepted:\n{html}"
    );
    assert!(
        !html.contains("<b>113</b>"),
        "the rejected census pair in front of it must never render:\n{html}"
    );
    assert!(!html.contains("<b>555</b>"), "no walk-back to an older commit's count:\n{html}");
    let at = html.find("tests green at the newest full-suite").expect("tests card");
    let card = &html[at.saturating_sub(60)..(at + 160).min(html.len())];
    assert!(card.contains(&mixed), "the card cites the mixed-message commit: {card}");
}

/// T239 pin 3 (mut-floor100: GATE_FLOOR lowered): the floor's exact value is
/// load-bearing. The known package subsets are pinned far below it (22/22,
/// 5/5 — a lowered floor of 100 would still reject them); the unpinned gap is
/// a MID-SIZE equal-operand subset like "nextest 113/113", which any floor
/// at or below 113 would admit. The pin holds the gap up: a below-floor
/// equal-operand mid-size count loses to an older full count and the card
/// never shows 113.
#[test]
fn t239_mid_size_equal_operand_subset_below_the_floor_loses() {
    let f = fixture();
    let full = commit(
        &f.chug,
        "merge: the pinned suite lands",
        Some("gates green: nextest 1496/1496 + clippy -D warnings"),
        "2026-09-22",
        true,
    );
    let mid = commit(
        &f.chug,
        "chore: mid-size package census",
        Some("census: nextest 113/113 so far"),
        "2026-09-23",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    assert!(
        html.contains("<b>1496</b><span>tests green at the newest full-suite gate count in a commit message"),
        "the mid-size subset must lose to the older full count:\n{html}"
    );
    assert!(
        !html.contains("<b>113</b>"),
        "the below-floor subset 113/113 must never render:\n{html}"
    );
    let at = html.find("tests green at the newest full-suite").expect("tests card");
    let card = &html[at.saturating_sub(60)..(at + 160).min(html.len())];
    assert!(card.contains(&full), "the card cites the full-count commit: {card}");
    assert!(!card.contains(&mid), "the card must not cite the mid-size subset commit: {card}");
}

// --- T240 — RELEASE region: the latest release, always current ----------------

/// The site half with the live page's shape: nav (with the stats link), hero,
/// and a stats section carrying its markers — and NO releases section yet, so
/// the T240 bootstrap leg (markers + section skeleton + nav anchor) is what
/// runs. Returns the pre-run page.
fn site_fixture_t240(dir: &Path) -> String {
    let page = concat!(
        "<html><body>\n",
        "<nav class=\"nav\"><div class=\"wrap nav-in\">\n",
        "  <a class=\"brand\" href=\"#hero\">chug<b>_</b></a>\n",
        "  <div class=\"nav-links\">\n",
        "    <a href=\"#how-it-works\">how it works</a>\n",
        "    <a href=\"#stats\">stats</a>\n",
        "    <a href=\"#features\">features</a>\n",
        "  </div>\n",
        "</div></nav>\n",
        "<header class=\"hero\" id=\"hero\"><div class=\"wrap\"><h1>chug</h1></div></header>\n",
        "<section id=\"how-it-works\"><div class=\"wrap\"><p>the loop is code, not conversation</p></div></section>\n",
        "<section id=\"stats\" class=\"live-stats\"><div class=\"wrap\">\n",
        "  <p class=\"kicker\">live stats</p>\n",
        "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n<!-- STATS:END -->\n",
        "</div></section>\n",
        "<p>sentinel-after</p>\n",
        "</body></html>\n"
    );
    std::fs::write(dir.join("index.html"), page).unwrap();
    commit(dir, "initial page", None, "2026-09-18", false);
    page.to_string()
}

/// An annotated tag at a pinned date — the tag object's creatordate (the
/// release date the region renders) is the tagger date, i.e. GIT_COMMITTER_DATE.
fn git_tag(dir: &Path, name: &str, msg: &str, date: &str) {
    git(dir, &["tag", "-a", name, "-m", msg], Some(date));
}

/// The chug fixture half with tags: the base fixture (T217-readable inputs, no
/// tags) plus an older release, the newer release carrying six bullets (one
/// past the cap, one over the 160-byte limit, one with emphasis + backticked
/// refs + an HTML-special char), and a NEWER pre-release that must be skipped
/// (pre-release channels are out of scope, T240).
fn chug_fixture_t240(dir: &Path) {
    chug_fixture(dir);
    git_tag(dir, "v0.1.0", "- older release one\n- older release two", "2026-09-20");
    git_tag(
        dir,
        "v0.2.0",
        concat!(
            "- **delegate collect** — structured child result with `goal_complete` summary & refs (`a1b2c3d`)\n",
            "- this bullet is deliberately long so that the one hundred and sixty byte cap forces the ",
            "word-boundary ellipsis of the T99 text rules and its tail beyond the cap must never render anywhere\n",
            "- plain bullet three\n",
            "- bullet four\n",
            "- bullet five\n",
            "- bullet six must not render past the cap"
        ),
        "2026-09-25",
    );
    git_tag(dir, "v0.3.0-rc1", "- pre-release must not render", "2026-09-26");
}

fn fixture_t240(tagged: bool) -> Fixture {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    if tagged {
        chug_fixture_t240(&chug);
    } else {
        chug_fixture(&chug);
    }
    site_fixture_t240(&site);
    Fixture { _keep: keep, chug, site }
}

/// Req 1+2+4: a fixture with two stable tags (and a newer pre-release) renders
/// the NEWEST STABLE tag of the LOCAL checkout — version, tag date, GitHub
/// link, and the top 3-5 release-note bullets under the T99 text rules (top
/// bullet emphasis-stripped, backticked refs -> <code>, & escaped; the
/// over-160-byte bullet word-truncated with its tail gone; the 6th bullet past the
/// cap never rendered). The section is bootstrapped between the hero and the
/// stats section, and the nav gains the releases anchor directly before the
/// stats link — all in one bootstrap commit, byte-identical and commit-free on
/// the second run.
#[test]
fn t240_two_tags_render_the_newest_stable_tag_with_date_and_capped_bullets() {
    let f = fixture_t240(true);
    let out = run_sync(&f, true);
    assert!(out.status.success(), "run 1: {:?}", String::from_utf8_lossy(&out.stderr));
    let html = page(&f);
    // bootstrap: exactly one marker pair; the section sits between the hero
    // and the stats section (req 1); the nav anchor rides directly before the
    // stats link (req 4 — the nav mirrors the section order)
    assert_eq!(count(&html, "<!-- RELEASE:BEGIN -->"), 1);
    assert_eq!(count(&html, "<!-- RELEASE:END -->"), 1);
    let hero = html.find("id=\"hero\"").expect("hero present");
    let rel_sec = html.find("<section id=\"releases\"").expect("releases section bootstrapped");
    let stats_sec = html.find("<section id=\"stats\"").expect("stats section present");
    assert!(hero < rel_sec && rel_sec < stats_sec, "the release section sits between the hero and STATS:\n{html}");
    let nav = html.find("href=\"#releases\">releases</a>").expect("nav anchor present");
    let nav_stats = html.find("href=\"#stats\">stats</a>").expect("stats nav link present");
    assert!(nav < nav_stats, "the releases anchor must precede the stats link:\n{html}");
    // req 2: the newest STABLE v* tag of the local checkout — never the older
    // tag, never the newer pre-release channel (out of scope)
    let rel = region(&html, "RELEASE");
    assert!(rel.contains("<b>v0.2.0</b>"), "the newest stable tag renders:\n{rel}");
    assert!(!html.contains("v0.1.0"), "the older tag must not render:\n{html}");
    assert!(!html.contains("v0.3.0-rc1"), "pre-release channels are out of scope:\n{html}");
    assert!(rel.contains("tagged 2026-09-25"), "the tag's own date renders:\n{rel}");
    assert!(!rel.contains("2026-09-20"), "the OLDER tag's date must not render:\n{rel}");
    assert!(
        rel.contains("releases/tag/v0.2.0"),
        "the release link targets the GitHub release page:\n{rel}"
    );
    // the top 3-5 bullets: capped at five, order preserved, T99 text rules
    assert_eq!(count(&rel, "<li>"), 5, "top 3-5: the 6th bullet must not render:\n{rel}");
    assert!(!rel.contains("bullet six"), "the past-cap bullet is gone:\n{rel}");
    assert!(
        rel.contains("<li>delegate collect — structured child result with <code>goal_complete</code> summary &amp; refs (<code>a1b2c3d</code>)</li>"),
        "top bullet: emphasis stripped, backticks -> <code>, & HTML-escaped:\n{rel}"
    );
    assert!(!rel.contains("**"), "markdown emphasis must never render:\n{rel}");
    assert!(rel.contains("…"), "the >160-byte bullet truncates at the cap:\n{rel}");
    assert!(!rel.contains("must never render anywhere"), "the truncated tail is gone:\n{rel}");
    let i1 = rel.find("delegate collect").unwrap();
    let i2 = rel.find("deliberately long").unwrap();
    let i3 = rel.find("plain bullet three").unwrap();
    let i4 = rel.find("bullet four").unwrap();
    let i5 = rel.find("bullet five").unwrap();
    assert!(i1 < i2 && i2 < i3 && i3 < i4 && i4 < i5, "bullet order is the tag message's:\n{rel}");
    // the bootstrap is its own commit; the sync commit records the release
    let subjects = all_commit_subjects(&f);
    assert!(
        subjects.iter().any(|s| s.contains("site: RELEASE region markers bootstrap (T240)")),
        "bootstrap commit missing: {subjects:?}"
    );
    assert_eq!(sync_commits(&f), 1, "one sync commit for the filled region");
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("release: v0.2.0"),
        "the sync commit records the rendered release:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
    // idempotence: unchanged inputs stay byte-identical, no further commits
    let before = page(&f);
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "second run must be byte-identical (req 3 idempotence)");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit when unchanged");
}

/// Req 3+5: a repo with NO v* tags (pre-v0.1 repos are legal) writes an EMPTY
/// region and does not error — no fallback version is ever rendered with real
/// data's authority (the T217 doctrine applied to the release facts) — and the
/// sync CONTINUES: the stats region still syncs and the commit records the
/// empty release.
#[test]
fn t240_zero_tags_writes_an_empty_region_and_does_not_error() {
    let f = fixture_t240(false);
    let out = run_sync(&f, true);
    assert!(
        out.status.success(),
        "no tags must not error: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = page(&f);
    assert_eq!(count(&html, "<!-- RELEASE:BEGIN -->"), 1);
    assert_eq!(count(&html, "<!-- RELEASE:END -->"), 1);
    assert!(
        region(&html, "RELEASE").trim().is_empty(),
        "no tags -> the region writes nothing:\n{:?}",
        region(&html, "RELEASE")
    );
    // the bootstrap still landed (section + markers + nav anchor) ...
    assert!(html.contains("<section id=\"releases\""), "section bootstrapped:\n{html}");
    assert!(html.contains("href=\"#releases\">releases</a>"), "nav anchor bootstrapped:\n{html}");
    let subjects = all_commit_subjects(&f);
    assert!(
        subjects.iter().any(|s| s.contains("site: RELEASE region markers bootstrap (T240)")),
        "bootstrap commit missing: {subjects:?}"
    );
    // ... and the sync CONTINUES: the stats region synced, the release recorded empty
    assert!(html.contains("<b>2/3</b>"), "stats still synced (the sync continues, req 3):\n{html}");
    assert_eq!(sync_commits(&f), 1);
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("release: none"),
        "the sync commit records the empty release:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
}

/// The marker-pair contract (T99) holds for RELEASE too: malformed markers are
/// warn + skip, best-effort, never a cycle failure — the page is left
/// untouched around them and the stats sync still lands.
#[test]
fn t240_malformed_release_markers_are_best_effort_skip() {
    let f = fixture_t240(true);
    let html = page(&f);
    // a stray BEGIN with no END: malformed (1/0)
    std::fs::write(
        f.site.join("index.html"),
        html.replace("<section id=\"stats\"", "<!-- RELEASE:BEGIN -->\n<section id=\"stats\""),
    )
    .unwrap();
    commit(&f.site, "operator: stray marker", None, "2026-09-26", false);
    let out = run_sync(&f, true);
    assert!(out.status.success(), "malformed RELEASE markers must not fail: {:?}", out.status.code());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("malformed RELEASE markers"), "warn names the fault: {err}");
    assert!(err.contains("skipping"), "best-effort skip: {err}");
    let html2 = page(&f);
    assert_eq!(count(&html2, "<!-- RELEASE:BEGIN -->"), 1, "the stray marker stays (untouched)");
    assert_eq!(count(&html2, "<!-- RELEASE:END -->"), 0);
    assert_eq!(count(&html2, "<section id=\"releases\""), 0, "no section bootstrapped");
    assert!(html2.contains("<b>2/3</b>"), "stats still synced");
    assert_eq!(sync_commits(&f), 1, "the sync still lands once");
}

// --- T241 — docs.html: the second page, fully machine-rendered ---------------
//
// scripts/site-sync.sh renders a SECOND site page — docs.html — with no
// marker regions at all: the README's `## Quickstart` section leads, then one
// section per runbooks/*.md file (byte-sorted, runbooks/README.md included),
// each rendered from the file's content at sync time (fences -> pre.code,
// headings shifted h2/h3/h4, flat lists, GFM pipe tables -> <table> per T244;
// inline per the T99 text rules). Same head/style base as index.html; docs ⇄
// home nav; an index of links at the top. Fail-closed: missing runbooks/ ->
// docs.html untouched, the sync continues. Byte-identical inputs render
// byte-identical output.

/// The chug half with the docs corpus: README (a `## Quickstart` section whose
/// `#`-comment fence lines must not end the extraction and whose `## Next`
/// section must not leak) plus a deliberately unsorted runbooks/ — including
/// the corpus's own README.md index and a fence carrying HTML-special bytes
/// (the escaping pin). Byte sort: README.md < alpha.md < zeta.md.
fn chug_fixture_t241_runbooks(dir: &Path, allow_empty_commit: bool) {
    let rb = dir.join("runbooks");
    std::fs::create_dir_all(&rb).unwrap();
    std::fs::write(
        dir.join("README.md"),
        concat!(
            "# chug\n\nIntro line.\n\n## Quickstart\n\n```bash\ncargo build\n",
            "cargo install --path .\n# a comment line inside the fence\n```\n\n",
            "Run `chug run --spec SPEC.md --goal \"Build X\"` — the `check:` line ",
            "gates completion.\n\n## Next\n\nAnother section — must not leak into ",
            "the docs quickstart.\n"
        ),
    )
    .unwrap();
    std::fs::write(
        rb.join("README.md"),
        "# Runbooks\n\n- one small chore → [quick-task.md](quick-task.md)\n",
    )
    .unwrap();
    std::fs::write(
        rb.join("zeta.md"),
        concat!(
            "# Runbook: zeta last\n\n**Use when** testing sort order. ",
            "**Arc: ~5 minutes.**\n\n## Run it\n\n```bash\nchug run --goal \"zeta\"\n```\n"
        ),
    )
    .unwrap();
    std::fs::write(
        rb.join("alpha.md"),
        concat!(
            "# Runbook: alpha first\n\n**Use when** testing sort order.\n\n",
            "## Run it\n\n```bash\necho \"<b>&</b> <x>\"\n```\n"
        ),
    )
    .unwrap();
    // the subject deliberately avoids the landed-item `t<N>:` shape so the
    // corpus commit never perturbs the facts the sync computes
    // allow_empty: the recovery leg re-adds an identical corpus over one that
    // was deleted, so there may be nothing to stage — the commit is fixture realism
    commit(dir, "docs: runbooks corpus + README quickstart", None, "2026-09-22", allow_empty_commit);
}

fn chug_fixture_t241(dir: &Path) {
    chug_fixture(dir);
    chug_fixture_t241_runbooks(dir, false);
}

/// The site half with the shapes the T241 bootstrap needs: a nav-links div, a
/// get-started section, a <style> block (the shared style base), and the stale
/// STATS region.
fn site_fixture_t241(dir: &Path) -> String {
    std::fs::write(
        dir.join("index.html"),
        concat!(
            "<html><head>\n<style>\n:root{--wrap:1060px}\n</style>\n</head><body>\n",
            "<nav class=\"nav\"><div class=\"wrap nav-in\">\n",
            "  <a class=\"brand\" href=\"#hero\">chug<b>_</b></a>\n",
            "  <div class=\"nav-links\">\n",
            "    <a href=\"#how-it-works\">how it works</a>\n",
            "    <a href=\"#get-started\">get started</a>\n",
            "  </div>\n",
            "  <a class=\"gh\" href=\"https://github.com/tampajohn/chug\">github</a>\n",
            "</div></nav>\n",
            "<header class=\"hero\" id=\"hero\"><div class=\"wrap\"><h1>chug</h1></div></header>\n",
            "<section id=\"get-started\"><div class=\"wrap\">\n",
            "  <p class=\"kicker\">get started</p>\n",
            "  <h2>Clone, build, chug</h2>\n",
            "  <div class=\"steps\">\n",
            "    <div class=\"step\">\n",
            "      <h3>1 · INSTALL</h3>\n",
            "      <pre class=\"code\">curl -fsSL https://chug.sh/install.sh | sh</pre>\n",
            "    </div>\n",
            "  </div>\n",
            "</div></section>\n",
            "<section id=\"stats\" class=\"live-stats\"><div class=\"wrap\">\n",
            "  <p class=\"kicker\">live stats</p>\n",
            "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n",
            "<!-- STATS:END -->\n</div></section>\n",
            "</body></html>\n"
        ),
    )
    .unwrap();
    commit(dir, "initial page", None, "2026-09-18", false)
}

fn fixture_t241() -> Fixture {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    chug_fixture_t241(&chug);
    site_fixture_t241(&site);
    Fixture { _keep: keep, chug, site }
}

fn docs(f: &Fixture) -> String {
    std::fs::read_to_string(f.site.join("docs.html")).expect("read docs.html")
}

#[test]
fn t241_docs_page_renders_runbooks_sorted_with_escaped_fences_and_nav() {
    let f = fixture_t241();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {:?}", out.status.code());
    let docs_before = docs(&f);
    // quickstart leads, sourced from README's Quickstart section only
    let qs = docs_before.find("<section id=\"quickstart\"").expect("quickstart section present");
    let r_readme = docs_before.find("<section id=\"runbook-readme\"").expect("runbooks index file rendered");
    let r_alpha = docs_before.find("<section id=\"runbook-alpha\"").expect("alpha rendered");
    let r_zeta = docs_before.find("<section id=\"runbook-zeta\"").expect("zeta rendered");
    assert!(qs < r_readme && r_readme < r_alpha && r_alpha < r_zeta, "quickstart leads, runbooks byte-sorted");
    assert!(docs_before[..r_readme].contains("cargo install --path ."), "README quickstart rendered");
    assert!(docs_before[..r_readme].contains("# a comment line inside the fence"), "fence content verbatim");
    assert!(!docs_before.contains("Another section"), "the next README section must not leak");
    // code fences render as the page's code style, HTML-escaped (req 1)
    assert!(docs_before.contains("echo \"&lt;b&gt;&amp;&lt;/b&gt; &lt;x&gt;\""), "fence bytes escaped");
    assert!(!docs_before.contains("echo \"<b>"), "raw fence bytes must not render");
    assert_eq!(count(&docs_before, "<pre class=\"code\">"), 3, "quickstart + 2 runbook fences");
    // headings shift: the runbook H1 is the section h2, its H2s are h3
    assert!(docs_before.contains("<h2>Runbook: alpha first</h2>"));
    assert!(docs_before.contains("<h3>Run it</h3>"));
    // T244: bold renders as <strong> (req 3 — was <b> under T241)
    assert!(docs_before.contains("<strong>Use when</strong>"), "bold emphasis per the T99 text rules");
    // the runbooks index file renders with its relative link rewritten (no dangling href)
    assert!(
        docs_before.contains("href=\"https://github.com/tampajohn/chug/blob/main/runbooks/quick-task.md\""),
        "relative links rewritten onto the GitHub blob base"
    );
    // index of links at the top (before the first content section)
    let idx = docs_before.find("href=\"#runbook-alpha\"").expect("index entry");
    assert!(idx < qs, "the link index sits at the top of the page");
    assert!(docs_before.contains("href=\"#runbook-zeta\""));
    // same head/style base as index.html, verbatim
    assert!(docs_before.contains(":root{--wrap:1060px}"), "index.html's <style> block carried over");
    // docs ⇄ home
    assert!(count(&docs_before, "href=\"https://chug.sh/\"") >= 3, "brand, home button, footer");
    // index.html: the nav link and the one-line get-started pointer (reqs 2+4)
    let html = page(&f);
    assert!(html.contains("<a href=\"https://chug.sh/docs.html\">docs</a>"), "nav gains the docs link");
    let gs = html.find("<section id=\"get-started\"").expect("get-started section");
    let ptr = html.find("docs page</a>").expect("one-line pointer");
    assert!(ptr > gs, "the pointer lives inside the get-started section");
    // finding 2 (validator M8): placement EARLY-but-inside was unpinned — bound the
    // pointer above the section close too, not only below the section start
    let gs_close = html[gs..].find("</section>").expect("get-started close") + gs;
    assert!(ptr < gs_close, "the pointer sits ABOVE the get-started close (inside, not merely below the start)");
    // req 5: the sync commits BOTH files when either changed (one commit)
    assert_eq!(sync_commits(&f), 1);
    let nameonly = git(&f.site, &["show", "--name-only", "--format="], None);
    let files = String::from_utf8_lossy(&nameonly.stdout);
    assert!(files.lines().any(|l| l.trim() == "index.html"), "index.html in the sync commit: {files}");
    assert!(files.lines().any(|l| l.trim() == "docs.html"), "docs.html in the same commit: {files}");
    let st = git(&f.site, &["status", "--porcelain"], None);
    assert!(String::from_utf8_lossy(&st.stdout).trim().is_empty(), "clean tree: both files committed");
    // idempotence (req 3): byte-identical re-render, no second commit, no dup links
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert!(String::from_utf8_lossy(&out2.stdout).contains("unchanged"), "no-op run says so");
    assert_eq!(docs(&f), docs_before, "docs.html byte-identical");
    assert_eq!(page(&f), html, "index.html byte-identical");
    assert_eq!(sync_commits(&f), 1, "no second commit");
    assert_eq!(count(&page(&f), "chug.sh/docs.html"), 2, "nav link + pointer, exactly once each");
}

#[test]
fn t241_missing_runbooks_leaves_docs_untouched_and_sync_continues() {
    let f = fixture_t241();
    std::fs::remove_dir_all(f.chug.join("runbooks")).unwrap();
    std::fs::write(f.site.join("docs.html"), "<html>sentinel-docs</html>\n").unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "the sync continues: {:?}", out.status.code());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("runbooks"), "warn names the missing corpus: {err}");
    assert_eq!(
        std::fs::read(f.site.join("docs.html")).unwrap(),
        b"<html>sentinel-docs</html>\n".to_vec(),
        "docs.html untouched (fail-closed)"
    );
    let html = page(&f);
    assert!(!html.contains("chug.sh/docs.html"), "never a link to a page that is not rendered");
    assert!(html.contains("<b>2/3</b>"), "stats still synced");
    assert_eq!(sync_commits(&f), 1, "the sync still lands once");
    // finding 1 (validator M6): the stray docs.html must never be SWEPT into the
    // sync commit — staging is guarded by DOCS_CHANGED (this sync rendered no
    // page), not by the file's mere existence on disk
    let nameonly = git(&f.site, &["show", "--name-only", "--format="], None);
    let files = String::from_utf8_lossy(&nameonly.stdout);
    assert!(!files.lines().any(|l| l.trim() == "docs.html"), "the stray docs.html is NOT swept into the fail-closed sync commit: {files}");
    // recovery: add the corpus -> the next sync renders the page + bootstraps the nav
    chug_fixture_t241_runbooks(&f.chug, true);
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    let docs2 = docs(&f);
    assert!(docs2.contains("<section id=\"runbook-alpha\""), "docs.html rendered once the corpus exists");
    assert!(docs2.contains("<section id=\"quickstart\""), "README quickstart rendered");
    let html2 = page(&f);
    assert!(html2.contains("<a href=\"https://chug.sh/docs.html\">docs</a>"), "nav link lands with the page");
    assert_eq!(sync_commits(&f), 2, "the recovery sync lands");
}

#[test]
fn t241_runbook_edit_updates_docs_and_commits_both_files() {
    let f = fixture_t241();
    assert!(run_sync(&f, true).status.success());
    let subjects_before = all_commit_subjects(&f).len();
    let docs_before = docs(&f);
    std::fs::write(
        f.chug.join("runbooks/alpha.md"),
        concat!(
            "# Runbook: alpha first\n\n**Edited.** A new line of body.\n\n",
            "## Run it\n\n```bash\necho hi\n```\n"
        ),
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success());
    let docs_after = docs(&f);
    assert_ne!(docs_before, docs_after, "the runbook edit re-renders docs.html");
    assert!(docs_after.contains("<strong>Edited.</strong>"), "the edit is on the page");
    assert_eq!(all_commit_subjects(&f).len(), subjects_before + 1, "one commit for the docs change");
    let st = git(&f.site, &["status", "--porcelain"], None);
    assert!(String::from_utf8_lossy(&st.stdout).trim().is_empty(), "BOTH files committed — clean tree");
    let nameonly = git(&f.site, &["show", "--name-only", "--format="], None);
    let files = String::from_utf8_lossy(&nameonly.stdout);
    assert!(files.lines().any(|l| l.trim() == "docs.html"), "the docs change is in the commit: {files}");
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(docs(&f), docs_after, "no further churn");
    assert_eq!(sync_commits(&f), 2, "still exactly two sync commits");
}

// --- T244 — docs.html inline markdown: tables, lists, inline code ------------
//
// The operator's complaint: chug.sh/docs.html reads like raw markdown in most
// sections — the T205 hosting table shipped as literal `| a | b |` text inside
// a pre block, and the render's inline pass was incomplete. T244 completes it:
// GFM pipe tables -> <table> (thead/tbody, escaped pipes stay literal cell
// content per the T216 rule), lists -> <ul>/<ol><li>, `code` -> <code>,
// **bold** -> <strong>, *italic* -> <em>, links with https-only hrefs.

/// The edge shapes the real corpus lacks, as a fixture runbook riding along
/// with the copied real corpus: escaped pipes in cells, a no-separator pipe
/// run (the pre.code fallback), *italic*, an http:// link, and a fence full
/// of pipes (fence content must never become a table).
const T244_PROBE: &str = r#"# Runbook: t244 render probe

Edge shapes pinned by tests/site_sync.rs — the real corpus carries the
table and the lists; this file carries the rest.

## Cells and pipes

| setting | value | notes |
|---|---|---|
| judge switch | `CHUG_JUDGE=daemon\|http\|off` | an escaped pipe is one cell |
| plain | a\|b | two words, one cell |

A pipe run with NO separator row stays a verbatim block:

| not | a | table |
| no | separator | here |

## Emphasis and links

Runs *italic*, **bold**, and `inline code` together.

- [insecure](http://insecure.example) renders as text, no <a>
- [secure](https://example.com/ok) keeps its https href
- [relative](probe.md) rewrites onto the GitHub blob base

## A fence full of pipes

```text
| not | a table |
|---|---|
| fence | content |
```
"#;

/// T244 req 5: pins against REAL corpus fixtures — this repo's own README.md
/// and every runbooks/*.md are copied into the fixture verbatim (plus the
/// probe runbook above). The T205 hosting table renders as <table> with 3
/// body rows; quick-task's `- ` list becomes <ul>; the quickstart's
/// `chug run …` span becomes <code>; and no literal `|---|` or backtick
/// survives outside a <pre> block.
#[test]
fn t244_docs_render_tables_lists_and_inline_from_the_real_corpus() {
    let f = fixture_t244();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {:?}", out.status.code());
    let d = docs(&f);

    // the T205 hosting table: <table> with thead + exactly 3 body rows
    let laya = docs_section(&d, "runbook-laya-hf-hosting");
    assert!(laya.contains("<table>"), "the hosting table renders as a table:\n{laya}");
    assert!(
        laya.contains("<thead><tr><th>class</th><th>where</th><th>gating</th></tr></thead>"),
        "the header row renders as th cells:\n{laya}"
    );
    assert_eq!(count(&laya, "<tr>"), 4, "1 header row + the 3 T205 body rows:\n{laya}");
    assert_eq!(count(&laya, "<tbody>"), 1);
    assert!(
        laya.contains("<td>base laya (<code>convaiinnovations/laya</code>, Apache 2.0)</td>"),
        "cell content renders inline markup:\n{laya}"
    );
    assert!(!laya.contains("|---|"), "no delimiter row survives in a rendered table:\n{laya}");

    // quick-task's `- ` list: <ul> with the item's inline code converted
    let qt = docs_section(&d, "runbook-quick-task");
    assert!(qt.contains("<ul>\n<li><code>LEDGER.md</code> — the model's external memory"),
        "the `- ` run renders as <ul><li>:\n{qt}");
    assert!(
        qt.contains("<li><code>.chug/transcript.jsonl</code> — every message, append-only.</li>"),
        "list items keep their inline code:\n{qt}"
    );
    assert!(qt.contains("</ul>"), "the list closes:\n{qt}");
    assert!(qt.contains("<strong>Use when</strong>"), "bold -> <strong>:\n{qt}");

    // the quickstart's inline `chug run …` span -> <code>
    let qs = docs_section(&d, "quickstart");
    assert!(
        qs.contains("<code>chug run --goal \"Fix the flaky login test\" --auto-spec</code>"),
        "the quickstart's `chug run` span renders as <code>:\n{qs}"
    );

    // probe: escaped pipes are CELL CONTENT, never a cell split (T216 rule)
    let probe = docs_section(&d, "runbook-t244-probe");
    assert!(
        probe.contains("<td><code>CHUG_JUDGE=daemon|http|off</code></td>"),
        "an escaped pipe stays a literal pipe inside its one cell:\n{probe}"
    );
    assert!(probe.contains("<td>a|b</td>"), "a bare escape renders as one cell:\n{probe}");
    assert!(!probe.contains("http\\"), "no escaped-pipe fragment anywhere:\n{probe}");

    // probe: emphasis + links
    assert!(probe.contains("<em>italic</em>"), "*italic* -> <em>:\n{probe}");
    assert!(probe.contains("<strong>bold</strong>"), "**bold** -> <strong>:\n{probe}");
    assert!(probe.contains("<li>insecure renders as text, no &lt;a&gt;</li>"),
        "a non-https href degrades to its link text:\n{probe}");
    assert!(!probe.contains("href=\"http://"), "no insecure href is ever emitted:\n{probe}");
    assert!(
        probe.contains("<a href=\"https://example.com/ok\">secure</a>"),
        "an https href renders as a link:\n{probe}"
    );
    assert!(
        probe.contains(
            "<a href=\"https://github.com/tampajohn/chug/blob/main/runbooks/probe.md\">relative</a>"
        ),
        "a relative link rewrites onto the GitHub blob base:\n{probe}"
    );

    // probe: a no-separator pipe run keeps the verbatim pre.code fallback
    assert!(
        probe.contains("<pre class=\"code\">\n| not | a | table |\n| no | separator | here |\n</pre>"),
        "a pipe run with no delimiter row stays verbatim (zero information loss):\n{probe}"
    );
    // probe: fence content is never a table — the |---| line survives only here
    assert!(
        probe.contains("<pre class=\"code\">\n| not | a table |\n|---|---|\n| fence | content |\n</pre>"),
        "fence content stays verbatim, never table markup:\n{probe}"
    );

    // req 5: no literal `|---|` or backtick survives OUTSIDE a <pre> block
    let bare = outside_pre(&d);
    assert!(!bare.contains("|---|"), "no delimiter row outside a pre block:\n{bare}");
    assert!(!bare.contains('`'), "no backtick outside a pre block:\n{bare}");

    // req 4: byte-stable — the second run re-renders byte-identically, no commit
    let before = d;
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(docs(&f), before, "unchanged corpus renders byte-identical docs.html");
    assert!(String::from_utf8_lossy(&out2.stdout).contains("unchanged"));
    assert_eq!(all_commit_subjects(&f).len(), n, "no second commit");
}

/// The t244 fixture: the standard facts fixture, then THIS repo's real
/// README.md and runbooks/*.md copied in verbatim (the pins read the live
/// corpus, so a corpus drift that breaks a pin is a render-relevant change),
/// plus the T244_PROBE edge-shape runbook.
fn fixture_t244() -> Fixture {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    chug_fixture(&chug);
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::copy(root.join("README.md"), chug.join("README.md")).unwrap();
    let rb = chug.join("runbooks");
    std::fs::create_dir_all(&rb).unwrap();
    for entry in std::fs::read_dir(root.join("runbooks")).expect("real runbooks/ present") {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "md") {
            std::fs::copy(&path, rb.join(entry.file_name())).unwrap();
        }
    }
    std::fs::write(rb.join("t244-probe.md"), T244_PROBE).unwrap();
    // the subject deliberately avoids the landed-item `t<N>:` shape so the
    // corpus commit never perturbs the facts the sync computes
    commit(&chug, "docs: t244 corpus fixture (real runbooks + README)", None, "2026-09-22", false);
    site_fixture_t241(&site);
    Fixture {
        _keep: keep,
        chug,
        site,
    }
}

/// docs_section HTML ID — the slice of the page from `<section id="ID">`
/// through its closing `</section>`.
fn docs_section(html: &str, id: &str) -> String {
    let open = format!("<section id=\"{id}\">");
    let start = html.find(&open).unwrap_or_else(|| panic!("section {id} missing"));
    let tail = &html[start..];
    let close = tail.find("</section>").expect("section closed") + "</section>".len();
    tail[..close].to_string()
}

/// outside_pre HTML — the page with every <pre …>…</pre> block removed, so a
/// pin can assert that no raw markdown byte survives in the rendered prose.
fn outside_pre(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find("<pre") {
        let after = &rest[i..];
        let close = after.find("</pre>").expect("every pre closes") + "</pre>".len();
        out.push_str(&rest[..i]);
        rest = &after[close..];
    }
    out.push_str(rest);
    out
}

// --- T327 — the COST region: daily Langfuse spend on the public page ---------
//
// The collector runs the hand-verified ClickHouse recipe (docker exec
// langfuse-clickhouse-1 ... on the Langfuse-local host); every fixture here
// drives it through the INJECTABLE CHUG_COST_CH_CMD instead — never docker.
// Fail-closed (the T217 letter): a collector that cannot reach ClickHouse,
// returns nothing, or returns rows that do not parse produces NO NUMBERS — a
// region with published content keeps it byte-identical (stale beats blank),
// a fresh region gets the explicit n/a placeholder, and a fabricated $0.00
// is never rendered. The failure never fails the sync.
//
// The canned rows mirror the REAL recipe output shapes: `anthropic-system.ai.
// kimi-*` / `glm-*` model names (the split is a contains-match), a
// `laya-local` row (calls + total only, outside the kimi/glm split), and the
// `\N` (ClickHouse TSV NULL) cells the live `GROUP BY 1,2` prints for
// uncosted non-generation groups — a NULL cost sums as 0 (the recipe's own
// sum() semantics), a NULL model lands outside the split.

/// The canned ClickHouse rows: the exact TSV shape the recipe prints
/// (toDate \t provided_model_name \t count() \t sum(total_cost)). Sync-now
/// pins 2026-11-20, so yesterday = 2026-11-19 ($0.50) and the tile window is
/// 2026-11-06..=2026-11-19 (max day 2026-11-07 at $1.50).
const T327_TSV: &str = concat!(
    "2026-11-06\tkimi-k2-0905-preview\t10\t0.5\n",
    "2026-11-06\tglm-4.6\t20\t0.25\n",
    "2026-11-07\tkimi-k2-0905-preview\t30\t1.5\n",
    "2026-11-12\tglm-4.6\t5\t0.0002\n",
    "2026-11-18\tlaya-local\t40\t0\n",
    "2026-11-19\tkimi-k2-0905-preview\t7\t0.35\n",
    "2026-11-19\tglm-4.6\t3\t0.15\n",
    "2026-11-19\t\\N\t5\t\\N\n", // the live NULL shape: 5 uncosted calls, no recorded cost
    "2026-11-20\tkimi-k2-0905-preview\t2\t0.05\n", // today's partial: in the table, never in the tile
);

/// The supervisor's own cheap-exit disposition record (goal note 6): the
/// tile's TRUE streak reads the newest such line's `streak=` field.
const T327_DISPOSITION: &str = "2026-11-19T10:00:00Z cheap-exit disposition (T258): - glm rows=0 new=0 deaths=0 bookkeeping=1 fresh=1 base=abc1234 streak=4 closed=0 laya=no dormant=no — launch skipped\n";

/// chug half of the T327 fixture: the standard facts fixture plus an optional
/// canned collector file (returned path) and an optional disposition line.
/// The site half is site_fixture_t240 — the nav + stats-section shape whose
/// bootstraps (T240 RELEASE, then T327 COST) fire.
fn fixture_t327(tsv: Option<&str>, disposition: Option<&str>) -> (Fixture, std::path::PathBuf) {
    let keep = tempfile::tempdir().unwrap();
    let chug = keep.path().join("chug");
    let site = keep.path().join("site");
    std::fs::create_dir_all(&chug).unwrap();
    std::fs::create_dir_all(&site).unwrap();
    git(&chug, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    git(&site, &["-c", "init.defaultBranch=main", "init", "-q"], None);
    chug_fixture(&chug);
    if let Some(d) = disposition {
        let log = chug.join(".chug/loopd/loopd.log");
        let mut cur = std::fs::read_to_string(&log).unwrap();
        cur.push_str(d);
        std::fs::write(&log, cur).unwrap();
    }
    site_fixture_t240(&site);
    // the docs corpus + shared style base: the T327 tile tests also pin the
    // docs.html per-day table, so the fixture carries the corpus the T241
    // fail-closed legs require (a <style> block in index.html, runbooks/*.md,
    // a README quickstart)
    let idx = site.join("index.html");
    let cur = std::fs::read_to_string(&idx).unwrap();
    std::fs::write(
        &idx,
        cur.replace(
            "<html><body>\n",
            "<html><head>\n<style>\n:root{--wrap:1060px}\n</style>\n</head><body>\n",
        ),
    )
    .unwrap();
    let rb = chug.join("runbooks");
    std::fs::create_dir_all(&rb).unwrap();
    std::fs::write(
        rb.join("alpha.md"),
        "# Runbook: alpha\n\nUse when the loop stalls.\n\n```\necho hi\n```\n",
    )
    .unwrap();
    std::fs::write(
        chug.join("README.md"),
        "# chug\n\n## Quickstart\n\ncargo install --path .\n",
    )
    .unwrap();
    commit(&chug, "docs: t327 fixture corpus", None, "2026-09-22", false);
    let tsv_path = keep.path().join("ch.tsv");
    if let Some(t) = tsv {
        std::fs::write(&tsv_path, t).unwrap();
    }
    (
        Fixture {
            _keep: keep,
            chug,
            site,
        },
        tsv_path,
    )
}

/// The T327 runner: pinned sync-now (the tile's as-of stamp rides it) and an
/// injectable collector command (None = a failing stub — the unreachable leg).
fn run_sync_t327(f: &Fixture, tsv: Option<&std::path::Path>, now: &str) -> Output {
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&f.chug);
    c.env("CHUG_SYNC_NOW", now).env("CHUG_SITE_SYNC_NO_PUSH", "1");
    match tsv {
        Some(p) => c.env("CHUG_COST_CH_CMD", format!("cat {}", p.display())),
        None => c.env("CHUG_COST_CH_CMD", "false"),
    };
    c.output().expect("spawn scripts/site-sync.sh")
}

/// Req 1+2: usable collector data renders the tile (yesterday's total, the
/// 7-day average, the TRUE streak, the as-of stamp) and the 14-character
/// pre-rendered sparkline; docs.html carries the per-day table (date, calls,
/// kimi $, glm $, total $), newest first; the region bootstraps between
/// RELEASE and STATS with the nav mirroring that order; the commit records
/// cost: ok; unchanged inputs are byte-identical with no second commit.
#[test]
fn t327_cost_data_renders_tile_sparkline_and_docs_table() {
    let (f, tsv) = fixture_t327(Some(T327_TSV), Some(T327_DISPOSITION));
    let out = run_sync_t327(&f, Some(&tsv), "2026-11-20");
    assert!(
        out.status.success(),
        "run 1: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = page(&f);
    // bootstrap: the cost section sits between the releases and stats
    // sections; the nav mirrors the order (req 4, the T240 mechanics)
    assert_eq!(count(&html, "<!-- COST:BEGIN -->"), 1);
    assert_eq!(count(&html, "<!-- COST:END -->"), 1);
    let rel_sec = html.find("<section id=\"releases\"").expect("releases bootstrapped");
    let cost_sec = html.find("<section id=\"cost\"").expect("cost section bootstrapped");
    let stats_sec = html.find("<section id=\"stats\"").expect("stats section present");
    assert!(rel_sec < cost_sec && cost_sec < stats_sec, "the cost section sits between RELEASE and STATS:\n{html}");
    let nav_rel = html.find("href=\"#releases\">releases</a>").expect("releases anchor");
    let nav_cost = html.find("href=\"#cost\">cost</a>").expect("cost anchor");
    let nav_stats = html.find("href=\"#stats\">stats</a>").expect("stats link");
    assert!(nav_rel < nav_cost && nav_cost < nav_stats, "the nav mirrors the section order:\n{html}");
    // the tile: yesterday's total (2026-11-19 = $0.35 kimi + $0.15 glm + the
    // NULL-cost group's 0), the 7-day average ($0.50 over the 7 COMPLETE days
    // ending yesterday — today's partial $0.05 never leaks in), the TRUE
    // streak from the disposition record, the as-of stamp
    let cost = region(&html, "COST");
    assert!(
        cost.contains("<b>$0.50</b><span>Langfuse spend yesterday (2026-11-19 UTC) · 7-day avg $0.07/day · TRUE streak 4 · as of 2026-11-20</span>"),
        "the tile renders yesterday/avg/streak/as-of:\n{cost}"
    );
    // the sparkline: 14 pre-rendered characters, oldest → newest, no JS —
    // 11-06 $0.75 → ▅, 11-07 $1.50 (max) → █, 11-08..11-18 (incl. the $0.00
    // laya day and the near-$0.0002 day) → ▁, 11-19 $0.50 → ▃
    assert!(
        cost.contains("<b>▅█▁▁▁▁▁▁▁▁▁▁▁▃</b><span>14-day spend shape"),
        "the pre-rendered sparkline:\n{cost}"
    );
    assert!(!cost.contains("<script"), "no JS anywhere in the region");
    // the per-day table lives in docs.html — never on the home page
    assert!(!html.contains("<tr><td>"), "no table rows on the home page:\n{html}");
    let docs_html = docs(&f);
    assert!(docs_html.contains("<section id=\"cost\">"), "docs cost section rendered");
    let rows = [
        "<tr><td>2026-11-20</td><td>2</td><td>$0.05</td><td>$0.00</td><td>$0.05</td></tr>",
        "<tr><td>2026-11-19</td><td>15</td><td>$0.35</td><td>$0.15</td><td>$0.50</td></tr>",
        "<tr><td>2026-11-18</td><td>40</td><td>$0.00</td><td>$0.00</td><td>$0.00</td></tr>",
        "<tr><td>2026-11-12</td><td>5</td><td>$0.00</td><td>$0.000200</td><td>$0.000200</td></tr>",
        "<tr><td>2026-11-07</td><td>30</td><td>$1.50</td><td>$0.00</td><td>$1.50</td></tr>",
        "<tr><td>2026-11-06</td><td>30</td><td>$0.50</td><td>$0.25</td><td>$0.75</td></tr>",
    ];
    for r in rows {
        assert!(docs_html.contains(r), "docs table row missing:\n{r}\n{docs_html}");
    }
    // newest first: today's partial (11-20) leads, 11-06 trails
    let i20 = docs_html.find("2026-11-20").unwrap();
    let i19 = docs_html.find("2026-11-19</td>").unwrap();
    let i06 = docs_html.find("2026-11-06</td>").unwrap();
    assert!(i20 < i19 && i19 < i06, "the table is newest-first:\n{docs_html}");
    // the NULL-shape pins: 11-19 counted 15 calls (7+3+5, the \N group's 5
    // included) at $0.50 (the \N cost summed as 0 — the recipe's own sum());
    // the laya-local day shows $0.00 with its 40 calls (data said zero, the
    // collector reached ClickHouse — that is an observed zero, not a fallback)
    assert!(docs_html.contains("<tr><td>2026-11-19</td><td>15</td>"), "the \\N group's calls count:\n{docs_html}");
    // the near-zero day never rounds away: $0.0002 renders at 6 decimals
    assert!(docs_html.contains("$0.000200"), "near-zero stays legible:\n{docs_html}");
    // the docs index gains the entry
    assert!(docs_html.contains("<li><a href=\"#cost\">Cost — daily Langfuse spend</a></li>"), "docs index entry:\n{docs_html}");
    // commits: one bootstrap commit + one sync commit recording cost: ok
    let subjects = all_commit_subjects(&f);
    assert!(
        subjects.iter().any(|s| s.contains("site: COST region markers bootstrap (T327)")),
        "bootstrap commit missing: {subjects:?}"
    );
    assert_eq!(sync_commits(&f), 1, "one sync commit for the filled region");
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("cost: ok"),
        "the sync commit records the cost state:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
    // idempotence: unchanged inputs (same pinned stamp) are byte-identical
    let before = page(&f);
    let docs_before = docs(&f);
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync_t327(&f, Some(&tsv), "2026-11-20");
    assert!(out2.status.success());
    assert_eq!(page(&f), before, "second run byte-identical (req 1 idempotence)");
    assert_eq!(docs(&f), docs_before, "docs byte-identical too");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit when unchanged");
}

/// The empty-output leg: the collector REACHES ClickHouse and returns zero
/// rows (an empty project, a wrong project_id, a wiped table) — the fresh
/// region gets the explicit placeholder, never a table/tile of fabricated
/// zeros, and the sync otherwise succeeds.
#[test]
fn t327_empty_collector_output_renders_placeholder_never_zeros() {
    let (f, tsv) = fixture_t327(Some(""), Some(T327_DISPOSITION));
    let out = run_sync_t327(&f, Some(&tsv), "2026-11-20");
    assert!(
        out.status.success(),
        "empty CH output must not fail the sync: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cost = region(&page(&f), "COST");
    assert!(cost.contains("<b>n/a</b>"), "the placeholder shape:\n{cost}");
    assert!(cost.contains("no cost data at this sync"), "the placeholder names the condition:\n{cost}");
    assert!(!cost.contains("$0."), "no fabricated dollar figures:\n{cost}");
    assert!(!cost.contains("TRUE streak"), "no streak without data to render beside");
    assert!(!docs(&f).contains("<section id=\"cost\">"), "no docs table from empty output:\n{}", docs(&f));
    assert_eq!(sync_commits(&f), 1);
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("cost: placeholder"),
        "the sync commit records the placeholder:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
}

/// The stale leg: a collector that was healthy and then cannot reach
/// ClickHouse leaves the PUBLISHED region content untouched, byte-identical —
/// stale beats blank — and never fails the sync (the region must never break
/// the deploy).
#[test]
fn t327_collector_failure_keeps_published_region_and_sync_succeeds() {
    let (f, tsv) = fixture_t327(Some(T327_TSV), Some(T327_DISPOSITION));
    assert!(run_sync_t327(&f, Some(&tsv), "2026-11-20").status.success());
    let before = page(&f);
    assert!(region(&before, "COST").contains("$0.50"), "published content present");
    let n = all_commit_subjects(&f).len();
    let out = run_sync_t327(&f, None, "2026-11-20"); // the collector "goes down"
    assert!(
        out.status.success(),
        "a down collector must never fail the sync: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(page(&f), before, "stale beats blank: the published region is untouched");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit for a stale-region run");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("stale beats blank"), "the warn names the doctrine: {err}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("unchanged"), "the run is a no-op");
}

/// The unreachable leg on a FRESH region (nothing published yet): the
/// placeholder lands, the sync succeeds, the commit records it.
#[test]
fn t327_collector_failure_on_a_fresh_region_writes_placeholder() {
    let (f, _tsv) = fixture_t327(None, Some(T327_DISPOSITION));
    let out = run_sync_t327(&f, None, "2026-11-20");
    assert!(
        out.status.success(),
        "a down collector must never fail the sync: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    let cost = region(&page(&f), "COST");
    assert!(cost.contains("<b>n/a</b>"), "the placeholder shape:\n{cost}");
    assert!(!cost.contains("$0."), "no fabricated zeros:\n{cost}");
    assert_eq!(sync_commits(&f), 1);
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("cost: placeholder"),
        "the sync commit records the placeholder:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
}

/// Malformed collector rows fail CLOSED: no partial render from a response
/// that does not match the recipe's shape — the region keeps published
/// content (or shows the placeholder when fresh), never the parseable subset.
#[test]
fn t327_malformed_collector_rows_fail_closed() {
    // a garbage row ahead of two good ones: the good rows must NOT render
    let (f, tsv) = fixture_t327(
        Some("not-a-date\tkimi\t3\t0.5\n2026-11-19\tkimi\t7\t0.35\n2026-11-19\tglm\t3\tzzz\n"),
        Some(T327_DISPOSITION),
    );
    let out = run_sync_t327(&f, Some(&tsv), "2026-11-20");
    assert!(out.status.success(), "malformed rows must not fail the sync");
    let cost = region(&page(&f), "COST");
    assert!(cost.contains("<b>n/a</b>"), "fail-closed placeholder:\n{cost}");
    assert!(!cost.contains("0.35"), "no partial numbers from the parseable rows:\n{cost}");
    // the same shape over PUBLISHED content: the content survives
    let (f2, tsv2) = fixture_t327(Some(T327_TSV), Some(T327_DISPOSITION));
    assert!(run_sync_t327(&f2, Some(&tsv2), "2026-11-20").status.success());
    let before = page(&f2);
    std::fs::write(&tsv2, "garbage line\n").unwrap();
    let out2 = run_sync_t327(&f2, Some(&tsv2), "2026-11-20");
    assert!(out2.status.success());
    assert_eq!(page(&f2), before, "malformed rows never gut published content");
}

/// The TRUE streak comes from the newest cheap-exit disposition's `streak=`
/// field in the supervisor's own log; with no such record (or a disposition
/// line without the field) the tile omits it — never a fabricated streak.
#[test]
fn t327_missing_disposition_streak_omits_the_field() {
    let (f, tsv) = fixture_t327(Some(T327_TSV), None);
    assert!(run_sync_t327(&f, Some(&tsv), "2026-11-20").status.success());
    let cost = region(&page(&f), "COST");
    assert!(cost.contains("$0.50"), "the tile still renders:\n{cost}");
    assert!(!cost.contains("TRUE streak"), "no streak field without a disposition record:\n{cost}");
    // a disposition line WITHOUT a streak= field: the newest record carrying
    // one wins — here none does, so the field stays omitted
    let (f2, tsv2) = fixture_t327(
        Some(T327_TSV),
        Some("2026-11-19T11:00:00Z cheap-exit disposition (T258): triage confidently empty — launch skipped\n"),
    );
    assert!(run_sync_t327(&f2, Some(&tsv2), "2026-11-20").status.success());
    assert!(
        !region(&page(&f2), "COST").contains("TRUE streak"),
        "a disposition without streak= omits the field:\n{}",
        region(&page(&f2), "COST")
    );
}

/// The marker-pair contract (T99 req 4 / T240 mechanics): malformed COST
/// markers are warn + skip — no section bootstrapped, the page untouched
/// around them, the stats sync still lands, the sync still succeeds.
#[test]
fn t327_malformed_cost_markers_are_warn_and_skip() {
    let (f, tsv) = fixture_t327(Some(T327_TSV), Some(T327_DISPOSITION));
    let html = page(&f);
    // a stray BEGIN with no END: malformed (1/0)
    std::fs::write(
        f.site.join("index.html"),
        html.replace("<section id=\"stats\"", "<!-- COST:BEGIN -->\n<section id=\"stats\""),
    )
    .unwrap();
    commit(&f.site, "operator: stray cost marker", None, "2026-09-26", false);
    let out = run_sync_t327(&f, Some(&tsv), "2026-11-20");
    assert!(
        out.status.success(),
        "malformed COST markers must not fail: {:?}",
        out.status.code()
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("malformed COST markers"), "warn names the fault: {err}");
    assert!(err.contains("skipping"), "best-effort skip: {err}");
    let html2 = page(&f);
    assert_eq!(count(&html2, "<!-- COST:BEGIN -->"), 1, "the stray marker stays");
    assert_eq!(count(&html2, "<!-- COST:END -->"), 0);
    assert_eq!(count(&html2, "<section id=\"cost\""), 0, "no section bootstrapped");
    assert!(html2.contains("<b>2/3</b>"), "stats still synced");
    assert_eq!(sync_commits(&f), 1, "the sync still lands once");
    // the docs table still renders (the collector ran; only the home region skipped)
    assert!(docs(&f).contains("<section id=\"cost\">"), "the docs table is independent of the home markers");
}

/// The docs table caps at the 14 days the collector covers: 16 days of canned
/// data render exactly 14 rows (newest first) — and the tile's 14-slot
/// sparkline covers its own complete-day window.
#[test]
fn t327_docs_table_caps_at_14_days() {
    let mut tsv = String::new();
    for d in 1..=16 {
        tsv.push_str(&format!("2026-11-{d:02}\tkimi-k2-0905-preview\t1\t0.10\n"));
    }
    let (f, tsv_path) = fixture_t327(Some(&tsv), Some(T327_DISPOSITION));
    let out = run_sync_t327(&f, Some(&tsv_path), "2026-11-17");
    assert!(out.status.success(), "run: {:?}", String::from_utf8_lossy(&out.stderr));
    let docs = docs(&f);
    assert_eq!(count(&docs, "<tr><td>"), 14, "the table caps at 14 rows:\n{docs}");
    assert!(docs.contains("<tr><td>2026-11-16</td>"), "the newest day leads:\n{docs}");
    assert!(docs.contains("<tr><td>2026-11-03</td>"), "the 14th-newest day is the tail:\n{docs}");
    assert!(!docs.contains("2026-11-02"), "day 15 is cut:\n{docs}");
    assert!(!docs.contains("2026-11-01"), "day 16 is cut:\n{docs}");
    // the tile window: the 14 complete days ending yesterday (11-03..11-16),
    // all equal — 14 identical max glyphs
    let cost = region(&page(&f), "COST");
    assert!(cost.contains("<b>██████████████</b>"), "14 flat max glyphs:\n{cost}");
    assert!(
        cost.contains("<b>$0.10</b><span>Langfuse spend yesterday (2026-11-16 UTC) · 7-day avg $0.10/day"),
        "yesterday and the 7-day average over the complete-day window:\n{cost}"
    );
}

// --- T329 — adopt.html: the adopt page, rendered from docs/adopt/ ------------
//
// scripts/site-sync.sh renders a THIRD site page — the training page that
// takes an engineer from "what is this" to "my first loop is running". The
// doctrine is T241's (fully machine-rendered, same style base, byte-stable,
// add/overwrite in the same commit as index.html) over a NEW corpus: the four
// curated fragments under docs/adopt/ (quickstart, mental-model,
// first-feature, faq — the page's fixed section order; the corpus's own
// README.md is the index and never renders; unknown fragments are warned and
// skipped). Fail-closed per T217: a missing fragment renders an explicit
// placeholder region and the sync continues; no canonical fragments at all ->
// adopt.html not written and the nav links not bootstrapped. Two publish
// gates over the composed page, both stale-beats-blank: every in-page anchor
// must resolve, and the T249 internal-info redaction patterns must not hit
// (the repo's lint covers the tracked fragments; this scan covers the page in
// the SITE repo, which no chug-repo lint reaches).

/// The adopt corpus half: the four canonical fragments (the corpus's own
/// README.md index rides along — never a section). first-feature carries an
/// in-page anchor to #faq (the only legal in-page link shape: a section id);
/// faq carries the cross-page docs#cost link that the link check must leave
/// alone.
fn chug_fixture_t329_adopt(dir: &Path) {
    let ad = dir.join("docs/adopt");
    std::fs::create_dir_all(&ad).unwrap();
    std::fs::write(
        ad.join("quickstart.md"),
        concat!(
            "# Quickstart\n\nOne command:\n\n```bash\n",
            "curl -fsSL https://chug.sh/install.sh | sh\n```\n\n",
            "Point `--model` at any provider that speaks the wire protocol.\n"
        ),
    )
    .unwrap();
    std::fs::write(
        ad.join("mental-model.md"),
        concat!(
            "# Mental model\n\n**The loop is code.**\n\n",
            "```\n[ model turn ] -> [ check: re-runs ] -> keep chugging\n```\n"
        ),
    )
    .unwrap();
    std::fs::write(
        ad.join("first-feature.md"),
        concat!(
            "# Your first feature\n\n1. write a spec with a check line\n",
            "2. file the TODO row\n\nSee the [FAQ](#faq) for cost.\n"
        ),
    )
    .unwrap();
    std::fs::write(
        ad.join("faq.md"),
        concat!(
            "# FAQ\n\n**Cost?**\n\nThe per-day table lives on the ",
            "[docs page](https://chug.sh/docs.html#cost).\n"
        ),
    )
    .unwrap();
    std::fs::write(
        ad.join("README.md"),
        "# adopt corpus\n\nThe corpus's own index — never a section.\n",
    )
    .unwrap();
    commit(dir, "docs: adopt corpus", None, "2026-09-23", false);
}

fn fixture_t329() -> Fixture {
    let f = fixture_t241();
    chug_fixture_t329_adopt(&f.chug);
    f
}

fn adopt(f: &Fixture) -> String {
    std::fs::read_to_string(f.site.join("adopt.html")).expect("read adopt.html")
}

#[test]
fn t329_adopt_page_renders_four_sections_and_links_both_navs() {
    let f = fixture_t329();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {:?}", out.status.code());
    let a = adopt(&f);
    // the four sections render in the page's fixed narrative order
    let s1 = a.find("<section id=\"quickstart\"").expect("quickstart section");
    let s2 = a.find("<section id=\"mental-model\"").expect("mental-model section");
    let s3 = a.find("<section id=\"first-feature\"").expect("first-feature section");
    let s4 = a.find("<section id=\"faq\"").expect("faq section");
    assert!(s1 < s2 && s2 < s3 && s3 < s4, "fixed section order:\n{a}");
    // fragment markdown renders through the docs renderer: fences escaped,
    // bold -> strong, the on-this-page index at the top
    assert!(a.contains("curl -fsSL https://chug.sh/install.sh | sh"), "fence verbatim");
    assert!(a.contains("<strong>The loop is code.</strong>"), "bold per the T99 text rules");
    assert!(!a.contains("<b>The loop is"), "never the old <b> shape");
    let idx = a.find("on this page").expect("page index");
    assert!(idx < s1, "the index sits at the top of the page");
    for h in ["#quickstart", "#mental-model", "#first-feature", "#faq"] {
        assert!(a.contains(&format!("href=\"{h}\"")), "index entry {h}");
    }
    // the corpus README is the index, never a section; its words never render
    assert!(!a.contains("corpus's own index"), "README.md must not render");
    // an in-fragment section anchor resolves (the link check passed); a
    // cross-page https anchor rides untouched
    assert!(a.contains("href=\"#faq\">FAQ</a>"), "in-page anchor rendered");
    assert!(
        a.contains("href=\"https://chug.sh/docs.html#cost\""),
        "cross-page anchor untouched"
    );
    // index.html: the adopt nav link, directly BEFORE the docs link (the
    // front door precedes the reference shelf)
    let html = page(&f);
    let na = html.find("href=\"https://chug.sh/adopt.html\">adopt</a>").expect("index nav adopt link");
    let nd = html.find("href=\"https://chug.sh/docs.html\">docs</a>").expect("index nav docs link");
    assert!(na < nd, "adopt precedes docs in the index nav:\n{html}");
    // docs.html (the T241 fixture ships runbooks): its nav links adopt only
    // because adopt actually rendered
    let d = docs(&f);
    assert!(d.contains("<a href=\"https://chug.sh/adopt.html\">adopt</a>"), "docs nav adopt link:\n{d}");
    // the sync commits all three pages in ONE sync commit; tree clean
    assert_eq!(sync_commits(&f), 1);
    let nameonly = git(&f.site, &["show", "--name-only", "--format="], None);
    let files = String::from_utf8_lossy(&nameonly.stdout);
    for p in ["index.html", "docs.html", "adopt.html"] {
        assert!(files.lines().any(|l| l.trim() == p), "{p} in the sync commit: {files}");
    }
    let st = git(&f.site, &["status", "--porcelain"], None);
    assert!(String::from_utf8_lossy(&st.stdout).trim().is_empty(), "clean tree");
    let subjects = all_commit_subjects(&f);
    assert!(
        subjects.iter().any(|s| s.contains("site: adopt page nav bootstrap (T329)")),
        "adopt nav bootstrap commit: {subjects:?}"
    );
    // idempotence: byte-identical re-render, no second commit, no dup links
    let before = (a.clone(), docs(&f), html.clone());
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert!(String::from_utf8_lossy(&out2.stdout).contains("unchanged"), "no-op run says so");
    assert_eq!(adopt(&f), before.0, "adopt.html byte-identical");
    assert_eq!(docs(&f), before.1, "docs.html byte-identical");
    assert_eq!(page(&f), before.2, "index.html byte-identical");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit when unchanged");
    assert_eq!(count(&page(&f), "chug.sh/adopt.html"), 1, "nav link exactly once");
}

/// The placeholder leg: a missing fragment renders an explicit placeholder
/// region in its slot — honest absence, never fabricated content — the sync
/// still succeeds, the nav still lands, and the commit records the note.
#[test]
fn t329_missing_fragments_render_placeholders_and_sync_succeeds() {
    let f = fixture_t329();
    std::fs::remove_file(f.chug.join("docs/adopt/mental-model.md")).unwrap();
    std::fs::remove_file(f.chug.join("docs/adopt/faq.md")).unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "a missing fragment must not fail the sync");
    let a = adopt(&f);
    assert!(a.contains("<section id=\"quickstart\""), "the present fragment renders");
    assert_eq!(count(&a, "No content yet"), 2, "one placeholder per missing fragment:\n{a}");
    assert!(a.contains("docs/adopt/mental-model.md"), "the placeholder names the missing file");
    assert!(a.contains("<section id=\"faq\""), "the placeholder keeps the section anchor");
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    let body = String::from_utf8_lossy(&log.stdout);
    assert!(body.contains("adopt: rendered 2 sections (mental-model faq missing)"), "the note records the placeholders:\n{body}");
    assert!(page(&f).contains("chug.sh/adopt.html"), "the nav still lands");
}

/// The empty-corpus leg (the T241 fail-closed shape): no canonical fragments
/// -> adopt.html not written, the nav link not bootstrapped, the sync
/// continues — and a stray adopt.html on the site is never swept into the
/// commit (staging is guarded by ADOPT_CHANGED, not the file's existence).
#[test]
fn t329_no_canonical_fragments_leaves_adopt_unwritten() {
    // leg 1: no docs/adopt/ dir at all
    let f = fixture_t241();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "the sync continues: {:?}", out.status.code());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("docs/adopt/"), "warn names the missing corpus: {err}");
    assert!(!f.site.join("adopt.html").exists(), "no page from a missing corpus");
    assert!(!page(&f).contains("chug.sh/adopt.html"), "never a link to a page that is not rendered");
    // leg 2: the dir exists but only the corpus's own README.md
    std::fs::create_dir_all(f.chug.join("docs/adopt")).unwrap();
    std::fs::write(f.chug.join("docs/adopt/README.md"), "# adopt corpus\n\nindex only.\n").unwrap();
    commit(&f.chug, "docs: adopt index only", None, "2026-09-23", true);
    std::fs::write(f.site.join("adopt.html"), "<html>sentinel-adopt</html>\n").unwrap();
    let before = std::fs::read(f.site.join("adopt.html")).unwrap();
    let n = all_commit_subjects(&f).len();
    // make the stats region move so THIS sync has a real commit to inspect —
    // the sweep guard (no adopt.html staging without ADOPT_CHANGED) then has
    // teeth: index.html moves while the stray sentinel must ride untouched
    let mut log = std::fs::read_to_string(f.chug.join(".chug/loopd/loopd.log")).unwrap();
    log.push_str("2026-09-27T10:00:00Z cycle OK: third\n");
    std::fs::write(f.chug.join(".chug/loopd/loopd.log"), log).unwrap();
    commit(&f.chug, "chore: one more cycle", None, "2026-09-27", true);
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    let err2 = String::from_utf8_lossy(&out2.stderr);
    assert!(err2.contains("no adopt fragments"), "warn names the empty corpus: {err2}");
    assert_eq!(
        std::fs::read(f.site.join("adopt.html")).unwrap(),
        before,
        "adopt.html untouched (fail-closed)"
    );
    assert!(!page(&f).contains("chug.sh/adopt.html"), "no nav link either");
    let nameonly = git(&f.site, &["show", "--name-only", "--format="], None);
    let files = String::from_utf8_lossy(&nameonly.stdout);
    assert!(files.lines().any(|l| l.trim() == "index.html"), "the sync commit landed: {files}");
    assert!(
        !files.lines().any(|l| l.trim() == "adopt.html"),
        "the stray adopt.html is NOT swept into the fail-closed sync commit: {files}"
    );
    assert_eq!(all_commit_subjects(&f).len(), n + 1, "the stats sync still lands once");
}

/// The link-check gate: a dangling in-page anchor is broken training content —
/// the page does not land and the previously published one rides byte-identical
/// (stale beats blank); the sync never fails.
#[test]
fn t329_dangling_anchor_keeps_published_page_and_sync_succeeds() {
    let f = fixture_t329();
    assert!(run_sync(&f, true).status.success());
    let before = adopt(&f);
    let n = all_commit_subjects(&f).len();
    std::fs::write(
        f.chug.join("docs/adopt/faq.md"),
        "# FAQ\n\nSee also [nothing](#nope).\n",
    )
    .unwrap();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "a dangling anchor must not fail the sync");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("dangling in-page anchor"), "the warn names the gate: {err}");
    assert!(err.contains("nope"), "the warn names the anchor: {err}");
    assert_eq!(adopt(&f), before, "stale beats blank: the published page rides");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit for a blocked render");
    // the fix-forward: repair the link and the next sync publishes again
    std::fs::write(
        f.chug.join("docs/adopt/faq.md"),
        "# FAQ\n\nSee also [the top](#quickstart).\n",
    )
    .unwrap();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    let a2 = adopt(&f);
    assert!(a2.contains("href=\"#quickstart\""), "the repaired link renders:\n{a2}");
    assert_eq!(all_commit_subjects(&f).len(), n + 1, "the recovery sync lands");
}

/// The hygiene gate: a T249 redaction-pattern hit in a fragment blocks the
/// publish — the composed page is scanned before it may land, the published
/// page rides byte-identical, and the warn names the pattern class (never the
/// line content). The literal itself is composed from pieces at runtime: this
/// file is tracked and tests/internal_info_lint.rs scans every tracked line,
/// so no source line here may carry a whole redaction literal.
#[test]
fn t329_hygiene_hit_blocks_publish_and_keeps_published_page() {
    let f = fixture_t329();
    assert!(run_sync(&f, true).status.success());
    let before = adopt(&f);
    let n = all_commit_subjects(&f).len();
    let needle = format!("cen{}l", "tra"); // the T249 class-4 literal, composed
    std::fs::write(
        f.chug.join("docs/adopt/mental-model.md"),
        format!("# Mental model\n\nThe {needle} routing hub owns this.\n"),
    )
    .unwrap();
    commit(&f.chug, "docs: poisoned fragment", None, "2026-09-24", false);
    let out = run_sync(&f, true);
    assert!(out.status.success(), "a hygiene hit must not fail the sync");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("internal-info pattern hit"), "the warn names the gate: {err}");
    assert!(err.contains("monorepo-intermediate"), "the warn names the pattern class: {err}");
    assert!(!err.contains(&needle), "the line content is never printed: {err}");
    assert_eq!(adopt(&f), before, "stale beats blank: the published page rides");
    assert_eq!(all_commit_subjects(&f).len(), n, "no commit for a blocked render");
    // the clean corpus publishes (the gate is not a false-positive machine):
    // the un-poisoned re-run lands again — byte-for-byte the original good
    // render, since the corpus is restored exactly
    chug_fixture_t329_adopt(&f.chug);
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(adopt(&f), before, "the restored corpus re-renders the original page byte-for-byte");
    assert_eq!(
        all_commit_subjects(&f).len(),
        n,
        "the restored render is byte-identical to the riding page — nothing to commit"
    );
}

/// The corpus walk: an unknown fragment is warned and skipped — the page
/// renders exactly the four curated sections, never whatever lands in the
/// directory (a stray note cannot leak onto the public site).
#[test]
fn t329_unknown_fragment_is_warned_and_skipped() {
    let f = fixture_t329();
    std::fs::write(
        f.chug.join("docs/adopt/stray-notes.md"),
        "# stray\n\nnote to self: nothing here is for the public site\n",
    )
    .unwrap();
    commit(&f.chug, "docs: stray note", None, "2026-09-24", true);
    let out = run_sync(&f, true);
    assert!(out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unknown adopt fragment docs/adopt/stray-notes.md"), "warn names the file: {err}");
    assert!(err.contains("skipped"), "the warn says what happened: {err}");
    let a = adopt(&f);
    assert!(!a.contains("note to self"), "the stray note never renders:\n{a}");
    assert_eq!(count(&a, "<section id="), 4, "exactly the four canonical sections:\n{a}");
    let log = git(&f.site, &["log", "-1", "--format=%B"], None);
    assert!(
        String::from_utf8_lossy(&log.stdout).contains("adopt: rendered 4 sections"),
        "the note is unchanged by the stray file:\n{}",
        String::from_utf8_lossy(&log.stdout)
    );
}

// --- T331 — the risk-gate story section on docs.html -------------------------
//
// docs/risk-gate.md — the leadership-review answer to "what stops it from
// doing something bad?" — renders through the SAME docs-page pipeline
// (docs_md) as one section on docs.html, after the runbooks and before the
// cost table. The adopt page's safety FAQ entry (docs/adopt/faq.md) links
// the section at https://chug.sh/docs.html#risk-gate — a cross-page anchor
// that must resolve on every render, which is why the section id exists in
// BOTH shapes: fail-closed per the T217 letter, a missing or unreadable
// source renders the explicit placeholder region in its slot (honest
// absence, never fabricated content, never filler) and the sync continues.

/// The risk-gate story source for the fixture's docs/risk-gate.md: an H1
/// (the section h2 + index label), a gates table, the escaped pipe the T216
/// rule requires inside a table cell, and a fence.
fn write_riskgate_md(dir: &Path) {
    let docs_dir = dir.join("docs");
    std::fs::create_dir_all(&docs_dir).unwrap();
    std::fs::write(
        docs_dir.join("risk-gate.md"),
        concat!(
            "# Why it's safe to let the loop write code\n\n",
            "The question this page answers: **what stops it from doing something bad?**\n\n",
            "## The gates\n\n",
            "| Gate | Command |\n",
            "|---|---|\n",
            "| Full suite | `cargo nextest run --release` |\n",
            "| Docs-only classifier | `git diff --name-only main...<branch> \\| grep -qvE '\\.md$'` |\n\n",
            "```bash\n",
            "chug run --spec SPEC.md --goal \"Build X\"\n",
            "```\n"
        ),
    )
    .unwrap();
}

/// The adopt FAQ fragment with the safety entry's cross-page link to the
/// rendered risk-gate section (the T329 FAQ corpus, plus the T331 link).
fn write_adopt_faq_with_riskgate_link(dir: &Path) {
    std::fs::write(
        dir.join("docs/adopt/faq.md"),
        concat!(
            "# FAQ\n\n**Cost?**\n\nThe per-day table lives on the ",
            "[docs page](https://chug.sh/docs.html#cost).\n\n",
            "**Is it safe to point at my repo?**\n\nThe trust boundary is ",
            "mechanical. The full story is on the [risk-gate page]",
            "(https://chug.sh/docs.html#risk-gate).\n"
        ),
    )
    .unwrap();
}

/// The T331 fixture: the T241 docs corpus + the T329 adopt corpus, then
/// docs/risk-gate.md and the adopt FAQ cross-link, committed.
fn fixture_t331() -> Fixture {
    let f = fixture_t329();
    write_riskgate_md(&f.chug);
    write_adopt_faq_with_riskgate_link(&f.chug);
    commit(
        &f.chug,
        "docs: t331 risk-gate story + adopt FAQ cross-link",
        None,
        "2026-09-24",
        false,
    );
    f
}

/// The T331 runner with the T327 canned collector (for the cost-ordering
/// pin) — CHUG_SYNC_NOW 2026-11-20, the T327_TSV window.
fn run_sync_t331_with_cost(f: &Fixture, tsv: &Path) -> Output {
    let mut c = Command::new("bash");
    c.arg(script_path()).arg(&f.site).arg(&f.chug);
    c.env("CHUG_SYNC_NOW", "2026-11-20");
    c.env("CHUG_SITE_SYNC_NO_PUSH", "1");
    c.env("CHUG_COST_CH_CMD", format!("cat {}", tsv.display()));
    c.output().expect("spawn scripts/site-sync.sh")
}

#[test]
fn t331_risk_gate_md_renders_as_a_docs_section_after_the_runbooks() {
    let f = fixture_t331();
    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {:?}", out.status.code());
    let d = docs(&f);
    // the section exists and orders after the runbook sections (the cost
    // ordering is pinned separately, with collector data)
    let qs = d.find("<section id=\"quickstart\"").expect("quickstart section");
    let rb = d.find("<section id=\"runbook-alpha\"").expect("runbook section");
    let rg = d
        .find("<section id=\"risk-gate\"")
        .expect("risk-gate section rendered");
    assert!(qs < rb && rb < rg, "quickstart leads, runbooks precede the risk-gate section");
    // rendered through the SAME docs_md pipeline: H1 -> section h2, the
    // gates table as <table>, the escaped pipe as one cell's literal
    // content (T216), the fence verbatim
    let sec = docs_section(&d, "risk-gate");
    assert!(
        sec.contains("<h2>Why it's safe to let the loop write code</h2>"),
        "H1 -> section h2:\n{sec}"
    );
    assert!(sec.contains("<table>"), "the gates table renders as a table:\n{sec}");
    assert!(
        sec.contains("<code>git diff --name-only main...&lt;branch&gt; | grep -qvE '\\.md$'</code>"),
        "the escaped pipe stays one cell's literal content (T216):\n{sec}"
    );
    assert!(sec.contains("chug run --spec SPEC.md"), "fence content renders:\n{sec}");
    // the on-this-page index carries the section at the top, labeled by the
    // source H1
    let idx = d.find("href=\"#risk-gate\"").expect("index entry");
    assert!(idx < qs, "the index entry sits at the top of the page");
    assert!(
        d.contains("<li><a href=\"#risk-gate\">Why it's safe to let the loop write code</a></li>"),
        "index label is the source H1:\n{d}"
    );
    // the adopt page's safety FAQ cross-link is an https href — rides the
    // render untouched (the T329 cross-page rule)
    let a = adopt(&f);
    assert!(
        a.contains("href=\"https://chug.sh/docs.html#risk-gate\""),
        "the adopt FAQ links the rendered section:\n{a}"
    );
    // idempotence: unchanged inputs, byte-identical page, no second commit
    let before = d;
    let n = all_commit_subjects(&f).len();
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert_eq!(docs(&f), before, "unchanged corpus renders byte-identical docs.html");
    assert_eq!(all_commit_subjects(&f).len(), n, "no second commit");
}

#[test]
fn t331_risk_gate_section_precedes_the_cost_table() {
    let f = fixture_t331();
    let tsv = f._keep.path().join("t331.tsv");
    std::fs::write(&tsv, T327_TSV).unwrap();
    let out = run_sync_t331_with_cost(&f, &tsv);
    assert!(out.status.success(), "sync failed: {:?}", out.status.code());
    let d = docs(&f);
    let rg = d.find("<section id=\"risk-gate\"").expect("risk-gate section rendered");
    let cost = d.find("<section id=\"cost\">").expect("cost table rendered");
    assert!(rg < cost, "the cost table still rides at the page's end (T327)");
    let idx = d.find("href=\"#risk-gate\"").expect("risk-gate index entry");
    let cost_idx = d.find("href=\"#cost\"").expect("cost index entry");
    assert!(idx < cost_idx, "the index keeps the same order: runbooks, risk gate, cost");
}

#[test]
fn t331_missing_risk_gate_source_renders_placeholder_and_sync_succeeds() {
    let f = fixture_t331();
    std::fs::remove_file(f.chug.join("docs/risk-gate.md")).unwrap();
    commit(
        &f.chug,
        "docs: remove the risk-gate source",
        None,
        "2026-09-25",
        true,
    );
    let out = run_sync(&f, true);
    assert!(out.status.success(), "the sync continues: {:?}", out.status.code());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("docs/risk-gate.md"),
        "the warn names the missing source: {err}"
    );
    let d = docs(&f);
    let sec = docs_section(&d, "risk-gate");
    assert!(sec.contains("No content yet"), "the placeholder says what is absent:\n{sec}");
    assert!(
        sec.contains("docs/risk-gate.md"),
        "the placeholder names the source file:\n{sec}"
    );
    assert!(
        !sec.contains("what stops it from doing something bad"),
        "no fabricated content in the placeholder:\n{sec}"
    );
    // the id + index entry still exist in the placeholder shape, so the
    // adopt page's cross-page link resolves on every render
    assert!(
        d.contains("<li><a href=\"#risk-gate\">Risk gate</a></li>"),
        "the index entry rides the placeholder:\n{d}"
    );
    let a = adopt(&f);
    assert!(
        a.contains("href=\"https://chug.sh/docs.html#risk-gate\""),
        "the adopt cross-link still rides:\n{a}"
    );
    // the sync still lands once (the placeholder changed docs.html)
    assert_eq!(sync_commits(&f), 1, "the placeholder render still commits");
    // recovery: restore the source -> the next sync renders the body into
    // the same slot
    write_riskgate_md(&f.chug);
    commit(
        &f.chug,
        "docs: restore the risk-gate source",
        None,
        "2026-09-26",
        false,
    );
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    let d2 = docs(&f);
    assert!(
        docs_section(&d2, "risk-gate").contains("what stops it from doing something bad"),
        "the restored source renders into the same slot"
    );
    assert!(
        d2.contains("<li><a href=\"#risk-gate\">Why it's safe to let the loop write code</a></li>"),
        "the index label follows the restored source"
    );
}

// --- T349 — the SHIPPED feed: the site renders shipped work straight from
// the TODO.md done record, and FEATURES.md's flagship backfill parses through
// the grid's own extractor legs ----------------------------------------------

/// T349 chug fixture: the base files the sync's facts need, then `n` item
/// commits at staggered dates (the feed's date source). Returns the commit
/// refs newest-last in commit order.
fn chug_fixture_t349_base(dir: &Path) {
    std::fs::create_dir_all(dir.join(".chug/loopd")).unwrap();
    std::fs::write(dir.join("EVALUATION.md"), "# EVALUATION — fixture\n").unwrap();
    std::fs::write(dir.join("TODO.md"), "# TODO\n").unwrap();
    std::fs::write(
        dir.join("FEATURES.md"),
        "# FEATURES\n\n| # | Feature | What | Benchmark |\n|---|---------|------|-----------|\n| F1 | **delegate collect** | Structured child result. | Claude Code Task |\n",
    )
    .unwrap();
    std::fs::write(dir.join(".chug/loopd/loopd.log"), "2026-09-19T00:00:00Z cycle OK: seed\n").unwrap();
    commit(dir, "seed: fixture base", None, "2026-09-19", false);
}

fn chug_fixture_t349_items(dir: &Path, n: usize, day0: usize) -> Vec<String> {
    let mut refs = Vec::new();
    for i in 0..n {
        let day = format!("2026-09-{:02}", day0 + i);
        std::fs::write(dir.join(format!("item{i}.txt")), format!("item {i}\n")).unwrap();
        refs.push(commit(dir, &format!("t{}: feed item {}", i + 1, i + 1), None, &day, true));
    }
    refs
}

/// T349 site fixture: the T241 anchors (style/nav/get-started, so docs.html
/// can render) + a feature grid (the SHIPPED feed renders directly above it)
/// + the STATS markers.
fn site_fixture_t349(dir: &Path) {
    let page = concat!(
        "<html><head><style>body{color:#eee}</style></head><body>\n",
        "<nav class=\"nav\"><div class=\"wrap nav-in\">\n",
        "  <a class=\"brand\" href=\"https://chug.sh\">chug<b>_</b></a>\n",
        "  <div class=\"nav-links\">\n",
        "    <a href=\"https://github.com/tampajohn/chug\">source</a>\n",
        "  </div>\n",
        "</div></nav>\n",
        "<section id=\"get-started\"><div class=\"wrap\"><p>install</p></div></section>\n",
        "<section id=\"features\"><div class=\"wrap\">\n",
        "  <div class=\"grid\">\n",
        "  <div class=\"card\">\n",
        "    <h3><span>tools</span> x4</h3>\n",
        "    <p>read_file and friends.</p>\n",
        "  </div>\n",
        "  </div>\n",
        "</div></section>\n",
        "<!-- STATS:BEGIN -->\n<div class=\"stats\"><b>stale</b></div>\n<!-- STATS:END -->\n",
        "</body></html>\n",
    );
    std::fs::write(dir.join("index.html"), page).unwrap();
    commit(dir, "initial page", None, "2026-09-18", false);
}

/// `t349_done_row` — a done row citing `ref` ("" = undated: no hex in notes),
/// `(title, spec)` empty cells = the malformed shape the feed must skip.
fn t349_done_row(id: &str, title: &str, spec: &str, r: &str) -> String {
    let notes = if r.is_empty() {
        "done — no ref cited".to_string()
    } else {
        format!("done {r} — landed")
    };
    format!("| {id} | {title} | {spec} | 1 | done | {notes} |\n")
}

#[test]
fn t349_shipped_feed_renders_newest_first_with_spec_links() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    // three verifiable commits, oldest first: 09-20, 09-21, 09-22
    let refs = chug_fixture_t349_items(&f.chug, 3, 20);
    let mut todo = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    // feed order must be newest-first: T34 (09-22) above T33 (09-21) above T32
    todo.push_str(&t349_done_row("T34", "gamma item", "specs/gamma.md", &refs[2]));
    todo.push_str(&t349_done_row("T33", "beta item", "specs/beta.md", &refs[1]));
    todo.push_str(&t349_done_row("T32", "alpha item", "specs/alpha.md", &refs[0]));
    // a malformed done row (empty spec cell): skipped, named count, not fatal
    todo.push_str(&t349_done_row("T31", "broken row", "", &refs[0]));
    // a done row citing no verifiable ref: skipped by the timeline's own
    // req-5 discipline (a git-derived date is verified or the row does not
    // render), with a named count — never fatal
    todo.push_str(&t349_done_row("T30", "unref item", "specs/unref.md", ""));
    // a pending row: not shipped, absent from the feed
    todo.push_str("| T5 | pending item | specs/pending.md | 2 | todo | x |\n");
    std::fs::write(f.chug.join("TODO.md"), todo).unwrap();
    site_fixture_t349(&f.site);   // writes + commits the page

    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {}", String::from_utf8_lossy(&out.stderr));
    let page = page(&f);
    // the region bootstrapped directly above the grid (never inside it), with
    // exactly one marker pair
    assert_eq!(count(&page, "<!-- SHIPPED:BEGIN -->"), 1);
    assert_eq!(count(&page, "<!-- SHIPPED:END -->"), 1);
    let grid_at = page.find("<div class=\"grid\">").unwrap();
    let ship_at = page.find("<!-- SHIPPED:BEGIN -->").unwrap();
    assert!(ship_at < grid_at, "the feed renders above the grid container");
    let ship = region(&page, "SHIPPED");
    // newest first
    let t34 = ship.find("<code>T34</code>").expect("newest item rendered");
    let t33 = ship.find("<code>T33</code>").expect("middle item rendered");
    let t32 = ship.find("<code>T32</code>").expect("oldest item rendered");
    assert!(t34 < t33 && t33 < t32, "newest-first ordering: {ship}");
    // dates + whole titles + spec links
    assert!(ship.contains("<li>2026-09-22 · <code>T34</code> · <a href=\"https://github.com/tampajohn/chug/blob/main/specs/gamma.md\">gamma item</a></li>"), "item line shape: {ship}");
    assert!(ship.contains("<li>2026-09-21 · <code>T33</code> · <a href=\"https://github.com/tampajohn/chug/blob/main/specs/beta.md\">beta item</a></li>"));
    assert!(ship.contains("<li>2026-09-20 · <code>T32</code> · <a href=\"https://github.com/tampajohn/chug/blob/main/specs/alpha.md\">alpha item</a></li>"));
    // the malformed row is absent; the pending row is absent
    assert!(!ship.contains("T31"), "malformed row skipped: {ship}");
    assert!(!ship.contains("T5"), "pending row not shipped: {ship}");
    // the unref'd row is skipped too (named, like the timeline's audit)
    assert!(!ship.contains("T30"), "unverifiable ref must not render: {ship}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("1 done row(s) cite no verifiable commit ref"), "named unref skip: {}", String::from_utf8_lossy(&out.stderr));
    // the T349 grid-annotation rule, exactly once
    assert_eq!(count(&ship, "The curated feature grid below is the flagship list; this feed is the complete shipped record."), 1);
    // the malformed skip is named, never fatal
    assert!(String::from_utf8_lossy(&out.stderr).contains("1 malformed done row"), "named skip count: {}", String::from_utf8_lossy(&out.stderr));
    // the bootstrap + the sync commit both landed
    assert!(all_commit_subjects(&f).iter().any(|m| m.contains("site: SHIPPED region markers bootstrap (T349)")));
    let bodies = git(&f.site, &["log", "--format=%B"], None);
    let bodies = String::from_utf8_lossy(&bodies.stdout);
    assert!(bodies.contains("shipped: ok (3 done rows, 1 malformed skipped)"), "commit records the feed state: {bodies}");
    // idempotent: a second sync changes nothing
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    assert!(String::from_utf8_lossy(&out2.stdout).contains("page unchanged") || String::from_utf8_lossy(&out2.stdout).contains("nothing to commit"), "feed is deterministic: {}", String::from_utf8_lossy(&out2.stdout));
}

#[test]
fn t349_feed_freshness_flip_appears_without_hand_edits() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    let refs = chug_fixture_t349_items(&f.chug, 2, 20);
    // one shipped row + one PENDING row the loop will flip done later
    let mut todo = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    todo.push_str(&t349_done_row("T33", "older item", "specs/older.md", &refs[0]));
    todo.push_str("| T50 | fresh item | specs/fresh.md | 1 | todo | x |\n");
    std::fs::write(f.chug.join("TODO.md"), todo).unwrap();
    site_fixture_t349(&f.site);   // writes + commits the page

    let out1 = run_sync(&f, true);
    assert!(out1.status.success());
    let p1 = region(&page(&f), "SHIPPED");
    assert!(!p1.contains("T50"), "pending row absent before the flip: {p1}");

    // the flip: operator/loop edits TODO.md's status cell and commits the row
    // (the standard orchestrator flip). The SITE page is never touched.
    let flipped = format!(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T33 | older item | specs/older.md | 1 | done | done {} — landed |\n| T50 | fresh item | specs/fresh.md | 1 | done | done {} — landed |\n",
        refs[0], refs[1]
    );
    std::fs::write(f.chug.join("TODO.md"), flipped).unwrap();
    commit(&f.chug, "flip t50: fresh item shipped", None, "2026-09-21", false);

    // no hand edit anywhere on the site: the NEXT sync alone picks the flip up
    let out2 = run_sync(&f, true);
    assert!(out2.status.success());
    let p2 = region(&page(&f), "SHIPPED");
    let t50 = p2.find("<code>T50</code>").expect("the flipped item renders at the next sync — the acceptance test");
    let t33 = p2.find("<code>T33</code>").expect("older item still rendered");
    assert!(t50 < t33, "the newly flipped item is the newest: {p2}");
    assert!(p2.contains("<li>2026-09-21 · <code>T50</code> · <a href=\"https://github.com/tampajohn/chug/blob/main/specs/fresh.md\">fresh item</a></li>"), "fresh item with its date + spec link: {p2}");
    assert!(String::from_utf8_lossy(&out2.stdout).contains("shipped ok"), "state recorded: {}", String::from_utf8_lossy(&out2.stdout));
}

#[test]
fn t349_empty_done_set_renders_placeholder() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    chug_fixture_t349_items(&f.chug, 1, 20);
    // every row pending — nothing shipped yet
    std::fs::write(
        f.chug.join("TODO.md"),
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T5 | pending item | specs/pending.md | 2 | todo | x |\n",
    )
    .unwrap();
    site_fixture_t349(&f.site);   // writes + commits the page

    let out = run_sync(&f, true);
    assert!(out.status.success());
    let ship = region(&page(&f), "SHIPPED");
    assert!(ship.contains("No shipped items recorded yet — TODO.md has no done rows; the feed fills in as the loop flips rows done."), "explicit empty shape: {ship}");
    assert!(!ship.contains("<li>"), "no item lines in the placeholder: {ship}");
    assert!(ship.contains("The curated feature grid below is the flagship list; this feed is the complete shipped record."), "the annotation rule holds on the empty shape");
    assert!(String::from_utf8_lossy(&out.stdout).contains("shipped placeholder"), "state recorded: {}", String::from_utf8_lossy(&out.stdout));
    let bodies = git(&f.site, &["log", "--format=%B"], None);
    let bodies = String::from_utf8_lossy(&bodies.stdout);
    assert!(bodies.contains("shipped: placeholder (0 done rows, 0 malformed skipped)"), "commit records the empty state: {bodies}");
}

#[test]
fn t349_feed_caps_at_20_and_collapses_to_the_full_record() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    // 22 shipped items at 22 distinct dates: the newest 20 render in full,
    // the older 2 collapse to one named line
    let refs = chug_fixture_t349_items(&f.chug, 22, 1);
    let mut todo = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    for (i, r) in refs.iter().enumerate() {
        todo.push_str(&t349_done_row(&format!("T{}", i + 1), &format!("cap item {}", i + 1), &format!("specs/c{}.md", i + 1), r));
    }
    std::fs::write(f.chug.join("TODO.md"), todo).unwrap();
    site_fixture_t349(&f.site);   // writes + commits the page

    let out = run_sync(&f, true);
    assert!(out.status.success());
    let ship = region(&page(&f), "SHIPPED");
    assert_eq!(count(&ship, "<li>"), 21, "20 items + the one collapse line: {ship}");
    // newest first across the cap: T22 (2026-09-22) leads
    let t22 = ship.find("<code>T22</code>").unwrap();
    let t3 = ship.find("<code>T3</code>").unwrap();
    assert!(t22 < t3, "newest-first across the cap boundary");
    assert!(ship.contains("…and 2 earlier shipped items"), "the collapse is named with its count: {ship}");
    // docs.html was not rendered (no runbooks) — the collapse must NOT link a
    // page that does not exist; the pointer names the source instead
    assert!(ship.contains("the full record lives in TODO.md"), "no dangling docs link: {ship}");
    assert!(!ship.contains("docs.html#shipped"), "never a link to a page that is not rendered: {ship}");
    assert!(String::from_utf8_lossy(&out.stdout).contains("shipped ok"), "state recorded: {}", String::from_utf8_lossy(&out.stdout));
}

#[test]
fn t349_features_backfill_parses_the_flagship_rows() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    let refs = chug_fixture_t349_items(&f.chug, 2, 20);
    let mut todo = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    todo.push_str(&t349_done_row("T34", "gamma item", "specs/gamma.md", &refs[1]));
    todo.push_str(&t349_done_row("T33", "beta item", "specs/beta.md", &refs[0]));
    std::fs::write(f.chug.join("TODO.md"), todo).unwrap();
    // the REAL backfilled FEATURES.md (this repo's own, copied at test time)
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root (the runtime resolution the manifest-dir pin mandates)");
    std::fs::copy(root.join("FEATURES.md"), f.chug.join("FEATURES.md")).unwrap();
    site_fixture_t349(&f.site);   // writes + commits the page

    let out = run_sync(&f, true);
    assert!(out.status.success(), "sync failed: {}", String::from_utf8_lossy(&out.stderr));
    let ft = region(&page(&f), "FEATURES");
    // same-or-more cards: the 15 pre-existing F-rows plus the 7 backfilled ones
    // all synthesize (>= 22 F-cards + the fixture's own card)
    assert!(count(&ft, "<div class=\"card\">") >= 23, "the backfill adds cards, none are lost: {}", count(&ft, "<div class=\"card\">"));
    // every backfilled flagship renders as a landed card, name cell parsed
    // per the existing extractor legs (text before the em-dash, LANDED badge)
    for (id, name) in [
        ("F17", "Dormant-mode cost program"),
        ("F18", "Ideation lane"),
        ("F19", "Loop subcommand"),
        ("F20", "Install profiles"),
        ("F21", "Decision-model providers: local Laya + proxy Jev"),
        ("F22", "Meta-harness pack"),
        ("F23", "Adopt + risk-gate docs surface"),
    ] {
        let card = format!("<h3><span>{name}</span><i class=\"q\">landed</i></h3>");
        assert!(ft.contains(&card), "{id} renders as a landed card: {ft}");
    }
    // the what cells parse per the existing legs: backticks -> <code>, no raw
    // pipes ever phantom-splitting a cell
    assert!(ft.contains("<code>chug loop</code>"), "F19's backtick cell converts: {ft}");
    assert!(ft.contains("<code>CHUG_DECISION_PROVIDER=auto,local,proxy</code>"), "F21's backtick cell converts: {ft}");
    // the shipped feed rides the same page (2 done rows, uncapped below 20)
    let ship = region(&page(&f), "SHIPPED");
    assert!(ship.contains("<code>T34</code>") && ship.contains("<code>T33</code>"), "the feed coexists with the grid: {ship}");
}

#[test]
fn t349_docs_page_gains_the_full_shipped_list() {
    let f = fixture();
    chug_fixture_t349_base(&f.chug);
    let refs = chug_fixture_t349_items(&f.chug, 22, 1);
    let mut todo = String::from(
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n",
    );
    for (i, r) in refs.iter().enumerate() {
        todo.push_str(&t349_done_row(&format!("T{}", i + 1), &format!("cap item {}", i + 1), &format!("specs/c{}.md", i + 1), r));
    }
    std::fs::write(f.chug.join("TODO.md"), todo).unwrap();
    chug_fixture_t241_runbooks(&f.chug, false);   // writes + commits the corpus
    site_fixture_t349(&f.site);   // writes + commits the page

    let out = run_sync(&f, true);
    assert!(out.status.success());
    let d = docs(&f);
    // the slot + anchor exist, after the risk-gate story and before the cost table
    let rg = d.find("id=\"risk-gate\"").unwrap();
    let sh = d.find("id=\"shipped\"").unwrap();
    assert!(rg < sh, "the shipped feed sits after the risk-gate story");
    if let Some(co) = d.find("id=\"cost\"") {
        assert!(sh < co, "the shipped feed sits before the cost table");
    } // no cost section on a fresh fixture (no collector data) — absent is fine
    assert!(d.contains("<li><a href=\"#shipped\">Shipped — the complete record</a></li>"), "on-this-page entry");
    // the FULL list: the docs page is uncapped — all 22 items render
    let sec = docs_section(&d, "shipped");
    assert_eq!(count(&sec, "<li>"), 22, "the docs feed is the complete record: {sec}");
    assert!(sec.contains("<li>2026-09-22 · <code>T22</code> · <a href=\"https://github.com/tampajohn/chug/blob/main/specs/c22.md\">cap item 22</a></li>"), "newest-first with dates + spec links: {sec}");
    // while the home page's feed caps at 20 + collapses — pointing at the docs
    // page, which exists now (rendered this same sync)
    let ship = region(&page(&f), "SHIPPED");
    assert_eq!(count(&ship, "<li>"), 21);
    assert!(ship.contains("the full record on the <a href=\"https://chug.sh/docs.html#shipped\">docs page</a>"), "the collapse links the live docs page: {ship}");
}

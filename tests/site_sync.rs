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
    assert!(!html.contains("fallback item"), "fallback row must dedupe against hand entry");
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

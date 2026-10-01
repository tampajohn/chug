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
/// TODO.md, two landed-item commits (t3 gate "suite 40 unit", t4 gate
/// "nextest 55/55") and a newest gate-less chore commit (walk-back leg).
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
        Some("review+post-merge nextest 55/55 + clippy"),
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
    // current test count = newest gate-quoting commit message (t4's nextest
    // 55/55 — NOT t3's older suite 40, NOT the gate-less chore HEAD)
    assert!(html.contains("<b>55</b>"), "tests card:\n{html}");
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
    // finding 3) on the matched existing card, which keeps position and prose
    assert!(
        ft.contains("<h3><span>delegate</span> sub-agents<i class=\"q\">landed</i></h3>"),
        "matched card badge: {ft}"
    );
    assert!(ft.contains("<p>Launch and collect child runs.</p>"));
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

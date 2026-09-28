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

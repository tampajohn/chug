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

// --- T99 — timeline + feature-grid regions -----------------------------------

/// A chug fixture with a done-row TODO.md whose notes cite real commits
/// (hashes come back from `commit`), plus a FEATURES.md with one checked-off
/// row, one TODO-referenced row and one queued row.
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
            "| F1 | ~~**delegate collect**~~ — LANDED T69 (cycle 33) | Structured child result: `goal_complete` summary + refs. | Claude Code Task |\n",
            "| F2 | **Streaming UX** | Text deltas to `sinks` as they arrive (TUI live typing). | all benchmarks |\n",
            "| F3 | **Web search** | Provider-pluggable search tool complementing web_fetch, with a deliberately long one-line what that runs well past the one hundred and sixty byte cap on purpose so the ellipsis lands. | Claude Code |\n"
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
    // req 1+5: one entry per verified done row, dated from git, newest first
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
    let i11 = tl.find(&entry(&refs[5])).unwrap();
    let i5 = tl.find(&entry(&refs[1])).unwrap();
    assert!(i11 < i5, "machine entries must be newest-first");
    // facts-only: the machine block (newest entry to region end) has no <p>
    assert!(!tl[i11..].contains("<p>"), "machine entries must not narrate");
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
    assert!(i25 < i24, "newest first");
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
    // landed badge (checked off in FEATURES.md) on the matched existing card,
    // which keeps its position and prose
    assert!(
        ft.contains("<h3><span>delegate</span> sub-agents<i class=\"q\">landed</i></h3>"),
        "matched card badge: {ft}"
    );
    assert!(ft.contains("<p>Launch and collect child runs.</p>"));
    assert!(ft.contains("    <div class=\"card\">\n      <h3><span>tools</span> ×13</h3>\n      <p>read_file etc.</p>\n    </div>"),
        "non-F card verbatim: {ft}");
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

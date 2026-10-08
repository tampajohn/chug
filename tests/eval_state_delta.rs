//! T260 — eval prompt slimming: the state+delta read path.
//!
//! The Phase-1 evaluator's dominant cost is the per-call corpus re-read
//! (~134k fresh + ~1.7M cache-read per orchestrator iteration, cycles
//! 195–198). The structural fix: the eval keeps its own schema-pinned state
//! (`.chug/eval-state.md`, rewritten at wrap) and reads a mechanical delta
//! (`.chug/eval-delta.md`, built by `scripts/eval-delta.sh` which loopd runs
//! beside the digest) instead of re-reading the digest, EVALUATION.md, TODO
//! and the code wholesale. The delta's `read-path:` verdict is the switch —
//! STATE-HIT reads ~2 small files; everything stale/invalid/absent is a
//! fail-closed FULL-READ, and a trip eval is ALWAYS a full read.
//!
//! These tests run the REAL script against throwaway `git init` fixture
//! repos (the tests guard the script, they do not reimplement it — the T46
//! digest-test pattern). Pinned here:
//!
//! 1. the verbatim-splice pin (T192 discipline): a rewrite is legal only as
//!    an append (or, at a full ring, an oldest-end rotation) with carried
//!    decision lines byte-identical — a paraphrased, middle-dropped,
//!    reordered, overfull or nothing-carried rewrite fails and forces the
//!    next eval full;
//! 2. the stale-marker fallback: state-missing, schema change, marker
//!    behind > 3 evals (boundary at exactly 3), drift flag, missing splice
//!    history, unreachable marker, no git — each fires FULL-READ with its
//!    named reason;
//! 3. the fresh-input drop is measurable from events (T184 fields): the
//!    delta carries the per-LOOP-SPEC-stream fresh-input/cache-read totals
//!    plus the state-recorded last-eval values the wrap's Outcomes
//!    before/after is computed from;
//! 4. the wiring + doctrine needles: loopd runs the delta beside the digest
//!    (best-effort, fail-closed) and META-META-SPEC carries the state+delta
//!    read path, the trip=full rule, the splice discipline and the >= 5x
//!    fresh-input acceptance metric.
//! 5. the production invocation contract (the aa755fb fix-up pins): loopd
//!    DIRECT-EXECS its scripts, so every loopd-invoked script must be
//!    committed 100755 — a `bash scripts/x.sh` test never sees a missing
//!    exec bit, execve in production does (EACCES, exit 126). Plus: the
//!    empty-previous-ring splice leg, newest-last chronology, and the
//!    explicit `features-md:` fact META-META-SPEC's read path keys on.

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn script_path() -> PathBuf {
    repo_root().join("scripts/eval-delta.sh")
}

const PINNED_NOW: &str = "2026-10-08T00:00:00Z";

/// Run the delta against `root` and return (stdout, delta.md text).
fn run_delta(root: &Path) -> (String, String) {
    let out = Command::new("bash")
        .arg(script_path())
        .arg(root)
        .env("CHUG_DELTA_NOW", PINNED_NOW)
        .output()
        .expect("spawn scripts/eval-delta.sh");
    assert!(
        out.status.success(),
        "eval-delta exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let delta = std::fs::read_to_string(root.join(".chug/eval-delta.md"))
        .expect("delta written");
    (String::from_utf8_lossy(&out.stdout).to_string(), delta)
}

fn verdict(delta: &str) -> &str {
    delta
        .lines()
        .find(|l| l.starts_with("read-path: "))
        .unwrap_or_else(|| panic!("no read-path line in:\n{delta}"))
        .trim_start_matches("read-path: ")
}

fn reason(delta: &str) -> String {
    delta
        .lines()
        .find(|l| l.starts_with("reason: "))
        .map(|l| l.trim_start_matches("reason: ").to_string())
        .unwrap_or_default()
}

/// One full state file with the pinned schema (v1) and a decisions ring.
fn state_text(eval_commit: &str, drift: &str, decisions: &[&str]) -> String {
    let mut s = format!(
        "# eval-state — the Phase-1 evaluator's maintained state (T260)\n\
         schema: 1\n\
         eval-commit: {eval_commit}\n\
         eval-at: 2026-10-07T12:00:00Z\n\
         state-drift: {drift}\n\
         fresh-input-last-eval: 134000\n\
         cache-read-last-eval: 1700000\n\
         health: green — gates green, no absorbed deaths\n\
         open-threads: none\n\
         pacing-streak: TRUE streak 0 (token-free subject per T248)\n\
         ## decisions (last 12, oldest first)\n"
    );
    for d in decisions {
        s.push_str("- ");
        s.push_str(d);
        s.push('\n');
    }
    s
}

/// A throwaway git repo with the corpus files committed (`.chug/` ignored,
/// matching the real repo) and pinned commit dates so the real-time events
/// fixtures written after the marker are always "since-marker".
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "chug-t260-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".chug")).expect("creating fixture dir");
        let f = Fixture { dir };
        f.git(&["init", "--quiet"]);
        f.git(&["config", "user.name", "t260-fixture"]);
        f.git(&["config", "user.email", "t260@fixture.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.write(".gitignore", ".chug/\n");
        f.write(
            "TODO.md",
            "| id | title | spec | pri | status | notes |\n\
             |----|-------|------|-----|--------|-------|\n\
             | T1 | a | specs/T1-s.md | 2 | done | n |\n\
             | T2 | b | specs/T2-s.md | 2 | todo | n |\n",
        );
        f.write("EVALUATION.md", "# Eval\n\n### Cycle 1 (2026-10-07) — seed\n");
        f.write("FEATURES.md", "# FEATURES.md — roadmap\n\n| F1 | a | b | c |\n");
        f.git(&["add", "."]);
        // Pinned date: strictly before the events fixtures' real mtimes.
        f.commit_dated("eval: cycle 1 wrap", "2026-10-07T09:00:00Z");
        f
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env_remove("GIT_AUTHOR_DATE")
            .env_remove("GIT_COMMITTER_DATE")
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("creating fixture subdir");
        }
        std::fs::write(path, contents).expect("writing fixture file");
    }

    fn commit_dated(&self, msg: &str, date: &str) -> String {
        self.git(&["add", "."]);
        let out = Command::new("git")
            .args(["commit", "--quiet", "-m", msg])
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .unwrap_or_else(|e| panic!("git commit: {e}"));
        assert!(
            out.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        self.git(&["rev-parse", "HEAD"])
    }

    /// One extra commit touching EVALUATION.md only (an eval wrap).
    fn eval_wrap(&self, msg: &str, date: &str) -> String {
        let path = self.dir.join("EVALUATION.md");
        let mut body = std::fs::read_to_string(&path).expect("reading EVALUATION.md");
        body.push_str(&format!("\n### {msg}\n"));
        std::fs::write(&path, body).expect("appending EVALUATION.md");
        self.commit_dated(msg, date)
    }

    fn head(&self) -> String {
        self.git(&["rev-parse", "HEAD"])
    }

    fn branch(&self) -> String {
        self.git(&["rev-parse", "--abbrev-ref", "HEAD"])
    }

    /// Stamp a file's mtime (local time, `touch -t [[CC]YY]MMDDhhmm[.ss]`).
    /// The fixture commit dates are pinned to 2026-10-07T09:00:00Z, so a
    /// stamp of 2026-10-09 12:00 local is "since-marker" and a stamp of
    /// 2026-10-04 00:00 local is not — for any sane timezone, and
    /// independent of the machine's real clock.
    fn stamp(&self, rel: &str, timespec: &str) {
        let out = Command::new("touch")
            .args(["-t", timespec, rel])
            .current_dir(&self.dir)
            .output()
            .unwrap_or_else(|e| panic!("touch {rel}: {e}"));
        assert!(out.status.success(), "touch {rel} failed");
    }

    /// A LOOP-SPEC orchestrator events stream with T184 cumulative token
    /// fields (the last iteration line is what the delta reports).
    fn loop_spec_events(&self, name: &str, fresh: u64, cache_read: u64, out: u64) {
        let mut body = String::from(
            "{\"type\":\"run_start\",\"ts\":\"2026-10-07T10:00:00.100Z\",\"mode\":\"run\",\
             \"model\":\"test-model\",\"spec\":\"/repo/LOOP-SPEC.md\",\"cwd\":\"/tmp/w\",\
             \"version\":\"0.1.0\",\"commit\":\"4ea73e3\",\"head_branch\":\"loop-t260\",\
             \"head_commit\":\"4ea73e3\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}\n",
        );
        body.push_str(&format!(
            "{{\"type\":\"iteration\",\"n\":1,\"input_tokens\":{fresh},\"output_tokens\":{out},\
             \"cache_read_input_tokens\":{cache_read},\"cache_creation_input_tokens\":4200,\
             \"ts\":\"2026-10-07T10:05:00.000Z\"}}\n"
        ));
        self.write(&format!(".chug/{name}"), &body);
    }

    /// A child (non-LOOP-SPEC) events stream: run_start names a specs/t file.
    fn child_events(&self, name: &str) {
        self.write(
            &format!(".chug/{name}"),
            "{\"type\":\"run_start\",\"ts\":\"2026-10-07T10:00:00.100Z\",\"mode\":\"run\",\
             \"model\":\"test-model\",\"spec\":\"/repo/specs/t9-x.md\",\"cwd\":\"/tmp/w\",\
             \"version\":\"0.1.0\",\"commit\":\"4ea73e3\",\"head_branch\":\"loop-t9\",\
             \"head_commit\":\"4ea73e3\",\"max_iters\":80,\"max_minutes\":50,\"max_tokens\":null}\n\
             {\"type\":\"iteration\",\"n\":1,\"input_tokens\":999999,\"output_tokens\":1,\
             \"cache_read_input_tokens\":999999,\"cache_creation_input_tokens\":0,\
             \"ts\":\"2026-10-07T10:05:00.000Z\"}\n",
        );
    }

    fn set_state(&self, text: &str, also_prev: bool) {
        self.write(".chug/eval-state.md", text);
        if also_prev {
            self.write(".chug/eval-state.prev.md", text);
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn ring(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("2026-10-07 d1791-{i:02} filed T{i}")).collect()
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

// ---- pin 1: the state-hit read path + the verbatim-splice discipline ----

#[test]
fn state_hit_verdict_with_verified_state_and_prev() {
    let f = Fixture::new("hit");
    let marker = f.head();
    f.set_state(&state_text(&marker, "none", &refs(&ring(3))), true);
    let (stdout, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
    assert!(reason(&delta).is_empty(), "a hit carries no reason: {delta}");
    assert!(
        stdout.contains("verdict=STATE-HIT"),
        "the loopd-logged summary names the verdict: {stdout}"
    );
    assert!(
        delta.contains("splice-check: ok (carried 3, rotated 0, appended 0)"),
        "{delta}"
    );
    assert!(delta.contains("evals-behind 0"), "{delta}");
}

#[test]
fn splice_plain_append_is_legal_below_the_cap() {
    let f = Fixture::new("append");
    let marker = f.head();
    let mut carried = ring(3);
    carried.push("2026-10-08 d1791-04 rejected skip".to_string());
    f.write(
        ".chug/eval-state.prev.md",
        &state_text(&marker, "none", &refs(&ring(3))),
    );
    f.set_state(&state_text(&marker, "none", &refs(&carried)), false);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
    assert!(
        delta.contains("splice-check: ok (carried 3, rotated 0, appended 1)"),
        "{delta}"
    );
}

#[test]
fn splice_full_ring_rotation_from_the_oldest_end_is_legal() {
    let f = Fixture::new("rot");
    let marker = f.head();
    let mut rotated = ring(12);
    rotated.remove(0);
    rotated.push("2026-10-08 d1791-13 filed T13".to_string());
    f.write(
        ".chug/eval-state.prev.md",
        &state_text(&marker, "none", &refs(&ring(12))),
    );
    f.set_state(&state_text(&marker, "none", &refs(&rotated)), false);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
    assert!(
        delta.contains("splice-check: ok (carried 11, rotated 1, appended 1)"),
        "{delta}"
    );
}

/// The T192 discipline's negative space: any rewrite that is not a verbatim
/// FIFO splice is summarization in disguise and forces the next eval full.
#[test]
fn splice_violations_force_full_read() {
    let cases: Vec<(&str, Vec<String>, &str)> = vec![
        (
            "paraphrased carried line",
            vec![
                "2026-10-07 d1791-01 filed T1".to_string(),
                "2026-10-07 d1791-02 filed T2 (revisited, closed)".to_string(),
                "2026-10-07 d1791-03 filed T3".to_string(),
            ],
            "rewritten",
        ),
        (
            "middle decision dropped",
            vec![
                "2026-10-07 d1791-01 filed T1".to_string(),
                "2026-10-07 d1791-03 filed T3".to_string(),
            ],
            "rewritten",
        ),
        (
            "reordered history",
            vec![
                "2026-10-07 d1791-02 filed T2".to_string(),
                "2026-10-07 d1791-01 filed T1".to_string(),
                "2026-10-07 d1791-03 filed T3".to_string(),
            ],
            "rewritten",
        ),
        (
            "ring overfull",
            ring(13).iter().map(|s| s.to_string()).collect(),
            "overfull",
        ),
        (
            "nothing carried from a full ring",
            ring(12).iter().map(|s| format!("{s} (rewritten)")).collect(),
            "nothing carried verbatim",
        ),
    ];
    for (what, current, needle) in cases {
        let f = Fixture::new(&format!("splice-{}", what.replace(' ', "-")));
        let marker = f.head();
        f.write(
            ".chug/eval-state.prev.md",
            &state_text(&marker, "none", &refs(&ring(3))),
        );
        // The "nothing carried" case needs a full previous ring.
        if needle == "nothing carried verbatim" {
            f.write(
                ".chug/eval-state.prev.md",
                &state_text(&marker, "none", &refs(&ring(12))),
            );
        }
        f.set_state(&state_text(&marker, "none", &refs(&current)), false);
        let (_, delta) = run_delta(&f.dir);
        assert_eq!(
            verdict(&delta),
            "FULL-READ REQUIRED",
            "{what}: {delta}"
        );
        let r = reason(&delta);
        assert!(
            r.starts_with("splice-violated") && r.contains(needle),
            "{what}: reason {r:?} must name the splice leg ({needle})"
        );
    }
}

// ---- pin 2: the stale-marker fallbacks (each reason fires FULL-READ) ----

#[test]
fn missing_state_is_full_read() {
    let f = Fixture::new("no-state");
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert_eq!(reason(&delta), "state-missing", "{delta}");
}

#[test]
fn schema_change_is_full_read() {
    let f = Fixture::new("schema");
    let marker = f.head();
    let mut s = state_text(&marker, "none", &refs(&ring(1)));
    s = s.replace("schema: 1", "schema: 2");
    f.set_state(&s, true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(
        reason(&delta).starts_with("schema-mismatch"),
        "{delta}"
    );
    assert!(reason(&delta).contains("'2'"), "{delta}");
}

#[test]
fn schema_field_missing_is_full_read() {
    let f = Fixture::new("schema-fields");
    let marker = f.head();
    // Drop the pacing-streak line (a required field).
    let s = state_text(&marker, "none", &refs(&ring(1)))
        .lines()
        .filter(|l| !l.starts_with("pacing-streak:"))
        .collect::<Vec<_>>()
        .join("\n");
    f.set_state(&format!("{s}\n"), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    let r = reason(&delta);
    assert!(
        r.starts_with("schema-mismatch") && r.contains("pacing-streak"),
        "{delta}"
    );
}

#[test]
fn marker_boundary_three_evals_hit_four_full() {
    // Exactly 3 EVALUATION.md commits since the marker: still a hit.
    let f = Fixture::new("boundary");
    let marker = f.head();
    f.eval_wrap("eval: cycle 2", "2026-10-07T10:00:00Z");
    f.eval_wrap("eval: cycle 3", "2026-10-07T11:00:00Z");
    f.eval_wrap("eval: cycle 4", "2026-10-07T11:30:00Z");
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "3 behind is in band: {delta}");

    // A 4th eval commit pushes the marker behind > 3: stale.
    f.eval_wrap("eval: cycle 5", "2026-10-07T12:30:00Z");
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    let r = reason(&delta);
    assert!(
        r.starts_with("marker-behind") && r.contains("4 evals > 3"),
        "{delta}"
    );
}

#[test]
fn drift_flag_is_full_read() {
    let f = Fixture::new("drift");
    let marker = f.head();
    f.set_state(&state_text(&marker, "flagged", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    let r = reason(&delta);
    assert!(
        r.starts_with("state-drift-flagged") && r.contains("flagged"),
        "{delta}"
    );
}

#[test]
fn missing_prev_snapshot_is_full_read_until_snapshotted() {
    let f = Fixture::new("bootstrap");
    let marker = f.head();
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), false);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(
        reason(&delta).starts_with("state-history-missing"),
        "{delta}"
    );
    // The failed run still snapshot the state: the NEXT run can splice-check.
    assert!(
        f.dir.join(".chug/eval-state.prev.md").is_file(),
        "the delta refreshes the splice snapshot best-effort"
    );
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "second look splices: {delta}");
}

#[test]
fn unreachable_marker_is_full_read() {
    // (a) the marker names a commit that does not exist
    let f = Fixture::new("unreachable");
    f.set_state(&state_text("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(
        reason(&delta).starts_with("marker-unreachable"),
        "{delta}"
    );
    // (b) the marker exists but is not an ancestor of HEAD (orphan commit)
    let f2 = Fixture::new("orphan-marker");
    let base = f2.branch();
    f2.git(&["checkout", "--orphan", "side"]);
    f2.write("SIDE.md", "orphan\n");
    f2.git(&["add", "."]);
    let orphan = f2.commit_dated("orphan commit", "2026-10-07T08:00:00Z");
    f2.git(&["checkout", "--quiet", &base]);
    f2.set_state(&state_text(&orphan, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f2.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(
        reason(&delta).starts_with("marker-unreachable"),
        "{delta}"
    );
}

#[test]
fn git_unavailable_is_full_read() {
    let f = Fixture::new("no-git");
    // Strip the repo: same tree, no .git (an exported/zip checkout).
    std::fs::remove_dir_all(f.dir.join(".git")).expect("removing .git");
    f.set_state(&state_text("4ea73e3", "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(reason(&delta).starts_with("git-unavailable"), "{delta}");
}

// ---- pin 3: the fresh-input drop is measurable from events (T184) -------

#[test]
fn t184_fresh_input_telemetry_is_mechanically_available() {
    let f = Fixture::new("telemetry");
    let marker = f.head();
    f.loop_spec_events("events-20261007-100000.jsonl", 134_000, 1_700_000, 28_000);
    f.child_events("events-t9-impl-20261007-100100.jsonl");
    f.stamp(".chug/events-20261007-100000.jsonl", "202610091200");
    f.stamp(".chug/events-t9-impl-20261007-100100.jsonl", "202610091200");
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
    // The LOOP-SPEC stream's last cumulative line (T184 fields), verbatim.
    let line = delta
        .lines()
        .find(|l| l.contains("events-20261007-100000.jsonl: fresh-input"))
        .unwrap_or_else(|| panic!("no T184 telemetry line in:\n{delta}"));
    assert!(
        line.contains("fresh-input 134000 | cache-read 1700000 | out 28000"),
        "the T184 cumulative numbers must reach the delta verbatim: {line}"
    );
    assert!(
        line.contains("since-marker yes"),
        "events written after the marker are since-marker: {line}"
    );
    // The child stream (specs/t9) is never a LOOP-SPEC stream.
    assert!(
        !delta.contains("events-t9-impl"),
        "child streams must not appear in the LOOP-SPEC telemetry: {delta}"
    );
    // The state-recorded values the wrap's before/after compares.
    assert!(
        delta.contains("state-recorded last-eval: fresh-input 134000 | cache-read 1700000"),
        "{delta}"
    );
}

#[test]
fn t184_telemetry_flags_streams_since_the_marker() {
    let f = Fixture::new("telemetry-window");
    let marker = f.head();
    f.loop_spec_events("events-20261006-090000.jsonl", 120_000, 1_500_000, 20_000);
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    // Backdate this stream below the marker commit (2026-10-07T09:00Z).
    f.stamp(".chug/events-20261006-090000.jsonl", "202610040000");
    let (_, delta) = run_delta(&f.dir);
    let line = delta
        .lines()
        .find(|l| l.contains("events-20261006-090000.jsonl: fresh-input"))
        .expect("telemetry line");
    assert!(
        line.contains("since-marker no"),
        "a stream older than the marker is not since-marker: {line}"
    );
}

// ---- the delta body: rows, deaths, changed files, cycle summaries -------

#[test]
fn delta_reports_rows_deaths_and_changed_files_since_marker() {
    let f = Fixture::new("delta-body");
    let marker = f.head();
    // Work lands (a source file + a spec), a row closes, a row is filed.
    f.write("src/thing.rs", "fn main() {}\n");
    f.write(
        "TODO.md",
        "| id | title | spec | pri | status | notes |\n\
         |----|-------|------|-----|--------|-------|\n\
         | T1 | a | specs/T1-s.md | 2 | done | n |\n\
         | T2 | b | specs/T2-s.md | 2 | done | n |\n\
         | T3 | c | specs/T3-s.md | 2 | todo | n |\n",
    );
    f.git(&["add", "."]);
    f.commit_dated("t3: landed a thing", "2026-10-07T10:30:00Z");
    // A child dies and a goal is rejected in a stream newer than the marker.
    f.write(
        ".chug/events-t3-impl-20261007-110000.jsonl",
        "{\"type\":\"abort\",\"reason\":\"budget: max-iters 80 exhausted\",\"ts\":\"2026-10-07T11:00:00Z\"}\n\
         {\"type\":\"abort\",\"reason\":\"budget: max-iters 80 exhausted\",\"ts\":\"2026-10-07T11:01:00Z\"}\n\
         {\"type\":\"goal\",\"outcome\":\"rejected\",\"reason\":\"clippy -D warnings failed\",\"ts\":\"2026-10-07T11:02:00Z\"}\n",
    );
    f.stamp(".chug/events-t3-impl-20261007-110000.jsonl", "202610091200");
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
    assert!(delta.contains("changed files: work 1 / bookkeeping 1"), "{delta}");
    assert!(delta.contains("- work files: src/thing.rs"), "{delta}");
    assert!(
        delta.contains("added T3 | closed T2 | removed none"),
        "{delta}"
    );
    assert!(
        delta.contains("child deaths since marker: 2 — budget: max-iters N exhausted x2"),
        "{delta}"
    );
    assert!(delta.contains("goal rejects since marker: 1"), "{delta}");
    // Cycle summaries only name EVALUATION.md-touching commits, never work.
    assert!(
        !delta.contains("t3: landed a thing"),
        "work commits are not cycle summaries: {delta}"
    );
}

#[test]
fn delta_without_any_marker_still_writes_a_full_read() {
    let f = Fixture::new("no-marker");
    // No state; the fixture's only commit DID touch EVALUATION.md, so the
    // fallback base exists — remove the state but keep the repo history.
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(verdict(&delta), "FULL-READ REQUIRED", "{delta}");
    assert!(reason(&delta) == "state-missing", "{delta}");
    assert!(
        delta.contains("## Delta since"),
        "the delta body still renders (fallback base = newest EVALUATION.md commit): {delta}"
    );
}

#[test]
fn delta_output_is_deterministic() {
    let f = Fixture::new("determinism");
    let marker = f.head();
    f.loop_spec_events("events-20261007-100000.jsonl", 134_000, 1_700_000, 28_000);
    f.set_state(&state_text(&marker, "none", &refs(&ring(2))), true);
    let (_, a) = run_delta(&f.dir);
    let (_, b) = run_delta(&f.dir);
    assert_eq!(a, b, "the delta must be byte-identical across runs");
    assert!(
        a.contains(&format!("generated-at: {PINNED_NOW}")),
        "the pinned clock lands: {a}"
    );
}

// ---- pin 4: the wiring + doctrine needles -------------------------------

#[test]
fn loopd_runs_the_delta_beside_the_digest_fail_closed() {
    let loopd = std::fs::read_to_string(repo_root().join("loopd.sh"))
        .expect("reading loopd.sh");
    assert_eq!(
        loopd.matches("scripts/eval-delta.sh").count(),
        1,
        "loopd must invoke the delta exactly once (beside the digest)"
    );
    assert!(
        loopd.contains("eval-delta: nonzero exit (best-effort, ignored — the cycle full-reads)"),
        "the guard message names the fail-closed default"
    );
    // Ordering: the delta runs after the digest (it reads the same corpus
    // window; the digest is the loopd-established pre-launch pattern).
    let digest_pos = loopd
        .find("scripts/eval-digest.sh >> ")
        .expect("the digest invocation");
    let delta_pos = loopd.find("scripts/eval-delta.sh >> ").expect("the delta invocation");
    assert!(digest_pos < delta_pos, "delta runs beside/after the digest");
}

#[test]
fn doctrine_carries_the_state_delta_read_path() {
    let spec = std::fs::read_to_string(repo_root().join("META-META-SPEC.md"))
        .expect("reading META-META-SPEC.md");
    let flat = spec.split_whitespace().collect::<Vec<_>>().join(" ");
    for (needle, what) in [
        (
            "state+delta read path (T260)",
            "the T260 read-path block is named",
        ),
        ("read-path: STATE-HIT", "the hit verdict is the switch"),
        (
            "read-path: FULL-READ REQUIRED",
            "the fail-closed verdict is named",
        ),
        (
            "A trip eval is ALWAYS a full read",
            "trip evals never trust the state (req 4)",
        ),
        (
            "the state file is never a trip eval's only input",
            "req 4's exact concern",
        ),
        (
            "verbatim splice, never summarization",
            "the T192 rewrite discipline (req 5 pin 1)",
        ),
        (
            "marker behind > 3 evals",
            "the staleness rule (req 2)",
        ),
        (
            ">= 5x fresh-input drop on state-hit cycles",
            "the acceptance metric (req 3)",
        ),
        (
            "features-md: changed|unchanged",
            "the explicit FEATURES.md movement line the delta emits (req: the \
             read path's Tier-1 re-read is keyed on it)",
        ),
        ("eval-commit:", "the schema's marker field"),
        ("state-drift:", "the schema's drift field"),
    ] {
        assert!(
            flat.contains(needle),
            "META-META-SPEC must state {what} ({needle:?})"
        );
    }
}

// ---- the aa755fb fix-up pins (F1–F4) ------------------------------------

/// The `  - ` item lines under the delta's `- <header_needle>` list header,
/// in printed order (until the first non-item line).
fn listed_lines_after(delta: &str, header_needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_list = false;
    for line in delta.lines() {
        if !in_list {
            if line.starts_with("- ") && line.contains(header_needle) {
                in_list = true;
            }
            continue;
        }
        match line.strip_prefix("  - ") {
            Some(rest) => out.push(rest.to_string()),
            None => break,
        }
    }
    out
}

fn committed_mode(rel: &str) -> String {
    let out = Command::new("git")
        .args(["ls-files", "-s", rel])
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("git ls-files {rel}: {e}"));
    assert!(
        out.status.success(),
        "git ls-files {rel} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("(untracked)")
        .to_string()
}

/// F1 — the production invocation contract. loopd.sh direct-execs the delta
/// (`scripts/eval-delta.sh >> "$LOG" 2>&1`, execve, no bash prefix), so the
/// committed mode MUST be 100755 like every sibling loopd script. A 100644
/// commit passes every `bash scripts/eval-delta.sh` test — which is exactly
/// how the F1 regression shipped — while production execve fails EACCES
/// (exit 126) and .chug/eval-delta.md is never built (fail-closed full
/// reads forever).
#[test]
fn eval_delta_is_committed_100755_like_every_sibling_loopd_script() {
    let mode = committed_mode("scripts/eval-delta.sh");
    assert_eq!(
        mode, "100755",
        "scripts/eval-delta.sh is committed {mode}; loopd.sh direct-execs it, \
         so execve fails EACCES (exit 126) on a fresh checkout and the whole \
         T260 read path is silently inert"
    );
    // The working tree must carry the bit too (the direct-exec test proves
    // the run itself, not just the index).
    let meta = std::fs::metadata(script_path()).expect("stat scripts/eval-delta.sh");
    assert!(
        meta.permissions().mode() & 0o111 != 0,
        "scripts/eval-delta.sh lost its exec bit in the working tree"
    );
}

/// F1 — the run itself, under the production invocation (direct execve, no
/// `bash` prefix). This is the contract the old tests could not see.
#[test]
fn eval_delta_runs_when_directly_executed_like_loopd_does() {
    let f = Fixture::new("direct-exec");
    let marker = f.head();
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let out = Command::new(script_path())
        .arg(&f.dir)
        .env("CHUG_DELTA_NOW", PINNED_NOW)
        .output()
        .expect(
            "direct execve of scripts/eval-delta.sh failed — loopd.sh invokes \
             it WITHOUT a bash prefix, so a missing exec bit (committed 100644) \
             fails EACCES here exactly as in production",
        );
    assert!(
        out.status.success(),
        "direct exec failed {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let delta =
        std::fs::read_to_string(f.dir.join(".chug/eval-delta.md")).expect("delta written");
    assert_eq!(verdict(&delta), "STATE-HIT", "{delta}");
}

/// F1's CLASS sweep (cycle-33 rule): the finding names a class —
/// production-invocation-contract gaps the test harness cannot see. Sweep
/// EVERY script loopd.sh direct-execs (not just eval-delta.sh): each must be
/// committed 100755 and carry the exec bit. The one sourced script
/// (`. scripts/loopd_env_loader.sh`) needs no exec bit and is excluded.
#[test]
fn every_loopd_invoked_script_is_committed_100755() {
    let loopd =
        std::fs::read_to_string(repo_root().join("loopd.sh")).expect("reading loopd.sh");
    let mut invoked: Vec<String> = Vec::new();
    for line in loopd.lines() {
        let trimmed = line.trim_start();
        // Comments are not invocations; `.` sources the loader (no execve,
        // no exec bit needed) and is deliberately out of the sweep.
        if trimmed.starts_with('#') || trimmed.starts_with(". ") {
            continue;
        }
        let mut rest = trimmed;
        while let Some(i) = rest.find("scripts/") {
            let after = &rest[i + "scripts/".len()..];
            let name: String = after
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
                .collect();
            if name.len() > 3 && name.ends_with(".sh") {
                invoked.push(name.clone());
            }
            let advance = if name.is_empty() { 1 } else { name.len() };
            rest = &after[advance..];
        }
    }
    invoked.sort();
    invoked.dedup();
    assert!(
        invoked.contains(&"eval-delta.sh".to_string()),
        "the scanner must find the T260 delta invocation: {invoked:?}"
    );
    assert!(
        invoked.len() >= 4,
        "the sweep must cover every loopd-invoked script (eval-digest, \
         eval-delta, orphan-reaper, site-sync), found: {invoked:?}"
    );
    for name in &invoked {
        let rel = format!("scripts/{name}");
        let mode = committed_mode(&rel);
        assert_eq!(
            mode, "100755",
            "{rel} is committed {mode}: loopd.sh direct-execs it, so execve \
             fails EACCES (exit 126) in production while every `bash {rel}` \
             test stays green — the exact F1 class"
        );
        let meta = std::fs::metadata(repo_root().join(&rel))
            .unwrap_or_else(|e| panic!("stat {rel}: {e}"));
        assert!(
            meta.permissions().mode() & 0o111 != 0,
            "{rel} lost its exec bit in the working tree"
        );
    }
}

/// F2 — the splice verifier's file split used the `NR==FNR` idiom, which
/// breaks when the FIRST file is empty: NR==FNR stays true through the
/// second file, every current line lands in the previous set, and a legal
/// append onto an empty previous ring read as a spurious violation (spurious
/// FULL-READ). An empty previous ring + a non-empty current ring must
/// splice-verify as a plain append.
#[test]
fn splice_empty_previous_ring_with_nonempty_current_is_a_legal_append() {
    let f = Fixture::new("empty-prev-ring");
    let marker = f.head();
    // The previous state's decisions ring is EMPTY (the bootstrap wrap's
    // state); this run's ring appends three decisions onto it.
    f.write(".chug/eval-state.prev.md", &state_text(&marker, "none", &[]));
    f.set_state(&state_text(&marker, "none", &refs(&ring(3))), false);
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(
        verdict(&delta),
        "STATE-HIT",
        "an empty previous ring + a non-empty current ring is a legal append: {delta}"
    );
    assert!(
        delta.contains("splice-check: ok (carried 0, rotated 0, appended 3)"),
        "{delta}"
    );
}

/// F3 — the delta labels its cycle-summary list "newest last", but `git log`
/// emits newest-FIRST: the output must be reversed (or the label corrected).
/// The chronology is pinned: the OLDEST wrap first, the NEWEST last.
#[test]
fn cycle_summaries_are_newest_last() {
    let f = Fixture::new("cycle-order");
    let marker = f.head();
    f.eval_wrap("eval: cycle 2 older wrap", "2026-10-07T10:00:00Z");
    f.eval_wrap("eval: cycle 3 newer wrap", "2026-10-07T11:00:00Z");
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    let lines = listed_lines_after(&delta, "cycle summaries");
    assert_eq!(lines.len(), 2, "both wraps are listed: {delta}");
    let older = lines
        .iter()
        .position(|l| l.contains("cycle 2 older wrap"))
        .expect("older wrap listed");
    let newer = lines
        .iter()
        .position(|l| l.contains("cycle 3 newer wrap"))
        .expect("newer wrap listed");
    assert!(
        older < newer,
        "the label says 'newest last' — the NEWEST wrap must print LAST: {lines:?}"
    );
}

/// F3's second instance — the T184 telemetry list says "newest last" while
/// its collection order is `ls -t` (newest-FIRST): the printed lines must be
/// reversed so the newest stream is last.
#[test]
fn t184_telemetry_lines_are_newest_last() {
    let f = Fixture::new("telem-order");
    let marker = f.head();
    f.loop_spec_events("events-20261006-090000.jsonl", 120_000, 1_500_000, 20_000);
    f.loop_spec_events("events-20261008-090000.jsonl", 130_000, 1_600_000, 21_000);
    // Older stream: 2026-10-08 09:00 local; newer: 2026-10-10 09:00 local
    // (both after the pinned 2026-10-07T09:00Z marker, for any sane TZ).
    f.stamp(".chug/events-20261006-090000.jsonl", "202610080900");
    f.stamp(".chug/events-20261008-090000.jsonl", "202610100900");
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    let lines = listed_lines_after(&delta, "T184 fresh-input telemetry");
    assert_eq!(lines.len(), 2, "both LOOP-SPEC streams are listed: {delta}");
    let older = lines
        .iter()
        .position(|l| l.contains("events-20261006-090000.jsonl"))
        .expect("older stream listed");
    let newer = lines
        .iter()
        .position(|l| l.contains("events-20261008-090000.jsonl"))
        .expect("newer stream listed");
    assert!(
        older < newer,
        "the label says 'newest last' — the NEWEST stream must print LAST: {lines:?}"
    );
}

/// F4 — META-META-SPEC's T260 block keys a conditional Tier-1 re-read on
/// FEATURES.md movement, but the delta never computed a FEATURES.md fact.
/// The fix: an explicit `features-md: changed|unchanged` line (which keeps
/// the doctrine sentence true), pinned here.
#[test]
fn delta_reports_features_md_movement() {
    let f = Fixture::new("features-md");
    let marker = f.head();
    f.set_state(&state_text(&marker, "none", &refs(&ring(1))), true);
    let (_, delta) = run_delta(&f.dir);
    assert!(
        delta.contains("- features-md: unchanged"),
        "an unchanged FEATURES.md must be EXPLICIT, not merely absent: {delta}"
    );
    // A roadmap append after the marker flips the fact to changed.
    f.write(
        "FEATURES.md",
        "# FEATURES.md — roadmap\n\n| F1 | a | b | c |\n| F2 | d | e | f |\n",
    );
    f.git(&["add", "."]);
    f.commit_dated("features: append F2", "2026-10-07T10:00:00Z");
    let (_, delta) = run_delta(&f.dir);
    assert_eq!(
        verdict(&delta),
        "STATE-HIT",
        "a features commit is not an eval wrap (no EVALUATION.md touch): {delta}"
    );
    assert!(
        delta.contains("- features-md: changed"),
        "FEATURES.md movement must reach the delta explicitly: {delta}"
    );
}

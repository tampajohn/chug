//! T259 — loopd Laya triage: the judge daemon gates non-trip eval launches.
//!
//! T258's cheap exit zeroes the MECHANICALLY empty case; the remaining
//! empty-day burn is borderline predicates (technically non-empty, usually
//! nothing) and valve-trip kimi evals. T259 inserts Laya (the T204 judge
//! daemon, ~$0 marginal cost) between the mechanical layer and System Two:
//! a compact state pack -> ONE needs-eval question + confidence, and a
//! confidence-gated cascade — confident-empty skips (the disposition
//! records the verdict, $0), confident-work routes the borderline eval to
//! glm, unsure escalates to kimi. Fail-open everywhere: daemon absent,
//! error, or >2s timeout falls back to EXACTLY the T258 routing, one note
//! per cycle. Every triage is recorded to the decision corpus (class
//! laya-triage) and the next look backfills the outcome — the F13
//! distillation corpus.
//!
//! These tests run the REAL loopd.sh against a FAKE judge daemon (a
//! unix-socket HTTP server serving canned /judge answers — the tests guard
//! the script, they do not reimplement it), pinning: the cascade routes
//! (fixture state packs), the threshold boundary, the valve trip's triage
//! (a confident-empty cancels a FRESH trip only), daemon-down fail-open,
//! the state pack verbatim, and the end-to-end run mode (the triaged
//! disposition + the decision records + one note per cycle).

#![cfg(unix)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use tempfile::TempDir;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let found = haystack.matches(needle).count();
    assert_eq!(
        found, expected,
        "{what}: expected {needle:?} exactly {expected}×, found {found}×"
    );
}

// --- fixture constants (the T258 test's shapes, reused) -----------------------

const FRESH_EPOCH: u64 = 1_790_510_400;
const FRESH_DAY: &str = "2026-09-27";
const GLM_ID: &str = "anthropic-system.ai.glm-5-3-flash";
const KIMI_ID: &str = "anthropic-system.ai.kimi-k3";
const TODO_ONE_DONE: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | done | notes |\n";
const TODO_TWO_DONE: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | done | notes |\n\
     | T2 | title | specs/T2-slug.md | 2 | done | notes |\n";
/// The canned /judge answer the fake daemon serves: the daemon's response
/// shape (answers.<qid>.choice + confidence) at the caller's verdict.
fn judge_answer(choice: &str, conf: f64) -> String {
    let no = 1.0 - conf;
    "{\"model\":\"laya-fixture\",\"answers\":{\"needs_eval\":{\"type\":\"choice\",\"choice\":\"".to_string()
        + choice
        + "\",\"probabilities\":{\"no\":"
        + &no.to_string()
        + ",\"yes\":"
        + &conf.to_string()
        + "},\"confidence\":"
        + &conf.to_string()
        + "}},\"usage\":{\"input_tokens\":42,\"output_tokens\":0}}"
}

// --- the fake judge daemon ------------------------------------------------------

/// One captured request body per socket path (keyed, so parallel tests in
/// one cargo-test process never see each other's traffic).
static CAPTURED: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn captured(sock: &Path) -> String {
    CAPTURED
        .lock()
        .expect("captured map")
        .get(&sock.to_string_lossy().to_string())
        .cloned()
        .unwrap_or_default()
}

/// Serve canned /judge answers on `sock` until the test process exits — a
/// minimal HTTP/1.1 over the T204 unix-socket transport (the 0600 mode is
/// the daemon's own concern; curl only needs connect+write). Captures every
/// POST /judge body so pins can assert the state pack verbatim.
fn serve_judge(sock: &Path, answer: String) {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).expect("bind the fixture judge socket");
    let key = sock.to_string_lossy().to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 16384];
            // Read until the head is complete AND the body satisfies
            // Content-Length — a single read may split the stream.
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                }
                let text = String::from_utf8_lossy(&buf).to_string();
                let Some(head_end) = text.find("\r\n\r\n") else {
                    continue;
                };
                let want = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().ok())
                    })
                    .flatten()
                    .unwrap_or(0);
                if buf.len() >= head_end + 4 + want {
                    break;
                }
            }
            let req = String::from_utf8_lossy(&buf).to_string();
            if req.starts_with("POST /judge") {
                let body = req
                    .split("\r\n\r\n")
                    .nth(1)
                    .unwrap_or("")
                    .to_string();
                CAPTURED
                    .lock()
                    .expect("captured map")
                    .insert(key.clone(), body);
            }
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                answer.len(),
                answer
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
}

// --- fixture harness (the T258 pattern) ----------------------------------------

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t259", "-c", "user.email=t259@example.test"])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn git_out(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn write_exec(path: &Path, body: &str) {
    std::fs::write(path, body).expect("write fixture stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fixture stub");
}

struct Fixture(TempDir);

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("loopd.sh"),
        std::fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh readable"),
    )
    .expect("copy loopd.sh into fixture dir");
    let scripts = tmp.path().join("scripts");
    std::fs::create_dir_all(&scripts).expect("fixture scripts dir");
    std::fs::copy(
        repo_root().join("scripts/loopd_env_loader.sh"),
        scripts.join("loopd_env_loader.sh"),
    )
    .expect("copy loopd_env_loader.sh into fixture dir");
    std::fs::write(tmp.path().join(".gitignore"), "/.chug/\n/target-shared/\n/bin/\n")
        .expect("write fixture .gitignore");
    git(tmp.path(), &["init", "-q"]);
    let f = Fixture(tmp);
    f.commit_tree(
        "eval: cycle-100 fresh eval — the baseline (T259 fixture)",
        Some(TODO_ONE_DONE),
        Some("# eval\n"),
    );
    f.pin_eval_mtime(FRESH_EPOCH);
    // F3d: a QUIET fixture must PROVE quiet — the log exists and records no
    // death, so deaths=0 is evidence of absence (the safe-side rule needs
    // the log to launch on unknowns).
    f.log_line("2026-09-27T12:00:00Z loopd start (pid 4242) — T259 fixture baseline record");
    f
}

impl Fixture {
    fn path(&self) -> &Path {
        self.0.path()
    }

    fn commit_tree(&self, subject: &str, todo: Option<&str>, eval: Option<&str>) {
        if let Some(todo) = todo {
            std::fs::write(self.path().join("TODO.md"), todo).expect("write fixture TODO.md");
        }
        if let Some(eval) = eval {
            std::fs::write(self.path().join("EVALUATION.md"), eval)
                .expect("write fixture EVALUATION.md");
        }
        git(self.path(), &["add", "-A"]);
        git(self.path(), &["commit", "-q", "--allow-empty", "-m", subject]);
    }

    fn commit_empty(&self, subject: &str) {
        self.commit_tree(subject, None, None);
    }

    /// A T237-token disposition chain — `n` consecutive empty wraps trip
    /// the T247 valve on the (n+1)th cycle.
    fn disposition_chain(&self, n: usize) {
        for i in 0..n {
            self.commit_empty(&format!(
                "eval: cycle-{} wrap notes (empty-delta disposition, {}st consecutive no-op)",
                101 + i,
                i + 1
            ));
        }
    }

    fn log_line(&self, line: &str) {
        let log = self.path().join(".chug/loopd/loopd.log");
        std::fs::create_dir_all(log.parent().unwrap()).expect("fixture log dir");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("open fixture loopd.log");
        writeln!(f, "{line}").expect("append fixture log line");
    }

    fn pin_eval_mtime(&self, epoch: u64) {
        use std::fs::FileTimes;
        let path = self.path().join("EVALUATION.md");
        if !path.exists() {
            std::fs::write(&path, "# eval\n").expect("write fixture EVALUATION.md");
        }
        let f = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open fixture EVALUATION.md for mtime pinning");
        f.set_times(FileTimes::new().set_modified(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(epoch),
        ))
        .expect("pin fixture mtime");
    }
}

fn verb_of(gate: &str) -> &str {
    gate.split_whitespace().next().expect("gate line has a verb")
}

fn model_of(gate: &str) -> &str {
    gate.split_whitespace().nth(1).expect("gate line has a model")
}

fn field_of(gate: &str, key: &str) -> String {
    gate.split_whitespace()
        .find(|f| f.starts_with(key))
        .map(|f| f[key.len()..].to_string())
        .unwrap_or_default()
}

/// Run the copied script's `predicate` probe (env cleared of the routing
/// knobs, then the caller's overrides applied) — prints the gate line.
fn run_predicate(root: &Path, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("predicate")
        .current_dir(root)
        .env_remove("CHUG_ROUTINE_TODAY")
        .env_remove("LOOP_ORCH_MODEL")
        .env_remove("LOOP_ROUTINE_MODEL")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn loopd.sh predicate");
    assert!(
        out.status.success(),
        "predicate probe exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// The borderline fixture: a bookkeeping-only delta (TODO.md rows re-drained
/// by an eval commit) with a fresh evaluation and a quiet log — the T258
/// borderline shape, exactly the case the triage layer exists to settle.
fn borderline_fixture() -> Fixture {
    let f = fixture();
    f.commit_tree(
        "eval: cycle-101 fresh eval — one row filed, queue re-drained",
        Some(TODO_TWO_DONE),
        None,
    );
    f.pin_eval_mtime(FRESH_EPOCH);
    f
}

/// A fixture one bounded run-mode pass (the T258 end-to-end shape: stub
/// ps/cargo, one iteration, 1s sleeps, no daemon ensure).
fn run_one_pass(f: &Fixture, sock: &Path) -> std::process::Output {
    let bin = f.path().join("bin");
    std::fs::create_dir_all(&bin).expect("fixture stub bin dir");
    write_exec(&bin.join("ps"), "#!/bin/sh\necho \"fake ps: no drivers\"\n");
    write_exec(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
    );
    Command::new("bash")
        .arg(f.path().join("loopd.sh"))
        .arg("run")
        .current_dir(f.path())
        .env("PATH", &path)
        .env("HOME", f.path())
        .env("CHUG_ROUTINE_TODAY", FRESH_DAY)
        .env("CHUG_DAEMON_SOCK", sock)
        .env("LOOPD_SLEEP_OK", "1")
        .env("LOOPD_SLEEP_FAIL", "1")
        .env("LOOPD_MAX_LOOPS", "1")
        .env("LOOP_DAEMON_ENSURE", "0")
        .env("LOOP_ORCH_MODEL", KIMI_ID)
        .env("LOOP_ROUTINE_MODEL", GLM_ID)
        .env("GIT_AUTHOR_NAME", "t259-fixture")
        .env("GIT_AUTHOR_EMAIL", "t259@example.test")
        .env("GIT_COMMITTER_NAME", "t259-fixture")
        .env("GIT_COMMITTER_EMAIL", "t259@example.test")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .stdout(Stdio::null())
        .output()
        .expect("spawn loopd.sh run")
}

fn fixture_log(f: &Fixture) -> String {
    std::fs::read_to_string(f.path().join(".chug/loopd/loopd.log"))
        .expect("read the fixture loopd.log")
}

fn fixture_decisions(f: &Fixture) -> Vec<String> {
    match std::fs::read_to_string(f.path().join(".chug/decisions.jsonl")) {
        Ok(text) => text.lines().filter(|l| !l.trim().is_empty()).map(String::from).collect(),
        Err(_) => Vec::new(),
    }
}

// --- req 2: the confidence-gated cascade routes the borderline eval -----------

/// Confident-empty (no, 0.93 >= 0.85): the borderline eval SKIPS and the
/// gate line records the verdict + confidence + route — $0 instead of a
/// glm eval that would find nothing.
#[test]
fn confident_empty_routes_the_borderline_eval_to_the_triaged_skip() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "skip", "confident-empty skips the launch: {gate}");
    assert_eq!(model_of(&gate), "-", "a triaged skip picks no orchestrator: {gate}");
    assert_eq!(field_of(&gate, "laya="), "no", "the verdict rides the line: {gate}");
    assert_eq!(field_of(&gate, "conf="), "0.93", "the confidence rides the line: {gate}");
    assert_eq!(field_of(&gate, "route="), "skip", "the route taken rides the line: {gate}");
}

/// Confident-work (yes, 0.9 >= 0.85): the borderline eval launches glm —
/// the System One layer never downgrades a judged-real delta to nothing.
#[test]
fn confident_work_routes_the_borderline_eval_to_glm() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("yes", 0.9));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "confident-work launches: {gate}");
    assert_eq!(
        model_of(&gate),
        GLM_ID,
        "the yes route is the routine model (the full provider-routed id): {gate}"
    );
    assert_eq!(field_of(&gate, "route="), "glm", "the route taken rides the line: {gate}");
}

/// Unsure (no, 0.42 < 0.85): the hard judgment stays System Two — the
/// borderline eval escalates to kimi, whatever the choice string said.
#[test]
fn low_confidence_escalates_the_borderline_eval_to_kimi() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.42));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "unsure still launches: {gate}");
    assert_eq!(
        model_of(&gate),
        KIMI_ID,
        "below TRIAGE_HIGH the escalation is the orchestrator model: {gate}"
    );
    assert_eq!(field_of(&gate, "route="), "kimi", "the route taken rides the line: {gate}");
}

/// The threshold boundary is INCLUSIVE at TRIAGE_HIGH: 0.85 itself routes
/// by the choice (skip), 0.849 falls below it (kimi) — the one place the
/// threshold lives must not drift a hair's breadth either way.
#[test]
fn the_threshold_boundary_is_inclusive_at_triage_high() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.85));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(
        field_of(&gate, "route="),
        "skip",
        "conf == TRIAGE_HIGH is at/above it — the choice decides: {gate}"
    );
    serve_judge(&sock, judge_answer("no", 0.849));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(
        field_of(&gate, "route="),
        "kimi",
        "a hair below TRIAGE_HIGH is unsure — kimi: {gate}"
    );
}

// --- req 2: the valve trip consults the SAME triage ----------------------------

/// A confident-empty trip on a FRESH evaluation takes the triaged
/// disposition ($0) — the whole point: the trip's kimi stream was the last
/// big empty-day burn.
#[test]
fn a_confident_empty_trip_takes_the_triaged_disposition() {
    let f = fixture();
    f.disposition_chain(3);
    f.pin_eval_mtime(FRESH_EPOCH);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.95));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "skip", "the trip, cancelled by a confident-empty: {gate}");
    assert!(gate.contains("streak=3"), "the valve position rides the line: {gate}");
    assert_eq!(field_of(&gate, "route="), "skip", "the taken route: {gate}");
}

/// Any other trip answer (yes, unsure) launches the REAL evaluation on the
/// orchestrator model — the triage layer never downgrades a trip — and the
/// route field reads kimi (the route TAKEN, never the triage's raw answer).
#[test]
fn a_trip_with_anything_else_launches_kimi_and_the_route_reads_kimi() {
    let f = fixture();
    f.disposition_chain(3);
    f.pin_eval_mtime(FRESH_EPOCH);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("yes", 0.9));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "the trip launches on any other answer: {gate}");
    assert_eq!(
        model_of(&gate),
        KIMI_ID,
        "the trip stays the real evaluation (the full orchestrator id): {gate}"
    );
    assert_eq!(field_of(&gate, "laya="), "yes", "the verdict is still recorded: {gate}");
    assert_eq!(
        field_of(&gate, "route="),
        "kimi",
        "route= is the route TAKEN, never the triage's raw answer: {gate}"
    );
}

/// A confident-empty on a STALE evaluation cannot cancel the trip: the
/// daily full evaluation is due (the T258 doctrine keeps it on kimi); the
/// verdict is still recorded and the taken route reads kimi.
#[test]
fn a_confident_empty_cannot_cancel_a_stale_trip() {
    let f = fixture();
    f.disposition_chain(3);
    // stale: the mtime 3 days before the fresh epoch (the routing test's
    // staleness arithmetic — anything beyond the 24h window)
    f.pin_eval_mtime(FRESH_EPOCH - 3 * 86_400);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.95));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "the due daily evaluation launches: {gate}");
    assert_eq!(model_of(&gate), KIMI_ID, "the daily evaluation stays kimi: {gate}");
    assert_eq!(field_of(&gate, "laya="), "no", "the verdict is still recorded: {gate}");
    assert_eq!(field_of(&gate, "route="), "kimi", "the taken route reads kimi: {gate}");
}

// --- req 3: fail-open everywhere ------------------------------------------------

/// Daemon down (no socket at the resolved path): the borderline arm falls
/// open to EXACTLY the T258 behavior — glm — and the gate line marks the
/// fail-open (laya=down).
#[test]
fn daemon_down_falls_open_to_the_t258_borderline_routing() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/no-daemon.sock"); // never bound
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "fail-open still launches: {gate}");
    assert_eq!(
        model_of(&gate),
        GLM_ID,
        "fail-open keeps T258 exactly — the borderline eval routes glm: {gate}"
    );
    assert_eq!(field_of(&gate, "laya="), "down", "the fail-open is visible in the line: {gate}");
}

/// Daemon down on a trip: the real evaluation launches kimi, exactly the
/// T247/T258 valve — the triage layer's absence never cheapens a trip.
#[test]
fn daemon_down_falls_open_to_the_t258_trip_routing() {
    let f = fixture();
    f.disposition_chain(3);
    f.pin_eval_mtime(FRESH_EPOCH);
    let sock = f.path().join(".chug/no-daemon.sock");
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(verb_of(&gate), "launch", "fail-open launches the trip: {gate}");
    assert_eq!(model_of(&gate), KIMI_ID, "the fail-open trip stays kimi: {gate}");
    assert_eq!(field_of(&gate, "laya="), "down", "the fail-open is visible in the line: {gate}");
}

/// The predicate probe WRITES NOTHING — including the triage's notes: a
/// daemon-down probe leaves no fail-open line in loopd.log (the probe
/// contract, extended to the triage layer; the run mode's one-note-per-cycle
/// is pinned in the end-to-end test below).
#[test]
fn the_probe_writes_nothing_even_when_the_triage_fails_open() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/no-daemon.sock");
    let before = fixture_log(&f);
    let _ = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(
        fixture_log(&f),
        before,
        "the probe notes nothing into loopd.log — the quiet flag holds: {:?}",
        fixture_log(&f)
    );
}

// --- req 1: the state pack rides the request verbatim ---------------------------

/// The /judge request carries the compact state pack (every field
/// supervisor-computed) and the ONE needs-eval question with the SPEC-3
/// sentence — the triage's whole input surface, pinned on the wire.
#[test]
fn the_state_pack_and_the_one_question_ride_the_judge_request() {
    let f = borderline_fixture();
    f.log_line("2099-01-01T00:00:00Z cycle ended WITHOUT goal complete (consecutive failures: 1)");
    f.pin_eval_mtime(FRESH_EPOCH);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("yes", 0.9));
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    assert_eq!(
        verb_of(&gate),
        "launch",
        "the death makes it borderline; the judged-yes routes glm: {gate}"
    );
    let body = captured(&sock);
    assert!(!body.is_empty(), "the POST /judge body was captured: {body:?}");
    assert!(
        body.contains("\"rows_added\""),
        "rows_added is a state field: {body}"
    );
    assert!(
        body.contains("\"rows_closed\""),
        "rows_closed is a state field: {body}"
    );
    assert!(body.contains("\"child_deaths\":\"1\""), "the death count rides the pack: {body}");
    assert!(
        body.contains("\"files_changed\":\"src=0 tests=0 specs=0 docs=0 book=1 other=0\""),
        "files changed by class ride the pack: {body}"
    );
    assert!(
        body.contains("\"digest_stats\""),
        "the digest stats ride the pack (missing degrades, never blocks): {body}"
    );
    assert!(body.contains("\"empty_streak\":\"0\""), "the streak position rides the pack: {body}");
    assert!(
        body.contains("\"bookkeeping_only_delta\":\"yes\""),
        "the bookkeeping leg rides the pack: {body}"
    );
    assert!(
        body.contains("\"evaluation_fresh\":\"yes\""),
        "the freshness leg rides the pack: {body}"
    );
    assert!(body.contains("\"valve_tripped\":\"no\""), "the valve leg rides the pack: {body}");
    assert!(
        body.contains("needs_eval") && body.contains("\"type\":\"choice\""),
        "exactly ONE question, a 2-way choice: {body}"
    );
    assert!(
        body.contains("Routing classification only, never a quality verdict"),
        "the SPEC-3 constraint is quoted in the instructions: {body}"
    );
    assert!(
        body.contains("\"no\":") && body.contains("\"yes\":"),
        "both criteria are stated: {body}"
    );
}

// --- req 4 + req 6: run mode — the disposition, the corpus, the backfill --------

/// End-to-end: a confident-empty borderline cycle writes the triaged
/// disposition (the subject names the verdict), records ONE laya-triage
/// decision (the state pack verbatim + verdict + confidence + route), notes
/// exactly one triage line, and — on the NEXT look (a mechanical skip, no
/// triage) — backfills the outcome naming that id (landed-clean: the delta
/// stayed bookkeeping-only). The F13 corpus, end to end.
#[test]
fn the_triaged_skip_writes_the_disposition_and_the_decision_records_end_to_end() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "one bounded pass exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fixture_log(&f);
    assert_eq!(
        log.matches("laya triage (T259): needs_eval=no conf=0.93 (HIGH=0.85) -> route=skip").count(),
        1,
        "exactly one triage note for the cycle: {log}"
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("empty-delta disposition"),
        "the T237 token rides the triaged subject too: {subject}"
    );
    assert!(
        subject.contains("T259 laya triage: needs-eval=no conf=0.93"),
        "the triage verdict is legible in the git record: {subject}"
    );
    assert!(
        subject.contains("TRUE streak 0→1"),
        "the streak handoff is unchanged: {subject}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "one decision record after pass 1: {decisions:?}");
    for needle in [
        "\"class\":\"laya-triage\"",
        "\"choice\":\"skip\"",
        "\"confidence\":0.93",
        "valve_tripped\\\":\\\"no\\\"",
        "verdict=needs_eval:no conf=0.93 route=skip",
    ] {
        assert!(
            decisions[0].contains(needle),
            "the record carries {needle}: {}",
            decisions[0]
        );
    }
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "the triage id is parked for the next look's backfill"
    );

    // Pass 2: the delta is STILL borderline (the T2 row is new against the
    // unchanged baseline — the production quiet-day rhythm), so the gate
    // triages again AND the parked pass-1 triage gets its outcome backfill:
    // the delta stayed bookkeeping-only through the next look, so the skip
    // held (landed-clean). Corpus order: triage₁, outcome₁, triage₂.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "the second bounded pass exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fixture_log(&f);
    assert_eq!(
        log.matches("laya triage (T259): needs_eval=").count(),
        2,
        "one triage note per cycle, both cycles: {log}"
    );
    assert_eq!(
        log.matches("outcome backfill").count(),
        1,
        "pass 2 backfilled the parked pass-1 triage's outcome: {log}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 3, "triage₁, outcome₁, triage₂: {decisions:?}");
    let first_id: String = decisions[0]
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(String::from)
        .expect("the triage record has an id");
    for needle in [
        "\"class\":\"outcome\"",
        &format!("\"subject\":\"{first_id}\""),
        "\"choice\":\"landed-clean\"",
        "what the loop found when it looked",
    ] {
        assert!(
            decisions[1].contains(needle),
            "the outcome record carries {needle}: {}",
            decisions[1]
        );
    }
    assert!(
        decisions[2].contains("\"class\":\"laya-triage\"") && decisions[2].contains("\"choice\":\"skip\""),
        "the second cycle recorded its own triage: {}",
        decisions[2]
    );
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "the second triage's id is parked for the next look"
    );
}

/// End-to-end, fail-open: daemon down, a borderline cycle still launches
/// glm (T258 exactly) and notes EXACTLY ONE fail-open line per cycle — one
/// after the first pass, two after the second, never a storm.
#[test]
fn the_fail_open_notes_one_line_per_cycle_never_a_storm() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/no-daemon.sock");
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "one bounded pass exits clean (the failed launch is a verdict, not a crash): {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fixture_log(&f);
    assert_eq!(
        log.matches("laya triage (T259): FAIL-OPEN").count(),
        1,
        "exactly one fail-open note for the cycle: {log}"
    );
    assert!(
        log.contains("eval gate (T258):"),
        "the borderline launch routed through the gate line: {log}"
    );
    assert!(
        log.contains(&format!("-> {GLM_ID}")),
        "fail-open keeps the T258 glm routing: {log}"
    );
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "the second pass exits clean");
    let log = fixture_log(&f);
    assert_eq!(
        log.matches("laya triage (T259): FAIL-OPEN").count(),
        2,
        "one note PER CYCLE — two cycles, two notes, never a storm: {log}"
    );
}

// --- req 5: the doctrine --------------------------------------------------------

/// LOOP-SPEC names the three layers with the SPEC-3 constraint quoted, and
/// points at the ONE place the thresholds live.
#[test]
fn loop_spec_names_the_triage_layer_between_mechanical_and_system_two() {
    let spec = std::fs::read_to_string(repo_root().join("LOOP-SPEC.md")).expect("LOOP-SPEC.md");
    // The spec is hard-wrapped at ~76 columns — flatten it before matching
    // so a needle spanning a line break still reads as prose.
    let flat = spec.replace('\n', " ");
    for needle in [
        "The triage layer (T259)",
        "mechanical",
        "Laya triage",
        "System Two",
        "laya does text classification ONLY — no counting, negation, or completion judgments",
        "TRIAGE_HIGH",
        "needs-eval",
    ] {
        assert!(
            flat.contains(needle),
            "the triage-layer paragraph names {needle:?}: {spec:?}"
        );
    }
}

/// The threshold's comment points back at the doctrine (the one place + the
/// pointer, both pinned).
#[test]
fn the_triage_threshold_lives_in_one_place_with_a_pointer() {
    let loopd = std::fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh");
    count_eq(&loopd, "TRIAGE_HIGH=0.85", 1, "the confidence threshold, one place");
    count_eq(
        &loopd,
        "The confidence threshold lives HERE, the one place (the LOOP-SPEC\n# triage-layer paragraph points back at this constant)",
        1,
        "the comment points at the doctrine",
    );
    // The cascade's routing literals live exactly once each — the fail-open
    // default and the two launch arms keep their T258 shapes.
    count_eq(&loopd, "verb=launch; model=$LOOP_ORCH_MODEL", 2, "both kimi arms unchanged");
    count_eq(
        &loopd,
        "verb=launch; model=$LOOP_ROUTINE_MODEL",
        1,
        "the borderline arm keeps its glm default (fail-open included)",
    );
    count_eq(
        &loopd,
        "laya triage (T259): FAIL-OPEN",
        1,
        "one fail-open emit site — the one-note-per-cycle guarantee is structural",
    );
    count_eq(&loopd, "laya_triage \"", 2, "both consult sites (the trip + the borderline)");
    count_eq(
        &loopd,
        "LAYA_TRIAGE_QUIET=1",
        1,
        "the probe's quiet flag, set at exactly one site",
    );
}

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

// --- T263 helpers: record-field parse, id convention, frozen clock ------------

/// The value of one TOP-LEVEL string field in a decision record line. The
/// writer's jq object order (id, ts, class, subject, inputs, options,
/// choice, confidence) puts every top-level field BEFORE the free-text
/// inputs, so the first occurrence of `"key":"` is the record's own field,
/// never an escaped copy inside the state pack (those carry `\"`).
fn record_field(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    line.split(&needle)
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(String::from)
}

/// The `^d[0-9]+-loopd[0-9]+$` id convention both corpus writers mint off
/// the shared LAYA_SEQ counter — checked shape-wise, no regex dependency.
fn is_loopd_minted_id(id: &str) -> bool {
    match id.strip_prefix('d').and_then(|rest| rest.split_once("-loopd")) {
        Some((epoch, seq)) => {
            !epoch.is_empty()
                && !seq.is_empty()
                && epoch.bytes().all(|b| b.is_ascii_digit())
                && seq.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

/// Pin `date -u +%s` — the corpus writers' epoch source — to a FIXED epoch
/// for the fixture: a stub `date` ahead of PATH that passes everything else
/// through to /bin/date. With the clock frozen, the id a fresh supervisor
/// process mints is fully determined (`d<epoch>-loopd<SEQ>`, LAYA_SEQ
/// starting at 0 each run — the documented per-run uniqueness seam), so the
/// tripwire leg can FORCE the id==subject shape instead of racing the real
/// clock across a process boundary.
fn pin_fake_clock(f: &Fixture, epoch: u64) {
    let bin = f.path().join("bin");
    std::fs::create_dir_all(&bin).expect("fixture stub bin dir");
    write_exec(
        &bin.join("date"),
        &format!(
            "#!/bin/sh\nif [ \"$2\" = \"+%s\" ]; then\n  echo {epoch}\nelse\n  exec /bin/date \"$@\"\nfi\n"
        ),
    );
}

fn ids_of(decisions: &[String]) -> Vec<String> {
    decisions.iter().filter_map(|d| record_field(d, "id")).collect()
}

/// Every id in the written corpus is distinct — the duplicate-id audit
/// count stays at zero for anything this supervisor writes.
fn assert_ids_unique(decisions: &[String]) {
    let ids = ids_of(decisions);
    assert_eq!(
        ids.len(),
        decisions.len(),
        "every record carries an id: {decisions:?}"
    );
    for (i, a) in ids.iter().enumerate() {
        for b in ids.iter().skip(i + 1) {
            assert_ne!(a, b, "duplicate id {a} in the written corpus: {decisions:?}");
        }
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
/// exactly one triage line, and — on the NEXT look, which the fixture keeps
/// BORDERLINE (the production quiet-day rhythm: the T2 row is still new
/// against the unchanged baseline, so the gate triages AGAIN) — backfills
/// the outcome naming that id (landed-clean: the delta stayed
/// bookkeeping-only). The F13 corpus, end to end.
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

// --- T259 fix-up F2: the four mutation survivors, each with its killer ---------

/// Extract one top-level shell function's source from loopd.sh — the textual
/// pins scope to the function so a needle count can never be satisfied by an
/// unrelated site.
fn loopd_fn_src(fn_name: &str) -> String {
    let loopd = std::fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh");
    let start = format!("{fn_name}() {{");
    let at = loopd
        .find(&start)
        .unwrap_or_else(|| panic!("loopd.sh defines {fn_name}"));
    let rest = &loopd[at..];
    let end = rest.find("\n}").expect("the function body closes");
    rest[..end].to_string()
}

/// A judge daemon that accepts the connection and answers only after
/// `delay` — the slow-socket fixture for the 2s fail-open bound: with the
/// bound the probe gives up first (fail-open), without it the answer lands.
fn serve_judge_delayed(sock: &Path, answer: String, delay: Duration) {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).expect("bind the slow fixture judge socket");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 16384];
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
            std::thread::sleep(delay);
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

/// m3-timeout-unbounded: the >2s timeout leg of req 3 is BEHAVIORAL, not
/// just documented. The fixture judge accepts, then answers the
/// confident-empty verdict only after 6s — long past the bound. The probe
/// must give up at 2s, fail open to the T258 glm routing, and return BEFORE
/// the late answer could influence the decision. A mutant that removes or
/// loosens `--max-time 2` lets the 6s answer land: route=skip — the very
/// decision the bound exists to prevent — and the pin fails (the late
/// answer's route reads skip, and the elapsed floor busts 4.5s).
#[test]
fn a_slow_judge_answer_past_two_seconds_fails_open_bounded() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge_delayed(&sock, judge_answer("no", 0.93), Duration::from_secs(6));
    let start = std::time::Instant::now();
    let gate = run_predicate(
        f.path(),
        &[
            ("CHUG_ROUTINE_TODAY", FRESH_DAY),
            ("CHUG_DAEMON_SOCK", sock.to_string_lossy().as_ref()),
        ],
    );
    let elapsed = start.elapsed();
    assert_eq!(
        field_of(&gate, "laya="),
        "down",
        "the 6s answer is past the bound — the triage failed open: {gate}"
    );
    assert_eq!(
        model_of(&gate),
        GLM_ID,
        "the fail-open keeps the T258 glm routing: {gate}"
    );
    assert!(
        elapsed < Duration::from_millis(4500),
        "the probe returned in {elapsed:?} — bounded well before the 6s answer landed"
    );
}

/// m3, textual half: the bound lives in laya_triage at exactly one site —
/// `--max-time 2` — so a removal or a loosened value cannot survive the pin.
#[test]
fn the_laya_triage_curl_is_bounded_at_two_seconds() {
    let body = loopd_fn_src("laya_triage");
    count_eq(&body, "--max-time 2", 1, "the triage call's fail-open bound");
}

/// m7-launch-backfill-flip, held side: a LAUNCHED triage (route=glm) whose
/// evaluation lands its artifacts — the baseline MOVES — backfills
/// landed-clean on the next look. Run mode end to end: pass 1 triages
/// yes/0.9, launches glm (the fixture's launch fails, a verdict death — the
/// record and the pending park regardless), the test then commits a fresh
/// EVALUATION.md (an `eval:` bookkeeping commit) so the next look's baseline
/// differs from the parked one.
#[test]
fn a_launched_triage_backfills_landed_clean_when_the_baseline_moved() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("yes", 0.9));
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 1 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "pass 1 recorded its triage: {decisions:?}");
    assert!(
        decisions[0].contains("\"choice\":\"glm\""),
        "the triage took the glm launch route: {}",
        decisions[0]
    );
    let first_id: String = decisions[0]
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(String::from)
        .expect("the triage record has an id");
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "the launch-side triage id is parked too"
    );

    // The evaluation's artifacts land: an `eval:` commit touching
    // EVALUATION.md — the baseline moves.
    f.commit_tree(
        "eval: cycle-102 fresh eval — the launched evaluation's artifacts",
        None,
        Some("# eval — artifacts landed\n"),
    );

    // Pass 2: the backfill reads the parked record against the MOVED
    // baseline -> landed-clean, before the cycle's own decision.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 2 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fixture_log(&f);
    assert!(
        log.contains(&format!("outcome backfill {first_id} -> landed-clean")),
        "the launch-side backfill names the id and the held label: {log}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 2, "triage then its outcome: {decisions:?}");
    for needle in [
        "\"class\":\"outcome\"",
        &format!("\"subject\":\"{first_id}\""),
        "\"choice\":\"landed-clean\"",
        "the baseline moved",
    ] {
        assert!(
            decisions[1].contains(needle),
            "the outcome record carries {needle}: {}",
            decisions[1]
        );
    }
}

/// m7, not-held side: the same launch, but the next look still finds the
/// baseline UNCHANGED (no evaluation artifacts landed) — the launch did not
/// hold, and the backfill reads fixed-up.
#[test]
fn a_launched_triage_backfills_fixed_up_when_the_baseline_did_not_move() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("yes", 0.9));
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 1 exits clean");
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "pass 1 recorded its triage: {decisions:?}");
    let first_id: String = decisions[0]
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(String::from)
        .expect("the triage record has an id");

    // Pass 2, no baseline move: the delta is still borderline (T2 new, the
    // pass-1 death after the baseline), the gate triages again — and the
    // parked launch triage backfills fixed-up first.
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 2 exits clean");
    let log = fixture_log(&f);
    assert!(
        log.contains(&format!("outcome backfill {first_id} -> fixed-up")),
        "the launch-side backfill reads the not-held label: {log}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 3, "triage, outcome, triage: {decisions:?}");
    assert!(
        decisions[1].contains("\"class\":\"outcome\"")
            && decisions[1].contains("\"choice\":\"fixed-up\"")
            && decisions[1].contains("the baseline is unchanged"),
        "the outcome record carries the not-held label: {}",
        decisions[1]
    );
    assert!(
        decisions[2].contains("\"class\":\"laya-triage\""),
        "the second cycle recorded its own triage: {}",
        decisions[2]
    );
}

/// A TODO.md whose single row is still `todo` — the mode=routine shape the
/// m9 dedup scenario needs (a non-triage cycle between two looks).
const TODO_ONE_TODO: &str = "| id | title | spec | pri | status | notes |\n\
     |----|-------|------|-----|--------|-------|\n\
     | T1 | title | specs/T1-slug.md | 2 | todo | notes |\n";

/// m9-pending-never-removed: ONE outcome per triage id, ever. The pending
/// park survives an intervening NON-triage cycle (a routine-mode cycle never
/// calls laya_record), and the next EVAL look consumes it exactly once — the
/// rm after the backfill means a look that backfills but parks NOTHING (a
/// mechanical skip: laya=none) leaves no stale park behind, so a fourth look
/// can never double-backfill the same id. Four bounded passes:
///   1. borderline triaged skip -> parks id X;
///   2. routine mode (a `todo` row + fresh eval, a green stub cycle) — no
///      laya_record, X survives;
///   3. MECHANICAL skip (predicate empty, laya=none) — the look still
///      backfills X once, and parks nothing: the pending file is GONE;
///   4. mechanical skip again — nothing left to backfill.
///
/// A mutant that drops the rm re-backfills X on pass 4 — a second outcome
/// record naming X and a surviving pending file — and both pins fail.
#[test]
fn a_pending_surviving_an_intervening_non_triage_cycle_backfills_exactly_once() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    // Pass 1: the triaged skip parks X.
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 1 exits clean");
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "pass 1 recorded its triage: {decisions:?}");
    let x_id: String = decisions[0]
        .split("\"id\":\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .map(String::from)
        .expect("the triage record has an id");

    // Pass 2: ROUTINE mode — a `todo` row plus the fresh evaluation; the
    // stubbed cycle child exits green (no death line, no commits). The
    // gate/laya_record never run; the parked X survives untouched.
    f.commit_tree(
        "eval: cycle-103 fresh eval — a row lands, the queue works",
        Some(TODO_ONE_TODO),
        None,
    );
    std::fs::create_dir_all(f.path().join("target/release")).expect("fixture chug dir");
    // The stub cycle child: green verdict shape (rc 0 + the goal-complete
    // marker the supervisor's verdict grep reads) — no death line, no
    // commits, no wrap writes.
    write_exec(
        &f.path().join("target/release/chug"),
        "#!/bin/sh\necho \"chug: goal complete\"\nexit 0\n",
    );
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 2 (routine) exits clean");
    let log = fixture_log(&f);
    assert!(
        !log.contains("cycle ended WITHOUT goal complete"),
        "the stub cycle exits green — no death line reshapes pass 3's gate: {log}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(
        decisions.len(),
        1,
        "a routine cycle records no triage and no outcome: {decisions:?}"
    );
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "the park survives the intervening non-triage cycle"
    );

    // Pass 3: eval again, predicate EMPTY (the row re-drained, no deaths) —
    // the mechanical skip arm, laya=none. The look still consumes the park:
    // X backfills exactly once, and NOTHING is parked after it.
    f.commit_tree(
        "eval: cycle-104 fresh eval — the queue re-drained",
        Some(TODO_ONE_DONE),
        None,
    );
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 3 exits clean");
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 2, "triage X, outcome X: {decisions:?}");
    assert_eq!(
        decisions[1]
            .split("\"subject\":\"")
            .nth(1)
            .and_then(|s| s.split('"').next()),
        Some(x_id.as_str()),
        "the outcome names the parked id: {}",
        decisions[1]
    );
    assert!(
        !f.path().join(".chug/loopd/triage-pending.json").exists(),
        "a look that backfills but parks nothing (laya=none) CONSUMES the park — \
         no stale pending survives it"
    );

    // Pass 4: the same mechanical skip — nothing left to backfill, and the
    // parked id X must never be backfilled twice.
    let out = run_one_pass(&f, &sock);
    assert!(out.status.success(), "pass 4 exits clean");
    let decisions = fixture_decisions(&f);
    let outcomes: Vec<&String> = decisions
        .iter()
        .filter(|d| d.contains("\"class\":\"outcome\""))
        .collect();
    assert_eq!(outcomes.len(), 1, "one outcome per triage id: {decisions:?}");
    assert_eq!(
        outcomes
            .iter()
            .filter(|d| d.contains(&format!("\"subject\":\"{x_id}\"")))
            .count(),
        1,
        "the parked id X was backfilled EXACTLY ONCE across four looks: {decisions:?}"
    );
}

/// m11-tripped-flag-dead: the record's valve_tripped derivation is live — a
/// run-mode trip-cancel (streak 3, fresh, confident-empty) records
/// valve_tripped:"yes" in the triage record's state pack, and the
/// disposition subject names the trip cancellation.
#[test]
fn a_trip_cancel_records_valve_tripped_yes_in_the_record() {
    let f = fixture();
    f.disposition_chain(3);
    f.pin_eval_mtime(FRESH_EPOCH);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.95));
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "the bounded pass exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let subject = git_out(f.path(), &["log", "-1", "--format=%s"]);
    assert!(
        subject.contains("T259 laya trip: needs-eval=no conf=0.95"),
        "the cancelled trip's disposition names the trip reason: {subject}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "one triage record: {decisions:?}");
    assert!(
        decisions[0].contains("\"choice\":\"skip\""),
        "the cancelled trip took the skip route: {}",
        decisions[0]
    );
    assert!(
        decisions[0].contains("valve_tripped\\\":\\\"yes\\\""),
        "the state pack's valve leg reads yes on a trip (streak 3): {}",
        decisions[0]
    );
}

// --- T263: the outcome record mints its OWN id, never the triage id -----------

/// T263 req 1 + req 4a: the outcome backfill's OWN id is a FRESH mint —
/// the triage writer's exact convention (d<epoch>-loopd<SEQ>, the shared
/// LAYA_SEQ counter) — and the parked triage id stays the record's
/// SUBJECT. Two consecutive triaged cycles: the corpus holds triage₁ (id
/// X), the outcome naming X as its subject, and triage₂ — and the
/// outcome's id differs from X and from EVERY other id in the written
/// corpus. The pre-fix writer passed the triage id as the outcome's own
/// id (arg 1 AND arg 4): a duplicate id and a self-subject record,
/// malformed on its face — measured on the live corpus as duplicate ids
/// 0→1 and malformed chain 1→3. RED-PROOF LEG: this test fails against
/// the pre-fix loopd.sh (the duplicate id) and passes on the fixed one.
#[test]
fn an_outcome_record_mints_its_own_id_never_the_parked_triage_id() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    // Pass 1: the triaged skip parks triage id X.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 1 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "pass 1 recorded its triage: {decisions:?}");
    let x_id = record_field(&decisions[0], "id").expect("the triage record has an id");

    // Pass 2: the gate triages again (the delta is still borderline — the
    // production quiet-day rhythm) and the parked X gets its outcome
    // backfill before triage₂ lands. Corpus order: triage₁, outcome₁,
    // triage₂.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 2 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 3, "triage₁, outcome₁, triage₂: {decisions:?}");
    assert!(
        decisions[1].contains("\"class\":\"outcome\""),
        "the middle record is the outcome: {}",
        decisions[1]
    );
    assert_eq!(
        record_field(&decisions[1], "subject").as_deref(),
        Some(x_id.as_str()),
        "the outcome's subject is STILL the parked triage id: {}",
        decisions[1]
    );
    let o_id = record_field(&decisions[1], "id").expect("the outcome record has an id");
    assert_ne!(
        o_id, x_id,
        "the outcome's OWN id is never the triage id it backfills: {decisions:?}"
    );
    assert_ids_unique(&decisions);
}

/// T263 req 2 + req 4b: the self-subject tripwire. With the clock frozen,
/// a fresh supervisor run mints the SAME id the previous run's triage
/// parked (`d<epoch>-loopd1` both times — LAYA_SEQ restarts at 0 per run,
/// the documented per-run uniqueness seam): the id==subject shape exactly,
/// forced deterministically instead of raced. The real end-to-end backfill
/// must log ONE WARN line to loopd.log naming the id and SKIP the corpus
/// write — the write stays best-effort, never blocking the cycle — and the
/// park is still consumed exactly once (the rm flow untouched): triage₂
/// proceeds and re-parks its own (different) id. RED pre-fix: the writer
/// happily wrote the duplicate instead of warning.
#[test]
fn a_self_subject_backfill_warns_and_writes_nothing() {
    let f = borderline_fixture();
    pin_fake_clock(&f, FRESH_EPOCH);
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    // Pass 1: the triage mints d<epoch>-loopd1 and parks it.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 1 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 1, "pass 1 recorded its triage: {decisions:?}");
    let x_id = record_field(&decisions[0], "id").expect("the triage record has an id");
    assert_eq!(
        x_id,
        format!("d{FRESH_EPOCH}-loopd1"),
        "the frozen clock pins the mint: {x_id}"
    );
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "X is parked for the next look"
    );

    // Pass 2: the backfill's own mint lands on d<epoch>-loopd1 — the
    // parked id exactly. The tripwire warns and skips; the cycle still
    // completes and triage₂ mints the NEXT sequence value.
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 2 exits clean (the corpus write never blocks the cycle): {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let log = fixture_log(&f);
    assert_eq!(
        log.matches(&format!("WARN outcome id == subject ({x_id})")).count(),
        1,
        "ONE WARN line naming the colliding id: {log}"
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 2, "triage₁ and triage₂, NO outcome record: {decisions:?}");
    assert!(
        !decisions.iter().any(|d| d.contains("\"class\":\"outcome\"")),
        "the self-subject write was SKIPPED — no outcome line in the corpus: {decisions:?}"
    );
    assert_ids_unique(&decisions);
    // The park was consumed exactly once even on the skipped look: triage₂
    // re-parked its own id (never X's — X can never double-backfill).
    assert!(
        f.path().join(".chug/loopd/triage-pending.json").exists(),
        "the skipped look still consumed X's park and triage₂ re-parked its own id"
    );
    let parked =
        std::fs::read_to_string(f.path().join(".chug/loopd/triage-pending.json"))
            .expect("read the parked record");
    let triage2_id = record_field(&decisions[1], "id").expect("triage₂ has an id");
    assert!(
        parked.contains(&triage2_id),
        "the surviving park names triage₂, not the skipped X: {parked}"
    );
}

/// T263 req 4c: the minted outcome id follows the SAME convention as the
/// triage ids — `^d[0-9]+-loopd[0-9]+$`, minted off the shared LAYA_SEQ
/// counter — so the corpus stays one id shape and the audit's id→class map
/// stays well-formed. Every id in a two-cycle corpus (triage₁, outcome₁,
/// triage₂) is checked.
#[test]
fn the_minted_outcome_id_follows_the_triage_id_convention() {
    let f = borderline_fixture();
    let sock = f.path().join(".chug/daemon.sock");
    serve_judge(&sock, judge_answer("no", 0.93));
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 1 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run_one_pass(&f, &sock);
    assert!(
        out.status.success(),
        "pass 2 exits clean: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let decisions = fixture_decisions(&f);
    assert_eq!(decisions.len(), 3, "triage₁, outcome₁, triage₂: {decisions:?}");
    assert!(
        decisions[1].contains("\"class\":\"outcome\""),
        "the middle record is the outcome whose minted id is under test: {}",
        decisions[1]
    );
    for d in &decisions {
        let id = record_field(d, "id").unwrap_or_default();
        assert!(
            is_loopd_minted_id(&id),
            "every minted id matches ^d[0-9]+-loopd[0-9]+$: {id:?} in {d}"
        );
    }
}

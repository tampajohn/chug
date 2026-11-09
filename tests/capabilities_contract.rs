//! T339 — the meta-harness capability contract, pinned at the process level
//! against the REAL binary (`CARGO_BIN_EXE_chug`).
//!
//! Pins the spec's Tests list end to end:
//! - fixture: `chug capabilities` parses as JSON, carries the required
//!   keys, and matches the pinned contract document
//!   (`tests/fixtures/capabilities/contract-v1.json`, the `$CHUG_VERSION`
//!   placeholder substituted) — the shape T340-T343 edit as they flip
//!   their keys;
//! - latency: the command completes <50ms with AND without a live daemon
//!   (a missing daemon reports false/null, never a hang) — measured as
//!   best-of-3 after a warm-up exec, because the budget pins the
//!   command's intrinsic cost (no daemon contact, no I/O: a probe or
//!   file-walk mutant measures in seconds, not microseconds), while
//!   spawn/scheduler noise on a loaded box is not the command's doing;
//! - contract doc link-check: every feature key in `docs/meta-harness.md`'s
//!   registry table appears in the command output — and vice versa (the
//!   doc and the command are one registry; either drifting alone is RED);
//! - forward-compat: an output with injected unknown keys/values parses
//!   IDENTICALLY through chug's own reference reader
//!   (`chug capabilities --check`, the `--check` canonical form) — the
//!   stability rule made executable;
//! - reader legs: a version-string feature value (the T340
//!   `lifecycle_events: "1"` shape) parses; out-of-contract values and
//!   missing identity blocks refuse non-zero; an absent features block
//!   reads as all-false ("absent = false").
//!
//! T48 doctrine: the repo root is resolved at RUNTIME
//! (`std::env::current_dir()`; cargo runs test binaries with cwd = the
//! package root). The daemon leg follows tests/daemon_lifecycle.rs's
//! pattern: a weightless stub (`CHUG_DAEMON_STUB=1`) on a per-test
//! tempdir `CHUG_HOME`, every wait deadline-bounded, the daemon killed by
//! a Drop guard so a failing assertion cannot leak an orphan.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

/// The pinned contract document (T339's output shape; T340-T343 edit it).
const FIXTURE: &str = "tests/fixtures/capabilities/contract-v1.json";
/// The contract doc the link-check runs against.
const DOC: &str = "docs/meta-harness.md";
/// The latency budget from the spec: <50ms per invocation.
const BUDGET: Duration = Duration::from_millis(50);
/// Bounded waits (the T6 stub-harness rule: a wedged child fails its test
/// in seconds; it can never hang the suite).
const DAEMON_DEADLINE: Duration = Duration::from_secs(60);
const CLI_DEADLINE: Duration = Duration::from_secs(30);

/// The five feature keys, in contract order — the registry both the
/// fixture and the doc table must agree with.
const FEATURE_KEYS: [&str; 5] = [
    "daemon_status",
    "lifecycle_events",
    "permission_broker",
    "usage_events",
    "status_mod",
];

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn out_text(out: &Output) -> String {
    format!(
        "--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Run `chug capabilities` (plus caller args) in the repo root with the
/// ambient env — the document is cwd- and env-independent by contract,
/// and asserting stdout parses as JSON IS the stdout-purity check (a
/// stray banner would break every parse in this file).
fn run_caps(extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("capabilities")
        .args(extra)
        .current_dir(repo_root())
        .stdin(Stdio::null())
        .output()
        .expect("spawn chug capabilities")
}

/// Absorb the macOS first-exec stall (Gatekeeper/syspolicyd assessment of
/// a freshly built binary can park a child in `_dyld_start` for tens of
/// seconds — see tests/daemon_lifecycle.rs): exec the binary once,
/// bounded, BEFORE any timed section. `--version` is instant and
/// side-effect-free.
fn warm_exec() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn the warm-up exec");
    let started = Instant::now();
    loop {
        match child.try_wait().expect("try_wait the warm-up exec") {
            Some(status) => {
                assert_eq!(status.code(), Some(0), "the warm-up exec failed");
                return;
            }
            None if started.elapsed() > CLI_DEADLINE => panic!("warm-up exec never exited"),
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
}

/// Parse `chug capabilities` stdout as JSON, asserting the process shape
/// on the way (exit 0, non-empty stdout).
fn caps_value(extra: &[&str]) -> Value {
    let out = run_caps(extra);
    assert!(out.status.success(), "{}", out_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout must parse as JSON ({e}): {stdout}"))
}

// ---------- fixture: parses, required keys, exact pinned shape ----------

/// The spec's fixture leg: the output parses as JSON, carries the required
/// keys, and matches the pinned contract document byte-for-byte at the
/// value level (the `$CHUG_VERSION` placeholder substituted with the
/// package version the test and the binary share).
#[test]
fn capabilities_output_matches_the_pinned_fixture() {
    let actual = caps_value(&[]);

    // Required keys, named one by one — a missing key reads as an
    // actionable failure, not a whole-document diff.
    for key in ["contract_version", "compat", "features", "versions"] {
        assert!(
            actual.get(key).is_some(),
            "required key `{key}` missing: {actual}"
        );
    }
    assert_eq!(actual["contract_version"], "1", "T339 ships contract v1");
    assert!(
        actual["compat"].as_str().is_some_and(|s| !s.is_empty()),
        "the compat promise rides the document: {actual}"
    );

    let fixture_raw = std::fs::read_to_string(repo_root().join(FIXTURE))
        .unwrap_or_else(|e| panic!("{FIXTURE} must exist and read: {e}"));
    let expected_raw = fixture_raw.replace("$CHUG_VERSION", env!("CARGO_PKG_VERSION"));
    let expected: Value = serde_json::from_str(&expected_raw)
        .unwrap_or_else(|e| panic!("{FIXTURE} (substituted) must parse: {e}"));

    assert_eq!(actual, expected, "the document drifted from {FIXTURE}");

    // The registry posture, spelled out: all five features false today —
    // T340-T343 flip theirs as they land — the chug version is this
    // build's, and the daemon version is null (no self-report in T339).
    let features = actual["features"].as_object().expect("features object");
    assert_eq!(
        features.len(),
        FEATURE_KEYS.len(),
        "the registry is exactly the five keys: {features:?}"
    );
    for key in FEATURE_KEYS {
        assert_eq!(
            features.get(key).map(Value::as_bool),
            Some(Some(false)),
            "{key} is false until its row lands"
        );
    }
    assert_eq!(actual["versions"]["chug"], env!("CARGO_PKG_VERSION"));
    assert!(
        actual["versions"]["daemon"].is_null(),
        "no daemon self-report in T339: {actual}"
    );
}

// ---------- latency: <50ms without and with a live daemon ----------

/// One timed `chug capabilities` run with a scoped env. Returns
/// (elapsed, output).
fn timed_caps(home: &Path) -> (Duration, Output) {
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("capabilities")
        .current_dir(repo_root())
        .env("CHUG_HOME", home) // scoped, never the test process's global env
        .env_remove("CHUG_DAEMON_SOCK")
        .env_remove("CHUG_DELEGATE_BIN")
        .env_remove("CHUG_LAYA_CHECKPOINT")
        .stdin(Stdio::null())
        .output()
        .expect("spawn chug capabilities");
    (started.elapsed(), out)
}

/// Best-of-3 latency assert. The budget pins the command's intrinsic cost
/// — a daemon probe (seconds) or a config walk (milliseconds) cannot hide
/// under it — while one scheduler hiccup inflating a single spawn is not
/// the command's doing. Every run must still exit 0 with parseable JSON.
fn assert_under_budget(home: &Path, context: &str) -> Value {
    let mut best: Option<(Duration, Value)> = None;
    for i in 0..3 {
        let (elapsed, out) = timed_caps(home);
        assert!(out.status.success(), "run {i}: {}", out_text(&out));
        let stdout = String::from_utf8_lossy(&out.stdout);
        let value: Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("run {i}: stdout must parse ({e}): {stdout}"));
        let better = best.as_ref().is_none_or(|(b, _)| elapsed < *b);
        if better {
            best = Some((elapsed, value));
        }
    }
    let (best_elapsed, value) = best.expect("three runs ran");
    assert!(
        best_elapsed < BUDGET,
        "{context}: best of 3 was {best_elapsed:?}, budget {BUDGET:?}"
    );
    value
}

/// Without a daemon: an empty tempdir as `CHUG_HOME` (no daemon home, no
/// socket) — the missing-daemon leg. The document must be identical to
/// the fixture posture AND fast; a daemon probe (even a failing one)
/// would blow the budget.
#[test]
fn capabilities_completes_under_50ms_without_a_daemon() {
    warm_exec();
    let home = tempfile::tempdir().expect("tempdir");
    let value = assert_under_budget(home.path(), "no live daemon");
    assert_eq!(
        value["features"]["daemon_status"],
        false,
        "no daemon: daemon_status false, never an error"
    );
    assert!(value["versions"]["daemon"].is_null());
}

/// One stub daemon (`CHUG_DAEMON_STUB=1`: real transport, lock, socket,
/// no model) on its own tempdir home. The Drop guard SIGKILLs and reaps
/// the child, so neither a failing assertion nor a successful one can
/// leak an orphan or a zombie.
struct StubDaemon {
    home: tempfile::TempDir,
    child: std::process::Child,
}

impl StubDaemon {
    fn spawn() -> Self {
        let home = tempfile::tempdir().expect("tempdir");
        let child = Command::new(env!("CARGO_BIN_EXE_chug"))
            .arg("daemon")
            .env("CHUG_HOME", home.path())
            .env("CHUG_DAEMON_STUB", "1")
            .env_remove("CHUG_DAEMON_SOCK")
            .env_remove("CHUG_DELEGATE_BIN")
            .env_remove("CHUG_LAYA_CHECKPOINT")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the stub daemon");
        let daemon = Self { home, child };
        daemon.wait_healthy();
        daemon
    }

    fn sock(&self) -> PathBuf {
        self.home.path().join("daemon.sock")
    }

    /// Bounded wait for `GET /healthz` to answer 200 — the model loads
    /// BEFORE the socket binds in a real daemon, so healthz never lies;
    /// the stub has no model and binds at once.
    fn wait_healthy(&self) {
        let started = Instant::now();
        loop {
            if matches!(uds_get(&self.sock(), "/healthz"), Ok((200, _))) {
                return;
            }
            if started.elapsed() > DAEMON_DEADLINE {
                panic!("the stub daemon never came up within {DAEMON_DEADLINE:?}");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for StubDaemon {
    fn drop(&mut self) {
        // SIGKILL + reap — no orphan, no zombie, even mid-failure.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One hand-rolled HTTP GET over the unix socket (the daemon's public
/// contract; no lib access from an integration test — the
/// daemon_lifecycle.rs shape).
fn uds_get(sock: &Path, path: &str) -> Result<(u16, String), String> {
    let mut stream = UnixStream::connect(sock).map_err(|e| e.to_string())?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let request = format!("GET {path} HTTP/1.1\r\nHost: t\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut raw = Vec::new();
    (&stream)
        .take(1 << 20)
        .read_to_end(&mut raw)
        .map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("malformed response: {text:?}"))?;
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

/// With a live daemon: the command stays under budget (it never contacts
/// the socket — a probe mutant would pay the round trip) and T339's
/// posture holds even with a daemon answering on the very home the
/// command is pointed at: `daemon_status` stays false and
/// `versions.daemon` stays null until T340's row lands the bounded probe
/// and the self-report (which edits this test).
#[test]
fn capabilities_completes_under_50ms_with_a_live_daemon() {
    warm_exec();
    let daemon = StubDaemon::spawn();
    // The daemon is genuinely live on THIS home before the timed runs.
    assert_eq!(
        uds_get(&daemon.sock(), "/healthz").map(|(s, _)| s),
        Ok(200),
        "stub daemon healthy"
    );
    let value = assert_under_budget(daemon.home.path(), "live daemon");
    assert_eq!(
        value["features"]["daemon_status"],
        false,
        "T339 posture: the feature is unshipped; a live daemon does not flip it \
         (the bounded-probe liveness report is T340's row)"
    );
    assert!(
        value["versions"]["daemon"].is_null(),
        "no self-report until T340 lands it"
    );
}

// ---------- contract doc link-check ----------

/// Extract the feature keys from `docs/meta-harness.md`'s `## Feature
/// keys` registry table: the first cell of every data row, backticks
/// stripped. Header and separator rows are skipped; any other table shape
/// fails the non-vacuousness guard below rather than silently matching
/// nothing.
fn doc_feature_keys(doc: &str) -> Vec<String> {
    let mut section = false;
    let mut keys = Vec::new();
    for line in doc.lines() {
        if line.starts_with("## ") {
            section = line == "## Feature keys";
            continue;
        }
        if !section || !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // split('|') on "| a | b |" yields ["", " a ", " b ", ""] — the
        // first cell is cells[1].
        let Some(first) = cells.get(1) else { continue };
        let key = first.trim_matches('`');
        if key == "key" || key.is_empty() || key.chars().all(|c| c == '-') {
            continue; // header / separator rows
        }
        keys.push(key.to_string());
    }
    keys
}

/// The spec's link-check leg: every feature key in the doc appears in the
/// command output — and every key in the output appears in the doc. The
/// two are one registry; drift in either direction is RED. Non-vacuous by
/// construction: both sides must carry exactly the five contract keys.
#[test]
fn docs_meta_harness_feature_keys_match_the_command_output() {
    let doc = std::fs::read_to_string(repo_root().join(DOC))
        .unwrap_or_else(|e| panic!("{DOC} must exist and read: {e}"));
    let documented = doc_feature_keys(&doc);
    assert_eq!(
        documented,
        FEATURE_KEYS,
        "{DOC}'s `## Feature keys` table must list exactly the contract registry \
         (in contract order) — the table parser found {documented:?}"
    );

    let actual = caps_value(&[]);
    let features = actual["features"].as_object().expect("features object");
    let mut in_output: Vec<&String> = features.keys().collect();
    in_output.sort();
    let mut expected: Vec<&str> = FEATURE_KEYS.to_vec();
    expected.sort();
    assert_eq!(
        in_output, expected,
        "the command's registry drifted from the contract"
    );
}

// ---------- forward-compat: the stability rule on our own reader ----------

/// Write `text` to a temp file and run `chug capabilities --check` over
/// it: the reference reader's canonical form on stdout, exit 0.
fn check_canonical(dir: &Path, name: &str, text: &str) -> String {
    let path = dir.join(name);
    std::fs::write(&path, text).expect("write the document fixture");
    let out = run_caps(&["--check", path.to_str().expect("utf-8 path")]);
    assert!(out.status.success(), "{}", out_text(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The spec's forward-compat leg: an output with injected unknown
/// keys/values parses IDENTICALLY through chug's own reader. Unknown keys
/// at every depth (top level, features, versions) and unknown values are
/// dropped by the canonical form; the clean document roundtrips to
/// itself, so the producer's output IS the reader's canonical form.
#[test]
fn forward_compat_injected_unknown_keys_parse_identically() {
    let original_out = run_caps(&[]);
    assert!(original_out.status.success(), "{}", out_text(&original_out));
    let original = String::from_utf8_lossy(&original_out.stdout).into_owned();

    let mut injected: Value = serde_json::from_str(&original).expect("the original parses");
    injected["future_section"] = serde_json::json!({ "anything": [1, 2, 3] });
    injected["future_scalar"] = serde_json::json!("ignored");
    injected["features"]["future_feature"] = serde_json::json!(true);
    injected["features"]["another_future"] = serde_json::json!("9.9.9");
    injected["versions"]["future_version"] = serde_json::json!("x");
    let injected_text = serde_json::to_string_pretty(&injected).expect("inject");

    let tmp = tempfile::tempdir().expect("tempdir");
    let clean = check_canonical(tmp.path(), "clean.json", &original);
    let injected_canon = check_canonical(tmp.path(), "injected.json", &injected_text);

    assert_eq!(
        injected_canon, clean,
        "unknown keys/values must parse identically through the reference reader"
    );
    assert_eq!(
        clean, original,
        "roundtrip: the producer's output is the reader's canonical form"
    );
}

/// The reader accepts the value shapes the contract allows: a
/// version-string feature (T340's `lifecycle_events: "1"` shape — the
/// "boolean or version string once shipped" rule) parses and survives
/// canonicalization, and unknown feature keys ride through the read
/// without breaking it.
#[test]
fn reader_accepts_version_string_features() {
    let doc = r#"{
        "contract_version": "1",
        "features": { "lifecycle_events": "1", "future_feature": true },
        "versions": { "chug": "0.0.0" }
    }"#;
    let tmp = tempfile::tempdir().expect("tempdir");
    let canonical = check_canonical(tmp.path(), "t340-shape.json", doc);
    let value: Value = serde_json::from_str(&canonical).expect("canonical parses");
    assert_eq!(
        value["features"]["lifecycle_events"], "1",
        "the shipped version string survives the roundtrip"
    );
    assert_eq!(
        value["features"]["daemon_status"], false,
        "absent features read as false (absent = false)"
    );
    assert!(
        value["features"].get("future_feature").is_none(),
        "unknown features are dropped — the consumer rule, in the canonical form"
    );
    assert_eq!(value["versions"]["daemon"], Value::Null);
}

/// The identity blocks are required and out-of-contract values refuse:
/// `--check` is a validator, loud about documents outside the contract.
#[test]
fn reader_refuses_malformed_documents() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let legs: [(&str, &str); 3] = [
        ("no-version.json", r#"{"versions":{"chug":"0.0.0"}}"#),
        ("no-versions.json", r#"{"contract_version":"1"}"#),
        (
            // A known feature carrying an out-of-contract value: the
            // contract allows booleans and version strings, nothing else.
            "number-feature.json",
            r#"{"contract_version":"1","versions":{"chug":"x"},"features":{"daemon_status":1}}"#,
        ),
    ];
    for (name, text) in legs {
        let path = tmp.path().join(name);
        std::fs::write(&path, text).expect("write the malformed fixture");
        let out = run_caps(&["--check", path.to_str().expect("utf-8 path")]);
        assert!(
            !out.status.success(),
            "{name} must refuse: {}",
            out_text(&out)
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("parsing"),
            "{name}: the refusal names the parse failure: {stderr}"
        );
        assert!(
            String::from_utf8_lossy(&out.stdout).trim().is_empty(),
            "{name}: a refused document prints nothing on stdout"
        );
    }

    // The absent-features leg is NOT malformed: "absent = false" — the
    // whole features block may be missing, and every feature reads false.
    let canonical = check_canonical(
        tmp.path(),
        "absent-features.json",
        r#"{"contract_version":"1","versions":{"chug":"x"}}"#,
    );
    let value: Value = serde_json::from_str(&canonical).expect("canonical parses");
    for key in FEATURE_KEYS {
        assert_eq!(value["features"][key], false, "{key}: absent = false");
    }
}

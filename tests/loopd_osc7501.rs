//! T354 — loopd emits OSC 7501 status natively (the terminal harness channel).
//!
//! Fleet Primary live-verified 2026-11-09: herdr's agent model is terminals
//! whose program reports status via OSC 7501 (Mitchell Hashimoto's Program
//! Status Protocol). A bare `chug loop run` used to emit nothing — status
//! null, no status dot, absent from the Agents list — and the fleet papered
//! over it with a per-harness shim (`chug-loop-agent`, herdr-fleet-mods
//! 252a6f8) that parsed loopd.log and re-emitted 7501. T354 moves the
//! emission into loopd.sh itself so the shim dies for herdr AND for every
//! future terminal harness:
//!
//! * **Emission legs** (the shim's proven semantics mapped onto loopd's own
//!   transitions): cycle start / ideation item start → `working`; cycle OK
//!   (the accepted goal), dormant, plain skip → `idle`; build-gate red,
//!   disposition-commit (delivery) failure, cycle death, the 3-strike HALTs
//!   → `blocked`, with the ONE-line reason on the msg channel headed by its
//!   T340 blocked-reason word (`budget` for the rc-1 budget/abort exit,
//!   `error` otherwise) — one source of truth for both surfaces.
//! * **Wire**: the id-less ROOT record —
//!   `ESC ] 7501 ; state=<s>[:msg=<base64>] ESC \` — msg is one base64
//!   line, the whole sequence ≤4096 bytes, no `id`/`app`/`progress` keys.
//! * **Gating (the compat contract)**: emit ONLY when stdout is a TTY AND
//!   `CHUG_OSC7501` is unset or `=1`; every redirected context (nohup,
//!   launchd, pipes, log files) receives ZERO escape bytes; `CHUG_OSC7501=0`
//!   kills even on a TTY.
//!
//! THE DEVIATION NOTE (the spec's reference was not readable): the
//! herdr-fleet-mods shim at commit 252a6f8 was ABSENT on the build host
//! (`~/workspace/herdr-fleet-mods` does not exist), so the byte-for-byte
//! shim match the spec asked for was impossible. The wire here follows the
//! PUBLISHED OSC 7501 specification semantics instead (the T354 semantics
//! table plus the protocol's field rules: required `state`, optional
//! base64 `msg`, `ESC \` or BEL terminator, 4096-byte cap, invalid state
//! drops the sequence) — the same id-less-root record the spec describes
//! the shim as implementing. The wire-compat fixture below therefore parses
//! every emitted byte with a spec-derived REFERENCE PARSER (the shim's
//! parser stand-in) and asserts the parse: state valid, msg decodes to the
//! exact reason, root record carries no id. Byte-compat is claimed at the
//! spec level, not the shim-diff level, and `docs/meta-harness.md` says so.
//!
//! Fixture doctrine: the pty is REAL (`libc::openpty` + `pre_exec` dup2 —
//! the gate's `[ -t 1 ]` sees an actual tty, and the capture is the exact
//! byte stream the terminal would see). The gating legs run through the
//! `osc` probe (the routing/sleep-ok/predicate pattern) so the REAL gated
//! emitter is exercised, never a reimplementation. The emission legs run the
//! REAL supervisor end to end in a sandbox (the tests/loopd_stale_binary.rs
//! pattern: hermetic ps/lsof/cargo stubs, LOOPD_SLEEP_* seams, the T254
//! max-loops bound as the terminator) and assert against both the captured
//! pty stream and the redirected log files — which must hold ZERO escape
//! bytes, because log parsers must never see one.

#![cfg(unix)]

#[path = "common/mod.rs"]
mod common;

// T172 — the cross-binary serialization domain for the deadline-bearing
// sandbox fixtures below (nextest runs each test as its own PROCESS; the
// static T151 domain cannot see sibling sandbox processes across binaries).
#[path = "support/load_lock.rs"]
mod t172_load_lock;

// T159: the tests in this file that spawn a REAL `loopd.sh` join T151's ONE
// shared test-timing serialization domain — the include compiles
// `src/testsupport.rs` itself (the same declaration, the same poison-tolerant
// `timing_guard()`, the fixed singleton export symbol), so no second lock
// exists anywhere in the repo. Discipline is T151's verbatim: the guard is
// the FIRST acquisition in a real-supervisor test body, held across spawn →
// assertion → cleanup.
#[path = "../src/testsupport.rs"]
mod testsupport;

use std::fs::{self, File};
use std::io::Read;
use std::os::fd::FromRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::io::RawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tempfile::TempDir;

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;

// ---------------------------------------------------------------------------
// The OSC 7501 wire: a spec-derived reference codec
// ---------------------------------------------------------------------------

/// The RFC 4648 standard-alphabet encoder — the fixtures' independent
/// expected-bytes builder. The emitter shells out to the platform `base64`;
/// the fixture computes the expected value itself rather than trusting the
/// shell tool (a broken `base64` must fail the fixture, not the assertion).
fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

/// The reference decoder (standard alphabet, `=` padding tolerated) — the
/// parser's msg-channel half.
fn b64_decode(s: &str) -> Option<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &b in s.as_bytes() {
        if b == b'=' {
            break;
        }
        let v = T.iter().position(|&t| t == b)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

/// One parsed OSC 7501 record (the id-less root record in loopd's emission —
/// `id` stays `None`; the field is modeled so the parser stays spec-faithful).
#[derive(Debug, PartialEq, Eq, Clone)]
struct Record7501 {
    state: String,
    id: Option<String>,
    kind: Option<String>,
    progress: Option<String>,
    app: Option<String>,
    title: Option<String>,
    msg: Option<String>,
}

fn rec(state: &str, msg: Option<&str>) -> Record7501 {
    Record7501 {
        state: state.to_string(),
        id: None,
        kind: None,
        progress: None,
        app: None,
        title: None,
        msg: msg.map(str::to_string),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The spec's terminator rule: BEL or ST, within the 4096-byte cap from
/// `from`. Returns the body slice and the offset just past the terminator.
fn find_terminator(bytes: &[u8], from: usize, cap: usize) -> Option<(&[u8], usize)> {
    let end = from.saturating_add(cap).min(bytes.len());
    let mut j = from;
    while j < end {
        if bytes[j] == BEL {
            return Some((&bytes[from..j], j + 1));
        }
        if bytes[j] == ESC && j + 1 < end && bytes[j + 1] == b'\\' {
            return Some((&bytes[from..j], j + 2));
        }
        j += 1;
    }
    None
}

/// The OSC 7501 reference parser — the RECEIVE side of the protocol,
/// implemented from the published spec's field rules (the stand-in for the
/// fleet shim's parser, which was not readable on this host — see the
/// header's deviation note). Rules that matter: `state` is the required key
/// and an invalid one DROPS the entire sequence; a malformed key=value pair
/// is skipped; unknown keys are ignored; `msg` is base64(UTF-8); the whole
/// sequence must terminate (BEL or ST) within 4096 bytes or it is dropped.
/// Returns the records in stream order.
fn parse_osc7501(bytes: &[u8]) -> Vec<Record7501> {
    const INTRO: &[u8] = b"\x1b]7501;";
    const MAX: usize = 4096;
    const STATES: [&str; 6] = ["idle", "working", "done", "blocked", "error", "clear"];
    let mut records = Vec::new();
    let mut i = 0usize;
    while let Some(off) = find(&bytes[i..], INTRO) {
        let at = i + off;
        let body_start = at + INTRO.len();
        let Some((body, next)) = find_terminator(bytes, body_start, MAX) else {
            break; // unterminated: drop the rest (a corrupt stream cannot loop us)
        };
        i = next;
        let mut current: Option<Record7501> = None;
        for pair in body.split(|&b| b == b':') {
            let Some(eq) = pair.iter().position(|&b| b == b'=') else {
                continue; // a malformed pair is skipped
            };
            let key = String::from_utf8_lossy(&pair[..eq]).to_string();
            let value = String::from_utf8_lossy(&pair[eq + 1..]).to_string();
            if key == "state" {
                if STATES.contains(&value.as_str()) {
                    current = Some(rec(&value, None));
                } else {
                    current = None; // an invalid state DROPS the entire sequence
                    break;
                }
                continue;
            }
            let Some(r) = current.as_mut() else { continue };
            match key.as_str() {
                "id" => r.id = Some(value),
                "kind" => r.kind = Some(value),
                "progress" => r.progress = Some(value),
                "app" => r.app = Some(value),
                "title" => r.title = Some(value),
                "msg" => r.msg = b64_decode(&value).and_then(|raw| String::from_utf8(raw).ok()),
                _ => {} // unknown keys are ignored
            }
        }
        if let Some(r) = current {
            records.push(r); // a sequence with no state at all is dropped
        }
    }
    records
}

fn esc_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == ESC).count()
}

fn contains_osc(bytes: &[u8]) -> bool {
    find(bytes, b"\x1b]7501;").is_some()
}

// ---------------------------------------------------------------------------
// The pty harness — the capture is the exact byte stream a terminal sees
// ---------------------------------------------------------------------------

/// Spawn `cmd` with its STDOUT on a fresh pty: the emitter's `[ -t 1 ]` sees
/// a real tty, and the master fd is the capture end. The parent holds only
/// the master; the child gets the slave on fd 1 (dup2 in `pre_exec`, the
/// async-signal-safe window between fork and exec).
fn spawn_on_pty(cmd: &mut Command) -> (Child, RawFd) {
    let mut master: RawFd = -1;
    let mut slave: RawFd = -1;
    let rc = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(rc, 0, "openpty failed: {}", std::io::Error::last_os_error());
    assert!(master >= 0 && slave >= 0, "openpty produced invalid fds");
    let child_slave = slave;
    // SAFETY: runs between fork and exec in the forked child; the only libc
    // calls here (dup2, close) are async-signal-safe, and the captured fd is
    // the one openpty handed this call.
    unsafe {
        cmd.pre_exec(move || {
            if libc::dup2(child_slave, libc::STDOUT_FILENO) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            if child_slave != libc::STDOUT_FILENO {
                libc::close(child_slave);
            }
            Ok(())
        });
    }
    let child = cmd.spawn().expect("spawn the fixture on the pty");
    // SAFETY: release the parent's slave handle so only the child (and its
    // children) hold the slave side; the master then reads EOF at their exit.
    unsafe { libc::close(slave) };
    (child, master)
}

/// Drain a pty master to EOF on a helper thread. The last slave close ends
/// the read with 0 (Linux) or EIO (macOS) — both break the drain.
fn drain_pty(master: RawFd) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut file = unsafe { File::from_raw_fd(master) };
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match file.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        let _ = tx.send(buf);
    });
    rx
}

/// Wait for a child with a hard cap; on a blowout kill it and return None.
/// The supervisor fixtures' real terminator is the stub-planted STOP file /
/// the T254 max-loops bound — this cap is the hang guard, never the path.
fn wait_bounded(child: &mut Child, cap: Duration) -> Option<std::process::ExitStatus> {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if start.elapsed() >= cap {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("try_wait failed: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// The probe fixtures (the `osc` subcommand — the real gated emitter, cheap)
// ---------------------------------------------------------------------------

/// A probe fixture root: the copied loopd.sh + its derived sourced fragments
/// (the loaders). The probe dispatches before anything else runs.
fn probe_fixture_root() -> TempDir {
    let keep = tempfile::tempdir().expect("probe fixture tempdir");
    common::copy_loopd_fixture(keep.path());
    keep
}

/// Run `loopd.sh osc <state> <reason>` with stdout PIPED (the redirected
/// context — nohup/launchd/CI shape) and return the output.
fn probe_piped(root: &Path, state: &str, reason: &str, envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("osc")
        .arg(state)
        .arg(reason)
        .current_dir(root)
        .env_remove("CHUG_OSC7501")
        .stdin(Stdio::null());
    common::pin_loopd_env_files(&mut cmd, root);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("run the piped osc probe")
}

/// Run `loopd.sh osc <state> <reason>` with stdout on a REAL pty and return
/// the captured byte stream.
fn probe_pty(root: &Path, state: &str, reason: &str, envs: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("osc")
        .arg(state)
        .arg(reason)
        .current_dir(root)
        .env_remove("CHUG_OSC7501")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    common::pin_loopd_env_files(&mut cmd, root);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let (mut child, master) = spawn_on_pty(&mut cmd);
    let rx = drain_pty(master);
    let done = wait_bounded(&mut child, Duration::from_secs(30));
    assert!(done.is_some(), "the osc probe hung past the 30s cap");
    rx.recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished")
}

/// (gating req) Piped stdout → ZERO escape bytes, whatever the state, the
/// reason, or the env knob says. The byte-count assert is the contract: a
/// log parser (or any redirected consumer) must never see one.
#[test]
fn piped_stdout_never_carries_an_escape_byte() {
    let root = probe_fixture_root();
    for (state, reason) in [
        ("working", ""),
        ("idle", "cycle ok"),
        ("blocked", "error: build failed (rc=2) — refusing to launch a stale binary"),
    ] {
        let out = probe_piped(root.path(), state, reason, &[]);
        assert!(
            out.status.success(),
            "the osc probe must exit 0 even when silent (rc={:?})",
            out.status.code()
        );
        assert_eq!(
            esc_count(&out.stdout),
            0,
            "piped stdout carried an escape byte for state={state} — the T354 \
             gating contract is broken (log parsers must never see one)"
        );
        assert!(
            out.stdout.is_empty(),
            "the gated probe must be fully byte-silent on a pipe, got {:?}",
            String::from_utf8_lossy(&out.stdout)
        );
    }
    // Even CHUG_OSC7501=1 cannot buy bytes back onto a pipe: the TTY leg is
    // conjunctive, the env knob is not an override.
    let out = probe_piped(root.path(), "idle", "cycle ok", &[("CHUG_OSC7501", "1")]);
    assert_eq!(esc_count(&out.stdout), 0, "CHUG_OSC7501=1 must not leak bytes onto a pipe");
    assert!(out.stdout.is_empty());
}

/// (gating req) CHUG_OSC7501=0 on a pty → zero bytes: the env kill outranks
/// the tty.
#[test]
fn chug_osc7501_0_kills_even_on_a_tty() {
    let root = probe_fixture_root();
    let capture = probe_pty(
        root.path(),
        "blocked",
        "error: build failed (rc=2) — refusing to launch a stale binary",
        &[("CHUG_OSC7501", "0")],
    );
    assert_eq!(
        esc_count(&capture),
        0,
        "CHUG_OSC7501=0 still emitted on a pty — the kill switch is broken"
    );
    assert!(capture.is_empty(), "the killed probe must be fully silent, got {capture:?}");
}

/// (gating req) TTY + unset → emitted, and the emission is EXACTLY the
/// id-less root working record (state only — no msg, no id, no app).
#[test]
fn tty_and_unset_emits_the_working_record() {
    let root = probe_fixture_root();
    let capture = probe_pty(root.path(), "working", "", &[]);
    let expected = "\x1b]7501;state=working\x1b\\";
    assert_eq!(
        capture,
        expected.as_bytes(),
        "the TTY+unset emission must be exactly the bare root working record"
    );
    let records = parse_osc7501(&capture);
    assert_eq!(records, vec![rec("working", None)]);
}

/// (gating req) TTY + CHUG_OSC7501=1 → emitted (the explicit enable leg).
#[test]
fn chug_osc7501_1_explicitly_enables_on_a_tty() {
    let root = probe_fixture_root();
    let capture = probe_pty(root.path(), "idle", "cycle ok", &[("CHUG_OSC7501", "1")]);
    let expected = format!(
        "\x1b]7501;state=idle:msg={}\x1b\\",
        b64_encode(b"cycle ok")
    );
    assert_eq!(capture, expected.into_bytes(), "the explicit-enable emission is wrong");
    assert_eq!(parse_osc7501(&capture), vec![rec("idle", Some("cycle ok"))]);
}

/// (emission req) The blocked msg channel carries the one-line reason
/// VERBATIM after the base64 round trip — the T340 vocabulary word and all.
#[test]
fn the_msg_channel_carries_the_blocked_reason() {
    let root = probe_fixture_root();
    let reason = "budget: cycle ended without goal complete (rc=1)";
    let capture = probe_pty(root.path(), "blocked", reason, &[]);
    let expected = format!(
        "\x1b]7501;state=blocked:msg={}\x1b\\",
        b64_encode(reason.as_bytes())
    );
    assert_eq!(capture, expected.into_bytes(), "the blocked emission is wrong");
    let records = parse_osc7501(&capture);
    assert_eq!(records, vec![rec("blocked", Some(reason))]);
    assert!(records[0].id.is_none(), "the root record must stay id-less");
}

/// (wire rule) msg is ONE human-readable line: a newline in the reason is
/// flattened to a space, and no raw newline ever rides the sequence.
#[test]
fn the_msg_channel_is_one_line() {
    let root = probe_fixture_root();
    let capture = probe_pty(root.path(), "working", "line one\nline two", &[]);
    assert!(
        !capture.contains(&b'\n'),
        "the emitted sequence itself must never contain a newline: {capture:?}"
    );
    let records = parse_osc7501(&capture);
    assert_eq!(records, vec![rec("working", Some("line one line two"))]);
}

/// (wire-compat req) Every emitted shape parses under the reference parser —
/// state valid, msg decodes to the exact reason, the record is the id-less
/// root, the terminator is ST, and the whole sequence sits inside the
/// protocol's 4096-byte cap.
#[test]
fn emitted_bytes_parse_under_the_spec_reference_parser() {
    let root = probe_fixture_root();
    let shapes: [(&str, &str); 3] = [
        ("working", ""),
        ("idle", "cycle ok"),
        ("blocked", "error: HALTED after 3 consecutive build failures"),
    ];
    for (state, reason) in shapes {
        let capture = probe_pty(root.path(), state, reason, &[]);
        // The terminator is ST (ESC \), never BEL, and the sequence respects
        // the protocol's 4096-byte total cap.
        assert!(capture.ends_with(b"\x1b\\"), "the sequence must terminate with ST: {capture:?}");
        assert!(
            !capture.contains(&BEL),
            "the emitter must not use the BEL terminator: {capture:?}"
        );
        assert!(capture.len() <= 4096, "the sequence exceeds the 4096-byte protocol cap");
        let records = parse_osc7501(&capture);
        assert_eq!(
            records,
            vec![rec(state, if reason.is_empty() { None } else { Some(reason) })],
            "the shim-parser stand-in must parse the native emission cleanly"
        );
        assert!(records[0].id.is_none(), "the record must be the id-less root");
    }
}

// ---------------------------------------------------------------------------
// The real-supervisor fixtures (the emission legs, end to end)
// ---------------------------------------------------------------------------

/// The stub process table: an EMPTY ps answer. The single-driver probe sees
/// no driver and proceeds; the orphan reaper scans nothing; the host's real
/// process table is never enumerated (the hermeticity contract).
const PS_STUB: &str = "#!/bin/sh\nexit 0\n";

/// A green build that installs the launchable stub binary (the
/// tests/loopd_stale_binary.rs pattern): the cycle completes with an
/// accepted verdict and the stub plants the STOP file, so the sandbox
/// supervisor runs probe → sweep → build → cycle → clean exit in seconds.
const GREEN_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
printf 'chug: goal complete\nsummary: the stub binary ran this cycle\n'
mkdir -p .chug && touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// A green build whose stub BINARY dies with the budget/abort exit (rc=1):
/// the supervisor's failure branch fires once with the T340 budget word, the
/// stub plants the STOP file, the loop exits cleanly — one full
/// working→blocked span to capture.
const FAILING_CHUG_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "stub chug: dying with the budget exit" >&2
exit 1
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// A red build (the gate itself): no binary is ever installed, no cycle ever
/// launches — the supervisor blocks ONCE and the planted STOP file bounds
/// the run at the next while check.
const RED_BUILD_CARGO: &str = r#"#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "stub cargo: build FAILED" >&2
exit 1
"#;

fn stub(path: &Path, body: &str) {
    fs::write(path, body).expect("write stub");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod stub");
}

fn copy_scripts_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).expect("read the repo scripts dir") {
        let entry = entry.expect("scripts entry");
        if entry.path().is_file() {
            fs::copy(entry.path(), to.join(entry.file_name()))
                .expect("copy the scripts entry");
        }
    }
}

/// The full-supervisor harness (the tests/loopd_stale_binary.rs /
/// tests/loopd_orphan_reaper.rs pattern): the REAL loopd.sh + scripts/, the
/// hermetic ps/lsof stubs first on PATH, the chosen cargo stub, HOME pinned
/// inside the sandbox, the sleep seams at 1s, and the T254 max-loops bound.
struct Sandbox {
    _keep: TempDir,
    root: PathBuf,
}

impl Sandbox {
    /// `cargo_stub` picks the arc: GREEN_CARGO (installs a stub chug that
    /// completes), FAILING_CHUG_CARGO (a stub chug that dies rc=1),
    /// RED_BUILD_CARGO (the build gate itself fails).
    fn new(cargo_stub: &str) -> Sandbox {
        let keep = tempfile::tempdir().expect("sandbox tempdir");
        let root = keep.path().to_path_buf();
        common::copy_loopd_fixture(&root);
        // loopd.sh also invokes scripts/ BY PATH (the reaper, the digest and
        // delta probes, site-sync) — copy the whole dir so every invoked
        // helper resolves inside the sandbox.
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts dir");
        copy_scripts_dir(Path::new("scripts"), &scripts);
        fs::create_dir_all(root.join("bin")).expect("bin dir");
        stub(&root.join("bin/ps"), PS_STUB);
        // The cwd leg's probe is lsof — stubbed beside ps so the helper's
        // `command -v lsof` resolves the hermetic stub, never the real one.
        stub(&root.join("bin/lsof"), PS_STUB);
        stub(&root.join("bin/cargo"), cargo_stub);
        // site-sync must NEVER leave the sandbox (a live site dir on this
        // host would get synced from fixture garbage): point the env default
        // at a path that does not exist (the warn + exit 0 leg).
        fs::write(root.join("NO-SUCH-SITE"), "").expect("site sentinel");
        Sandbox { _keep: keep, root }
    }

    /// Spawn the supervisor with stdout on a REAL pty (the emission channel)
    /// and hand back the child plus the capture receiver. `extra_envs` rides
    /// last (the dormant fixture's git identity).
    fn run_on_pty(&self, max_loops: u32, extra_envs: &[(&str, &str)]) -> (Child, mpsc::Receiver<Vec<u8>>) {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh")
            .arg("run")
            .current_dir(&self.root)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.root.join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("CHUG_SITE_DIR", self.root.join("NO-SUCH-SITE"))
            .env("CHUG_SITE_SYNC_NO_PUSH", "1")
            // HOME stays in the sandbox: no installed chug, no daemon socket,
            // and no global git config can reach the fixture.
            .env("HOME", &self.root)
            // The sleep seams (the tests/loopd_stale_binary.rs pattern).
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            // T254 — the fixture-leak fail-safe: the bound is the run's real
            // terminator when no stub plants a STOP file.
            .env("LOOPD_MAX_LOOPS", max_loops.to_string())
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        // Hygiene: the host's loop-relevant exports must never flip a fixture
        // (the T332 hermeticity contract); the loaders are pinned to absent
        // paths by the T346 guard below, these removals cover direct exports.
        for var in [
            "CHUG_OSC7501",
            "CHUG_AUTO_UPDATE",
            "CHUG_DAEMON_BIN",
            "CHUG_DAEMON_SOCK",
            "CHUG_DECISION_PROVIDER",
            "CHUG_DECISION_PROXY_URL",
            "CHUG_DECISION_TOKEN",
            "LOOP_ORCH_MODEL",
            "LOOP_ROUTINE_MODEL",
            "LOOP_IDEATION_MODEL",
            "DORMANT",
        ] {
            cmd.env_remove(var);
        }
        common::pin_loopd_env_files(&mut cmd, &self.root);
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let (child, master) = spawn_on_pty(&mut cmd);
        let rx = drain_pty(master);
        (child, rx)
    }

    /// Spawn the supervisor the redirected way — stdout PIPED (the nohup /
    /// launchd shape the compat contract is written for).
    fn run_piped(&self, max_loops: u32) -> Output {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh")
            .arg("run")
            .current_dir(&self.root)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.root.join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("CHUG_SITE_DIR", self.root.join("NO-SUCH-SITE"))
            .env("CHUG_SITE_SYNC_NO_PUSH", "1")
            .env("HOME", &self.root)
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_MAX_LOOPS", max_loops.to_string())
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        for var in [
            "CHUG_OSC7501",
            "CHUG_AUTO_UPDATE",
            "CHUG_DAEMON_BIN",
            "CHUG_DAEMON_SOCK",
            "CHUG_DECISION_PROVIDER",
            "CHUG_DECISION_PROXY_URL",
            "CHUG_DECISION_TOKEN",
            "LOOP_ORCH_MODEL",
            "LOOP_ROUTINE_MODEL",
            "LOOP_IDEATION_MODEL",
            "DORMANT",
        ] {
            cmd.env_remove(var);
        }
        common::pin_loopd_env_files(&mut cmd, &self.root);
        cmd.output().expect("run the piped supervisor fixture")
    }

    fn log_path(&self) -> PathBuf {
        self.root.join(".chug/loopd/loopd.log")
    }

    fn read_log(&self) -> String {
        fs::read_to_string(self.log_path()).unwrap_or_default()
    }

    /// Every file the supervisor redirected into (the supervisor log plus
    /// every cycle log) — the redirected contexts that must hold ZERO escape
    /// bytes.
    fn redirected_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.log_path()];
        let cycle_dir = self.root.join(".chug/loopd/cycles");
        if let Ok(entries) = fs::read_dir(&cycle_dir) {
            for entry in entries.flatten() {
                files.push(entry.path());
            }
        }
        files
    }
}

/// `git init` + a fresh EVALUATION.md commit INSIDE the sandbox — the
/// dormancy anchor rides the commit's date (now), so the weekly audit timer
/// reads NOT DUE and the dormant skip takes the suspended-valve arm. The
/// identity rides env (HOME is pinned away from any global git config).
fn init_git_with_evaluation_commit(root: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("HOME", root)
            .env("GIT_AUTHOR_NAME", "t354-fixture")
            .env("GIT_AUTHOR_EMAIL", "t354-fixture@fixture.invalid")
            .env("GIT_COMMITTER_NAME", "t354-fixture")
            .env("GIT_COMMITTER_EMAIL", "t354-fixture@fixture.invalid")
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init", "-q"]);
    fs::write(
        root.join("EVALUATION.md"),
        "# Eval\n\nT354 fixture seed — the dormancy anchor rides this commit's date.\n",
    )
    .expect("write EVALUATION.md");
    run(&["add", "EVALUATION.md"]);
    run(&["commit", "-q", "-m", "eval: fixture seed"]);
}

/// (emission req, green arc) A completing cycle emits working at the cycle
/// start and idle at the cycle OK — in that order — and the redirected log
/// files hold ZERO escape bytes.
#[test]
fn a_green_cycle_emits_working_then_idle() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the green sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the green sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the green arc must complete a cycle:\n{log}");

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: eval cycle on anthropic-system.ai.kimi-k3")),
            rec("idle", Some("cycle ok")),
        ],
        "the green arc must emit exactly working-then-idle, in order:\n{capture:?}"
    );
    // The working span precedes the rest state (the harness's status-dot
    // story depends on the ORDER, not just the set).
    let at_working = find(&capture, b"state=working").expect("working in capture");
    let at_idle = find(&capture, b"state=idle").expect("idle in capture");
    assert!(at_working < at_idle, "working must be emitted before idle");

    // THE COMPAT CONTRACT, on the real redirected files: log parsers must
    // never see an escape byte.
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte — the T354 \
             gating contract is broken on the real supervisor path",
            file.display()
        );
    }
}

/// (compat req, redirected arc) The SAME green arc with stdout PIPED — the
/// nohup/launchd shape — produces zero escape bytes on stdout and zero in
/// every redirected file. Non-harness operation is byte-identical.
#[test]
fn a_piped_supervisor_run_stays_byte_silent() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let out = sandbox.run_piped(4);
    assert!(out.status.success(), "the piped green sandbox must exit cleanly: {:?}", out.status);
    assert_eq!(
        esc_count(&out.stdout),
        0,
        "a piped supervisor run emitted an escape byte on stdout — the T354 \
         gating contract is broken in the nohup/launchd shape"
    );
    assert!(!contains_osc(&out.stdout));
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the piped arc must still complete its cycle:\n{log}");
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(esc_count(&bytes), 0, "{} carries an escape byte", file.display());
    }
}

/// (emission req, failure arc) A cycle that dies with the budget/abort exit
/// emits working then blocked, and the blocked msg channel carries the T340
/// budget word with the rc — the one source of truth for both surfaces.
#[test]
fn a_dying_cycle_emits_working_then_blocked_with_the_reason() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = Sandbox::new(FAILING_CHUG_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the failing-chug sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the failing-chug sandbox must still exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle ended WITHOUT goal complete"),
        "the failure arc must run its failure branch:\n{log}"
    );

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: eval cycle on anthropic-system.ai.kimi-k3")),
            rec("blocked", Some("budget: cycle ended without goal complete (rc=1)")),
        ],
        "the failure arc must emit working-then-blocked with the budget reason:\n{capture:?}"
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(esc_count(&bytes), 0, "{} carries an escape byte", file.display());
    }
}

/// (emission req, gate-red arc) A red build blocks WITHOUT any working span:
/// the launch is refused, so `working` must never be emitted on that
/// iteration — the watching terminal sees blocked and stays blocked.
#[test]
fn a_red_build_gate_emits_blocked_without_a_working_span() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = Sandbox::new(RED_BUILD_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the red-build sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the red-build sandbox must exit cleanly via STOP: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("build FAILED (rc=1)"),
        "the gate-red arc must hit the build-failure branch:\n{log}"
    );

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![rec("blocked", Some("error: build failed (rc=1) — refusing to launch a stale binary"))],
        "the gate-red arc must emit exactly one blocked record and never working:\n{capture:?}"
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(esc_count(&bytes), 0, "{} carries an escape byte", file.display());
    }
}

/// (emission req, dormant arc) The dormant cheap-exit emits idle naming the
/// dormancy, never working — the loop went quiet ON PURPOSE. The disposition
/// commit still lands (the git record the bookkeeping reads).
#[test]
fn the_dormant_skip_emits_idle_and_never_working() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = Sandbox::new(GREEN_CARGO);
    init_git_with_evaluation_commit(&sandbox.root);
    let identity = [
        ("GIT_AUTHOR_NAME", "t354-fixture"),
        ("GIT_AUTHOR_EMAIL", "t354-fixture@fixture.invalid"),
        ("GIT_COMMITTER_NAME", "t354-fixture"),
        ("GIT_COMMITTER_EMAIL", "t354-fixture@fixture.invalid"),
    ];
    // Two dormant iterations, then the T254 bound terminates the run.
    let (mut child, rx) = sandbox.run_on_pty(2, &identity);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the dormant sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the dormant sandbox must exit cleanly via the bound: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("dormant cheap-exit (T325)"),
        "the dormant arc must take the suspended-valve skip:\n{log}"
    );

    let records = parse_osc7501(&capture);
    assert!(
        !records.is_empty(),
        "the dormant arc must emit at least one record:\n{capture:?}"
    );
    for record in &records {
        assert_eq!(
            *record,
            rec("idle", Some("dormant: queue empty")),
            "every dormant emission is the idle record naming the dormancy:\n{capture:?}"
        );
    }

    // The disposition record still landed (the delivery the bookkeeping
    // reads) — the git log carries the cheap-exit subject.
    let subjects = Command::new("git")
        .args(["log", "--format=%s"])
        .current_dir(&sandbox.root)
        .env("HOME", &sandbox.root)
        .output()
        .expect("git log");
    let subjects = String::from_utf8_lossy(&subjects.stdout).to_string();
    assert!(
        subjects.contains("empty-delta disposition"),
        "the dormant skip must land its disposition commit:\n{subjects}"
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(esc_count(&bytes), 0, "{} carries an escape byte", file.display());
    }
}

// ---------------------------------------------------------------------------
// The wiring pins (static, source-text — the house pattern)
// ---------------------------------------------------------------------------

fn read(rel: &str) -> String {
    let root = std::env::current_dir().expect("cargo runs test binaries at the package root");
    fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

fn count_eq(haystack: &str, needle: &str, want: usize, what: &str) {
    let got = haystack.matches(needle).count();
    assert_eq!(got, want, "count of {needle:?} must be {want}: {what}");
}

fn need(haystack: &str, needle: &str, what: &str) -> usize {
    haystack
        .find(needle)
        .unwrap_or_else(|| panic!("{what}: the needle {needle:?} is missing"))
}

/// The body of the `loopd_osc7501_emit` function — everything from its
/// definition line to its closing brace at column 0.
fn emitter_body(loopd: &str) -> String {
    let start = need(loopd, "loopd_osc7501_emit() {", "the emitter is defined");
    let end = loopd[start..]
        .find("\n}")
        .map(|at| start + at)
        .unwrap_or_else(|| panic!("the emitter function is closed"));
    loopd[start..end].to_string()
}

#[test]
fn the_emitter_exists_once_with_the_gating_contract_inline() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "loopd_osc7501_emit() {", 1, "the emitter is defined exactly once");
    let body = emitter_body(&loopd);

    // The gate, in order: TTY first, then the env case, then the writer.
    let at_tty = need(&body, "[ -t 1 ] || return 0", "the TTY gate is the first leg");
    let at_env = need(
        &body,
        "case \"${CHUG_OSC7501-}\" in",
        "the env gate comes after the TTY leg",
    );
    let at_write = need(
        &body,
        "printf '\\033]7501;%s\\033\\\\'",
        "the wire writer comes after both gates",
    );
    assert!(
        at_tty < at_env && at_env < at_write,
        "the gate order is TTY → env → emit; anything else reorders the \
         contract (the TTY leg must be able to silence before the env is \
         even read)"
    );
    // The env case accepts ONLY unset/empty and exactly 1.
    count_eq(&body, "''|1) ;;", 1, "the emit arm accepts unset/empty and exactly 1");
    count_eq(&body, "*) return 0 ;;", 1, "every other value kills (default-safe)");

    // The writer is EPIPE-proof and the function can never trip set -e.
    count_eq(
        &body,
        "printf '\\033]7501;%s\\033\\\\' \"$seq\" || true",
        1,
        "the writer is guarded against a closed stdout",
    );
    assert!(
        body.trim_end().ends_with("return 0"),
        "the emitter must end in an unconditional return 0 (it is called as a \
         bare statement under set -euo pipefail in the run loop)"
    );
    // The one-line msg rule is enforced in the emitter, not left to callers.
    count_eq(&body, "msg=${msg//$'\\n'/ }", 1, "newlines are flattened before encoding");

    // THE COMPAT CONTRACT, statically: the emitter writes ONLY to stdout —
    // never to a file, never through a redirect. Log parsers must never see
    // an escape byte because the emitter is physically incapable of writing
    // one into a log.
    assert!(
        !body.contains(">>") && !body.contains("$LOG"),
        "the emitter body must not redirect anywhere — stdout is the only \
         channel and the gate owns the silence"
    );
    // The terminator is ST (ESC \) — never the BEL variant.
    count_eq(&body, "\\007", 0, "no octal BEL terminator in the emitter");
    count_eq(&body, "\\a", 0, "no BEL escape in the emitter");
}

#[test]
fn every_emission_leg_is_wired_to_its_transition() {
    let loopd = read("loopd.sh");

    // working — the cycle start (the supervisor's one working span) sits
    // after the cycle-start log line and BEFORE the chug launch.
    let at_log = need(
        &loopd,
        "cycle start -> $cycle_log ($mode cycle, orchestrator $orch_model)",
        "the cycle-start log line",
    );
    let at_emit = need(
        &loopd,
        "loopd_osc7501_emit working \"cycle start: $mode cycle on $orch_model\"",
        "the cycle-start working emission",
    );
    let at_launch = need(&loopd, "chug_out=\"$(CARGO_TARGET_DIR=", "the chug launch");
    assert!(
        at_log < at_emit && at_emit < at_launch,
        "the working emission must sit between the cycle-start log and the \
         launch (emitted when the span opens, never after the child runs)"
    );

    // working — the ideation (item) start, inside ideation_cycle.
    let at_ideation_log = need(
        &loopd,
        "ideation cycle (T326): timer due — launching $IDEATION_SPEC on $LOOP_IDEATION_MODEL (log $ilog)",
        "the ideation log line",
    );
    let at_ideation_emit = need(
        &loopd,
        "loopd_osc7501_emit working \"ideation cycle: proposing outward-facing features\"",
        "the ideation working emission",
    );
    assert!(
        at_ideation_log < at_ideation_emit,
        "the ideation working emission must follow its log line"
    );

    // idle — the cycle OK (= goal complete): after the verdict line, before
    // the post-cycle housekeeping (site-sync).
    let at_ok = need(&loopd, "cycle OK: $summary", "the cycle-OK log line");
    let at_ok_emit = need(&loopd, "loopd_osc7501_emit idle \"cycle ok\"", "the cycle-OK idle emission");
    let at_sync = need(&loopd, "scripts/site-sync.sh >> \"$LOG\"", "the site-sync call");
    assert!(
        at_ok < at_ok_emit && at_ok_emit < at_sync,
        "the idle emission must follow the verdict and precede post-cycle \
         housekeeping (the state flips the moment the goal is verified)"
    );

    // blocked — the cycle death, with the T340 vocabulary word mapped from
    // the child's exit code (rc 1 is the budget/abort exit per driver.rs).
    let at_death = need(
        &loopd,
        "cycle ended WITHOUT goal complete (consecutive failures: $fails)",
        "the cycle-death log line",
    );
    let at_word = need(&loopd, "1) osc_word=budget ;;", "the budget word mapping");
    let at_death_emit = need(
        &loopd,
        "loopd_osc7501_emit blocked \"$osc_word: cycle ended without goal complete (rc=$chug_rc)\"",
        "the cycle-death blocked emission",
    );
    assert!(
        at_death < at_word && at_word < at_death_emit,
        "the blocked emission must follow the death log with the vocabulary \
         word resolved in between"
    );

    // blocked — the build gate red, before any launch could follow.
    let at_build = need(
        &loopd,
        "build FAILED (rc=$build_rc) — refusing to launch:",
        "the build-failure log line",
    );
    let at_build_emit = need(
        &loopd,
        "loopd_osc7501_emit blocked \"error: build failed (rc=$build_rc) — refusing to launch a stale binary\"",
        "the build-gate blocked emission",
    );
    assert!(at_build < at_build_emit, "the gate-red emission follows its log line");

    // blocked — both 3-strike HALTs, as the LAST word before each exit 1.
    for (halt_log, halt_emit, what) in [
        (
            "HALTED after 3 consecutive build failures",
            "loopd_osc7501_emit blocked \"error: HALTED after 3 consecutive build failures\"",
            "the build HALT",
        ),
        (
            "HALTED after 3 consecutive failures",
            "loopd_osc7501_emit blocked \"error: HALTED after 3 consecutive cycles without goal complete\"",
            "the cycle HALT",
        ),
    ] {
        let at_log = need(&loopd, halt_log, &format!("{what} log line"));
        let at_emit = need(&loopd, halt_emit, &format!("{what} blocked emission"));
        let at_exit = loopd[at_log..]
            .find("exit 1")
            .map(|at| at_log + at)
            .unwrap_or_else(|| panic!("{what}: the exit 1 is missing"));
        assert!(
            at_emit < at_exit,
            "{what} must emit blocked BEFORE the exit — the last word the \
             terminal ever sees from a wedged supervisor is blocked"
        );
    }

    // idle — the held skips: dormant names the dormancy, the plain skip is
    // the loop at rest; both fire on the pacing path before the sleep.
    let at_pacing = need(
        &loopd,
        "cheap-exit pacing: sleeping ${skip_secs}s",
        "the pacing log line",
    );
    let at_dormant_emit = need(
        &loopd,
        "loopd_osc7501_emit idle \"dormant: queue empty\"",
        "the dormant idle emission",
    );
    let at_skip_emit = need(
        &loopd,
        "loopd_osc7501_emit idle \"cycle skipped: no evaluation needed now\"",
        "the plain-skip idle emission",
    );
    let at_sleep = need(&loopd, "sleep \"$skip_secs\"", "the pacing sleep");
    assert!(
        at_pacing < at_dormant_emit && at_dormant_emit < at_skip_emit && at_skip_emit < at_sleep,
        "the skip-arm emissions must sit between the pacing log and the sleep"
    );

    // blocked — the disposition commit (delivery) failure, before the
    // fall-through launch.
    let at_commit = need(
        &loopd,
        "cheap-exit disposition commit FAILED",
        "the disposition-failure log line",
    );
    let at_commit_emit = need(
        &loopd,
        "loopd_osc7501_emit blocked \"error: disposition commit failed — falling through to a real eval\"",
        "the disposition-failure blocked emission",
    );
    assert!(
        at_commit < at_commit_emit,
        "the delivery-failure emission follows its log line"
    );
}

#[test]
fn the_emission_counts_are_exact() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "loopd_osc7501_emit working", 2, "working: cycle start + ideation start");
    count_eq(
        &loopd,
        "loopd_osc7501_emit idle",
        3,
        "idle: cycle OK + dormant skip + plain skip",
    );
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked",
        5,
        "blocked: cycle death + build red + disposition failure + both HALTs",
    );
    // Total call sites: 2 working + 3 idle + 5 blocked + the probe arm = 11
    // (the definition line does not carry the trailing space).
    count_eq(
        &loopd,
        "loopd_osc7501_emit ",
        11,
        "every emission goes through the one gated emitter — no side door",
    );
}

#[test]
fn the_osc_probe_arm_and_usage_line_exist() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "  osc)", 1, "the probe subcommand arm exists");
    let at_arm = need(&loopd, "  osc)", "the osc arm");
    let at_call = need(
        &loopd,
        "loopd_osc7501_emit \"${2:-working}\" \"${3:-}\"",
        "the probe drives the REAL gated emitter",
    );
    let at_status = need(&loopd, "  status)", "the next arm");
    assert!(
        at_arm < at_call && at_call < at_status,
        "the osc arm dispatches to the emitter and exits before the run mode"
    );
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate] [osc]",
        1,
        "the usage line names the probe subcommand",
    );
    // COMPAT: the sibling files' pins own shorter prefixes of this line —
    // keep each of them matching exactly once so my append cannot break the
    // tests this file is not allowed to touch.
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate]",
        1,
        "the T259/T237-era usage needles stay intact (substring pins)",
    );
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok]",
        1,
        "the T237 needle stays intact",
    );
}

/// T159 membership pin (the reaper's lock-scope pin shape, one file later):
/// every test in THIS file that spawns a real supervisor (`Sandbox::new`)
/// takes the T151 timing guard as its FIRST acquisition, then the T172
/// cross-binary load lock, both before the sandbox spawn. A future
/// through-loopd fixture added without the join is RED by construction.
#[test]
fn the_real_supervisor_fixtures_join_the_t151_timing_domain() {
    let src = read("tests/loopd_osc7501.rs");
    assert!(
        src.contains("#[path = \"../src/testsupport.rs\"]\nmod testsupport;"),
        "the timing-domain join must be the #[path] include of \
         src/testsupport.rs — any other lock source is a second, independent \
         domain (the T151 finding)"
    );
    let timing_line = "let _timing = testsupport::timing_guard();";
    let t172_line = "let _t172_load = t172_load_lock::family_guard(\"loopd-osc7501\");";
    let mut guarded: Vec<&str> = Vec::new();
    for chunk in src.split("\n#[test]").skip(1) {
        let name = chunk
            .trim_start_matches('\n')
            .strip_prefix("fn ")
            .and_then(|rest| rest.split(['(', '<']).next())
            .unwrap_or("")
            .trim();
        assert!(!name.is_empty(), "a test chunk failed to yield its fn name");
        if name == "the_real_supervisor_fixtures_join_the_t151_timing_domain" {
            continue; // this pin's own chunk mentions the markers as text
        }
        if !chunk.contains("Sandbox::new(") {
            continue; // probe fixtures spawn no supervisor and take no guard
        }
        let at_timing = chunk.find(timing_line).unwrap_or_else(|| {
            panic!(
                "{name} spawns a real loopd but never takes the T151 timing \
                 guard — the through-loopd family must stay joined to the one \
                 shared serialization domain"
            )
        });
        let at_t172 = chunk.find(t172_line).unwrap_or_else(|| {
            panic!("{name} must take the T172 cross-binary load lock (nextest \
                    runs each test as its own PROCESS)")
        });
        let at_sandbox = chunk.find("Sandbox::new(").expect("chunk has the spawn");
        assert!(
            at_timing < at_t172 && at_t172 < at_sandbox,
            "{name} must take the T151 timing guard FIRST, then the T172 \
             cross-binary load lock, both before the sandbox spawn — the one \
             global lock order everywhere the two domains meet"
        );
        guarded.push(name);
    }
    assert_eq!(
        guarded.len(),
        5,
        "the through-loopd family is 5 fixtures (green, piped, dying, \
         gate-red, dormant) — a new real-supervisor fixture must join the \
         domain or this pin's count is lying about coverage"
    );
}

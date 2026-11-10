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
//!   `ESC ] 7501 ; state=<s>[:app=chug][:msg=<base64>] ESC \` — msg is one base64
//!   line, the whole sequence ≤4096 bytes, no `id`/`progress` keys (T360
//!   adds the constant `app=chug` label — herdr Agents-list grouping parity
//!   with the wrapper's bytes).
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
        // T360 — the app label rides EVERY record (app=chug, herdr
        // Agents-list grouping parity with the wrapper's bytes).
        app: Some("chug".to_string()),
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
// The pilot-supervisor probe fixtures (T358 — the script `chug loop run`
// actually spawns: same gate, same wire, same behavioral surface)
// ---------------------------------------------------------------------------

/// The pilot probe fixture root: the generated supervisor materialized
/// EXACTLY where `chug loop run` writes it (`<root>/.chug/loopd/loopd.sh` —
/// its ROOT is `$(dirname "$0")/../..`, so the layout is load-bearing),
/// with every sourced fragment derived from the copied script text (the
/// copy_loopd_fixture pattern — a hand-listed set goes stale).
fn copy_pilot_fixture(dir: &Path) {
    let repo_root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let state = dir.join(".chug/loopd");
    let scripts = state.join("scripts");
    fs::create_dir_all(&scripts).expect("pilot fixture scripts dir");
    fs::write(
        state.join("loopd.sh"),
        fs::read_to_string(repo_root.join("src/loop_cmd/loopd-pilot.sh"))
            .expect("the pilot template readable"),
    )
    .expect("write the pilot fixture");
    let script_text = fs::read_to_string(state.join("loopd.sh")).expect("reread the copy");
    for line in script_text.lines() {
        let Some(rest) = line.trim().strip_prefix(". \"$(dirname \"$0\")/") else {
            continue;
        };
        let Some(name) = rest.strip_suffix('"') else {
            continue;
        };
        let src = repo_root.join(name);
        assert!(
            src.is_file(),
            "the pilot template sources {name} but it is missing from the \
             checkout — the pilot fixture cannot be built (add the fragment \
             to the repo or fix the sourcing line)"
        );
        let base = name.rsplit('/').next().expect("fragment has a basename");
        fs::copy(&src, scripts.join(base))
            .unwrap_or_else(|e| panic!("copy {name} into the pilot fixture dir: {e}"));
    }
    assert!(
        scripts.read_dir().expect("pilot fixture scripts dir").count() >= 3,
        "the derived pilot fragment set is implausibly small — the template \
         should source at least the two loaders and the shared OSC 7501 \
         emitter; check the sourcing lines"
    );
}

/// A piped pilot `osc` probe: the exit code and the BYTES on stdout —
/// a redirected context, so the compat contract says zero escape bytes.
fn pilot_probe_piped(root: &Path, state: &str, reason: &str, envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join(".chug/loopd/loopd.sh"))
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
    cmd.output().expect("run the piped pilot osc probe")
}

/// A pty pilot `osc` probe — the captured stdout bytes.
fn pilot_probe_pty(root: &Path, state: &str, reason: &str, envs: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join(".chug/loopd/loopd.sh"))
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
    let (mut child, master) = spawn_on_pty(&mut cmd);
    let rx = drain_pty(master);
    let _ = wait_bounded(&mut child, Duration::from_secs(10));
    rx.recv_timeout(Duration::from_secs(5))
        .expect("the pty drain thread finished")
}

/// (gating req, pilot) A piped pilot probe — any state, any reason —
/// receives ZERO escape bytes.
#[test]
fn pilot_piped_stdout_never_carries_an_escape_byte() {
    let root = tempfile::tempdir().expect("pilot probe fixture tempdir");
    copy_pilot_fixture(root.path());
    for (state, reason) in [
        ("working", ""),
        ("working", "cycle start: 1 open row(s)"),
        ("idle", "cycle ok"),
        ("idle", "dormant: queue empty"),
        ("blocked", "budget: iteration budget exceeded (40/40)"),
    ] {
        let out = pilot_probe_piped(root.path(), state, reason, &[]);
        assert!(
            out.status.success(),
            "the pilot osc probe must always exit 0 ({state}): {:?}",
            out.status
        );
        assert_eq!(
            esc_count(&out.stdout),
            0,
            "a piped pilot probe ({state}) emitted an escape byte"
        );
        assert!(
            !contains_osc(&out.stdout),
            "a piped pilot probe ({state}) emitted an OSC sequence"
        );
    }
}

/// (gating req, pilot) CHUG_OSC7501=0 silences the pilot on a real pty.
#[test]
fn pilot_chug_osc7501_0_kills_even_on_a_tty() {
    let root = tempfile::tempdir().expect("pilot probe fixture tempdir");
    copy_pilot_fixture(root.path());
    let out = pilot_probe_pty(
        root.path(),
        "working",
        "cycle start: 1 open row(s)",
        &[("CHUG_OSC7501", "0")],
    );
    assert!(
        out.is_empty(),
        "CHUG_OSC7501=0 must silence the pilot on a pty, got {out:?}"
    );
}

/// (gating req, pilot) A pty + unset CHUG_OSC7501 emits the working record.
#[test]
fn pilot_tty_and_unset_emits_the_working_record() {
    let root = tempfile::tempdir().expect("pilot probe fixture tempdir");
    copy_pilot_fixture(root.path());
    let capture = pilot_probe_pty(root.path(), "working", "cycle start: 1 open row(s)", &[]);
    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![rec("working", Some("cycle start: 1 open row(s)"))],
        "the pty pilot probe must emit the working record: {capture:?}"
    );
    assert_eq!(records[0].state, "working");
    assert!(records[0].id.is_none(), "the id-less root record");
}

/// (parity req — the drift guard, behavioral half) The SAME canned
/// transition driven through BOTH supervisors' `osc` probes on real ptys
/// produces byte-identical 7501 output: ONE function body
/// (scripts/osc7501.sh, sourced by both) produces the bytes, and this
/// fixture fails the moment either supervisor grows its own wire.
#[test]
fn the_same_transition_emits_byte_identical_sequences_through_both_supervisors() {
    let loopd_root = probe_fixture_root();
    let pilot_root = tempfile::tempdir().expect("pilot probe fixture tempdir");
    copy_pilot_fixture(pilot_root.path());
    for (state, reason) in [
        ("working", ""),
        ("working", "cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3"),
        ("idle", "cycle ok"),
        ("idle", "dormant: queue empty"),
        ("blocked", "budget: iteration budget exceeded (40/40)"),
        ("blocked", "error: HALTED after 3 consecutive cycles without goal complete"),
    ] {
        let from_loopd = probe_pty(loopd_root.path(), state, reason, &[]);
        let from_pilot = pilot_probe_pty(pilot_root.path(), state, reason, &[]);
        assert_eq!(
            from_loopd, from_pilot,
            "the two supervisors' 7501 bytes diverged for state={state} \
             reason={reason:?} — the shared-emitter parity (T358) is broken"
        );
        let records = parse_osc7501(&from_pilot);
        assert_eq!(
            records,
            vec![rec(state, if reason.is_empty() { None } else { Some(reason) })],
            "the identical bytes must also parse to the expected record"
        );
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
/// id-less root working record (state + the T360 app label — no msg, no id).
#[test]
fn tty_and_unset_emits_the_working_record() {
    let root = probe_fixture_root();
    let capture = probe_pty(root.path(), "working", "", &[]);
    let expected = "\x1b]7501;state=working:app=chug\x1b\\";
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
        "\x1b]7501;state=idle:app=chug:msg={}\x1b\\",
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
        "\x1b]7501;state=blocked:app=chug:msg={}\x1b\\",
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
// The T360 blocked leg (the 2535e8af field finding) + the app= label — the
// cycle-log scan, driven through the `osc` probe (the cheap pattern)
// ---------------------------------------------------------------------------

/// The FP acceptance's dead-langfuse marker, in kind verbatim: the delivery
/// failure line a completing cycle leaves in its cycle log (the 2535e8af
/// field finding — failure markers live in cycle-*.log, NEVER in the
/// supervisor stream). Under the scan's 200-char cap.
const DELIVERY_MARKER: &str =
    "observability delivery failing (run continues): langfuse: dial tcp 127.0.0.1:3000: connect: connection refused";

/// Write a cycle log into a probe fixture root's `.chug/loopd/` — the
/// derivation surface the field finding names. The probe's own reason
/// argument stays the clean `cycle ok`, so any blocked+msg the fixtures
/// below see can ONLY have come from the log.
fn write_cycle_log(root: &Path, name: &str, body: &str) {
    let state = root.join(".chug/loopd");
    fs::create_dir_all(&state).expect("the loopd state dir");
    fs::write(state.join(name), body).expect("write the cycle log");
}

/// (T360 — the cycle-log fixture) A failure marker existing ONLY in the
/// newest cycle-*.log turns the idle transition into `blocked` with the
/// marker line as the msg. RED against a supervisor-stream-only derivation
/// by construction: the probe's reason argument is the clean `cycle ok`,
/// so no supervisor-stream line carries the marker — the acceptance FP's
/// RED leg (cycle completes "cycle OK", goes dormant, native emission says
/// idle while the log carries the delivery failure verbatim).
#[test]
fn a_cycle_log_marker_alone_blocks_the_idle_transition() {
    let root = probe_fixture_root();
    write_cycle_log(
        root.path(),
        "cycle-20261110-120000.log",
        &format!(
            "[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n\
             {DELIVERY_MARKER}\n\
             [loopd 2026-11-10T12:05:00Z] verdict: goal complete (rc=0)\n"
        ),
    );
    let capture = probe_pty(root.path(), "idle", "cycle ok", &[]);
    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![rec("blocked", Some(DELIVERY_MARKER))],
        "the marker lives ONLY in the cycle log, so only a cycle-log-scanning \
         derivation can put it on the msg channel — the idle transition must \
         surface blocked, not idle:\n{capture:?}"
    );
}

/// (T360 — the clear fixture) A fresh, clean cycle start clears the block
/// naturally: the scan always reads the NEWEST cycle-*.log, and a new cycle
/// creates a new (clean) log — the docs/osc-7501.md contract.
#[test]
fn a_fresh_clean_cycle_log_clears_the_block() {
    let root = probe_fixture_root();
    write_cycle_log(
        root.path(),
        "cycle-20261110-120000.log",
        &format!(
            "[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n\
             {DELIVERY_MARKER}\n\
             [loopd 2026-11-10T12:05:00Z] verdict: goal complete (rc=0)\n"
        ),
    );
    let blocked = probe_pty(root.path(), "idle", "cycle ok", &[]);
    assert_eq!(
        parse_osc7501(&blocked),
        vec![rec("blocked", Some(DELIVERY_MARKER))],
        "precondition: the poisoned log blocks the idle transition:\n{blocked:?}"
    );
    // The next cycle start created a NEWER, clean log (the poisoned one is
    // the previous cycle's forensic record). The scan reads the newest and
    // finds nothing — the idle transition is idle again.
    write_cycle_log(
        root.path(),
        "cycle-20261110-130000.log",
        "[loopd 2026-11-10T13:00:00Z] T81 routing: eval cycle\n\
         [loopd 2026-11-10T13:05:00Z] verdict: goal complete (rc=0)\n",
    );
    let capture = probe_pty(root.path(), "idle", "cycle ok", &[]);
    assert_eq!(
        parse_osc7501(&capture),
        vec![rec("idle", Some("cycle ok"))],
        "a fresh clean cycle start clears the block — the newest log is \
         clean, so the idle transition stays idle:\n{capture:?}"
    );
}

/// (T360 — the parity fixture) The same poisoned-log probe through BOTH
/// supervisors' `osc` probes on real ptys emits byte-identical blocked
/// bytes — the scan is a SHARED helper in scripts/osc7501.sh (the T358
/// doctrine: one body, both supervisors, parity by construction).
#[test]
fn the_cycle_log_blocked_leg_is_byte_identical_through_both_supervisors() {
    let loopd_root = probe_fixture_root();
    let pilot_root = tempfile::tempdir().expect("pilot probe fixture tempdir");
    copy_pilot_fixture(pilot_root.path());
    let poisoned = format!(
        "[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n\
         {DELIVERY_MARKER}\n\
         [loopd 2026-11-10T12:05:00Z] verdict: goal complete (rc=0)\n"
    );
    write_cycle_log(loopd_root.path(), "cycle-20261110-120000.log", &poisoned);
    write_cycle_log(pilot_root.path(), "cycle-20261110-120000.log", &poisoned);
    let from_loopd = probe_pty(loopd_root.path(), "idle", "cycle ok", &[]);
    let from_pilot = pilot_probe_pty(pilot_root.path(), "idle", "cycle ok", &[]);
    assert_eq!(
        from_loopd, from_pilot,
        "the two supervisors' blocked bytes diverged on the same cycle-log \
         content — the T360 scan must be ONE shared body (T358 parity)"
    );
    assert_eq!(
        parse_osc7501(&from_pilot),
        vec![rec("blocked", Some(DELIVERY_MARKER))],
        "the shared bytes must carry the marker on the msg channel:\n{from_pilot:?}"
    );
}

/// (T360 — the app= fixture) The native bytes carry `app=chug` on EVERY
/// state (herdr Agents-list grouping parity with the wrapper's bytes —
/// the operator finding: herdr showed `app: None` for the native dot).
#[test]
fn every_native_record_carries_the_app_chug_label() {
    let root = probe_fixture_root();
    for (state, reason) in [
        ("working", ""),
        ("idle", "cycle ok"),
        ("done", "goal complete"),
        ("blocked", "error: HALTED after 3 consecutive build failures"),
    ] {
        let capture = probe_pty(root.path(), state, reason, &[]);
        let records = parse_osc7501(&capture);
        assert_eq!(records.len(), 1, "one record for state={state}: {capture:?}");
        assert_eq!(
            records[0].app.as_deref(),
            Some("chug"),
            "state={state}: the native emission must carry app=chug on the \
             wire (Agents-list grouping parity with the wrapper), got {capture:?}"
        );
    }
}

/// (T360 — the pattern fixture) The scan's four failure patterns, exactly
/// as specified: `observability delivery failing` and `gate red` as
/// substrings, `error` and `panic` as WHOLE words (bounded regex — the
/// inflections `errors`/`panicked` do not match, so a clean cycle's
/// summary lines can never trip the block). Plus the 200-char msg bound.
#[test]
fn the_cycle_log_scan_matches_the_four_failure_patterns() {
    let root = probe_fixture_root();
    let log = root.path().join(".chug/loopd/cycle-20261110-120000.log");
    fs::create_dir_all(log.parent().unwrap()).expect("the loopd state dir");
    // Positive: each pattern ALONE drives the block, its line riding the
    // msg channel verbatim.
    let positives: [(&str, &str); 4] = [
        (
            "observability delivery failing (run continues): langfuse: connection refused",
            "the delivery phrase (substring)",
        ),
        ("gate red: specs/t360 check failed", "the gate-red phrase (substring)"),
        ("wrap failed with an error: gate rc=1", "error as a whole word"),
        ("panic: the driver unwind reached the sink", "panic as a whole word"),
    ];
    for (line, what) in positives {
        fs::write(
            &log,
            format!("[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n{line}\n"),
        )
        .expect("write the pattern log");
        let capture = probe_pty(root.path(), "idle", "cycle ok", &[]);
        assert_eq!(
            parse_osc7501(&capture),
            vec![rec("blocked", Some(line))],
            "{what} must drive the blocked emission with the marker line as \
             the msg:\n{capture:?}"
        );
    }
    // Negative: the whole-word rule's look-alikes — a clean cycle's
    // summaries must never trip the block (the fixture-noise guard).
    for (line, what) in [
        ("errors: 0 warnings: 0 — the wrap finished clean", "the plural `errors`"),
        ("thread 'main' panicked at driver.rs:921", "the inflection `panicked`"),
    ] {
        fs::write(
            &log,
            format!("[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n{line}\n"),
        )
        .expect("write the look-alike log");
        let capture = probe_pty(root.path(), "idle", "cycle ok", &[]);
        assert_eq!(
            parse_osc7501(&capture),
            vec![rec("idle", Some("cycle ok"))],
            "{what} is not a whole-word match — the idle transition must \
             stay idle:\n{capture:?}"
        );
    }
    // The 200-char bound: an over-cap marker line rides the msg channel
    // bounded (head intact, tail gone) — ONE short line on the wire.
    let long_line = format!("error: {}", "x".repeat(220));
    fs::write(
        &log,
        format!("[loopd 2026-11-10T12:00:00Z] T81 routing: eval cycle\n{long_line}\n"),
    )
    .expect("write the over-cap log");
    let capture = probe_pty(root.path(), "idle", "cycle ok", &[]);
    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![rec("blocked", Some(&format!("error: {}", "x".repeat(193))))],
        "the marker must ride the msg channel capped at 200 chars \
         (`error: ` + 193 x's):\n{capture:?}"
    );
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
        // The cycle logs live DIRECTLY in .chug/loopd/ (loopd.sh writes
        // "$STATE/cycle-<ts>.log", STATE=.chug/loopd) — and they are exactly
        // where the T142 launch redirects the child's stderr, so they are
        // the redirected contexts the compat contract is about. (A "cycles"
        // subdir would never exist — the 2026-11-10 fixture gap.)
        let state_dir = self.root.join(".chug/loopd");
        if let Ok(entries) = fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cycle-") && name.ends_with(".log") {
                    files.push(entry.path());
                }
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
/// This sandbox's stub dies WITHOUT the driver's failure marker (no
/// `chug: abort:` line reaches the cycle log), so the msg here is the
/// CANNED rc line — the guaranteed no-marker fallback of the T356 round-3
/// derivation (the marker-derived shapes live in tests/loopd_heartbeat.rs's
/// dying and plant-only fixtures).
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
    // The no-marker shape, deliberate: this stub wrote no `chug: abort:`
    // line, so the derivation finds nothing and the canned rc line stands.
    let cycle_log = fs::read_dir(sandbox.root.join(".chug/loopd"))
        .expect("the loopd state dir")
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("cycle-") && n.ends_with(".log"))
                .unwrap_or(false)
        })
        .map(|p| fs::read_to_string(p).unwrap_or_default())
        .unwrap_or_default();
    assert!(
        !cycle_log.contains("chug: abort:"),
        "this arc's cycle log must carry no driver marker (the canned \
         fallback's precondition): {cycle_log:?}"
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
// The pilot supervisor, end to end (T358 — the script `chug loop run`
// actually spawns, through its full run loop on a pty)
// ---------------------------------------------------------------------------

/// The fixture queue: one open row — the supervisor's empty-queue gate would
/// otherwise go dormant without ever launching a cycle.
const PILOT_QUEUE: &str = "# t358 fixture queue

| id | title | spec | pri | status | notes |
|----|-------|------|-----|--------|-------|
| T1 | fixture work | specs/t001-fixture-work.md | 1 | todo | t358 fixture |
";

/// The pilot's green-cycle stub: completes (the pilot's verdict is the rc,
/// the T142 shape) and plants the STOP file, so the sandbox supervisor runs
/// probe → gate → launch → cycle OK → clean exit in seconds. The pilot never
/// builds, so — unlike the loopd.sh sandboxes — there is no cargo stub at
/// all: the cycle launches CHUG_LOOP_BIN, which this stub IS.
const PILOT_GREEN_CHUG: &str = r#"#!/bin/sh
printf 'chug: goal complete\nsummary: the stub binary ran this cycle\n'
mkdir -p .chug && touch .chug/STOP-LOOP
exit 0
"#;

/// The pilot's dying stub: exits rc=1 (the budget/abort exit) AFTER writing
/// the driver's real death marker to STDERR — exactly the stream the pilot
/// redirects into the per-cycle log. The marker exists ONLY in the cycle
/// log: the supervisor's own log gets the canned rc line, never the marker
/// (the field finding's precondition, asserted in the fixture).
const PILOT_DYING_CHUG: &str = r#"#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "stub chug: dying with the driver marker" >&2
echo "chug: abort: iteration budget exceeded (40/40)" >&2
exit 1
"#;

/// The pilot's markerless-death stub: dies rc=1 WITHOUT any marker (the
/// SIGKILL shape — a death with no dying words). The blocked msg must then
/// come from the newest OTHER cycle log that carries one (the planted stale
/// log), suffixed with its source log.
const PILOT_MARKERLESS_CHUG: &str = r#"#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "stub chug: dying without a marker" >&2
exit 1
"#;

/// The pilot's forged-order stub: a model-echoed look-alike lands EARLY in
/// the cycle log (a mid-run line on the same stderr stream the pilot
/// redirects) and the driver's REAL death-time marker lands LAST — the T142
/// anti-forgery shape (the sink writes the death marker after every model
/// line, so a look-alike can never outlive it). The blocked msg must derive
/// from the LAST marker, never the early one.
const PILOT_FORGED_ORDER_CHUG: &str = r#"#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "model narrative: turning to the next item" >&2
echo "chug: abort: forged look-alike" >&2
echo "model narrative: still mid-run after the forgery" >&2
echo "chug: abort: the real death reason" >&2
exit 1
"#;

/// The pilot's panicking stub: dies rc=101 (a Rust unwind's exit code) with
/// the `chug: panicked:` unwind line as the ONLY failure marker — no abort
/// line anywhere (the thread-panic line above it matches neither pattern).
/// The blocked msg must carry the panic's reason (the pattern is otherwise
/// pinned only by static presence in the source text).
const PILOT_PANICKED_CHUG: &str = r#"#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "thread 'main' panicked at driver: goal not accepted" >&2
echo "chug: panicked: the loop driver panicked: assertion failed: goal accepted" >&2
exit 101
"#;

/// The long death reason — past the 120-char cap the derivation applies to
/// keep the msg channel one line. ONE source for the stub's marker and the
/// fixture's expectation (the two cannot drift).
const PILOT_LONG_DEATH_REASON: &str = "upstream 429: rate limited until 2026-11-10T12:00:00Z; request id req-9f8c1e2a-77d4-4b3e-9a51-cc0b1d2e3f4a; retry with backoff or rotate the key";

/// The stub carrying that long marker: dies rc=1 (the budget/abort exit)
/// after writing the over-cap marker to stderr.
fn pilot_long_marker_stub() -> String {
    format!(
        "#!/bin/sh\nmkdir -p .chug && touch .chug/STOP-LOOP\necho \"chug: abort: {}\" >&2\nexit 1\n",
        PILOT_LONG_DEATH_REASON
    )
}

/// The full pilot-supervisor harness: the generated supervisor materialized
/// where `chug loop run` writes it (copy_pilot_fixture), the hermetic ps
/// stub first on PATH, HOME pinned inside the sandbox, the sleep seams at
/// 1s, and the stub chug as CHUG_LOOP_BIN (the production launch shape).
/// No cargo stub: the pilot never builds. Termination is the stub's planted
/// STOP file — `wait_bounded` is the hang fail-safe (the T6 rule: a wedged
/// child fails the test in seconds, it can never hang the suite).
struct PilotSandbox {
    _keep: TempDir,
    root: PathBuf,
}

impl PilotSandbox {
    fn new() -> PilotSandbox {
        let keep = tempfile::tempdir().expect("pilot sandbox tempdir");
        let root = keep.path().to_path_buf();
        copy_pilot_fixture(&root);
        fs::write(root.join("TODO.md"), PILOT_QUEUE).expect("write the fixture queue");
        fs::write(
            root.join("LOOP-SPEC.md"),
            "# t358 fixture contract\ncheck: true\n",
        )
        .expect("write the fixture contract");
        fs::create_dir_all(root.join("bin")).expect("bin dir");
        stub(&root.join("bin/ps"), PS_STUB);
        PilotSandbox { _keep: keep, root }
    }

    fn install_chug_stub(&self, body: &str) {
        stub(&self.root.join("bin/chug"), body);
    }

    /// The base environment for both run shapes: the production launch env
    /// (CHUG_LOOP_BIN is what `chug loop run` exports), the hermeticity
    /// removals, the T346 loader pins, the 1s sleep seams.
    fn base_cmd(&self) -> Command {
        let mut cmd = Command::new("bash");
        cmd.arg(self.root.join(".chug/loopd/loopd.sh"))
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
            .env("CHUG_LOOP_BIN", self.root.join("bin/chug"))
            .env("HOME", &self.root)
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_EMPTY_SLEEP", "1")
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
            "CHUG_LOOPD_BIN",
            "LOOP_ORCH_MODEL",
            "LOOP_ROUTINE_MODEL",
            "LOOP_IDEATION_MODEL",
            "DORMANT",
        ] {
            cmd.env_remove(var);
        }
        common::pin_loopd_env_files(&mut cmd, &self.root);
        cmd
    }

    /// Spawn the pilot supervisor with stdout on a REAL pty.
    fn run_on_pty(&self) -> (Child, mpsc::Receiver<Vec<u8>>) {
        let (child, master) = spawn_on_pty(&mut self.base_cmd());
        let rx = drain_pty(master);
        (child, rx)
    }

    /// Spawn the pilot supervisor the redirected way — stdout PIPED.
    fn run_piped(&self) -> Output {
        self.base_cmd().output().expect("run the piped pilot fixture")
    }

    fn log_path(&self) -> PathBuf {
        self.root.join(".chug/loopd/loopd.log")
    }

    fn read_log(&self) -> String {
        fs::read_to_string(self.log_path()).unwrap_or_default()
    }

    /// Every file the pilot redirected into (the supervisor log plus every
    /// cycle log) — the redirected contexts that must hold ZERO escape
    /// bytes.
    fn redirected_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.log_path()];
        let state_dir = self.root.join(".chug/loopd");
        if let Ok(entries) = fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cycle-") && name.ends_with(".log") {
                    files.push(entry.path());
                }
            }
        }
        files
    }

    /// The cycle logs only — the derivation's surface (the field finding:
    /// the failure markers land here, never in the supervisor stream).
    /// Sorted, so name order is the chronological order the names encode.
    fn cycle_logs(&self) -> Vec<PathBuf> {
        let state_dir = self.root.join(".chug/loopd");
        let mut logs = Vec::new();
        if let Ok(entries) = fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("cycle-") && name.ends_with(".log") {
                    logs.push(entry.path());
                }
            }
        }
        logs.sort();
        logs
    }
}

/// (emission req, pilot green arc) A completing pilot cycle emits working at
/// the cycle start and idle at the cycle OK — in that order — and every
/// redirected file holds ZERO escape bytes.
#[test]
fn a_pilot_green_cycle_emits_working_then_idle() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_GREEN_CHUG);
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the green pilot sandbox hung past the 90s cap");
    assert!(status.success(), "the green pilot sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK"), "the green pilot arc must complete a cycle:\n{log}");

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec("idle", Some("cycle ok")),
        ],
        "the green pilot arc must emit exactly working-then-idle, in order:\n{capture:?}"
    );
    let at_working = find(&capture, b"state=working").expect("working in the capture");
    let at_idle = find(&capture, b"state=idle").expect("idle in the capture");
    assert!(at_working < at_idle, "working must precede idle");

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
    }
}

/// (compat req, pilot) A piped pilot supervisor run — the nohup / launchd
/// shape a bare `chug loop run` takes under a daemon manager — stays byte
/// silent on stdout AND across every redirected file, while the cycle
/// itself completes.
#[test]
fn a_piped_pilot_supervisor_run_stays_byte_silent() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_GREEN_CHUG);
    let out = sandbox.run_piped();
    assert!(
        out.status.success(),
        "the piped pilot run must exit cleanly: {:?}",
        out.status
    );
    assert_eq!(esc_count(&out.stdout), 0, "piped stdout carries an escape byte");
    assert!(!contains_osc(&out.stdout), "piped stdout carries an OSC sequence");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK"), "the piped pilot arc must still complete a cycle:\n{log}");
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
    }
}

/// (field finding — RED-proof) A cycle death whose ONLY failure marker is in
/// the per-cycle log still produces blocked+msg — and the msg content is
/// UNREACHABLE from the supervisor stream: loopd.log carries no marker
/// (asserted below), so a derivation that read only loopd.log could only
/// ever have emitted the canned rc line, never this msg. RED against a
/// supervisor-stream-only derivation by construction.
#[test]
fn a_pilot_dying_cycle_derives_the_blocked_msg_from_the_cycle_log_only() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_DYING_CHUG);
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the dying pilot sandbox hung past the 90s cap");
    assert!(
        status.success(),
        "one failed cycle must not halt the pilot (3 to halt): {status:?}"
    );
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");

    // THE PRECONDITION: the marker exists, and ONLY in the cycle log.
    let logs = sandbox
        .redirected_files()
        .into_iter()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("cycle-"))
        })
        .collect::<Vec<_>>();
    assert_eq!(logs.len(), 1, "exactly one cycle log: {logs:?}");
    let cycle_text = fs::read_to_string(&logs[0]).unwrap_or_default();
    assert!(
        cycle_text.contains("chug: abort: iteration budget exceeded (40/40)"),
        "the stub must have written the driver marker into the cycle log:\n{cycle_text:?}"
    );
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle ended WITHOUT goal complete"),
        "the failure branch must have run:\n{log}"
    );
    assert!(
        !log.contains("iteration budget exceeded"),
        "THE FIELD FINDING, asserted: the failure marker does NOT reach the \
         supervisor stream — a supervisor-stream-only derivation could never \
         produce this fixture's expected msg:\n{log}"
    );

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec("blocked", Some("budget: iteration budget exceeded (40/40)")),
        ],
        "the death must emit working-then-blocked, the msg derived from the \
         CYCLE log and headed by its T340 blocked-reason word (rc=1 → \
         budget):\n{capture:?}"
    );
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
    }
}

/// (fallback walk) A death with NO marker in its own log (the SIGKILL
/// shape) falls back to the newest OTHER cycle log that carries one — the
/// loop's last RECORDED failure reason, suffixed with its source log.
#[test]
fn a_pilot_death_without_a_marker_falls_back_to_the_newest_marked_cycle_log() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_MARKERLESS_CHUG);
    // Plant a PREDECESSOR cycle log carrying the loop's last RECORDED
    // failure reason (mtime earlier than the death's own log — written now,
    // before the run).
    fs::write(
        sandbox.root.join(".chug/loopd/cycle-19700101-000000.log"),
        "[loopd old] verdict: no goal complete (rc=2)\nchug: abort: token budget exceeded\n",
    )
    .expect("plant the stale cycle log");
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the markerless pilot sandbox hung past the 90s cap");
    assert!(status.success(), "one failed cycle must not halt: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");
    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec(
                "blocked",
                Some("budget: token budget exceeded [cycle-19700101-000000.log]")
            ),
        ],
        "the markerless death must derive from the newest OTHER marked cycle \
         log, suffixed with its source:\n{capture:?}"
    );
}

/// (M-D killing fixture — the ordering pin) The derivation is
/// LAST-marker-wins: the sink writes the death marker after every model line
/// (the T142 anti-forgery property), so an EARLY model-echoed look-alike
/// must never win the msg channel over the driver's LATER death-time marker.
/// Validator mutant M-D flipped the awk to first-wins and the whole target
/// stayed green (43/43) — this is the behavioral pin it lacked: RED against
/// the first-wins flip, where the emitted msg becomes the forged reason.
#[test]
fn a_pilot_blocked_msg_takes_the_last_death_marker_over_an_early_forged_look_alike() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_FORGED_ORDER_CHUG);
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the forged-order pilot sandbox hung past the 90s cap");
    assert!(
        status.success(),
        "one failed cycle must not halt the pilot (3 to halt): {status:?}"
    );
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");

    // THE PRECONDITION: both markers sit in the ONE cycle log, the forged
    // look-alike EARLY and the real death marker LAST in file order — the
    // order the line-by-line derivation reads.
    let logs = sandbox.cycle_logs();
    assert_eq!(logs.len(), 1, "exactly one cycle log: {logs:?}");
    let cycle_text = fs::read_to_string(&logs[0]).unwrap_or_default();
    let at_forged = cycle_text
        .find("chug: abort: forged look-alike")
        .expect("the early forged look-alike is in the cycle log");
    let at_real = cycle_text
        .find("chug: abort: the real death reason")
        .expect("the real death marker is in the cycle log");
    assert!(
        at_forged < at_real,
        "the forged look-alike must PRECEDE the real death marker in file \
         order — this fixture pins the LATER-marker-wins ordering:\n{cycle_text:?}"
    );

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec("blocked", Some("budget: the real death reason")),
        ],
        "the blocked msg must derive from the LAST marker in the cycle log — \
         the death-time sink line, never the early model-echoed look-alike \
         (the T142 anti-forgery property on the 7501 msg channel):\n{capture:?}"
    );
    // The anti-forgery half, stated on its own so a first-wins mutant fails
    // HERE with the property named.
    let blocked = records
        .iter()
        .find(|r| r.state == "blocked")
        .expect("the death emitted a blocked record");
    let msg = blocked.msg.as_deref().unwrap_or_default();
    assert!(
        !msg.contains("forged"),
        "THE ANTI-FORGERY PROPERTY: an early model-echoed look-alike must \
         never win the msg channel over the driver's death-time marker: {msg:?}"
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
    }
}

/// (the dead-pattern killing fixture) `chug: panicked: ` is otherwise pinned
/// only by static presence — the source-text pin asserts the needle exists
/// in the awk, never that a panic line DRIVES anything. A cycle log whose
/// ONLY failure marker is the panic unwind line must still produce
/// blocked+msg carrying that reason. RED against the pattern drop (the awk's
/// panicked leg deleted): no marker matches, the derivation falls to the
/// canned rc line, and the msg loses the reason.
#[test]
fn a_pilot_panicked_marker_alone_carries_the_panic_reason_on_the_msg_channel() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(PILOT_PANICKED_CHUG);
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the panicking pilot sandbox hung past the 90s cap");
    assert!(
        status.success(),
        "one failed cycle must not halt the pilot (3 to halt): {status:?}"
    );
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");

    // THE PRECONDITION: the panic unwind line is the death log's ONLY
    // failure marker — no abort line anywhere, so a wrong-pattern
    // derivation has nothing else to grab.
    let logs = sandbox.cycle_logs();
    assert_eq!(logs.len(), 1, "exactly one cycle log: {logs:?}");
    let cycle_text = fs::read_to_string(&logs[0]).unwrap_or_default();
    assert!(
        cycle_text.contains("chug: panicked: the loop driver panicked: assertion failed: goal accepted"),
        "the stub must have written the panic unwind marker into the cycle log:\n{cycle_text:?}"
    );
    assert!(
        !cycle_text.contains("chug: abort: "),
        "the precondition: NO abort marker — the panicked line is the only one:\n{cycle_text:?}"
    );

    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec(
                "blocked",
                Some("error: the loop driver panicked: assertion failed: goal accepted")
            ),
        ],
        "the panicked marker alone must drive the msg channel (rc=101 → the \
         error word), the panic's reason carried verbatim:\n{capture:?}"
    );
}

/// (the cap fixture) The derivation caps the reason at 120 chars — a
/// provider error string can be long and the msg channel stays one line —
/// unpinned until now (no fixture drove an over-cap marker). RED against
/// the cap lift (substr's 120 dropped): the msg carries the FULL reason and
/// the equality below fails on the tail.
#[test]
fn a_pilot_death_reason_over_120_chars_is_capped_on_the_msg_channel() {
    // The fixture's own precondition: an over-cap ASCII reason (so [..120]
    // is a char boundary), ONE source shared by the stub and the
    // expectation.
    assert!(
        PILOT_LONG_DEATH_REASON.len() > 120 && PILOT_LONG_DEATH_REASON.is_ascii(),
        "the fixture's reason must be an over-cap ASCII line (len {})",
        PILOT_LONG_DEATH_REASON.len()
    );
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-osc7501");
    let sandbox = PilotSandbox::new();
    sandbox.install_chug_stub(&pilot_long_marker_stub());
    let (mut child, rx) = sandbox.run_on_pty();
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the over-cap pilot sandbox hung past the 90s cap");
    assert!(
        status.success(),
        "one failed cycle must not halt the pilot (3 to halt): {status:?}"
    );
    let capture = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the pty drain thread finished");

    // THE PRECONDITION: the FULL marker is in the cycle log — any loss is
    // the derivation's cap, never the log's.
    let logs = sandbox.cycle_logs();
    assert_eq!(logs.len(), 1, "exactly one cycle log: {logs:?}");
    let cycle_text = fs::read_to_string(&logs[0]).unwrap_or_default();
    let full_marker = format!("chug: abort: {PILOT_LONG_DEATH_REASON}");
    assert!(
        cycle_text.contains(&full_marker),
        "the stub must have written the FULL over-cap marker into the cycle log:\n{cycle_text:?}"
    );

    let expected = format!("budget: {}", &PILOT_LONG_DEATH_REASON[..120]);
    let records = parse_osc7501(&capture);
    assert_eq!(
        records,
        vec![
            rec("working", Some("cycle start: 1 open row(s) on anthropic-system.ai.kimi-k3")),
            rec("blocked", Some(&expected)),
        ],
        "the over-cap reason must ride the msg channel truncated to 120 chars \
         (the derivation's cap): head intact, tail gone, nothing more:\n{capture:?}"
    );
    // The cap half, stated on its own so a lifted cap fails HERE with the
    // property named.
    let blocked = records
        .iter()
        .find(|r| r.state == "blocked")
        .expect("the death emitted a blocked record");
    let msg = blocked.msg.as_deref().unwrap_or_default();
    assert!(
        !msg.contains(&PILOT_LONG_DEATH_REASON[120..]),
        "THE CAP: nothing past 120 chars of the reason may survive the msg \
         channel (msg len {}): {msg:?}",
        msg.len()
    );
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

/// The body of a shell function — from its definition line to its closing
/// brace at column 0 (the fn_body shape the heartbeat pins use; a local copy
/// because each test binary compiles its own test module).
fn fn_body(src: &str, name: &str) -> String {
    let sig = format!("{name}() {{");
    let start = need(src, &sig, &format!("{name} is defined"));
    let end = src[start..]
        .find("\n}")
        .map(|at| start + at)
        .unwrap_or_else(|| panic!("{name} is closed"));
    src[start..end].to_string()
}

/// The body of the SHARED emitter (`osc7501_emit` in scripts/osc7501.sh) —
/// the ONE function body that produces the 7501 bytes in both supervisors.
fn shared_emitter_body(shared: &str) -> String {
    let start = need(shared, "osc7501_emit() {", "the shared emitter is defined");
    let end = shared[start..]
        .find("\n}")
        .map(|at| start + at)
        .unwrap_or_else(|| panic!("the shared emitter function is closed"));
    shared[start..end].to_string()
}

/// (T358 — the drift guard, static half) The gating contract and the wire
/// live in the SHARED emitter exactly once: both supervisors source this
/// file (loopd.sh relative to its own path; the pilot via the copy
/// `chug loop run` materializes beside it), and NEITHER re-defines the
/// bytes — the composition wrapper in loopd.sh delegates, the pilot's
/// emission name delegates dot-only. A second copy anywhere is the drift
/// this pin exists to kill.
#[test]
fn both_supervisors_source_the_shared_emitter_and_never_redefine_it() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        ". \"$(dirname \"$0\")/scripts/osc7501.sh\"",
        1,
        "loopd.sh sources the shared emitter (the derived-fragment convention)",
    );
    count_eq(
        &loopd,
        "\nosc7501_emit() {",
        0,
        "loopd.sh never re-defines the shared emitter",
    );
    count_eq(&loopd, "loopd_osc7501_emit() {", 1, "the composition wrapper is defined exactly once");
    count_eq(
        &loopd,
        "osc7501_emit \"$state\" \"$msg\"",
        1,
        "the wrapper delegates the dot to the shared emitter",
    );

    let pilot = read("src/loop_cmd/loopd-pilot.sh");
    count_eq(
        &pilot,
        ". \"$(dirname \"$0\")/scripts/osc7501.sh\"",
        1,
        "the pilot sources the shared emitter (materialized beside it)",
    );
    count_eq(
        &pilot,
        "\nosc7501_emit() {",
        0,
        "the pilot never re-defines the shared emitter",
    );
    count_eq(
        &pilot,
        "loopd_osc7501_emit() {",
        1,
        "the pilot's one emission name is defined exactly once",
    );
    count_eq(
        &pilot,
        "osc7501_emit \"$state\" \"$msg\"",
        1,
        "the pilot's name delegates the (scan-rewritten) transition dot-only \
         to the shared emitter (the T356 heartbeat line surface stays \
         loopd-only; the T360 scan rewrites state/msg before the dot)",
    );
}

/// The T354 gating contract, still enforced — now at its single enforcement
/// point (the shared emitter both supervisors source).
#[test]
fn the_shared_emitter_carries_the_gating_contract_inline() {
    let shared = read("scripts/osc7501.sh");
    count_eq(&shared, "osc7501_emit() {", 1, "the shared emitter is defined exactly once");
    let body = shared_emitter_body(&shared);

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
         bare statement under set -euo pipefail in both supervisors' run \
         loops)"
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
    // The wire is the ID-LESS root record: the sequence opens on the state
    // key and carries the T360 constant app=chug label on EVERY record (herdr
    // Agents-list grouping parity with the wrapper) — no id/progress key.
    count_eq(
        &body,
        "seq=\"state=$state:app=chug\"",
        1,
        "the record opens on the state key and carries the app=chug label",
    );
    count_eq(&body, "id=", 0, "the emitter never assembles an id key (the id-less root)");
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
    // T356 fix-up round 3: the reason is DERIVED from the cycle log's driver
    // marker and composed once into $fail_msg — this sandbox's stub dies
    // WITHOUT a marker (no `chug: abort:` line anywhere), so the msg here is
    // the canned rc line, the guaranteed no-marker fallback (the
    // loopd_heartbeat.rs fixtures carry the marker-derived shapes).
    let at_death = need(
        &loopd,
        "cycle ended WITHOUT goal complete (consecutive failures: $fails)",
        "the cycle-death log line",
    );
    let at_word = need(&loopd, "1) osc_word=budget ;;", "the budget word mapping");
    let at_death_emit = need(
        &loopd,
        "loopd_osc7501_emit blocked \"$fail_msg\"",
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

/// (T358 wiring pin) The pilot template's state map, in the run loop's real
/// order — and the blocked msg's derivation reads the CYCLE LOG (the field
/// finding: failure markers never reach the supervisor stream), never the
/// supervisor log.
#[test]
fn the_pilot_template_carries_the_shared_emitter_and_its_state_map() {
    let pilot = read("src/loop_cmd/loopd-pilot.sh");

    // The shared emitter is sourced BEFORE the emission name that delegates
    // to it (a wrapper calling an unsourced function would be a startup
    // death under set -e).
    let at_source = need(&pilot, ". \"$(dirname \"$0\")/scripts/osc7501.sh\"", "the source line");
    let at_wrapper = need(&pilot, "loopd_osc7501_emit() {", "the emission name");
    assert!(
        at_source < at_wrapper,
        "the shared emitter must be sourced before the pilot's emission name"
    );

    // working — the cycle launch is the one working span.
    let at_start_log = need(
        &pilot,
        "cycle start -> $cycle_log ($rows open row(s), orchestrator $CHUG_LOOP_MODEL)",
        "the cycle-start log line",
    );
    let at_working = need(
        &pilot,
        "loopd_osc7501_emit working \"cycle start: $rows open row(s) on $CHUG_LOOP_MODEL\"",
        "the cycle-start working emission",
    );
    let at_launch = need(&pilot, "chug_out=\"$(CARGO_TARGET_DIR=", "the cycle launch");
    assert!(
        at_start_log < at_working && at_working < at_launch,
        "the working emission follows its log line and precedes the launch"
    );

    // idle — the dormant queue, beside the empty-queue gate's log line.
    let at_dormant_log = need(
        &pilot,
        "queue empty (0 open rows) — dormant, re-probing in ${SLEEP_EMPTY}s",
        "the dormant log line",
    );
    let at_dormant_emit = need(
        &pilot,
        "loopd_osc7501_emit idle \"dormant: queue empty\"",
        "the dormant idle emission",
    );
    assert!(
        at_dormant_log < at_dormant_emit,
        "the dormant emission follows its log line"
    );

    // idle — the cycle OK, before the between-cycles sleep.
    let at_ok_log = need(&pilot, "echo \"$(ts) cycle OK\" >> \"$LOG\"", "the cycle-OK log line");
    let at_ok_emit = need(&pilot, "loopd_osc7501_emit idle \"cycle ok\"", "the cycle-OK emission");
    assert!(at_ok_log < at_ok_emit, "the cycle-OK emission follows its log line");

    // blocked — the cycle death: the T340 word (rc 1 = budget), then the
    // CYCLE-LOG derivation, then ONE composition into $fail_msg.
    let at_death = need(
        &pilot,
        "cycle ended WITHOUT goal complete (consecutive failures: $fails)",
        "the death log line",
    );
    let at_word = need(&pilot, "1) osc_word=budget ;;", "the T340 word mapping");
    let at_derive = need(
        &pilot,
        "fail_reason=$(pilot_cycle_fail_reason \"$cycle_log\") || fail_reason=\"\"",
        "the cycle-log derivation",
    );
    let at_compose = need(
        &pilot,
        "fail_msg=\"$osc_word: cycle ended without goal complete (rc=$chug_rc)\"",
        "the canned fallback composition",
    );
    let at_derive_emit = need(
        &pilot,
        "loopd_osc7501_emit blocked \"$fail_msg\"",
        "the death emission",
    );
    assert!(
        at_death < at_word && at_word < at_derive && at_derive < at_compose
            && at_compose < at_derive_emit,
        "the blocked emission must follow the death log with the word, the \
         cycle-log derivation, and the one composition in between"
    );

    // blocked — the HALT, as the LAST word before the exit 1.
    let at_halt_log = need(
        &pilot,
        "HALTED after 3 consecutive failures",
        "the HALT log line",
    );
    let at_halt_emit = need(
        &pilot,
        "loopd_osc7501_emit blocked \"error: HALTED after 3 consecutive cycles without goal complete\"",
        "the HALT emission",
    );
    let at_halt_exit = pilot[at_halt_emit..]
        .find("exit 1")
        .map(|at| at_halt_emit + at)
        .unwrap_or_else(|| panic!("the exit after the HALT"));
    assert!(
        at_halt_log < at_halt_emit && at_halt_emit < at_halt_exit,
        "the HALT emission is the supervisor's last word before the exit"
    );

    // THE FIELD FINDING, statically: the derivation's legs read CYCLE logs.
    let derive = fn_body(&pilot, "pilot_cycle_fail_reason");
    assert!(
        derive.contains("\"$STATE\"/cycle-*.log"),
        "the fallback walk must scan the cycle logs"
    );
    assert!(
        !derive.contains("$LOG") && !derive.contains("loopd.log"),
        "the derivation must never read the supervisor stream (loopd.log) — \
         that is the silent-never-fire shape the field finding killed"
    );
    let fail_line = fn_body(&pilot, "pilot_log_fail_line");
    assert!(
        fail_line.contains("\"chug: abort: \"") && fail_line.contains("\"chug: panicked: \""),
        "the failure patterns are the driver's real death markers (the abort \
         line and the panic unwind line), both machine-written into the \
         cycle log"
    );

    // The probe arm: the T354 pattern, so the fixtures drive the real gate.
    count_eq(&pilot, "  osc)", 1, "the probe subcommand arm exists");
    let at_arm = need(&pilot, "  osc)", "the osc arm");
    let at_probe_call = need(
        &pilot,
        "loopd_osc7501_emit \"${2:-working}\" \"${3:-}\"",
        "the probe's emission call",
    );
    let at_probe_exit = pilot[at_probe_call..]
        .find("exit 0")
        .map(|at| at_probe_call + at)
        .unwrap_or_else(|| panic!("the probe's clean exit"));
    assert!(
        at_arm < at_probe_call && at_probe_call < at_probe_exit,
        "the probe arm emits through the real gated emitter and exits clean"
    );
    // The chug-binary resolution sits in the RUN path: the probe (and
    // stop/status) must work on a host without the binary.
    let at_case = need(&pilot, "case \"${1:-run}\" in", "the mode dispatch");
    let at_bin = need(&pilot, "CHUG_BIN=\"${CHUG_LOOP_BIN:-}\"", "the chug resolution");
    assert!(
        at_case < at_bin,
        "the chug-binary resolution must come after the mode dispatch — the \
         probe arm works on a host without the binary"
    );
}

/// (T358 materializer pin) The shared emitter rides beside the generated
/// pilot exactly like the loader fragments — embedded from the LIVE
/// checkout file via include_str!, written by write_supervisor_assets, so
/// a release carrying this item carries the bytes the parity fixture pins.
#[test]
fn the_materializer_writes_the_shared_emitter_beside_the_pilot() {
    let src = read("src/loop_cmd.rs");
    count_eq(
        &src,
        "include_str!(\"../scripts/osc7501.sh\")",
        1,
        "the emitter is embedded from the live checkout (no copy to drift)",
    );
    count_eq(
        &src,
        "(\"osc7501.sh\", OSC7501_EMITTER)",
        1,
        "write_supervisor_assets writes it into .chug/loopd/scripts/",
    );
    count_eq(
        &src,
        "const OSC7501_EMITTER",
        1,
        "the emitter constant is declared exactly once",
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

    // T358 — the pilot's state map, the same counts discipline: the
    // transitions a `chug loop run` spawn can present.
    let pilot = read("src/loop_cmd/loopd-pilot.sh");
    count_eq(
        &pilot,
        "loopd_osc7501_emit working",
        1,
        "working: the cycle launch (the pilot's one working span)",
    );
    count_eq(
        &pilot,
        "loopd_osc7501_emit idle",
        2,
        "idle: the dormant queue + the cycle OK",
    );
    count_eq(
        &pilot,
        "loopd_osc7501_emit blocked",
        2,
        "blocked: the cycle death + the HALT",
    );
    count_eq(
        &pilot,
        "loopd_osc7501_emit ",
        6,
        "total call sites: 1 working + 2 idle + 2 blocked + the probe arm",
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
/// every test in THIS file that spawns a real supervisor (`Sandbox::new` —
/// the loopd.sh family AND the T358 `PilotSandbox` family, whose name
/// carries the same needle) takes the T151 timing guard as its FIRST
/// acquisition, then the T172 cross-binary load lock, both before the
/// sandbox spawn. A future through-loopd or through-pilot fixture added
/// without the join is RED by construction.
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
        12,
        "the real-supervisor family is 12 fixtures (loopd.sh: green, piped, \
         dying, gate-red, dormant; pilot: green, piped, dying, markerless, \
         forged-order, panicked, over-cap) — a new real-supervisor fixture \
         must join the domain or this pin's count is lying about coverage"
    );
}

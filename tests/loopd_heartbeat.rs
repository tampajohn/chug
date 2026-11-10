//! T356 — loopd emits the terminal heartbeat: compact live status lines on
//! stdout (the human sibling of the T354 status dot).
//!
//! Operator report 2026-11-09 (watching the herdr-spawned loop): "in the
//! chug log / terminal view, i don't see any real info about what's going
//! on." Mechanism confirmed: loopd.sh writes everything to
//! `.chug/loopd/loopd.log` and prints nothing to stdout — a terminal watcher
//! sees a silent process. T354 gave herdr the status DOT (OSC 7501); T356
//! gives the human the status LINE:
//!
//! * **Lines** (appended, never carriage-return redraws — herdr keeps
//!   scrollback): a cycle-start line (cycle number, routing, the routed
//!   model's short name, the queue's top open row), iteration milestones
//!   (one per LOOPD_HB_EVERY=25 iterations, read live from the cycle's own
//!   `.chug/events.jsonl`, with the last tool name), merge-flip lines (one
//!   per observed merge commit — the queue's serial delivery), the dormant
//!   line (the next probe's pacing), and `BLOCKED: <reason>` lines whose
//!   reason text is the T354 msg channel text BYTE-IDENTICALLY (one source
//!   string, two destinations — composed inside the ONE dual-surface
//!   emitter, so the dot and the line can never disagree).
//! * **Gating (the same contract as T354, own knob)**: emit ONLY when stdout
//!   is a TTY AND `CHUG_LOOP_VERBOSE` is unset or `=1`; every redirected
//!   context (nohup, launchd, pipes, log files) receives ZERO lines;
//!   `CHUG_LOOP_VERBOSE=0` kills even on a TTY. The knobs are independent:
//!   `CHUG_OSC7501=0` silences the dot, never the line, and vice versa.
//! * **Volume**: a 200-iteration cycle emits ≤ 15 lines (8 milestones at
//!   the 25-iteration cadence + the cycle-start and cycle-done lines, with
//!   merge flips bounded by LOOP-SPEC's serial one-merge-per-row delivery).
//!
//! Fixture doctrine (the T354 file's, verbatim where it applies): the pty
//! is REAL (`libc::openpty` + `pre_exec` dup2 — the gates' `[ -t 1 ]` see
//! actual ttys and the capture is the exact byte stream the terminal would
//! see). The probe legs run through the `osc` probe (the routing/sleep-ok/
//! predicate pattern) so the REAL dual-surface emitter is exercised, never a
//! reimplementation. The transition legs run the REAL supervisor end to end
//! in a sandbox (the tests/loopd_osc7501.rs pattern: hermetic ps/lsof/cargo
//! stubs, the LOOPD_SLEEP_* seams, the new LOOPD_HB_POLL seam, the T254
//! max-loops bound as the terminator) and assert against the captured pty
//! stream AND the redirected log files — which must hold ZERO heartbeat
//! lines and ZERO escape bytes, because log parsers must never see either.

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

// ---------------------------------------------------------------------------
// The 7501 receive side (the T354 reference parser, trimmed to what the
// consistency legs need: the state and the decoded msg of every record)
// ---------------------------------------------------------------------------

/// The reference decoder (standard alphabet, `=` padding tolerated) — the
/// parser's msg-channel half. Independent of the emitter's platform `base64`
/// on purpose: a broken `base64` must fail the fixture, not the assertion.
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

#[derive(Debug, PartialEq, Eq, Clone)]
struct Record7501 {
    state: String,
    msg: Option<String>,
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
        if bytes[j] == 0x07 {
            return Some((&bytes[from..j], j + 1));
        }
        if bytes[j] == ESC && j + 1 < end && bytes[j + 1] == b'\\' {
            return Some((&bytes[from..j], j + 2));
        }
        j += 1;
    }
    None
}

/// The OSC 7501 reference parser (the T354 parser's receive half): `state`
/// is the required key, `msg` is base64(UTF-8), the sequence must terminate
/// within 4096 bytes. Returns the records in stream order.
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
                    current = Some(Record7501 { state: value, msg: None });
                } else {
                    current = None; // an invalid state DROPS the entire sequence
                    break;
                }
                continue;
            }
            let Some(r) = current.as_mut() else { continue };
            if key == "msg" {
                r.msg = b64_decode(&value).and_then(|raw| String::from_utf8(raw).ok());
            }
        }
        if let Some(r) = current {
            records.push(r); // a sequence with no state at all is dropped
        }
    }
    records
}

// ---------------------------------------------------------------------------
// The heartbeat receive side: the appended `[loop ...]` lines
// ---------------------------------------------------------------------------

/// The heartbeat lines in a captured pty stream, in order. The 7501
/// sequences carry NO newline (the wire rule), so they can sit mid-line in
/// the raw capture — strip every sequence FIRST, then split; a tty's ONLCR
/// turns each appended `\n` into `\r\n`, so the trailing `\r` goes too.
/// The result is exactly what a terminal's scrollback shows as lines.
fn heartbeat_lines(capture: &[u8]) -> Vec<String> {
    let mut scrubbed = Vec::with_capacity(capture.len());
    let mut i = 0usize;
    while i < capture.len() {
        if capture[i] == ESC
            && find(&capture[i..], b"\x1b]7501;") == Some(0)
            && find_terminator(capture, i + 7, 4096).is_some()
        {
            i = find_terminator(capture, i + 7, 4096).map(|(_, next)| next).expect("checked above");
            continue;
        }
        scrubbed.push(capture[i]);
        i += 1;
    }
    String::from_utf8_lossy(&scrubbed)
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_string())
        .filter(|l| l.starts_with("[loop"))
        .collect()
}

fn esc_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == ESC).count()
}

// ---------------------------------------------------------------------------
// The pty harness — the capture is the exact byte stream a terminal sees
// ---------------------------------------------------------------------------

/// Spawn `cmd` with its STDOUT on a fresh pty: the emitters' `[ -t 1 ]` see
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
/// the read with 0 (Linux) or EIO (macOS) — both break the drain. The
/// supervisor SIGKILL-backstops its watcher before it continues, so EOF
/// lands within ~1s of the child's exit; a longer hold would be a leak the
/// recv timeout names loudly rather than a capture silently truncated.
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
// The probe fixtures (the `osc` probe — the real dual-surface emitter, cheap)
// ---------------------------------------------------------------------------

/// A probe fixture root: the copied loopd.sh + its derived sourced fragments
/// (the loaders). The probe dispatches before anything else runs.
fn probe_fixture_root() -> TempDir {
    let keep = tempfile::tempdir().expect("probe fixture tempdir");
    common::copy_loopd_fixture(keep.path());
    keep
}

/// Run `loopd.sh osc <state> <reason> [hb-detail]` with stdout PIPED (the
/// redirected context — nohup/launchd/CI shape) and return the output.
fn probe_piped(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("osc")
        .args(args)
        .current_dir(root)
        .env_remove("CHUG_LOOP_VERBOSE")
        .env_remove("CHUG_OSC7501")
        .stdin(Stdio::null());
    common::pin_loopd_env_files(&mut cmd, root);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("run the piped osc probe")
}

/// Run `loopd.sh osc <state> <reason> [hb-detail]` with stdout on a REAL pty
/// and return the captured byte stream.
fn probe_pty(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Vec<u8> {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("osc")
        .args(args)
        .current_dir(root)
        .env_remove("CHUG_LOOP_VERBOSE")
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
    rx.recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished")
}

/// (gating req) Piped stdout → ZERO lines, whatever the state, the reason,
/// the detail, or the env knob says. The byte-count assert is the contract:
/// a log parser (or any redirected consumer) must never see a heartbeat
/// line — and never an escape byte either (the T354 contract rides along).
#[test]
fn piped_stdout_never_carries_a_line() {
    let root = probe_fixture_root();
    for args in [
        vec!["blocked", "budget: cycle ended without goal complete (rc=1)"],
        vec!["working", "cycle start: eval cycle on X", "cycle 7 routing=eval (kimi-k3)"],
        vec!["idle", "dormant: queue empty", "queue empty — dormant, next probe 300s"],
    ] {
        let out = probe_piped(root.path(), &args, &[]);
        assert!(
            out.status.success(),
            "the probe must exit 0 even when silent (rc={:?})",
            out.status.code()
        );
        assert!(
            out.stdout.is_empty(),
            "piped stdout carried bytes for args={args:?} — the T356 gating \
             contract is broken (log parsers must never see a heartbeat line): {:?}",
            String::from_utf8_lossy(&out.stdout)
        );
        assert_eq!(esc_count(&out.stdout), 0, "piped stdout carried an escape byte");
    }
    // Even CHUG_LOOP_VERBOSE=1 cannot buy a line back onto a pipe: the TTY
    // leg is conjunctive, the env knob is not an override.
    let out = probe_piped(
        root.path(),
        &["working", "cycle start", "detail"],
        &[("CHUG_LOOP_VERBOSE", "1")],
    );
    assert!(out.stdout.is_empty(), "CHUG_LOOP_VERBOSE=1 must not leak lines onto a pipe");
}

/// (gating req) CHUG_LOOP_VERBOSE=0 on a pty → zero heartbeat lines: the env
/// kill outranks the tty. The DOT still rides (the knobs are independent) —
/// its presence here proves the line's kill is the line's own knob, not the
/// dot's.
#[test]
fn chug_loop_verbose_0_kills_even_on_a_tty() {
    let root = probe_fixture_root();
    let capture = probe_pty(
        root.path(),
        &["working", "cycle start: eval cycle on X", "cycle 7 routing=eval (kimi-k3)"],
        &[("CHUG_LOOP_VERBOSE", "0")],
    );
    assert!(
        heartbeat_lines(&capture).is_empty(),
        "CHUG_LOOP_VERBOSE=0 still emitted a heartbeat line on a pty — the \
         kill switch is broken: {capture:?}"
    );
    let records = parse_osc7501(&capture);
    assert_eq!(records.len(), 1, "the dot must be untouched by the line's knob");
    assert_eq!(records[0].state, "working");
}

/// (gating req) TTY + CHUG_LOOP_VERBOSE=1 → the line is emitted (the
/// explicit enable leg). The line is the detail VERBATIM — the `[loop
/// HH:MM:SS]` clock is the run loop's cycle-start composition (the spec
/// shows it there only), never the emitter's addition.
#[test]
fn chug_loop_verbose_1_explicitly_enables_on_a_tty() {
    let root = probe_fixture_root();
    let capture = probe_pty(
        root.path(),
        &["working", "cycle start: eval cycle on X", "[loop 14:32:05] cycle 7 routing=eval (kimi-k3)"],
        &[("CHUG_LOOP_VERBOSE", "1")],
    );
    let lines = heartbeat_lines(&capture);
    assert_eq!(lines.len(), 1, "exactly one heartbeat line: {capture:?}");
    assert_eq!(
        lines[0], "[loop 14:32:05] cycle 7 routing=eval (kimi-k3)",
        "the line is the detail verbatim — no emitter-added decoration: {lines:?}"
    );
}

/// (single-call req) A TTY + unset → the line AND the dot from the ONE
/// emitter call: the appended line with the detail verbatim (one physical
/// line, no redraw) plus exactly one parsed 7501 record.
#[test]
fn a_transition_line_and_its_dot_come_from_one_call() {
    let root = probe_fixture_root();
    let capture = probe_pty(
        root.path(),
        &["working", "cycle start: eval cycle on X", "[loop 14:32:05] cycle 7 routing=eval (kimi-k3)"],
        &[],
    );
    let lines = heartbeat_lines(&capture);
    assert_eq!(lines.len(), 1, "exactly one heartbeat line: {capture:?}");
    assert_eq!(
        lines[0], "[loop 14:32:05] cycle 7 routing=eval (kimi-k3)",
        "the line is the detail verbatim (the run loop's cycle-start site \
         owns the clock prefix, per the spec's shapes)"
    );
    // Appended, never redrawn: the raw stream's only \r are the tty's own
    // CRLF translations at line ends — never a bare CR redraw inside a line.
    for line in String::from_utf8_lossy(&capture).split('\n') {
        let line = line.trim_end_matches('\r');
        assert!(
            !line.contains('\r'),
            "a bare carriage return inside a line is a redraw — the heartbeat \
             is appended lines only: {line:?}"
        );
    }
    let records = parse_osc7501(&capture);
    assert_eq!(records.len(), 1, "the same call emitted the dot");
    assert_eq!(records[0].state, "working");
    assert_eq!(records[0].msg.as_deref(), Some("cycle start: eval cycle on X"));
}

// ---------------------------------------------------------------------------
// The real-supervisor fixtures (the transition legs, end to end)
// ---------------------------------------------------------------------------

/// The stub process table: an EMPTY ps answer. The single-driver probe sees
/// no driver and proceeds; the orphan reaper scans nothing; the host's real
/// process table is never enumerated (the hermeticity contract).
const PS_STUB: &str = "#!/bin/sh\nexit 0\n";

/// A green build that installs the launchable stub binary (the
/// tests/loopd_osc7501.rs pattern): the cycle completes with an accepted
/// verdict and the stub plants the STOP file, so the sandbox supervisor runs
/// probe → sweep → build → cycle → clean exit in seconds.
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

/// The milestone stub's generator: a green build whose stub BINARY plays a
/// real cycle's event stream — `batches` batches of 10 merged iteration
/// lines (the watcher's 1s poll seam interleaves), one tool_result per batch
/// (the milestone's action name), a merge commit at the midpoint (the flip
/// line), then the accepted verdict. `initial_delay` holds the FIRST batch
/// back that many seconds — the stale-events fixtures widen the driver's
/// rotate-after-watcher-birth window (production ≈100ms) so the RED shape
/// is deterministic under any scheduler. `batch_sleep` paces the batches
/// (the 200-iteration volume arc runs 1s to keep the fixture bounded). The
/// sandbox is a git repo with a TODO.md carrying the open T042 row, so the
/// lines carry the item field too.
fn milestone_cargo_stub(batches: u32, initial_delay: u32, batch_sleep: u32) -> String {
    let merge_batch = batches / 2;
    format!(
        r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || {{ echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }}
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug
sleep {initial_delay}
iter=0
batch=0
while [ "$batch" -lt {batches} ]; do
  batch=$((batch + 1))
  i=0
  while [ "$i" -lt 10 ]; do
    iter=$((iter + 1))
    printf '{{"type":"iteration","ts":"2026-11-09T00:00:00Z","n":%d,"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}\n' "$iter" >> .chug/events.jsonl
    i=$((i + 1))
  done
  printf '{{"type":"tool_result","ts":"2026-11-09T00:00:00Z","name":"bash","ok":true,"is_error":false,"duration_ms":5,"preview":"stub batch"}}\n' >> .chug/events.jsonl
  if [ "$batch" -eq {merge_batch} ]; then
    echo "fixture item body" > fixture-item.md
    git add fixture-item.md
    git commit -q -m "merge: t356 fixture item landed (validator PASS)"
  fi
  sleep {batch_sleep}
done
# A 3s tail after the last write: the watcher's 1s polls must SEE the final
# iteration count (the 200-threshold milestone) before the cycle's kill —
# under suite load a tight write-to-kill window loses the sighting (the
# mechanism-not-timeouts doctrine: widen the window, never the bound).
sleep 3
printf 'chug: goal complete\nsummary: the milestone stub ran this cycle\n'
touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#
    )
}

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

/// The full-supervisor harness (the tests/loopd_osc7501.rs pattern): the
/// REAL loopd.sh + scripts/, the hermetic ps/lsof stubs first on PATH, the
/// chosen cargo stub, HOME pinned inside the sandbox, the sleep seams at 1s,
/// and the T254 max-loops bound.
struct Sandbox {
    _keep: TempDir,
    root: PathBuf,
}

impl Sandbox {
    /// `cargo_stub` picks the arc: GREEN_CARGO (installs a stub chug that
    /// completes), FAILING_CHUG_CARGO (a stub chug that dies rc=1),
    /// RED_BUILD_CARGO (the build gate itself fails), milestone_cargo_stub()
    /// (a stub that plays a real cycle's event stream and lands a merge —
    /// parameterized: batches, first-batch delay, batch pacing).
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
    /// last — AFTER the removals below — so the LOOPD_HB_POLL seam and the
    /// dormant/milestone fixtures' git identity land (the T354 pattern).
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
            // The sleep seams (the tests/loopd_osc7501.rs pattern) plus the
            // T356 watcher seam: a 1s poll makes the milestone fixture's
            // batches observable without waiting the 20s production cadence.
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_HB_POLL", "1")
            // T254 — the fixture-leak fail-safe: the bound is the run's real
            // terminator when no stub plants a STOP file.
            .env("LOOPD_MAX_LOOPS", max_loops.to_string())
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        // Hygiene: the host's loop-relevant exports must never flip a fixture
        // (the T332 hermeticity contract); the loaders are pinned to absent
        // paths by the pin_loopd_env_files guard, these removals cover direct
        // exports — including BOTH T356 knobs and the watcher seams.
        for var in [
            "CHUG_LOOP_VERBOSE",
            "CHUG_OSC7501",
            "LOOPD_HB_EVERY",
            "LOOPD_HB_POLL",
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
    /// Run the real supervisor with stdout PIPED (the redirected context) and
    /// return the output. `extra_envs` rides the launch (the same shape as
    /// `run_on_pty`) — the piped-milestone fixture needs the 1s poll seam.
    fn run_piped(&self, max_loops: u32, extra_envs: &[(&str, &str)]) -> Output {
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
            "CHUG_LOOP_VERBOSE",
            "CHUG_OSC7501",
            "LOOPD_HB_EVERY",
            "LOOPD_HB_POLL",
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
        cmd.output().expect("run the piped supervisor fixture")
    }

    fn log_path(&self) -> PathBuf {
        self.root.join(".chug/loopd/loopd.log")
    }

    fn read_log(&self) -> String {
        fs::read_to_string(self.log_path()).unwrap_or_default()
    }

    /// Every file the supervisor redirected into (the supervisor log plus
    /// every cycle log) — the redirected contexts that must hold ZERO
    /// heartbeat lines and ZERO escape bytes.
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
            .env("GIT_AUTHOR_NAME", "t356-fixture")
            .env("GIT_AUTHOR_EMAIL", "t356-fixture@fixture.invalid")
            .env("GIT_COMMITTER_NAME", "t356-fixture")
            .env("GIT_COMMITTER_EMAIL", "t356-fixture@fixture.invalid")
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
        "# Eval\n\nT356 fixture seed — the dormancy anchor rides this commit's date.\n",
    )
    .expect("write EVALUATION.md");
    run(&["add", "EVALUATION.md"]);
    run(&["commit", "-q", "-m", "eval: fixture seed"]);
}

/// `git init` + a seed commit INSIDE the sandbox — a repo for the watcher's
/// flip leg, WITHOUT an EVALUATION.md: the freshness predicate stays false,
/// so the seeded open row routes the cycle to the eval arm (the T356
/// milestone fixture's routing depends on this — an EVALUATION.md commit
/// would flip the route to the routine arm and re-key every assertion).
fn init_git_with_seed_commit(root: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("HOME", root)
            .env("GIT_AUTHOR_NAME", "t356-fixture")
            .env("GIT_AUTHOR_EMAIL", "t356-fixture@fixture.invalid")
            .env("GIT_COMMITTER_NAME", "t356-fixture")
            .env("GIT_COMMITTER_EMAIL", "t356-fixture@fixture.invalid")
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(&["init", "-q"]);
    fs::write(root.join("SEED.md"), "# T356 fixture seed\n").expect("write SEED.md");
    run(&["add", "SEED.md"]);
    run(&["commit", "-q", "-m", "seed: t356 fixture"]);
}

/// The git identity the milestone stub's commit inherits (extra_envs).
const GIT_IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "t356-fixture"),
    ("GIT_AUTHOR_EMAIL", "t356-fixture@fixture.invalid"),
    ("GIT_COMMITTER_NAME", "t356-fixture"),
    ("GIT_COMMITTER_EMAIL", "t356-fixture@fixture.invalid"),
];

/// The queue head the milestone sandbox starts with: one open row, so the
/// heartbeat's item field names it (LOOP-SPEC works the queue serially in
/// file order — the FIRST open row is the item the cycle will work).
fn seed_todo_queue(root: &Path) {
    fs::write(
        root.join("TODO.md"),
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T042 | fixture item title for the heartbeat line | specs/t042.md | 2 | open | the queue head |\n",
    )
    .expect("write TODO.md");
}

/// (transition req, green arc) A completing cycle emits the cycle-start line
/// (cycle number, routing, model short name) and the cycle-done line (the
/// accepted goal's own summary) — in that order — and the redirected files
/// hold ZERO heartbeat lines and ZERO escape bytes.
#[test]
fn a_green_cycle_emits_the_cycle_start_and_done_lines() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the green sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the green sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the green arc must complete a cycle:\n{log}");

    let lines = heartbeat_lines(&capture);
    assert_eq!(
        lines.len(),
        2,
        "the green arc emits exactly the cycle-start and cycle-done lines:\n{capture:?}"
    );
    // The spec's cycle-start shape carries the UTC clock — the ONLY line
    // that does (the other heartbeat lines are bare `[loop] …`).
    assert!(
        lines[0].starts_with("[loop ") && lines[0].contains("] cycle 1 routing=eval (kimi-k3)"),
        "the cycle-start line carries the [loop HH:MM:SS] clock, the launch \
         number, the routing, and the routed model's short name: {lines:?}"
    );
    // No TODO.md in this sandbox → no open row → the item field is omitted
    // (an empty hint degrades away; it never prints a bare `item=`).
    assert!(
        !lines[0].contains("item="),
        "with no open row the line must omit the item field: {lines:?}"
    );
    assert!(
        lines[1].starts_with("[loop]") && lines[1].contains("cycle done — goal complete"),
        "the cycle-done line names the verdict (bare [loop] — the clock is \
         the cycle-start's alone): {lines:?}"
    );
    assert!(
        lines[1].contains("the stub binary ran this cycle"),
        "the cycle-done line carries the accepted goal's own summary: {lines:?}"
    );
    // Order: the start line precedes the done line (a terminal reads the
    // span opening before it closes).
    let at_start = find(&capture, b"cycle 1 routing=eval").expect("start line in capture");
    let at_done = find(&capture, b"cycle done").expect("done line in capture");
    assert!(at_start < at_done, "the start line must precede the done line");

    // THE COMPAT CONTRACT, on the real redirected files: log parsers must
    // never see a heartbeat line or an escape byte.
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
        assert!(
            find(&bytes, b"[loop").is_none(),
            "{} (a redirected context) carries a heartbeat line — the T356 \
             gating contract is broken on the real supervisor path",
            file.display()
        );
    }
}

/// (compat req, redirected arc) The SAME green arc with stdout PIPED — the
/// nohup/launchd shape — produces zero bytes on stdout and zero heartbeat
/// lines in every redirected file. Non-harness operation is byte-identical.
#[test]
fn a_piped_supervisor_run_stays_byte_silent() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let out = sandbox.run_piped(4, &[]);
    assert!(out.status.success(), "the piped green sandbox must exit cleanly: {:?}", out.status);
    assert!(
        out.stdout.is_empty(),
        "a piped supervisor run emitted bytes on stdout — the T356 gating \
         contract is broken in the nohup/launchd shape: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the piped arc must still complete its cycle:\n{log}");
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(find(&bytes, b"[loop").is_none(), "{} carries a heartbeat line", file.display());
    }
}

/// (compat req, TTY arc) The SAME green arc with CHUG_LOOP_VERBOSE=0 on a
/// pty — the operator's opt-out — produces zero heartbeat lines while the
/// cycle still completes and the dot still rides.
#[test]
fn chug_loop_verbose_0_silences_a_real_supervisor_run() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let (mut child, rx) =
        sandbox.run_on_pty(4, &[("CHUG_LOOP_VERBOSE", "0")]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the silenced sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the silenced sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    assert!(
        heartbeat_lines(&capture).is_empty(),
        "CHUG_LOOP_VERBOSE=0 still emitted a heartbeat line on a real run: {capture:?}"
    );
    assert!(
        !parse_osc7501(&capture).is_empty(),
        "the dot must be untouched by the line's knob: {capture:?}"
    );
}

/// (transition req, dormant arc) The dormant cheap-exit emits its line naming
/// the dormancy AND the next probe (the pacing the operator watches) — and
/// never a working line, because the loop went quiet ON PURPOSE.
#[test]
fn the_dormant_skip_emits_its_line_naming_the_next_probe() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(GREEN_CARGO);
    init_git_with_evaluation_commit(&sandbox.root);
    // Two dormant iterations, then the T254 bound terminates the run.
    let (mut child, rx) = sandbox.run_on_pty(2, &GIT_IDENTITY);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the dormant sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the dormant sandbox must exit cleanly via the bound: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("dormant cheap-exit (T325)"),
        "the dormant arc must take the suspended-valve skip:\n{log}"
    );

    let lines = heartbeat_lines(&capture);
    assert!(
        !lines.is_empty(),
        "the dormant arc must emit at least one line:\n{capture:?}"
    );
    for line in &lines {
        assert!(
            line.starts_with("[loop]") && line.contains("queue empty — dormant, next probe 1s"),
            "every dormant line names the dormancy and the NEXT PROBE (the \
             LOOPD_SLEEP_OK seam pins it at 1s here), under the bare [loop] \
             prefix — the spec's dormant shape carries no clock:\n{lines:?}"
        );
        assert!(
            !line.contains("working"),
            "the dormant arc never claims work: {lines:?}"
        );
    }
}

/// (consistency req, failure arc) A cycle that dies with the budget/abort
/// exit emits the BLOCKED line whose reason text is the T354 msg channel
/// text BYTE-IDENTICALLY — one source string, two destinations, on the real
/// supervisor path. The blocked line carries no clock prefix: the reason IS
/// the payload (the spec's shape).
#[test]
fn a_dying_cycle_emits_the_blocked_line_matching_the_dot() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(FAILING_CHUG_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the failing-chug sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the failing-chug sandbox must still exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle ended WITHOUT goal complete"),
        "the failure arc must run its failure branch:\n{log}"
    );

    let records = parse_osc7501(&capture);
    let blocked = records
        .iter()
        .find(|r| r.state == "blocked")
        .expect("the failure arc must emit a blocked dot");
    let msg = blocked.msg.as_deref().expect("the blocked dot carries the msg");
    assert_eq!(
        msg, "budget: cycle ended without goal complete (rc=1)",
        "the T340 budget word rides the msg channel"
    );

    let lines = heartbeat_lines(&capture);
    let blocked_lines: Vec<&String> =
        lines.iter().filter(|l| l.contains("BLOCKED:")).collect();
    assert_eq!(
        blocked_lines.len(),
        1,
        "exactly one BLOCKED line on the real failure path:\n{capture:?}"
    );
    let reason = blocked_lines[0]
        .split_once("BLOCKED: ")
        .map(|(_, rest)| rest.to_string())
        .expect("the BLOCKED: prefix");
    assert_eq!(
        reason, msg,
        "THE CONSISTENCY CONTRACT: the heartbeat's reason text and the 7501 \
         msg are the SAME string, byte for byte (one source, two \
         destinations)"
    );
    assert!(
        !blocked_lines[0].starts_with("[loop "),
        "the blocked line has no clock prefix — the reason IS the payload: {}",
        blocked_lines[0]
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(find(&bytes, b"[loop").is_none(), "{} carries a heartbeat line", file.display());
    }
}

/// (consistency req, gate-red arc) A red build blocks WITHOUT any working
/// span: the launch is refused, so the terminal sees the BLOCKED line (its
/// reason matching the dot's msg) and never a cycle-start line.
#[test]
fn a_red_build_gate_emits_the_blocked_line_and_never_a_start() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(RED_BUILD_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the red-build sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the red-build sandbox must exit cleanly via STOP: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("build FAILED (rc=1)"),
        "the gate-red arc must hit the build-failure branch:\n{log}"
    );

    let records = parse_osc7501(&capture);
    let blocked = records
        .iter()
        .find(|r| r.state == "blocked")
        .expect("the gate-red arc must emit a blocked dot");
    let msg = blocked.msg.as_deref().expect("the blocked dot carries the msg");

    let lines = heartbeat_lines(&capture);
    let blocked_lines: Vec<&String> =
        lines.iter().filter(|l| l.contains("BLOCKED:")).collect();
    assert_eq!(blocked_lines.len(), 1, "exactly one BLOCKED line:\n{capture:?}");
    let reason = blocked_lines[0]
        .split_once("BLOCKED: ")
        .map(|(_, rest)| rest.to_string())
        .expect("the BLOCKED: prefix");
    assert_eq!(reason, msg, "the line's reason is the dot's msg, byte for byte");
    assert!(
        lines.iter().all(|l| !l.contains("routing=")),
        "a refused launch never emits a cycle-start line:\n{lines:?}"
    );
}

/// (transition req, milestone + flip arc, and the VOLUME GUARD — at the
/// spec's own 200-iteration shape) A cycle whose stub plays a real event
/// stream (200 iterations in twenty 10-iteration batches, one tool_result
/// per batch, a merge commit mid-run, the queue's open T042 row seeded)
/// emits: the cycle-start line WITH the item field, a milestone line at
/// EVERY crossed threshold 25..200 (eight — each naming the item and the
/// last tool), one flip line for the merge — and NOTHING beyond the volume
/// bound (≤ 15 lines for the whole cycle; no `iter 225` — the stub stopped
/// at 200; exactly one line per crossed threshold, never per poll).
#[test]
fn milestones_and_flips_stay_within_the_volume_bound() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(&milestone_cargo_stub(20, 0, 1));
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    let mut envs = vec![("LOOPD_HB_POLL", "1")];
    envs.extend(GIT_IDENTITY);
    let envs: Vec<(&str, &str)> = envs.into_iter().collect();
    let (mut child, rx) = sandbox.run_on_pty(4, &envs);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the milestone sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the milestone sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the milestone arc must complete a cycle:\n{log}");

    let lines = heartbeat_lines(&capture);
    let joined = lines.join("\n");

    // The cycle-start line carries the item field (the queue's open row).
    assert!(
        lines[0].contains("cycle 1 routing=eval (kimi-k3) item=T042 — fixture item title"),
        "the cycle-start line names the queue's open row: {lines:?}"
    );

    // Milestones: one line per CROSSED threshold, 25 through 200 — the stub
    // ran the spec's full 200-iteration shape, so all eight thresholds are
    // reported exactly once each (the one-line-per-crossed-threshold latch
    // at 200 INCLUDED — the volume fixture runs the same budget the launch
    // pins, not a shrunk stand-in). No shape is beyond the real count:
    // `iter 225/200` never appears.
    let milestones: Vec<&String> =
        lines.iter().filter(|l| l.contains(" iter ") && l.contains("/200")).collect();
    assert_eq!(
        milestones.len(),
        8,
        "200 iterations at the 25 cadence cross exactly eight thresholds:\n{joined}"
    );
    for k in 1..=8u32 {
        let threshold = k * 25;
        let expected = format!("[loop] T042 iter {threshold}/200 — bash");
        assert_eq!(
            milestones[(k - 1) as usize].as_str(),
            expected.as_str(),
            "milestone {threshold} is the spec's exact shape (bare item ID, \
             the last tool): {milestones:?}"
        );
    }
    assert!(
        !joined.contains("iter 225/200"),
        "no milestone beyond the real iteration count: {joined}"
    );
    assert_eq!(
        lines.iter().filter(|l| l.contains("iter 200/200")).count(),
        1,
        "the at-budget threshold is latched to ONE line, never per poll:\n{joined}"
    );

    // The flip: the stub's merge commit is observed as ONE line, the spec's
    // shape — `[loop] T042 done (merge <sha>) — <merge subject, 60>` (the
    // `merge:` subject prefix is stripped; it is redundant beside the sha).
    let flips: Vec<&String> = lines.iter().filter(|l| l.contains("done (merge ")).collect();
    assert_eq!(flips.len(), 1, "exactly one flip line for the one merge:\n{joined}");
    assert!(
        flips[0].starts_with("[loop]")
            && flips[0].contains("T042 done (merge ")
            && flips[0].contains(" — t356 fixture item landed (validator PASS)"),
        "the flip line names the item, the merge sha, and the subject \
         (capped at 60): {flips:?}"
    );
    let sha = flips[0]
        .split("done (merge ")
        .nth(1)
        .and_then(|rest| rest.split(')').next())
        .expect("the flip line carries the sha")
        .to_string();
    assert_eq!(sha.len(), 7, "the flip names the SHORT sha (git rev-parse --short): {sha}");

    // THE VOLUME GUARD: the whole cycle's heartbeat stays within the bound.
    assert!(
        lines.len() <= 15,
        "a full cycle must stay within 15 heartbeat lines (no scrollback \
         spam), got {}:\n{joined}",
        lines.len()
    );

    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(find(&bytes, b"[loop").is_none(), "{} carries a heartbeat line", file.display());
    }
}

// ---------------------------------------------------------------------------
// The stale-events fixtures (the F1 fix-up round, validator-blocking): a
// previous cycle's events log is on the watcher's read surface at birth —
// `loopd_hb_watch 200 &` starts microseconds before the chug launch, and
// the driver rotates .chug/events.jsonl only at its own launch — so the
// milestone leg's FIRST poll read the predecessor's file and LATCHED its
// iteration count. RED (captured on the pre-fix tree, both shapes):
//
//   stale n=173 → `[loop] T042 — fixture item title for the heartbeat line
//   iter 150/200 — bash` emitted at cycle start (the stale (173/25)*25
//   threshold), then the `shown` latch suppressed every real milestone
//   below 175 — the fresh cycle's 25 and 50 lines never appeared;
//
//   stale n=200 → `iter 200/200` emitted, then ZERO milestones for the
//   entire cycle (the latch sat at the max).
//
// Every production cycle ≥2 whose predecessor reached ≥25 iterations
// misreported. The prior fixtures never saw it because every sandbox
// starts in a fresh tempdir with no pre-existing .chug/events.jsonl —
// these two plant the predecessor's file (both the n<max and the n≥max
// shapes) and demand ZERO false lines and FULL real-milestone delivery.
// ---------------------------------------------------------------------------

/// A previous cycle's stale events log — the shape a real predecessor
/// leaves: one `run_start` head, one merged iteration line per turn, a
/// tool_result every 25 (the watcher's milestone read takes the LAST
/// iteration's `n` and the last tool_result's `name`).
fn stale_events_log(last_n: u32) -> String {
    let mut s = String::from(
        "{\"type\":\"run_start\",\"ts\":\"2026-11-08T23:00:00Z\",\"mode\":\"run\",\"model\":\"anthropic-system.ai.kimi-k3\",\"spec\":\"LOOP-SPEC.md\",\"cwd\":\"/fixture\",\"version\":\"0.0.0\",\"commit\":\"stale\",\"head_branch\":\"main\",\"head_commit\":\"stale000\",\"max_iters\":200,\"max_minutes\":360,\"max_tokens\":null,\"max_tokens_per_request\":32768,\"goal_sha256\":\"stale\",\"goal_pack\":null,\"approve\":null,\"plan_sha256\":null}\n",
    );
    for n in 1..=last_n {
        s.push_str(&format!(
            "{{\"type\":\"iteration\",\"ts\":\"2026-11-08T23:00:00Z\",\"n\":{n},\"input_tokens\":1,\"output_tokens\":1,\"cache_read_input_tokens\":0,\"cache_creation_input_tokens\":0}}\n"
        ));
        if n % 25 == 0 {
            s.push_str(
                "{\"type\":\"tool_result\",\"ts\":\"2026-11-08T23:00:00Z\",\"name\":\"bash\",\"ok\":true,\"is_error\":false,\"duration_ms\":5,\"preview\":\"stale predecessor batch\"}\n",
            );
        }
    }
    s
}

/// Plant a previous cycle's events log in the sandbox — the shape every
/// production cycle ≥2 starts with (the predecessor's own `.chug/
/// events.jsonl`, still sitting there when the watcher is born). The
/// fresh-tempdir doctrine hid this from every prior fixture.
fn plant_stale_events(root: &Path, last_n: u32) {
    let dir = root.join(".chug");
    fs::create_dir_all(&dir).expect("sandbox .chug dir");
    fs::write(dir.join("events.jsonl"), stale_events_log(last_n))
        .expect("plant the stale predecessor events log");
}

/// The shared stale-arc assertions: the cycle completed, the predecessor's
/// thresholds (150 off a stale 173, 200 off a stale 200) never surfaced,
/// the fresh cycle's own 25/50 milestones arrived, and every redirected
/// file stayed clean.
fn assert_stale_arc_clean(sandbox: &Sandbox, capture: &[u8]) {
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle OK:"),
        "the stale-events arc must still complete its cycle:\n{log}"
    );
    let lines = heartbeat_lines(capture);
    let joined = lines.join("\n");
    assert!(
        !joined.contains("iter 150/200"),
        "a FALSE milestone line was emitted off the PREVIOUS cycle's events \
         log (the F1 first-poll staleness):\n{joined}"
    );
    assert!(
        !joined.contains("iter 200/200"),
        "a FALSE at-budget milestone line was emitted off the PREVIOUS \
         cycle's events log:\n{joined}"
    );
    assert!(
        joined.contains("iter 25/200") && joined.contains("iter 50/200"),
        "the fresh cycle's real milestones must be delivered — a stale \
         latch suppresses everything below it:\n{joined}"
    );
    let milestones: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains(" iter ") && l.contains("/200"))
        .collect();
    assert_eq!(
        milestones.len(),
        2,
        "the fresh cycle ran 60 iterations — exactly two crossed thresholds:\n{joined}"
    );
    for m in &milestones {
        assert!(
            m.contains("T042") && m.contains(" — bash"),
            "the milestone names the item and the last tool from the cycle's \
             OWN events log: {milestones:?}"
        );
    }
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(find(&bytes, b"[loop").is_none(), "{} carries a heartbeat line", file.display());
    }
}

/// (F1, stale n<max shape) A predecessor log ending at iteration 173 must
/// yield ZERO false lines and full real-milestone delivery for the fresh
/// cycle (the RED shape is quoted in the section comment above). The stub
/// holds its first batch back 2s — the widened rotate-after-birth window —
/// so the pre-fix watcher's first polls ALWAYS see the planted file and the
/// post-fix watcher's pre-rotation is exercised against a real gap.
#[test]
fn stale_events_below_the_budget_never_misreport_the_cycle() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(&milestone_cargo_stub(6, 2, 2));
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    plant_stale_events(&sandbox.root, 173);
    let mut envs = vec![("LOOPD_HB_POLL", "1")];
    envs.extend(GIT_IDENTITY);
    let envs: Vec<(&str, &str)> = envs.into_iter().collect();
    let (mut child, rx) = sandbox.run_on_pty(4, &envs);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the stale-below sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the stale-below sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    assert_stale_arc_clean(&sandbox, &capture);
}

/// (F1, stale n≥max shape) A predecessor log ending at iteration 200 — the
/// ceiling — must yield ZERO false lines (the pre-fix watcher emitted
/// `iter 200/200` at birth and then went milestone-silent for the whole
/// cycle) and full real-milestone delivery.
#[test]
fn stale_events_at_the_budget_never_misreport_the_cycle() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(&milestone_cargo_stub(6, 2, 2));
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    plant_stale_events(&sandbox.root, 200);
    let mut envs = vec![("LOOPD_HB_POLL", "1")];
    envs.extend(GIT_IDENTITY);
    let envs: Vec<(&str, &str)> = envs.into_iter().collect();
    let (mut child, rx) = sandbox.run_on_pty(4, &envs);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the stale-at sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the stale-at sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    assert_stale_arc_clean(&sandbox, &capture);
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
/// brace at column 0.
fn fn_body(src: &str, name: &str) -> String {
    let sig = format!("{name}() {{");
    let start = need(src, &sig, &format!("{name} is defined"));
    let end = src[start..]
        .find("\n}")
        .map(|at| start + at)
        .unwrap_or_else(|| panic!("{name} is closed"));
    src[start..end].to_string()
}

/// (single-source req) The ONE dual-surface emitter: the TTY gate first, the
/// heartbeat leg BEFORE the dot's env gate (independent knobs), the blocked
/// composition FROM the msg (the consistency contract true by construction),
/// and the gated `loopd_hb_emit` writer carrying the line's own contract.
#[test]
fn the_emitter_composes_both_surfaces_from_one_call() {
    let loopd = read("loopd.sh");
    let body = fn_body(&loopd, "loopd_osc7501_emit");

    // The TTY gate is the FIRST leg — both surfaces are TTY-only, and the
    // gate must be able to silence everything before either env is read.
    let at_tty = need(&body, "[ -t 1 ] || return 0", "the TTY gate is the first leg");
    // The heartbeat leg sits BEFORE the dot's env gate: CHUG_OSC7501=0 must
    // never silence the line, and CHUG_LOOP_VERBOSE=0 never the dot.
    let at_hb = need(&body, "loopd_hb_emit", "the heartbeat leg");
    let at_env = need(&body, "case \"${CHUG_OSC7501-}\" in", "the dot's env gate");
    let at_write = need(&body, "printf '\\033]7501;%s\\033\\\\'", "the wire writer");
    assert!(
        at_tty < at_hb && at_hb < at_env && at_env < at_write,
        "the order is TTY → line → dot-env → dot-wire; anything else lets one \
         channel's knob silence the other"
    );
    // The blocked composition reads the MSG — the reason text is byte-
    // identical on both surfaces BY CONSTRUCTION (the consistency fixtures'
    // assertion), never by a duplicated string at a call site.
    need(&body, "[loop] BLOCKED: $msg", "the blocked line composes from the msg");
    // The detail line is the detail VERBATIM (the F2 shapes: the `[loop
    // HH:MM:SS]` clock is the run loop's cycle-start composition — the spec
    // shows it there ONLY — never an emitter-added prefix).
    need(&body, "loopd_hb_emit \"$hb\"", "the human line is the detail verbatim");

    // The writer: the TTY gate, the env case, the guarded printf — the same
    // contract shape as the dot's, with ITS OWN env knob.
    let hb = fn_body(&loopd, "loopd_hb_emit");
    let at_hb_tty = need(&hb, "[ -t 1 ] || return 0", "the line's TTY gate is the first leg");
    let at_hb_env = need(&hb, "case \"${CHUG_LOOP_VERBOSE-}\" in", "the line's env gate");
    let at_hb_write = need(&hb, "printf '%s\\n' \"$1\" || true", "the guarded line writer");
    assert!(at_hb_tty < at_hb_env && at_hb_env < at_hb_write, "gate order: TTY → env → write");
    count_eq(&hb, "''|1) ;;", 1, "the emit arm accepts unset/empty and exactly 1");
    count_eq(&hb, "*) return 0 ;;", 1, "every other value kills (default-safe)");
    assert!(
        hb.trim_end().ends_with("return 0"),
        "the writer must end in an unconditional return 0 (it is called as a \
         bare statement under set -euo pipefail in the run loop)"
    );
    // THE COMPAT CONTRACT, statically: the writer emits ONLY to stdout —
    // never to a file, never through a redirect. Log parsers must never see
    // a heartbeat line because the writer is physically incapable of writing
    // one into a log.
    assert!(
        !hb.contains(">>") && !hb.contains("$LOG"),
        "the writer body must not redirect anywhere — stdout is the only \
         channel and the gate owns the silence"
    );
}

/// (wiring req) Every transition wires the line beside its dot, in the run
/// loop's real order — and the watcher's max is the launch's iteration
/// budget (the same 200 literal the T81/T230 pins hold byte-stable: bump
/// one, bump both).
#[test]
fn every_transition_wires_the_line_beside_its_dot() {
    let loopd = read("loopd.sh");

    // The cycle start: composition between the log line and the emission.
    let at_log = need(
        &loopd,
        "cycle start -> $cycle_log ($mode cycle, orchestrator $orch_model)",
        "the cycle-start log line",
    );
    let at_compose = need(
        &loopd,
        "hb_detail=\"[loop $(date -u +%H:%M:%S)] cycle $cycles routing=$mode",
        "the line composition (the spec's clocked cycle-start shape)",
    );
    let at_emit = need(
        &loopd,
        "loopd_osc7501_emit working \"cycle start: $mode cycle on $orch_model\"",
        "the cycle-start working emission",
    );
    assert!(
        at_log < at_compose && at_compose < at_emit,
        "the cycle-start line is composed beside its dot (one transition, \
         both surfaces, in the run loop's order)"
    );
    // The item hint rides only when the queue has an open row.
    need(&loopd, "if [ -n \"$hb_item\" ]; then", "the item field is conditional");

    // The watcher rides the launch: started before, killed after. The
    // predecessor's events log is rotated aside BEFORE the watcher is
    // backgrounded (the F1 fix — its first poll must never read a stale
    // iteration count; see the stale-events fixtures above). The needles
    // are line-precise (own line, two-space indent) so the helpers' doc
    // comments that MENTION the calls can never satisfy them.
    let at_prerot = need(&loopd, "\n  loopd_hb_prerotate\n", "the events-log pre-rotation call");
    let at_watch = need(&loopd, "\n  loopd_hb_watch 200 &\n", "the watcher start");
    let at_launch = need(&loopd, "chug_out=\"$(CARGO_TARGET_DIR=", "the chug launch");
    let at_kill = need(&loopd, "kill \"$hb_watch_pid\" 2>/dev/null || true", "the watcher kill");
    let at_wait = need(&loopd, "wait \"$hb_watch_pid\" 2>/dev/null || true", "the watcher reap");
    assert!(
        at_prerot < at_watch && at_watch < at_launch && at_launch < at_kill && at_kill < at_wait,
        "the pre-rotation, then the watcher, span exactly the launch capture — \
         never a skip path, never a verdict leg"
    );
    // The tandem: the same 200 literal on both sides of the launch (the
    // line-precise needle — the prerotate comment MENTIONs the watcher).
    count_eq(&loopd, "--max-iters 200", 1, "the launch's iteration budget (T121/T198)");
    count_eq(&loopd, "\n  loopd_hb_watch 200 &\n", 1, "the watcher's max is the same budget");

    // The held skips: the dormant line names the next probe, the plain skip
    // names the skip — both between the pacing log and the sleep.
    let at_pacing = need(&loopd, "cheap-exit pacing: sleeping ${skip_secs}s", "the pacing log");
    let at_dormant = need(
        &loopd,
        "loopd_osc7501_emit idle \"dormant: queue empty\" \"[loop] queue empty — dormant, next probe ${skip_secs}s\"",
        "the dormant line",
    );
    let at_skip = need(
        &loopd,
        "loopd_osc7501_emit idle \"cycle skipped: no evaluation needed now\" \"[loop] no evaluation needed — skipped, next probe ${skip_secs}s\"",
        "the plain-skip line",
    );
    let at_sleep = need(&loopd, "sleep \"$skip_secs\"", "the pacing sleep");
    assert!(
        at_pacing < at_dormant && at_dormant < at_skip && at_skip < at_sleep,
        "the skip-arm lines sit between the pacing log and the sleep"
    );

    // The cycle done: the line beside the verdict dot, before site-sync.
    let at_ok = need(&loopd, "cycle OK: $summary", "the cycle-OK log line");
    let at_ok_emit = need(
        &loopd,
        "loopd_osc7501_emit idle \"cycle ok\" \"[loop] cycle done — goal complete$hb_tail\"",
        "the cycle-done line",
    );
    let at_sync = need(&loopd, "scripts/site-sync.sh >> \"$LOG\"", "the site-sync call");
    assert!(at_ok < at_ok_emit && at_ok_emit < at_sync, "the done line follows the verdict");

    // The ideation start: the line rides the same call as its dot.
    need(
        &loopd,
        "loopd_osc7501_emit working \"ideation cycle: proposing outward-facing features\" \"ideation cycle — proposing outward-facing features (T326)\"",
        "the ideation line",
    );
}

/// (blocked req) The five blocked transitions pass NO detail — their lines
/// compose from the msg inside the emitter (the single-source property), so
/// there is exactly one blocked-line composition in the file.
#[test]
fn the_blocked_lines_compose_inside_the_emitter_only() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "[loop] BLOCKED: $msg", 1, "ONE blocked-line composition");
    // Every blocked call site stays two-arg (the detail would bypass the
    // msg-sourced composition and fork the reason text).
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked \"$osc_word: cycle ended without goal complete (rc=$chug_rc)\"",
        1,
        "the cycle-death site is two-arg",
    );
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked \"error: build failed (rc=$build_rc) — refusing to launch a stale binary\"",
        1,
        "the build-red site is two-arg",
    );
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked \"error: HALTED after 3 consecutive build failures\"",
        1,
        "the build-HALT site is two-arg",
    );
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked \"error: HALTED after 3 consecutive cycles without goal complete\"",
        1,
        "the cycle-HALT site is two-arg",
    );
    count_eq(
        &loopd,
        "loopd_osc7501_emit blocked \"error: disposition commit failed — falling through to a real eval\"",
        1,
        "the disposition-failure site is two-arg",
    );
}

/// (item req) The item hint reads the queue's FIRST open row — LOOP-SPEC
/// works the queue strictly serially in file order — with the same
/// todo/open cell rule as `todo_rows`, capped at 60 chars, degrading to
/// silence (no row, no TODO.md, no crash under set -e).
#[test]
fn the_item_hint_reads_the_queue_head_defensively() {
    let loopd = read("loopd.sh");
    let body = fn_body(&loopd, "top_open_item");
    need(&body, "[ -f \"${1:-TODO.md}\" ] || return 0", "a missing TODO.md degrades to silence");
    need(&body, "s == \"todo\" || s == \"open\"", "the same open-cell rule as todo_rows");
    need(&body, "substr(t, 1, 60)", "the title is capped at the spec's 60 chars");
    count_eq(&loopd, "top_open_item() {", 1, "the helper is defined exactly once");
    // Two read sites: the cycle-start line (hb_item) and the watcher's
    // milestone line (item) — both rc-latched, both degrading to silence.
    count_eq(&loopd, "hb_item=$(top_open_item) || hb_item=\"\"", 1, "the cycle-start read is rc-latched (T142)");
    count_eq(&loopd, "item=$(top_open_item) || item=\"\"", 1, "the watcher read is rc-latched (T142)");
}

/// (watcher req) The watcher's knobs default safely, the poll loop cannot
/// outlive its supervisor, and the milestone line is latched per threshold
/// (never per poll).
#[test]
fn the_watcher_defaults_safely_and_cannot_outlive_its_supervisor() {
    let loopd = read("loopd.sh");
    let body = fn_body(&loopd, "loopd_hb_watch");
    // The knob arms: unset/empty/zero/non-numeric degrade to the defaults —
    // a garbage value must never break production (the T254 arming shape).
    count_eq(&body, "''|0|*[!0-9]*) every=25 ;;", 1, "the cadence default is 25");
    count_eq(&body, "''|0|*[!0-9]*) poll=20 ;;", 1, "the poll default is 20s");
    // Liveness: a dead supervisor ends the loop; the poll-count backstop
    // caps any leak that survives both the parent kill and the orphan check.
    need(&body, "kill -0 \"$PPID\" 2>/dev/null || return 0", "a dead supervisor ends the loop");
    need(&body, "[ \"$polls\" -ge 1440 ]", "the leak backstop");
    // The threshold latch: one line per crossed 25-iteration multiple.
    need(
        &body,
        "[ \"$n\" -ge $((shown + every)) ] && [ \"$shown\" -lt \"$max\" ]",
        "the milestone fires on a CROSSED threshold",
    );
    need(&body, "shown=$(((n / every) * every))", "the line reports the threshold, not the poll");
    // The flip legs: the pre-cycle HEAD is primed, never reported; only
    // merge subjects flip (the queue's serial delivery per LOOP-SPEC).
    need(&body, "first=0 # prime: the pre-cycle HEAD is context, never a flip", "the prime leg");
    count_eq(&body, "merge:*)", 1, "only merge subjects flip");
    // Every best-effort leg is rc-latched: neither set -e nor pipefail can
    // kill the watcher mid-cycle (the T142 house style). Two latched loop
    // exits (the parent-death check, the sleep slice); the poll-count
    // backstop is a plain guarded return inside its if.
    count_eq(&body, "|| return 0", 2, "the latched loop-exit legs");
    count_eq(&body, ") || act=\"\"", 1, "the action read is latched");
    count_eq(&body, "item=$(top_open_item) || item=\"\"", 1, "the item read is latched");
    count_eq(&body, ") || sha=\"\"", 1, "the HEAD read is latched");
    count_eq(&body, ") || subj=\"\"", 1, "the subject read is latched");
    // The poll wait is sliced to 1s: a killed watcher never leaves a
    // long-sleeping orphan holding the pty stdout (the T354 fixtures' drain
    // bounds assume ≤1s stragglers).
    count_eq(&body, "sleep 1 || return 0", 1, "the sliced poll wait");
    count_eq(&body, "[ \"$slept\" -lt \"$poll\" ]", 1, "the slice loop bounds at the poll knob");
}

/// (F1 fix-up req, static) The pre-rotation is the milestone leg's primer at
/// the read surface: the predecessor's events log is rotated aside (under
/// the driver's OWN cycle-stream archive name, so the wrap's streak
/// derivation sees the identical corpus) before the watcher exists. The
/// stale-events fixtures prove the behavior; this pin makes the mechanism
/// itself mutation-resistant (drop the call, or the guard, or the name
/// shape, and the static gate goes RED even if a fixture never runs).
#[test]
fn the_watcher_never_reads_a_predecessor_events_log() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "loopd_hb_prerotate() {", 1, "the pre-rotation is defined exactly once");
    let body = fn_body(&loopd, "loopd_hb_prerotate");
    // The guard is the driver's own rotate rule: a MISSING or EMPTY log is a
    // no-op ([ -s ] ⟺ rotate_fresh's len > 0), everything else moves.
    need(&body, "[ -s .chug/events.jsonl ] || return 0", "an absent/empty log is a no-op");
    // The archive name is the cycle-stream shape (`events-<YYYYmmdd>-<HHMMSS>
    // .jsonl`, collisions -2..-10 like src/archive.rs) — the wrap's
    // cycle-stream enumeration keys on that exact name shape.
    need(&body, "date -u +%Y%m%d-%H%M%S", "the UTC cycle-stream timestamp");
    need(&body, "events-$ts$n.jsonl", "the driver's archive-name shape");
    count_eq(&body, "|| true", 1, "the mv is best-effort (a failed move defers to the driver)");
    assert!(
        body.trim_end().ends_with("return 0"),
        "the pre-rotation must end in an unconditional return 0 (a bare \
         statement under set -euo pipefail in the run loop)"
    );
    // The call site: inside the run loop, after the cycle-start emission,
    // BEFORE the watcher is backgrounded (the ordering pin above asserts the
    // watcher/launch pair; this one anchors the call to the watcher — both
    // needles are line-precise so doc-comment mentions never satisfy them).
    let at_call = need(&loopd, "\n  loopd_hb_prerotate\n", "the run-loop call site");
    let at_watch = need(&loopd, "\n  loopd_hb_watch 200 &\n", "the watcher start");
    assert!(
        at_call < at_watch,
        "the pre-rotation must precede the watcher's birth — the whole point \
         is that no watcher poll ever sees the predecessor's file"
    );
}

/// (volume req, static) The cadence arithmetic the behavioral volume guard
/// rides: the default 25-iteration cadence against the launch's 200-iteration
/// budget bounds a cycle at 8 milestone lines, + the cycle-start and
/// cycle-done lines = 10 ≤ 15; merge flips ride LOOP-SPEC's serial
/// one-merge-per-row delivery and the behavioral fixture measures the total
/// at the spec's own 200-iteration shape.
#[test]
fn the_volume_arithmetic_holds() {
    let loopd = read("loopd.sh");
    // The cadence default (the watcher knob) and the launch budget (the
    // T121/T198 pin) — the two numbers the bound is computed from.
    count_eq(&loopd, "''|0|*[!0-9]*) every=25 ;;", 1, "the cadence default");
    count_eq(&loopd, "--max-iters 200", 1, "the launch budget");
    let every: u64 = 25;
    let max: u64 = 200;
    let milestones = max / every; // 8
    let fixed = 2usize; // cycle-start + cycle-done
    assert!(
        milestones as usize + fixed <= 15,
        "the 200-iteration bound at the 25 cadence must fit the volume guard"
    );
    assert_eq!(milestones, 8, "200/25 crosses eight thresholds");
}

/// (probe req) The probe drives the REAL dual-surface emitter: no 4th
/// argument silences the line (the T354 wire fixtures' exact dot-only
/// captures), a 4th argument asks for the human line — the same call.
#[test]
fn the_probe_drives_the_real_dual_surface_emitter() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "  osc)", 1, "the probe subcommand arm exists");
    let at_arm = need(&loopd, "  osc)", "the osc arm");
    // The detail-conditional: empty → the line is silenced (dot-only probe).
    let at_case = need(&loopd, "case \"${4:-}\" in", "the probe's detail conditional");
    let at_force = need(&loopd, "'') CHUG_LOOP_VERBOSE=0 ;;", "the dot-only arm");
    let at_call = need(
        &loopd,
        "loopd_osc7501_emit \"${2:-working}\" \"${3:-}\" \"${4:-}\"",
        "the probe drives the REAL dual-surface emitter",
    );
    let at_status = need(&loopd, "  status)", "the next arm");
    assert!(
        at_arm < at_case && at_case < at_force && at_force < at_call && at_call < at_status,
        "the probe arm conditions the line's knob, then drives the emitter \
         once, and exits before the run mode"
    );
    // EXACTLY ONE emitter call in the probe arm — a second would fork the
    // surfaces and break the T354 count pin's arithmetic (11 total sites).
    count_eq(&loopd, "loopd_osc7501_emit \"${2:-working}\"", 1, "one probe call site");
    count_eq(
        &loopd,
        "usage: loopd.sh [run|stop|status|routing] [sleep-ok] [predicate] [osc]",
        1,
        "the usage line keeps the probe (the T354-era needles stay intact)",
    );
}

/// T159 membership pin (the T354 file's shape, one file later): every test
/// in THIS file that spawns a real supervisor (`Sandbox::new`) takes the
/// T151 timing guard as its FIRST acquisition, then the T172 cross-binary
/// load lock, both before the sandbox spawn. A future through-loopd fixture
/// added without the join is RED by construction.
#[test]
fn the_real_supervisor_fixtures_join_the_t151_timing_domain() {
    let src = read("tests/loopd_heartbeat.rs");
    assert!(
        src.contains("#[path = \"../src/testsupport.rs\"]\nmod testsupport;"),
        "the timing-domain join must be the #[path] include of \
         src/testsupport.rs — any other lock source is a second, independent \
         domain (the T151 finding)"
    );
    let timing_line = "let _timing = testsupport::timing_guard();";
    let t172_line = "let _t172_load = t172_load_lock::family_guard(\"loopd-heartbeat\");";
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
        10,
        "the through-loopd family is 10 fixtures (green, piped, silenced, \
         dormant, dying, gate-red, milestone, stale-below, stale-at, \
         piped-milestone) — a new real-supervisor fixture must join the \
         domain or this pin's count is lying about coverage"
    );
}

/// (F4 fix-up, gating req behavioral tooth) The TTY gate, piped, on the
/// milestone arc: the watcher RUNS the same 60-iteration event stream (the
/// same stub + the 1s poll seam the pty milestone fixture uses, so the
/// emit calls certainly HAPPEN — the pty twin proves the arc fires), but
/// stdout is a pipe: the byte-assert is ZERO bytes on stdout. A dropped
/// `[ -t 1 ]` gate in `loopd_hb_emit` leaks the milestone and flip lines
/// straight into the redirected stream — this fixture kills it
/// behaviorally (M2's static pin keeps the regression guard, this gives
/// it the tooth).
#[test]
fn a_piped_milestone_arc_leaks_no_lines_into_the_redirect() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-heartbeat");
    let sandbox = Sandbox::new(&milestone_cargo_stub(6, 0, 2));
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    let mut envs = vec![("LOOPD_HB_POLL", "1")];
    envs.extend(GIT_IDENTITY);
    let out = sandbox.run_piped(4, &envs);
    assert!(
        out.status.success(),
        "the piped milestone arc must exit cleanly: {:?}",
        out.status
    );
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle OK:"),
        "the piped milestone arc must complete its cycle:\n{log}"
    );
    assert!(
        out.stdout.is_empty(),
        "piped stdout must receive ZERO bytes even on an arc whose watcher \
         emitted lines on the pty twin (the TTY gate is the \
         whole contract): {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(find(&bytes, b"[loop").is_none(), "{} carries a heartbeat line", file.display());
    }
}

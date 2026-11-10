//! T359 — the loop PTY's verbosity dial: `CHUG_LOOP_VERBOSE` levels 0-3.
//!
//! Operator 2026-11-10 (the herdr tab is a PTY and humans watch it): "I
//! would like the chug PTY interface exposed to herdr to have a flag that
//! shows verbose details about the turns, maybe streaming the logs to the
//! interface if folks want to see more of what's going on?" T356 answered
//! "is it alive" (the compact heartbeat); T359 adds the dial between
//! silent and full detail:
//!
//! * **0 — silent**: no lines, no tee (the 7501 dot is the dot's own knob).
//! * **1 — compact heartbeat** (the T356 default on a TTY): the cycle
//!   start, milestones, flips, dormant, `BLOCKED:` — ≤15 lines per
//!   200-iteration cycle (the volume guard lives in tests/loopd_heartbeat.rs
//!   and still holds; this family re-proves "compact only" per level).
//! * **2 — verbose**: level 1 PLUS per-turn lines — one per tool call
//!   (`[loop] T042 iter 37 — tool bash: <summary, 80>` — the `...`
//!   truncation marker past 80 chars, errors in full up to the driver's own
//!   preview cap) and each model text block capped at ~3 lines (the same
//!   marker on truncation).
//! * **3 — trace**: the cycle's full output streamed — every new
//!   `cycle-*.log` line teed to stdout as it lands, unfiltered and
//!   unprefixed: the stream IS the cycle log.
//!
//! **Gating (the standing contract, extended)**: stdout must be a TTY — a
//! redirected context is level 0 whatever the knob says (logs are never
//! polluted); on a TTY, unset/empty is level 1 (the T356 default,
//! byte-identical to pre-T359) and any value outside `0..3` degrades to 0.
//! The T354 7501 emission is unaffected: the dot and the line stay in sync,
//! and the heartbeat's source-string rule extends to level 2's summaries.
//!
//! **Mechanism**: a line-count-latched tail of the files the loop already
//! writes (the cycle's `.chug/events.jsonl` for tool results, the cycle log
//! for model text and the trace) — `wc -l` counts only COMPLETE lines, so a
//! half-written stream line (the model text's partial deltas) waits for its
//! newline and is consumed exactly once: never split, never repeated, and
//! never double-written (the child's stdout is captured and its stderr goes
//! to the cycle log, so the tee is the only thing these lines ever reach the
//! TTY through). The turn/trace latency budget is the poll cadence: at
//! level ≥2 the watcher polls on the 1s slice, so a written line reaches
//! stdout within the spec's ≤2s.
//!
//! Fixture doctrine (the T354/T356 files', verbatim where it applies): the
//! pty is REAL (`libc::openpty` + `pre_exec` dup2 — the gates' `[ -t 1 ]`
//! see actual ttys and the capture is the exact byte stream the terminal
//! would see). The transition legs run the REAL supervisor end to end in a
//! sandbox (the hermetic ps/lsof/cargo stubs, the LOOPD_SLEEP_* seams, the
//! T254 max-loops bound as the terminator) and assert against the captured
//! pty stream AND the redirected log files.

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
// The receive side: scrubbed lines — every line the terminal would show
// ---------------------------------------------------------------------------

/// The captured stream with every 7501 sequence stripped (they carry NO
/// newline, so they sit mid-line in the raw capture; a tty's ONLCR turns
/// each appended `\n` into `\r\n`). The result is the byte text a terminal's
/// scrollback holds.
fn scrub_7501(capture: &[u8]) -> String {
    let mut scrubbed = Vec::with_capacity(capture.len());
    let mut i = 0usize;
    while i < capture.len() {
        if capture[i] == ESC
            && find(&capture[i..], b"\x1b]7501;") == Some(0)
            && find_terminator(capture, i + 7, 4096).is_some()
        {
            i = find_terminator(capture, i + 7, 4096)
                .map(|(_, next)| next)
                .expect("checked above");
            continue;
        }
        scrubbed.push(capture[i]);
        i += 1;
    }
    String::from_utf8_lossy(&scrubbed).into_owned()
}

/// Every text line the terminal would show, in order — blank lines KEPT
/// (the trace's own blank lines are part of the stream).
fn stream_lines(capture: &[u8]) -> Vec<String> {
    scrub_7501(capture)
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

/// The heartbeat lines in a captured pty stream, in order (the T356 shapes —
/// the level-1 surface the higher levels build on).
fn heartbeat_lines(capture: &[u8]) -> Vec<String> {
    stream_lines(capture)
        .into_iter()
        .filter(|l| l.starts_with("[loop"))
        .collect()
}

fn esc_count(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| b == ESC).count()
}

/// The heartbeat EMISSION shapes (`[loop] …` bare, `[loop HH:MM:SS] …`
/// clocked) — NOT the supervisor's own `[loopd …]` cycle-log stamps, which
/// legitimately carry `[loop` as a prefix of `[loopd` (the T81 header and
/// the verdict stamp are the supervisor's file writes, pre-T359). The
/// redirected-file contract is about the emitted lines: log parsers must
/// never see a heartbeat line.
fn is_heartbeat_shape(line: &str) -> bool {
    if line.starts_with("[loopd") {
        return false;
    }
    if line.starts_with("[loop]") {
        return true;
    }
    line.starts_with("[loop ") && line.as_bytes().get(6).is_some_and(|b| b.is_ascii_digit())
}

fn carries_heartbeat_line(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).lines().any(is_heartbeat_shape)
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
/// supervisor kills its watcher before it continues (with one bounded
/// drain window at level ≥2, the tee's own latency budget), so EOF lands
/// within seconds of the child's exit; a longer hold would be a leak the
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

/// The latency fixture's drain: like [`drain_pty`], but each read is sent
/// AS IT ARRIVES with its arrival timestamp — the cumulative buffer plus
/// the Instant that read returned. The latency bound is measured from a
/// cycle-log line's write (observed on disk) to the arrival of the chunk
/// that completes its emission on the pty.
fn drain_pty_timed(master: RawFd) -> mpsc::Receiver<(Vec<u8>, Instant)> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut file = unsafe { File::from_raw_fd(master) };
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match file.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    if tx.send((buf.clone(), Instant::now())).is_err() {
                        break;
                    }
                }
            }
        }
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
// The sandbox (the tests/loopd_heartbeat.rs harness, + the LOOPD_HB_DRAIN
// scrub — the tee's drain seam must never be settable from the host)
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

/// A green build whose stub BINARY dies with the budget/abort exit (rc=1)
/// after writing the driver's OWN failure marker to stderr — the exact shape
/// src/events.rs's ConsoleSink produces on Event::Aborted. At level 3 the
/// tee streams that marker line verbatim while the death site composes the
/// blocked msg from it — the consistency fixture's arc.
const FAILING_CHUG_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug && touch .chug/STOP-LOOP
echo "chug: abort: iteration budget exceeded" >&2
echo "stub chug: dying with the budget exit" >&2
exit 1
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// The turn-detail arc (level 2): a green build whose stub BINARY plays a
/// real cycle's turn stream — three iteration lines, one OK tool_result
/// with a >80-char preview (the 80-char cap + the `...` truncation marker),
/// one ERROR tool_result with a >80-char preview (errors pass IN FULL —
/// no supervisor-side cap), and a 5-physical-line model text block on
/// stderr (the ~3-line cap + the `...` marker; the launch leg's stderr
/// redirect plants the block in the cycle log exactly as the ConsoleSink
/// would). The sandbox is a git repo with the open T042 row, so the lines
/// carry the item field.
const TURN_DETAIL_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug
printf '%s\n' '{"type":"iteration","ts":"2026-11-10T00:00:00Z","n":1,"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}' >> .chug/events.jsonl
printf '%s\n' '{"type":"iteration","ts":"2026-11-10T00:00:01Z","n":2,"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}' >> .chug/events.jsonl
printf '%s\n' '{"type":"tool_result","ts":"2026-11-10T00:00:02Z","name":"bash","ok":true,"is_error":false,"duration_ms":5,"preview":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA tail"}' >> .chug/events.jsonl
printf '%s\n' '{"type":"iteration","ts":"2026-11-10T00:00:03Z","n":3,"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}' >> .chug/events.jsonl
printf '%s\n' '{"type":"tool_result","ts":"2026-11-10T00:00:04Z","name":"bash","ok":false,"is_error":true,"duration_ms":9,"preview":"build FAILED: the full error text the operator must see whole, uncapped by the dial -- ERR-TAIL-9f2c"}' >> .chug/events.jsonl
echo "[chug] model: first line of the model text block" >&2
echo "second physical line of the block" >&2
echo "third physical line of the block" >&2
echo "fourth physical line of the block" >&2
echo "fifth physical line of the block" >&2
sleep 3
printf 'chug: goal complete\nsummary: the turn-detail stub ran this cycle\n'
touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// The trace arc (level 3): a green build whose stub BINARY writes a few
/// distinct stderr lines (some `[chug]`-shaped, some not — the tee is
/// unfiltered), sleeps so the watcher's 1s polls see them all, then prints
/// the accepted stdout block. With LOOPD_HB_DRAIN=4 the post-cycle appends
/// (the captured stdout block, the verdict stamp) are also streamed before
/// the kill, so the level-3 stream matches the cycle log line for line.
const TRACE_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug
echo "[chug] iteration 1 / 200 (3 messages)" >&2
echo "[chug] model: trace line one -- T359-TRACE-MODEL" >&2
echo "plain stderr trace line two -- T359-TRACE-PLAIN" >&2
echo "[chug] tool bash -> ok" >&2
sleep 3
printf 'chug: goal complete\nsummary: the trace stub ran this cycle\n'
touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// The latency arc (level ≥2): the stub writes ONE unique marker line to
/// stderr (the cycle log's write the fixture times against) IMMEDIATELY at
/// start, then holds the cycle open 4s so the fixture has the window to
/// observe write → stdout without racing the exit.
const LATENCY_CARGO: &str = r#"#!/bin/sh
set -e
[ -n "$CARGO_TARGET_DIR" ] || { echo 'stub cargo: CARGO_TARGET_DIR unset' >&2; exit 1; }
mkdir -p "$CARGO_TARGET_DIR/release"
cat > "$CARGO_TARGET_DIR/release/chug" <<'STUB'
#!/bin/sh
mkdir -p .chug
echo "[chug] model: T359-LATENCY-PROBE-7f3a9c" >&2
sleep 4
printf 'chug: goal complete\nsummary: the latency stub ran this cycle\n'
touch .chug/STOP-LOOP
exit 0
STUB
chmod +x "$CARGO_TARGET_DIR/release/chug"
echo "stub-build: green"
exit 0
"#;

/// The milestone stub's generator (the T356 volume arc, copied from
/// tests/loopd_heartbeat.rs for the level-1 "compact only" fixture): a
/// green build whose stub BINARY plays a real cycle's event stream —
/// `batches` batches of 10 merged iteration lines, one tool_result per
/// batch, a merge commit at the midpoint, then the accepted verdict.
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
# iteration count before the cycle's kill (the mechanism-not-timeouts
# doctrine: widen the window, never the bound).
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

/// The full-supervisor harness (the tests/loopd_heartbeat.rs pattern): the
/// REAL loopd.sh + scripts/, the hermetic ps/lsof stubs first on PATH, the
/// chosen cargo stub, HOME pinned inside the sandbox, the sleep seams at 1s,
/// and the T254 max-loops bound.
struct Sandbox {
    _keep: TempDir,
    root: PathBuf,
}

impl Sandbox {
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
        stub(&root.join("bin/lsof"), PS_STUB);
        stub(&root.join("bin/cargo"), cargo_stub);
        // site-sync must NEVER leave the sandbox (a live site dir on this
        // host would get synced from fixture garbage): point the env default
        // at a path that does not exist (the warn + exit 0 leg).
        fs::write(root.join("NO-SUCH-SITE"), "").expect("site sentinel");
        Sandbox { _keep: keep, root }
    }

    /// The env hygiene + seams shared by both spawn shapes. The verbose
    /// knobs are scrubbed here and passed per-fixture via `extra_envs` (the
    /// T332 hermeticity contract: no host export may flip a fixture) —
    /// including the T359 drain seam.
    fn base_cmd(&self, max_loops: u32, hb_poll: &str) -> Command {
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
            // T356 watcher seam (a 1s poll; at level ≥2 the level itself
            // pins the 1s slice, so the seam is belt-and-braces).
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_HB_POLL", hb_poll)
            // T254 — the fixture-leak fail-safe: the bound is the run's real
            // terminator when no stub plants a STOP file.
            .env("LOOPD_MAX_LOOPS", max_loops.to_string())
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        for var in [
            "CHUG_LOOP_VERBOSE",
            "CHUG_OSC7501",
            "LOOPD_HB_EVERY",
            "LOOPD_HB_POLL",
            "LOOPD_HB_DRAIN",
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
        cmd
    }

    /// Spawn the supervisor with stdout on a REAL pty (the emission channel)
    /// and hand back the child plus the capture receiver. `extra_envs` rides
    /// last — AFTER the removals — so the level dial and the git identity
    /// land (the T354 pattern).
    fn run_on_pty(
        &self,
        max_loops: u32,
        extra_envs: &[(&str, &str)],
    ) -> (Child, mpsc::Receiver<Vec<u8>>) {
        let mut cmd = self.base_cmd(max_loops, "1");
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let (child, master) = spawn_on_pty(&mut cmd);
        let rx = drain_pty(master);
        (child, rx)
    }

    /// The latency fixture's spawn: the timed drain (per-read arrival
    /// timestamps) instead of the EOF drain.
    fn run_on_pty_timed(
        &self,
        max_loops: u32,
        extra_envs: &[(&str, &str)],
    ) -> (Child, mpsc::Receiver<(Vec<u8>, Instant)>) {
        let mut cmd = self.base_cmd(max_loops, "1");
        for (k, v) in extra_envs {
            cmd.env(k, v);
        }
        let (child, master) = spawn_on_pty(&mut cmd);
        let rx = drain_pty_timed(master);
        (child, rx)
    }

    /// Spawn the supervisor the redirected way — stdout PIPED (the nohup /
    /// launchd shape the compat contract is written for).
    fn run_piped(&self, max_loops: u32, extra_envs: &[(&str, &str)]) -> Output {
        let mut cmd = self.base_cmd(max_loops, "20");
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
    /// every cycle log) — the redirected contexts.
    fn redirected_files(&self) -> Vec<PathBuf> {
        let mut files = vec![self.log_path()];
        let state_dir = self.root.join(".chug/loopd");
        if let Ok(entries) = fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("cycle-") {
                    files.push(entry.path());
                }
            }
        }
        files
    }

    /// The cycle log the run just wrote (the single-cycle fixtures have
    /// exactly one).
    fn cycle_log(&self) -> PathBuf {
        let state_dir = self.root.join(".chug/loopd");
        let mut logs: Vec<PathBuf> = fs::read_dir(&state_dir)
            .expect("the loopd state dir")
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("cycle-") && n.ends_with(".log"))
                    .unwrap_or(false)
            })
            .collect();
        logs.sort();
        assert_eq!(logs.len(), 1, "exactly one cycle log expected: {logs:?}");
        logs.remove(0)
    }
}

/// `git init` + a seed commit INSIDE the sandbox — a repo for the watcher's
/// flip leg, WITHOUT an EVALUATION.md: the freshness predicate stays false,
/// so the seeded open row routes the cycle to the eval arm.
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

/// The queue head the seeded sandbox starts with: one open row, so the
/// lines carry the item field (LOOP-SPEC works the queue serially in file
/// order — the FIRST open row is the item the cycle will work).
fn seed_todo_queue(root: &Path) {
    fs::write(
        root.join("TODO.md"),
        "# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n| T042 | fixture item title for the heartbeat line | specs/t042.md | 2 | open | the queue head |\n",
    )
    .expect("write TODO.md");
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

/// (static, the dial) ONE resolver owns the level semantics: the TTY gate
/// is its first leg (a redirected context is level 0 whatever the knob
/// says), unset/empty is 1 (the T356 default — the compat anchor), 0..3 are
/// the levels themselves, and ANY other value degrades to 0 (default-safe —
/// an unreadable knob must never turn detail on). The resolver runs as a
/// PLAIN STATEMENT setting VERBOSE_LEVEL — never a command substitution:
/// inside $() the substituted stdout is the capture pipe, so a `[ -t 1 ]`
/// leg there could only ever see the pipe and the dial would resolve to 0
/// everywhere. The watcher resolves once at birth and pins the 1s poll at
/// level ≥2; the emitter's gate is the level-≥1 contract inline (no fork
/// per line); the header documents the default.
#[test]
fn the_dial_is_one_resolver_with_the_tty_contract_first() {
    let loopd = read("loopd.sh");
    count_eq(&loopd, "loopd_verbose_resolve() {", 1, "the level resolver is defined once");
    let r = fn_body(&loopd, "loopd_verbose_resolve");
    let at_tty = need(&r, "VERBOSE_LEVEL=0\n  [ -t 1 ] || return 0", "the TTY gate is the first leg");
    let at_default = need(&r, "''|1) VERBOSE_LEVEL=1 ;;", "unset/empty is the T356 default");
    let at_zero = need(&r, "0) VERBOSE_LEVEL=0 ;;", "level 0 is the silent arm");
    let at_two = need(&r, "2) VERBOSE_LEVEL=2 ;;", "level 2 is the verbose arm");
    let at_three = need(&r, "3) VERBOSE_LEVEL=3 ;;", "level 3 is the trace arm");
    let at_junk = need(&r, "*) VERBOSE_LEVEL=0 ;;", "any other value degrades to 0");
    assert!(
        at_tty < at_default && at_default < at_zero && at_zero < at_two && at_two < at_three
            && at_three < at_junk,
        "the resolver reads TTY → dial, in the dial's own order"
    );

    // The watcher resolves ONCE at birth (never per poll — no fork per
    // second) and pins the poll at the 1s slice on the streaming levels
    // (the ≤2s latency budget IS the poll cadence).
    let w = fn_body(&loopd, "loopd_hb_watch");
    need(&w, "loopd_verbose_resolve\n  level=$VERBOSE_LEVEL", "the watcher resolves the level once");
    need(&w, "case \"$level\" in 2|3) poll=1 ;; esac", "level ≥2 pins the 1s poll");

    // The emitter's gate: the compact lines are levels ≥1 — unset (=1) and
    // every level above 1; 0 and any unreadable value kill (default-safe).
    let hb = fn_body(&loopd, "loopd_hb_emit");
    count_eq(&hb, "''|1|2|3) ;;", 1, "the compact lines are levels ≥1");
    count_eq(&hb, "*) return 0 ;;", 1, "0 and any unreadable value kill (default-safe)");

    // The header documents the default (the spec's docs req).
    need(
        &loopd,
        "1 compact heartbeat (the DEFAULT on a TTY",
        "the header documents the default (1 on a TTY, 0 otherwise)",
    );
}

/// (static, the mechanism) The verbose legs are latched tails of files the
/// loop already writes — never new logging, never a double-write: the tee's
/// only source is the cycle log the supervisor itself owns, the turn lines'
/// sources are the cycle's own events log and cycle log, both tails are
/// LINE-COUNT-latched (a half-written stream line waits for its newline and
/// is consumed exactly once), the turn-filter's model-block state never
/// survives a cycle, and the drain window is the supervisor's, level-gated
/// and bounded (default 2s — the spec's own latency budget).
#[test]
fn the_verbose_legs_tail_the_files_the_loop_already_writes() {
    let loopd = read("loopd.sh");
    let w = fn_body(&loopd, "loopd_hb_watch");

    // Both tails are line-count-latched (awk's END NR counts only COMPLETE
    // lines — and prints it unpadded, where `wc -l` pads on some platforms
    // and the digit validation would zero a padded count into silence).
    need(
        &w,
        "ev_total=$(awk 'END { print NR }' .chug/events.jsonl 2>/dev/null) || ev_total=0",
        "the events tail latches on the line count",
    );
    need(
        &w,
        "clog_total=$(awk 'END { print NR }' \"$clog\" 2>/dev/null) || clog_total=0",
        "the cycle-log tail latches on the line count",
    );
    // The tool-call tail reads the cycle's OWN events log (one line per
    // tool_result: the tool, the iteration, an 80-char summary; errors
    // in full up to the driver's own preview cap).
    need(&w, "index($0, \"\\\"type\\\":\\\"tool_result\\\"\")", "the tool-call tail keys on the driver's event type");
    need(&w, "substr(prev, 1, 80) \"...\"", "the ok summary is capped at 80 with the truncation marker");
    // The model-text tail caps a block at ~3 lines with the same marker and
    // its state persists across polls (a block split over two polls still
    // caps once).
    need(&w, "print \"[loop] \" pre \"model: ...\"", "the model block's truncation marker");
    need(&w, "statefile=\"$STATE/hb-tail.state\"", "the model-block state persists across polls");
    // The trace: every new cycle-log line VERBATIM — the tee's range print,
    // no filter, no prefix.
    need(&w, "sed -n \"$((clog_seen + 1)),${clog_total}p\" \"$clog\"", "the tee prints the new range verbatim");
    need(&w, "# The trace (level 3): every new cycle-log line VERBATIM", "the tee is the level-3 arm, named");

    // The call site: the watcher rides the launch WITH the cycle log, the
    // tail state is reset before its birth, and the drain window is
    // level-gated with the 2s default.
    need(&loopd, "\n  loopd_hb_watch 200 \"$cycle_log\" &\n", "the watcher rides the launch with the cycle log");
    need(&loopd, "rm -f \"$STATE/hb-tail.state\"", "the tail state never survives a cycle");
    need(&loopd, "loopd_verbose_resolve\n  case \"$VERBOSE_LEVEL\" in", "the drain resolves the level once per cycle");
    need(&loopd, "drain=\"${LOOPD_HB_DRAIN:-2}\"", "the drain window defaults to the spec's 2s budget");
    // The kill trio still trails the launch (the watcher can never outlive
    // the cycle — the backstop now includes the drain window).
    let at_launch = need(&loopd, "chug_out=\"$(CARGO_TARGET_DIR=", "the chug launch");
    let at_drain = need(&loopd, "drain=\"${LOOPD_HB_DRAIN:-2}\"", "the drain window");
    let at_kill = need(&loopd, "kill \"$hb_watch_pid\" 2>/dev/null || true", "the watcher kill");
    assert!(
        at_launch < at_drain && at_drain < at_kill,
        "the drain window must sit between the capture's return and the kill — \
         the tee's last lines get exactly their own latency budget"
    );
}

/// (static, the level-2 volume extension) The T356 volume guard's arithmetic
/// is untouched — the milestone cadence (LOOPD_HB_EVERY=25 against the
/// 200-iteration budget) still bounds the level-1 lines at 8 + 2 per cycle,
/// and the turn-detail legs are keyed OFF that same budget — the level-1
/// surface never grew. (The ≤15 bound is behavioral in
/// tests/loopd_heartbeat.rs; this pin keeps the knobs' arithmetic from
/// drifting while the levels land.)
#[test]
fn the_level_1_volume_arithmetic_is_untouched() {
    let loopd = read("loopd.sh");
    let w = fn_body(&loopd, "loopd_hb_watch");
    count_eq(&w, "''|0|*[!0-9]*) every=25 ;;", 1, "the cadence default");
    count_eq(&w, "''|0|*[!0-9]*) poll=20 ;;", 1, "the poll default");
    need(&w, "shown=$(((n / every) * every))", "the one-line-per-crossed-threshold latch");
}

// ---------------------------------------------------------------------------
// The behavioral fixtures — one per level
// ---------------------------------------------------------------------------

/// (level 0) A real green cycle at `CHUG_LOOP_VERBOSE=0` on a pty shows the
/// operator NOTHING: zero lines of any shape (the scrubbed stream is empty
/// beyond the 7501 dots, which carry no newline of their own). The cycle
/// still completes; the dot still rides (the dot's knob is its own).
#[test]
fn level_0_on_a_pty_produces_zero_lines() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(GREEN_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(4, &[("CHUG_LOOP_VERBOSE", "0")]);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the level-0 sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the level-0 sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the level-0 arc must complete a cycle:\n{log}");

    let scrubbed = scrub_7501(&capture);
    assert!(
        scrubbed.lines().all(|l| l.trim_end_matches('\r').trim().is_empty()),
        "CHUG_LOOP_VERBOSE=0 must show ZERO lines (no heartbeat, no turn \
         detail, no tee): {scrubbed:?}"
    );
    assert!(
        !parse_osc7501(&capture).is_empty(),
        "the dot must be untouched by the line's dial: {capture:?}"
    );
}

/// (level 1) The compact heartbeat only: every line is a T356 `[loop …]`
/// line (no turn detail, no tee), and the whole cycle stays within the 15
/// line volume bound. Explicit `=1` on a pty — the dial's default arm, so
/// this is also the byte-compat anchor.
#[test]
fn level_1_is_the_compact_heartbeat_within_the_volume_bound() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(&milestone_cargo_stub(3, 0, 0));
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    let mut envs: Vec<(&str, &str)> = vec![("CHUG_LOOP_VERBOSE", "1")];
    envs.extend(GIT_IDENTITY);
    let (mut child, rx) = sandbox.run_on_pty(4, &envs);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the level-1 sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the level-1 sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the level-1 arc must complete a cycle:\n{log}");

    let lines = heartbeat_lines(&capture);
    let joined = lines.join("\n");
    assert!(
        lines.len() <= 15,
        "the compact level must stay within the volume bound, got {}:\n{joined}",
        lines.len()
    );
    assert!(
        lines.iter().any(|l| l.contains("cycle 1 routing=eval (kimi-k3)")),
        "the cycle-start line rides: {joined}"
    );
    // COMPACT ONLY: every emitted line is a T356 heartbeat line — no raw
    // cycle-log content, no turn detail, no tee.
    let scrubbed_lines = stream_lines(&capture);
    for line in &scrubbed_lines {
        assert!(
            line.is_empty() || line.starts_with("[loop"),
            "level 1 must be the compact heartbeat ONLY — a non-heartbeat \
             line leaked into the stream: {line:?}"
        );
    }
}

/// (level 2) The turn detail: level 1's lines PLUS the per-turn lines —
/// each tool call as `[loop] T042 iter <n> — tool <name>: <summary, 80>`
/// (the `...` truncation marker past 80), errors IN FULL (no supervisor-side
/// cap — the driver's own preview cap is the ceiling), each model text block
/// capped at ~3 lines with the same marker. And NOT the tee: no raw
/// cycle-log line (the `[chug]` shapes, the `[loopd]` header) leaks through.
#[test]
fn level_2_adds_turn_detail_lines_with_truncation_markers() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(TURN_DETAIL_CARGO);
    init_git_with_seed_commit(&sandbox.root);
    seed_todo_queue(&sandbox.root);
    let mut envs: Vec<(&str, &str)> = vec![("CHUG_LOOP_VERBOSE", "2")];
    envs.extend(GIT_IDENTITY);
    let (mut child, rx) = sandbox.run_on_pty(4, &envs);
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the level-2 sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the level-2 sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the level-2 arc must complete a cycle:\n{log}");

    let lines = heartbeat_lines(&capture);
    let joined = lines.join("\n");

    // Level 1 rides (level 2 = level 1 PLUS turn detail).
    assert!(
        lines.iter().any(|l| l.contains("cycle 1 routing=eval (kimi-k3) item=T042")),
        "the cycle-start line rides with the item field:\n{joined}"
    );
    assert!(
        lines.iter().any(|l| l.contains("cycle done — goal complete")),
        "the cycle-done line rides:\n{joined}"
    );

    // THE TOOL CALL: the ok tool_result's preview is >80 chars; the line
    // caps at 80 with the `...` truncation marker. The iteration number is
    // the cycle's own (the stub wrote three iteration lines).
    let ok_line_prefix = format!("[loop] T042 iter 3 — tool bash: {}", "A".repeat(80));
    let tool_lines: Vec<&String> =
        lines.iter().filter(|l| l.contains("— tool bash: ")).collect();
    assert_eq!(
        tool_lines.len(),
        1,
        "exactly one ok tool-call line (one tool_result):\n{joined}"
    );
    assert!(
        tool_lines[0].starts_with(&ok_line_prefix) && tool_lines[0].ends_with("..."),
        "the 80-char cap with the `...` truncation marker:\n{tool_lines:?}\n(want prefix {:?})",
        ok_line_prefix
    );

    // ERRORS IN FULL: the error tool_result's preview is >80 chars too, and
    // NONE of it may be capped — the whole preview (through the ERR-TAIL
    // sentinel, the string's last characters) reaches the line.
    let err_line: Vec<&String> =
        lines.iter().filter(|l| l.contains("tool bash ERROR: ")).collect();
    assert_eq!(err_line.len(), 1, "exactly one error tool line:\n{joined}");
    assert!(
        err_line[0].contains("build FAILED: the full error text the operator must see \
                               whole, uncapped by the dial -- ERR-TAIL-9f2c"),
        "the error preview passes IN FULL (the tail sentinel proves no \
         supervisor-side cap):\n{err_line:?}"
    );

    // THE MODEL TEXT BLOCK: 5 physical lines in the cycle log → 3 emitted
    // (the ~3-line cap) + one `...` truncation marker; lines 4-5 swallowed.
    let model_lines: Vec<&String> = lines.iter().filter(|l| l.contains(" model: ")).collect();
    assert_eq!(
        model_lines.len(),
        4,
        "a 5-line model block emits 3 lines + the truncation marker:\n{joined}"
    );
    assert_eq!(
        model_lines[0].as_str(),
        "[loop] T042 model: first line of the model text block",
        "the block's first line rides under the [loop] prefix:\n{joined}"
    );
    assert_eq!(
        model_lines[1].as_str(),
        "[loop] T042 model: second physical line of the block",
        "continuation line 2 rides:\n{joined}"
    );
    assert_eq!(
        model_lines[2].as_str(),
        "[loop] T042 model: third physical line of the block",
        "continuation line 3 rides:\n{joined}"
    );
    assert_eq!(
        model_lines[3].as_str(),
        "[loop] T042 model: ...",
        "the truncation marker replaces the rest of the over-long block:\n{joined}"
    );

    // AND NOT THE TEE: no raw cycle-log shape leaks — the `[chug]` lines,
    // the `[loopd]` header/verdict lines, and the raw model block stay off
    // the stream (the turn lines are the REWRITTEN shapes, never echoes).
    for line in stream_lines(&capture) {
        assert!(
            line.is_empty() || line.starts_with("[loop"),
            "level 2 must not leak a raw cycle-log line (that is level 3's \
             tee): {line:?}"
        );
    }
}

/// (level 3) The trace: the cycle's full output streamed — every new
/// cycle-log line teed to stdout as it lands, unfiltered and unprefixed,
/// so the stream IS the cycle log (line for line, in order), including the
/// header, the raw `[chug]` shapes, the captured stdout block, and the
/// verdict stamp. LOOPD_HB_DRAIN=4 gives the tee its bounded window over
/// the cycle-end appends (the default is the spec's own 2s budget).
#[test]
fn level_3_streams_the_full_cycle_log() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(TRACE_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(
        4,
        &[("CHUG_LOOP_VERBOSE", "3"), ("LOOPD_HB_DRAIN", "4")],
    );
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the level-3 sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the level-3 sandbox must exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the level-3 arc must complete a cycle:\n{log}");

    let cycle_log = fs::read_to_string(sandbox.cycle_log()).expect("read the cycle log");
    // Level 3 is level 1 PLUS the tee, so the stream carries the heartbeat
    // emission lines BESIDE the trace — the tee's contribution is the
    // stream minus those emission shapes (the stub writes no [loop]-shaped
    // content of its own, so the subtraction is exact here). What remains
    // must BE the cycle log: line for line, in order, no filter, no prefix.
    let streamed: Vec<String> = stream_lines(&capture)
        .into_iter()
        .filter(|l| !is_heartbeat_shape(l))
        .collect();
    let logged: Vec<&str> = cycle_log.lines().collect();
    assert_eq!(
        streamed, logged,
        "the level-3 tee must BE the cycle log — line for line, in order, \
         no filter, no prefix:\nSTREAM: {streamed:?}\nLOG:    {logged:?}"
    );
}

/// (gating req) Piped stdout at level 3 → ZERO bytes. The dial's TTY gate
/// is conjunctive: a redirected context is level 0 whatever the knob says —
/// even the full trace never reaches a pipe (log parsers must never see it).
#[test]
fn piped_level_3_produces_zero_bytes() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(TRACE_CARGO);
    let out = sandbox.run_piped(4, &[("CHUG_LOOP_VERBOSE", "3")]);
    assert!(
        out.status.success(),
        "the piped level-3 arc must exit cleanly: {:?}",
        out.status
    );
    assert!(
        out.stdout.is_empty(),
        "piped stdout received bytes at level 3 — the TTY gate is the whole \
         contract, the trace must never reach a redirect: {:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the piped arc must still complete its cycle:\n{log}");
    for file in sandbox.redirected_files() {
        let bytes = fs::read(&file).unwrap_or_default();
        assert!(
            !carries_heartbeat_line(&bytes),
            "{} carries a heartbeat line (the supervisor's own `[loopd …]` \
             cycle-log stamps are its file writes, pre-T359 — the EMISSION \
             shapes are the contract)",
            file.display()
        );
        assert_eq!(
            esc_count(&bytes),
            0,
            "{} (a redirected context) carries an escape byte",
            file.display()
        );
    }
}

/// (latency req) A cycle-log line reaches stdout ≤2s after it is written, at
/// level ≥2. The stub writes ONE unique marker to stderr the moment it
/// starts (the launch leg's redirect lands it in the cycle log), then holds
/// the cycle open. The fixture polls the cycle log for the marker (the
/// write, observed on disk at a 10ms cadence — the observation may lag the
/// write by up to one poll, so the measured elapsed UNDERSTATES the true
/// latency by at most one poll) and times the marker's arrival on the pty
/// (the chunk that completes the emission). At level ≥2 the watcher polls
/// on the 1s slice, so the bound holds with slack.
#[test]
fn a_cycle_log_line_reaches_stdout_within_2s() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(LATENCY_CARGO);
    const MARKER: &[u8] = b"T359-LATENCY-PROBE-7f3a9c";
    let (mut child, rx) = sandbox.run_on_pty_timed(4, &[("CHUG_LOOP_VERBOSE", "2")]);

    // The write: poll the cycle log for the marker at a 10ms cadence. The
    // state dir itself may not exist yet (the supervisor creates it in its
    // first seconds) — the same poll loop waits for it.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut t_write: Option<Instant> = None;
    while Instant::now() < deadline {
        if let Ok(entries) = fs::read_dir(sandbox.root.join(".chug/loopd")) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("cycle-"))
                    .unwrap_or(false)
                    && fs::read(&path)
                        .unwrap_or_default()
                        .windows(MARKER.len())
                        .any(|w| w == MARKER)
                {
                    t_write = Some(Instant::now());
                    break;
                }
            }
        }
        if t_write.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let t_write = t_write.expect("the marker never reached the cycle log");

    // The emission: the chunk that completes the marker on the pty.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut t_seen: Option<Instant> = None;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok((chunk, at)) => {
                if find(&chunk, MARKER).is_some() {
                    t_seen = Some(at);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(e) => panic!("the pty drain thread died: {e}"),
        }
    }
    let t_seen = t_seen.expect("the marker never reached stdout");

    let elapsed = t_seen.duration_since(t_write);
    assert!(
        elapsed <= Duration::from_secs(2),
        "a cycle-log line must reach stdout within 2s at level ≥2 (the \
         streaming levels' latency budget), took {elapsed:?}"
    );

    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the latency sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the latency sandbox must exit cleanly: {status:?}");
    let log = sandbox.read_log();
    assert!(log.contains("cycle OK:"), "the latency arc must complete a cycle:\n{log}");
}

/// (gating req, the consistency contract at the trace level) At level 3 the
/// 7501 blocked msg STILL equals the level-1 blocked line's reason — one
/// source string, two destinations, unchanged by the dial — while the tee
/// streams the driver's own failure marker verbatim (errors in full, the
/// raw trace) BESIDE the blocked line (the supervisor's transition), each
/// keeping its own shape.
#[test]
fn the_blocked_msg_still_equals_the_blocked_line_at_level_3() {
    let _timing = testsupport::timing_guard();
    let _t172_load = t172_load_lock::family_guard("loopd-verbose");
    let sandbox = Sandbox::new(FAILING_CHUG_CARGO);
    let (mut child, rx) = sandbox.run_on_pty(
        4,
        &[("CHUG_LOOP_VERBOSE", "3"), ("LOOPD_HB_DRAIN", "4")],
    );
    let status = wait_bounded(&mut child, Duration::from_secs(90));
    let status = status.expect("the level-3 failing sandbox supervisor hung past the 90s cap");
    assert!(status.success(), "the failing sandbox must still exit cleanly: {status:?}");
    let capture = rx
        .recv_timeout(Duration::from_secs(25))
        .expect("the pty drain thread finished");
    let log = sandbox.read_log();
    assert!(
        log.contains("cycle ended WITHOUT goal complete"),
        "the failure arc must run its failure branch:\n{log}"
    );

    let expected = "budget: iteration budget exceeded";
    let records = parse_osc7501(&capture);
    let blocked = records
        .iter()
        .find(|r| r.state == "blocked")
        .expect("the failure arc must emit a blocked dot");
    assert_eq!(
        blocked.msg.as_deref(),
        Some(expected),
        "the blocked msg must carry the cycle-log marker's reason:\n{capture:?}"
    );

    // The heartbeat line matches the dot byte for byte (the consistency
    // contract rides the same single composition — at the trace level too).
    let lines = heartbeat_lines(&capture);
    let blocked_lines: Vec<&String> = lines.iter().filter(|l| l.contains("BLOCKED:")).collect();
    assert_eq!(
        blocked_lines.len(),
        1,
        "exactly one BLOCKED line on the level-3 path:\n{capture:?}"
    );
    let reason = blocked_lines[0]
        .split_once("BLOCKED: ")
        .map(|(_, rest)| rest.to_string())
        .expect("the BLOCKED: prefix");
    assert_eq!(
        reason, expected,
        "THE CONSISTENCY CONTRACT at level 3: the heartbeat's reason and the \
         7501 msg are the same derived string"
    );

    // The tee streamed the driver's own failure marker VERBATIM (the raw
    // trace), and the streamed trace never displaced the transition lines.
    assert!(
        stream_lines(&capture).iter().any(|l| l == "chug: abort: iteration budget exceeded"),
        "the level-3 tee must stream the driver's abort marker verbatim \
         (errors in full):\n{capture:?}"
    );
}

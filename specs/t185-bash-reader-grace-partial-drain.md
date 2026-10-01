# T185 — bash reader-grace discards already-read output when a grandchild holds the pipe

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

`run_shell` (src/tools.rs ~line 1104) spawns `sh -c` with piped
stdout/stderr and one reader thread per pipe:

    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        let _ = out_tx.send(buf);   // ONE send, at EOF only
    });

The caller side waits with `recv_capped` (~line 1253):

    match rx.recv_timeout(READER_GRACE /* 5s */) {
        Ok(buf) => (buf, true),
        Err(mpsc::RecvTimeoutError::Timeout) => (Vec::new(), false),
        ...                                    // ^ buffer DISCARDED
    }

When a process outlives the direct `sh` child while holding a pipe
write-end open (an orphaned grandchild — e.g. an alarm-killed cargo's
rustc, a backgrounded process the command forgot to redirect), the
reader thread stays blocked in `read_to_end`, EOF never comes, the 5s
grace expires, and the ENTIRE buffer that side already accumulated —
potentially every byte of the command's real output — is thrown away.
The tool result returns empty for that side plus
`(output truncated: reader did not drain)`. The doc comment above
`run_shell` ("if a reader has not seen EOF after the grace period,
whatever output was captured is returned with a truncation note") is
FALSE in the timeout leg — nothing was handed over the channel yet, so
nothing is returned.

Evidence (cycle-85 eval §2.3): 13 fires across the cycles-83–84 delta
(orchestrator streams ×7, validator streams ×6, both model families) —
escalated from the cycle-83 triage watch-list (d1790836694-10, one
instance). Each fire costs a 5s stall + total loss of that side's
output + usually a re-run iteration. The orphan-holds-pipe shape is
already a known test scenario (src/tools.rs ~line 1947).

estimate: ~130 changed lines (src ~50 + tests ~80).

## Requirements

1. **Already-read bytes survive.** Restructure so output read before
   the grace expiry is returned: readers forward what they have read
   SO FAR (e.g. chunked sends as reads complete) and the receive side
   accumulates until channel-disconnect (clean EOF) or grace expiry
   (keep what arrived, mark undrained). The current "one send at EOF,
   discard on timeout" shape goes away.
2. **Bounds unchanged.** Total wait after child exit/kill stays ≤
   READER_GRACE per side (the leaked-thread tradeoff stays — blocking
   the driver remains the non-negotiable); the bash timeout and
   process-group kill semantics are untouched.
3. **Honesty legs byte-identical.** The note texts
   `(output truncated: reader did not drain)` and
   `(output truncated: reader did not drain after kill)` keep their
   exact bytes and their firing conditions (note present iff a side
   failed to drain); the fast path (both sides EOF within grace) is
   byte-identical to today for the same command.
4. **Doc comment fixed** to state the new truth (partial output kept,
   note marks the undrained side).

## Tests

- The orphan leg (a grandchild inherits and holds the stdout pipe after
  the direct child exits — the existing ~1947 scenario): the result now
  CONTAINS the bytes the command wrote before exiting AND carries the
  drain note; assert the pre-fix code would have returned them empty
  (RED-proof stated in the commit message: run the new assertion
  against the unfixed receive path and show it failing).
- Symmetric stderr leg (orphan holds stderr; stdout drained clean →
  stdout bytes intact, note present).
- Clean fast-path leg: both sides drain → no note, output byte-identical.
- A bound leg: total wall after child exit never exceeds a small
  multiple of READER_GRACE even with a never-EOF orphan (no deadlock;
  leaked-thread shape retained).
- Existing output-cap/truncation (`truncate_middle`) and timeout tests
  stay green unmodified.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, full
  `cargo test` green.
- RED-proof in the commit message (requirement: the orphan leg's new
  bytes-survive assertion fails against the pre-fix code).
- No change to the note strings, READER_GRACE value, timeout, or kill
  semantics (the diff shows them untouched outside the receive
  restructure and the doc comment).

## Out of scope

- Changing READER_GRACE's value or the note wording.
- Reaping orphaned grandchildren (a driver-exit-hygiene question the
  cycle-85 eval rejected as unproven — d1790856539-8 / EVALUATION.md
  cycle-85 §2.6).

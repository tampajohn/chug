# T56 — README: document loopd self-re-exec + dedupe the output_truncated double description

One concern: README's "Continuous mode (`loopd.sh`)" section is silent on
T50's self-re-exec (user-visible supervisor behavior: merged `078cb77`,
active in every supervisor started since), and the Autonomous-mode events
bullet re-explains the `output_truncated` mechanism ~30 lines after the
Truncated-output advisory bullet already defined it — two paraphrases of
one mechanism, a drift surface (cycle-24 eval §6 b/c).

## Repo context

- T50 (merged `078cb77`, cycle 23): loopd fingerprints its own script
  (`SELF_CKSUM`) before the cycle loop and re-execs itself BETWEEN cycles
  when the file changes — merged loopd edits now activate without an
  operator restart; a pending STOP still wins at the while condition. The
  T50 merge deferred README to "in-script + spec", leaving the
  user-facing loopd section documenting the pidfile refusal, HALTED, the
  digest refresh and target-shared caching but never the re-exec.
- `README.md` Autonomous mode: the "Truncated-output advisory" bullet
  defines the mechanism (response hit the API output-token ceiling →
  loop injects the chunking-remedy message → one `output_truncated` line
  per advisory). The "Events log" bullet later re-explains it
  ("output-truncated advisories (one `output_truncated` line per injected
  advisory — a response hit the API output-token ceiling and the loop
  named the chunking remedy)").
- T51 (merged `372f920`) is the style precedent: trim spec-grade
  duplication, keep every user-facing semantic, integrate — don't append.

## Requirements

1. **Document the re-exec** in the "Continuous mode (`loopd.sh`)"
   section, INTEGRATED into the existing flow (the paragraph covering
   state/pidfile/HALTED is the natural home): one or two sentences —
   the supervisor fingerprints its own script and re-execs itself between
   cycles when it changes, so loopd edits merged to main activate without
   an operator restart (a pending STOP still wins). No spec-grade
   mechanism detail (cksum, same-pid guard) — that lives in
   `specs/t50-loopd-self-reexec.md`.
2. **Dedupe the output_truncated double description:** the Events-log
   bullet keeps its field mention (`output-truncated advisories (one
   `output_truncated` line per injected advisory)`) but drops the
   repeated mechanism gloss (`— a response hit the API output-token
   ceiling and the loop named the chunking remedy`); the
   Truncated-output advisory bullet remains the single definition.
3. README only; no other file; no other section touched (cycle-23 carry
   iii: req-4 read strictly — the intro line and every other section stay
   byte-identical).

## Tests

Static pins are not warranted for prose this size (T51 precedent: spec
check + orchestrator gates sufficed). The spec check below is the guard.

## Acceptance

- `grep -c 're-exec' README.md` ≥ 1 inside the loopd section.
- The string `output-token ceiling` occurs EXACTLY once in README.md
  (today: twice).
- README stays accurate about everything else (orchestrator cold-read of
  the two edited regions).

check: test "$(grep -c 'output-token ceiling' README.md)" -eq 1 && grep -q 're-exec' README.md && ! grep -q 'one exception' README.md

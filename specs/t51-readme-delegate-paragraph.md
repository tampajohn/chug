# T51 — README delegate paragraph: fix the stale "one exception" + trim spec-grade detail

check: ! grep -q "one exception" README.md && grep -q "two documented exceptions" README.md

## Context

Cycle-18's README audit found the Tools intro calling delegate "the one
documented exception" to cwd sandboxing — stale since T37 made web_fetch
the second. T41 fixed the INTRO (now: "delegate and web_fetch are the two
documented exceptions"). But the delegate PARAGRAPH still says "Delegate
paths are the one exception to cwd sandboxing" — the drift moved, it didn't
die. The sandbox exception is now stated three times in one section (intro,
delegate paragraph, web_fetch paragraph), once wrongly.

Separately, the delegate paragraph has accreted to ~15 lines and carries
spec-grade mechanism detail that belongs to (and already lives in)
`specs/t23-delegate-tool.md`: own-process-group/SIGHUP-ignored/nohup-parity
spawn semantics, bounded ≤64KiB tail reads. The README's Tools section is a
user-facing reference: it should carry what the tool DOES (two actions,
required/optional args, defaults, wait_secs semantics, the sandbox
exception) and not the kernel-level spawn mechanics.

Docs-only row; cycle-22 eval F1 / §6(b).

## Requirements

1. Remove or rewrite the delegate paragraph's stale "the one exception to
   cwd sandboxing" sentence so the exception is stated accurately (the
   intro's "two documented exceptions" line already carries the fact; the
   delegate paragraph may keep one short consistent clause — e.g. that its
   absolute `cwd`/`spec` target child worktrees outside your own cwd — but
   must not say "one exception" or otherwise contradict the intro).
2. Trim the spec-grade mechanism detail from the delegate paragraph
   (process-group/SIGHUP/nohup-parity spawn semantics; the ≤64KiB bounded
   tail-read bound). Keep every user-facing semantic: two actions;
   launch's required/optional args and the 40/35 defaults; the optional
   `--max-tokens` passthrough (T39); launch returns at spawn and never
   waits; status reports liveness + events summary + log tail; wait_secs
   long-poll with early return on state change or liveness flip and the
   600s max; worktree creation/building/harvest/kill stay with your bash.
3. The README's `delegate` schema summary (the tool listing line and the
   paragraph) must remain TRUE after the trim — no claim removed that
   isn't carried elsewhere in the README or isn't spec-grade.
4. No other README section edited. No code, spec, or doctrine file edited.
   Net effect: README line count drops slightly (target −3 to −8 lines;
   this is a target, not a hard bound — truth beats brevity).

## Tests

- None new (docs-only). Existing suite must stay green: `cargo test` (with
  the T47 `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared`
  export the goal carries) — README content is pinned by at least one
  src/tools.rs test reading README.md, so the suite is the regression net
  for accidental structural damage.

## Acceptance

- The spec's check line passes from the worktree:
  `! grep -q "one exception" README.md && grep -q "two documented exceptions" README.md`.
- The delegate paragraph retains every user-facing semantic listed in
  requirement 2 (reviewer checklist).
- Gates green.

## Notes

- Validation: OPTIONAL per LOOP-SPEC §2 step 4 (docs-only; T16/T31/T35
  precedent — orchestrator gates + diff review suffice).
- Out of scope: restructuring the Tools section, touching the web_fetch
  paragraph (its "Like delegate..." clause is accurate and may stay),
  shortening the events-log or MCP blocks (audited at cycle 22 §6(d) —
  justified).

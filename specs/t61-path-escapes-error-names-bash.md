# T61 — `resolve_safe` error string names the `bash` fallback at fire time

check: cargo test && cargo clippy --all-targets -- -D warnings

## Repo context

`tool error: path escapes cwd: /tmp/…` is the digest's top recurring
tool-error class and still fires ~1×/orchestrator-cycle AFTER T41
(cycle-26 eval §3: cycles 22–25 show 1, 0, 0, 1; glm children add more
from /tmp worktrees). T41 (`7170fb4`) put the remedy — "cross-tree
reads/writes go through `bash`" — into the tool DESCRIPTIONS and path
property strings, but the fire-time ERROR string itself is still the
bare `path escapes cwd: {path}` from `resolve_safe` (`src/tools.rs`).
The model reads a description once at turn 0; the error surface is what
it sees at the moment of need. Every occurrence self-recovers in one
iteration via bash, so this is a compounding ~1–2 iterations/cycle DX
cost, not a correctness issue.

## Requirements

1. `resolve_safe`'s refusal message gains a suffix naming the fallback,
   e.g. `path escapes cwd: {path} — cross-tree paths go through bash`
   (implementer may adjust the connective punctuation; the load-bearing
   tokens are the existing `path escapes cwd: {path}` prefix —
   byte-identical — plus the word `bash` naming the escape hatch). The
   message must stay a single line.
2. The suffix is added at the error-construction site(s) in
   `resolve_safe` only — the behavior (which paths refuse) is unchanged.
3. The existing pin `resolve_safe_error_names_path_escapes_cwd` (which
   asserts `starts_with("path escapes cwd: ")`) keeps passing and gains
   an assertion that the message names `bash`.
4. Nothing else changes: no description text (T41's pins are
   byte-sensitive), no other error strings.

## Tests

- Updated `resolve_safe_error_names_path_escapes_cwd`: prefix unchanged,
  `bash` named, single-line.
- A second leg (or the same test) covering the other refusal path if
  `resolve_safe` constructs the error at more than one site — both
  refusal messages name `bash`.
- Full suite + clippy green.

## Acceptance

- `check:` passes in the impl worktree.
- Non-vacuousness: reverting the suffix fails the updated pin
  (implementer hand-checks, then reverts).
- No `|` in the TODO row's notes cell (T40).

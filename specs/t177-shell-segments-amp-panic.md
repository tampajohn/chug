# T177 — shell_segments `&&&` slice-panic (T164 lint robustness)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test todo_consistency

## Repo context

tests/todo_consistency.rs's `shell_segments` (:277-306) splits a spec
`check:` line on shell separators. The `&&` arm matches a second `&` via
`line[i + 1..].starts_with('&')` and returns width 2, but — unlike the
`||` arm, which calls `chars.next()` — it does NOT consume the second
`&`. The next loop iteration therefore sees that second `&` again at
`i+1`, and for input containing `&&&` it matches the `&&` guard once more
(`line[i+2..]` starts with `&`) and computes
`segments.push((sep, &line[start..i]))` with `start = i_prev + 2 > i` —
a slice panic (`byte index 2 is out of bounds` / start > end). The T164
validator named this a non-blocking observation ("latent slice-panic on a
malformed `&&&`"); the cycle-79 eval confirmed it by code read
(EVALUATION.md §2.5). A malformed check line crashes the corpus lint with
an opaque panic instead of a lint finding — and todo_consistency runs in
every docs-only gate floor, so the crash surface is every cycle's gates.

estimate: ~40 changed lines (a ~3-line guard/consume fix + 2 regression
tests + comment; tests-only).

## Requirements

1. Fix the tokenizer so ANY `&` run is handled without panic: make the
   `&&` arm consume the second `&` (the `||` arm's shape) — a trailing
   lone `&` and runs like `&&&` / `&&&&` tokenize deterministically
   (child documents the chosen segmentation for odd runs in the commit
   message; `&&&` must not panic and must not silently merge real
   segments in the common `a && b` case).
2. Zero behavior change for every well-formed input: the existing
   todo_consistency suite (16 tests) stays green byte-for-byte — the fix
   touches only the malformed-input path.
3. The same audit glance at the `||` and `;` arms for the same class;
   if another arm shares the bug shape, fix it in the same commit and
   name it (the sweep-the-family doctrine — one class, one round).
4. Regression tests: (a) `&&&`, `&&&&`, trailing `&`, and `& ` at end of
   line tokenize without panic; (b) a `check:` line containing `&&&`
   produces lint segments (any deterministic segmentation) instead of
   crashing `cargo test --test todo_consistency`; (c) the well-formed
   `a && b` segmentation is unchanged (two segments, `&&` separator).

## Tests

`cargo test --test todo_consistency` — the new regression tests FAIL on
the unfixed tokenizer (RED-prove (a): state the panic message in the
commit message) and pass after.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `git diff main -- 'src/*.rs'` empty (tests-only).

## Out of scope

- Changing lint rules/semantics for well-formed lines, the T164 carried
  leg-b/`||` observations (weighed and rejected at the cycle-79 eval),
  any production src/ code.

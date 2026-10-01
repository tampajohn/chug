# T181 — bash tool description: BSD-sed has no GNU `,+N` range form

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --bin chug bash_description

## Repo context

The cycle-83 eval (§2.4) found the T22-class firing again one level
down: TWO orchestrator streams in the delta (cycle-80 glm
`events-20260930-050917`, cycle-81/82 kimi `events-20260930-202447`)
show `sed: N: ",+Np` errors — models reach for GNU sed's `,+N`
range form ("print N lines from the match") on macOS, where sed is
BSD and rejects it. Both recovered in 1–2 iterations (awk or
`sed -n 'N,Mp'`), but the T22 precedent is exact: a recurring
cross-platform coreutils trap across BOTH model families gets one
sentence in the bash tool description — the only universal surface
every child sees (impl children never read META-SPEC).

The bash description already carries T22's macOS-timeout mirage note;
this row appends ONE sibling sentence. The live description is
assembled in src/tools.rs (the `bash` schema builder); T22's pin
`bash_description_warns_macos_has_no_timeout_and_pins_perl_alarm_idiom`
asserts the LIVE `tool_schemas()` output in tools.rs's test module.

estimate: ~25 changed lines (one description sentence + one pin leg /
pin-file assertions). The sentence+pin shape prices ~25 naive →
~40-60 landed (cycle-83 calibration); still trivially small.

## Requirements

1. Append ONE sentence to the bash tool description, immediately
   after the T22 timeout sentence, carrying these load-bearing tokens
   verbatim: `` `,+N` `` and `BSD` and one working alternative idiom
   (`awk` or `sed -n 'N,Mp'` with absolute line numbers). Example
   shape (wording may vary, tokens may not): "macOS sed is BSD sed:
   GNU range forms like `,+N` do not exist — use `awk` or
   `sed -n 'N,Mp'` with absolute line numbers."
2. No other description text changes; the 120s/`CHUG_BASH_TIMEOUT`
   wording and sentence count around it stay intact except for the
   +1 sentence.
3. Pin: extend (or add a sibling of) the T22 pin in tools.rs's test
   module asserting the LIVE `tool_schemas()` bash description
   contains the `,+N` token, the BSD token, and an alternative-idiom
   token — assert against the live schema, not a copied literal
   (T22's non-vacuousness shape). RED-proof: revert the description
   sentence → the pin fails (record in the commit message).

## Tests

The pin legs above; plus the existing T22 pin must still pass
unmodified (the new sentence must not break its sentence-count or
final-position assertions — if T22's pin asserts an exact sentence
count, extend it by exactly 1 in the same commit).

## Acceptance

- `cargo test --bin chug bash_description` green; full
  `cargo test --bin chug` green (the description is src-carried;
  no README/docs surface in this row); clippy `-D warnings` clean.
- One commit; no TODO.md/LEDGER.md edits.

## Out of scope

- Sweeping other GNU/BSD deltas (install-name, `-i` syntax, date
  flags) — one recurring trap per row, T22's shape. File new rows
  from future digest evidence.

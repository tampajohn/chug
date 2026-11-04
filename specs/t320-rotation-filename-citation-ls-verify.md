# t320 — rotation-filename citation compose-time `ls`-verification (the filename-ts misquote family)

One concern: corridor texts (eval bodies, wrap one-liners, hand-offs,
state open-threads) cite rotated events files by name
(`events-<ts>.jsonl`), and the citation is composed from memory or
derived from another stream's timestamp — never verified against the
tree. The filename-ts is the SUCCESSOR launch's timestamp (the
rotation mechanism: a stream's events file is renamed when the NEXT
run starts in the same cwd), so a computed or remembered citation
drifts by seconds from the true rotated name, or points at a different
stream's file outright. THREE committed sightings since the cycle-2923
prose-correction precedent, every figure byte-TRUE, the machine never
reading the name:

1. The 709th eval body (d6d93b4)'s 677th-establishment citation names
   `events-20261104-104607` for the 2947 stream — that is the 2943
   stream's file (87/1,556); the 2947 stream's TRUE file is
   `events-20261104-114931` (97/2,279 re-derived CONCURS). The same
   body's OWN 678th-establishment citation was CORRECT
   (`events-20261104-142147` for the 2951 stream) — the error is
   citation-level, not comprehension-level.
2. The cycle-2956 one-liner's compaction mis-attaches
   `events-20261104-104607` to the 678th establishment (the 2951
   stream) — the 709th body had 142147 correct; the compaction
   introduced a NEW misquote the body did not carry.
3. The 710th eval body (302cc62)'s 677th backstop hand-off names
   `events-20261104-114929` — 2 seconds off the TRUE 114931.

Detection cost at each sighting: the reader identifies the stream by
content (run_start) and re-derives the datum anyway — the figure is
never wrong — but the citation misleads a cold reader and compounds:
sighting (2) was itself born by copying sighting (1)'s wrong name into
the one-liner. This spec lands the compose-time rule that kills the
class at the authoring surface.

## Repo context

- The rotation mechanism: when a NEW `chug run` starts in a cwd, the
  previous live `.chug/events.jsonl` is rotated to
  `.chug/events-<ts>.jsonl` where `<ts>` is the NEW run's launch
  timestamp (YYYYMMDD-HHMMSS, UTC). A stream is therefore identified by
  the file whose `run_start` begins its own interval; the filename ts
  belongs to its SUCCESSOR. Any citation computed from the stream's
  OWN anchors (first/last iteration timestamps) is systematically
  wrong — the ts matches no anchor of the stream it names.
- The precedent class: the cycle-2923 carried-file-reference mis-slot
  rode as a prose-correction near-miss ("the figure byte-TRUE, the
  naming cosmetic — corrected forward per T70"). This spec's three
  sightings are the class's repeat evidence — the trigger for the
  doctrine row per the repeated-class filing model (the T299
  BSD-flag family's shape: first fire seeds the watch, the repeat
  files the row).
- The doctrine home: META-META-SPEC.md's probe-discipline
  neighborhood — beside the T284 probe-hygiene paragraph (exit codes
  and accessor shapes) is the natural slot: the same compose-time
  layer (the probe/citation composed before any read runs), one new
  paragraph immediately AFTER the T284 paragraph.
- The T285 pin-surface probe: `grep -rn 'matches("' tests/*.rs` — zero
  matches across tests/. The META-META-SPEC-reading pin files (they
  read the doctrine from the runtime checkout):
  tests/eval_outcomes_carry.rs, tests/eval_state_delta.rs,
  tests/loop_spec_cheap_exit.rs, tests/loop_spec_check_wall.rs,
  tests/loop_spec_doctrine_prune.rs,
  tests/loop_spec_eval_read_path_pointer.rs,
  tests/loop_spec_recovery.rs, tests/loop_spec_scoped_walk.rs,
  tests/loop_spec_validation_lane.rs, tests/todo_consistency.rs.
  These carry exact-count census pins ("must state {what} exactly
  once") and positional pins over named doctrine sentences. The
  composed paragraph's tokens to window-check BEFORE finalizing:
  `rotation`, `filename`, `citation`, `events-`, `ls`, `successor`.
  Reconciliation rule per T285: move the count with the pin's own
  contract-change comment, or reword the paragraph to dodge; every
  perturbed pin is declared in the impl child's commit message.

## Requirements

1. ONE new paragraph in META-META-SPEC.md, immediately AFTER the T284
   probe-hygiene paragraph ("**Probe hygiene (T284, exit codes and
   accessor shapes).**"), carrying this rule (the mandated text's
   shape, ~6-9 lines):
   **Rotation-filename citation hygiene (T320, ls-verify at compose
   time).** A rotated events-filename citation
   (`events-<ts>.jsonl`) in corridor text — an eval body, a wrap
   one-liner, a hand-off, a state open-threads bullet — is verified
   against the tree at compose time, never composed from memory or
   derived from the stream's own anchors: the filename-ts is the
   SUCCESSOR run's launch timestamp (the rotation mechanism), so a
   computed ts drifts by seconds and a remembered name can point at a
   different stream's file. The compose-time act: probe
   `ls .chug/events-<candidate>.jsonl` (existence) and, when the
   citation binds a figure, confirm the stream by content
   (`jq -c 'select(.type=="run_start") | .ts' <file>`) BEFORE the
   commit — or generate the citation from the `ls` output captured
   into a shell variable, never literal-compose the name. The class
   evidence: three committed sightings — the 709th eval body's 677th
   citation (104607 for the 2947 stream, the 2943 stream's file), the
   cycle-2956 one-liner's 678th mis-attachment (104607 for the 2951
   stream, the body correct at 142147), the 710th eval body's backstop
   hand-off (114929 vs the TRUE 114931) — every figure byte-TRUE, the
   machine never reading the name; the cycle-2923 carried-file
   mis-slot is the precedent near-miss. The rule stays a compose-time
   habit, NOT a gate: a text-level detector over citation prose is the
   brittle-awk class, rejected.
2. The paragraph's tokens are window-checked against the pin files
   named in repo-context BEFORE finalizing (the T285 probe); every
   perturbed exact-count pin is reconciled in the same commit and
   declared in the commit message.
3. No other repo file changes.

## Tests

`cargo test --release` on the pin-file battery below — all green (the
current pins + any reconciled counts).

## Acceptance

- META-META-SPEC.md carries the new T320 paragraph immediately after
  the T284 paragraph, shaped on the requirement above (the rule, the
  compose-time act, the three-sighting evidence, the not-a-gate
  boundary).
- Every pin file in the repo-context battery is green; any reconciled
  exact-count pin carries its contract-change comment and is declared
  in the commit message.
- No other repo file changes.

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs; grep -q 'Rotation-filename citation hygiene (T320' META-META-SPEC.md && cargo test --release --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_scoped_walk --test loop_spec_validation_lane --test todo_consistency --test internal_info_lint -- --test-threads=4

estimate: ~20 changed lines (one doctrine paragraph + pin reconciliations if any)

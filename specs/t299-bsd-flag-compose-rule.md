check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -qF 'bsd-flag-compose' LOOP-SPEC.md && grep -qF '[-belnstuv]' LOOP-SPEC.md && cargo test --release --test todo_consistency --test loop_spec_recovery --test loop_spec_scoped_walk --test internal_info_lint

# T299 — LOOP-SPEC compose-time BSD/GNU portability rule (bsd-flag-compose family, 2nd-fire trigger)

## One concern

The host's userland is **macOS BSD**, and ad-hoc bash legs keep getting
composed from **GNU muscle memory** — a flag or range form the GNU tool
accepts and the BSD tool refuses. The failures are benign (the tool
prints its usage line, the probe re-composes same-leg, zero mutation)
but they burn a leg per fire. The family now reads **four watches** in
eval-state's open threads:

1. sed GNU-range-form census **4** (`,+N` and kin — BSD sed wants
   `sed -n 'N,Mp'` or awk).
2. grep repetition-limit census **3**.
3. tool-PATH census **1**.
4. BSD cat flag census **2** — the 1111 wrap's seed (a byte-inspection
   probe composed `cat -A`; BSD cat's designed refusal
   `illegal option -- A`, stderr visible, zero mutation) and the 1115
   wrap's REPEAT (the IDENTICAL `| cat -A | head -5` compose in the
   LIVE_CTX structure probe). The repeat is the trigger: the 1111
   eval's carried watch says **a 2nd sighting of ANY BSD-tool/GNU-flag
   slip weighs the family doctrine row** — this row is that weighing's
   file disposition (supplement #3, d1792238859-6).

The row is the ONE compose-time rule, not a mechanical lint: a
detector over compose text is the brittle-awk class and stays rejected
(T293). The rule: **verify the flag against the tool's own usage line
BEFORE composing on macOS** — and for byte inspection reach
`od -c` / `sed -n l` / `python3`, never `cat -A` (BSD cat's flag set is
`[-belnstuv]`).

## Repo context

- Carrier: LOOP-SPEC.md Phase 2 step 5, a NEW short paragraph
  immediately after the **sed-assertion paragraph** ("When editing repo
  files with `sed` … when sed is necessary, assert.") and before the
  "Update README.md" sentence — the same ad-hoc-compose discipline
  surface.
- Pin surface: `tests/loop_spec_recovery.rs` (the sed-assertion rule's
  existing pin file — `SED_NO_MATCH` at ~line 797).
- Precedent shape: T297's single-paragraph doctrine clause (name the
  rule, name the firing shape, cite the fires) and its pin-leg form
  (token exactly once + phrase exactly once + placement, RED-proven).

## Requirements

1. The new paragraph carries the literal token `bsd-flag-compose`
   exactly once in LOOP-SPEC.md and names the ONE compose-time rule:
   verify the flag against the tool's own usage line BEFORE composing
   on macOS (the BSD userland differs from GNU muscle memory).
2. The paragraph names the family with its counts at filing (sed
   GNU-range-form census 4, grep repetition-limit census 3, tool-PATH
   census 1, BSD cat flag census 2) and cites the two `cat -A` fires
   (the 1111 seed + the 1115 repeat — the repeat named as the trigger's
   fire); the byte-inspection reach (`od -c` / `sed -n l` / `python3`,
   BSD cat's flag set is `[-belnstuv]`, `cat -A` is GNU) appears with
   the literal `[-belnstuv]` exactly once in LOOP-SPEC.md.
3. The paragraph explicitly KEEPS the mechanical-lint rejection (a
   compose-text detector is the brittle-awk class, T293) — the rule is
   a compose-time habit, not a gate.
4. Pin legs in `tests/loop_spec_recovery.rs`: the `bsd-flag-compose`
   token exactly once in LOOP-SPEC.md; the `[-belnstuv]` literal
   exactly once; the paragraph's placement AFTER the sed-assertion
   paragraph (the `sed exits 0 on no-match` needle) and BEFORE the
   "Update README.md" sentence. RED-proven pre-merge by
   deletion/revert mutants.
5. No other file touched beyond LOOP-SPEC.md + the pin file (doctrine
   row, SOLO, kimi REQUIRED validation — a LOOP-SPEC edit never takes
   the T189 lane).

estimate: ~25 lines all-in (paragraph ~10 + pin legs ~15)

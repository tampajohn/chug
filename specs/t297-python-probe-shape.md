check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'python-probe-shape' META-META-SPEC.md && grep -q 'list indices must be integers' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_scoped_walk --test internal_info_lint

# T297 — META-META-SPEC probe-hygiene: name the PYTHON accessor-shape variant (python-probe-shape census, 4 fires)

## One concern

The T284 probe-hygiene paragraph's accessor-shape root covers **jq**
projections composed from schema memory (the jq-probe-slip family, 10
sightings). The same root fires through **python** probe compositions —
a `json.load(s)`-then-walk composed from an ASSUMED file/value shape
instead of the inspected shape — and the paragraph never names it. The
python-TypeError-probe watch has counted FOUR sightings, and its carried
trigger — **a fourth sighting files the census row** — is this row's
filing authority:

1. trip 223 (cycle-1022 wrap, below-bar FIRST): an ad-hoc probe's python
   arithmetic on a probe value — `TypeError` str-minus-str — exit-1,
   stderr visible, read-only, corrected same-leg; the watch named.
2. The todos.json JSON-array-shape list-indices variant (SECOND, named
   at the eval, disposition UNCHANGED watch→watch): a probe iterating
   `.chug/todos.json` as JSONL when the file is ONE JSON array —
   `TypeError: list indices must be integers or slices, not str`.
3. The todos.json AttributeError watch leg (THIRD, below-bar, named).
4. trip 228 (cycle 1042, the 20:27:47.006Z leg): the SAME todos.json
   JSON-array-shape list-indices variant AGAIN — sighting #2's exact
   variant re-firing: `[x['id'] for x in t]` over
   `json.loads(line)` returning the array. Zero casualty (read-only
   probe, the answer re-composed off `json.load`'s list shape
   same-window); the repeat is the evidence that naming alone did not
   hold the correction.

Every sighting is the same root the paragraph already names for jq — a
probe composed from habit instead of inspection — through python's
noisy-failure surface (python exits nonzero naming the mis-shape; it
never prints `null` silently). The same inspect-first habit covers both
(a `head -c`/`type()`/`keys` shape probe before the walk).

## Repo context

- Carrier: META-META-SPEC.md, inside the **Probe hygiene (T284)**
  paragraph's accessor-shape root (the paragraph already names the
  ARRAY-shaped `.chug/todos.json` in its jq leg — the python variant is
  the same file, the same root, a different tool's failure surface).
- Pin surface: `tests/loop_spec_scoped_walk.rs` (the T284 paragraph's
  existing pin file — `PROBE_HYGIENE_LEAD` + placement ordering legs).
- Precedent shape: T270/T271's single-paragraph doctrine clauses (name
  the rule, name the firing shape, cite the fires).

## Requirements

1. The T284 paragraph's accessor-shape root EXTENDS to name the python
   variant beside jq, carrying the literal token `python-probe-shape`
   and the literal phrase `list indices must be integers`. The
   extension: python's `json.load(s)`-then-walk composed from an
   assumed shape fails NOISILY (`TypeError: list indices must be
   integers or slices, not str` on the ARRAY-shaped `.chug/todos.json`;
   `AttributeError` on an assumed attribute); the python-probe-shape
   census is named with its FOUR fires (the trip-223 str-minus-str
   first, the todos.json list-indices second, the todos.json
   AttributeError third, the cycle-1042 list-indices REPEAT fourth —
   the repeat named as the trigger's fire); the same inspect-first
   habit (a shape probe before the walk) covers python and jq alike.
2. The paragraph's existing jq text is preserved (an EXTENSION, never a
   rewrite of the jq legs); the paragraph stays ONE paragraph.
3. Pin legs in `tests/loop_spec_scoped_walk.rs`: the
   `python-probe-shape` token exactly once in META-META-SPEC.md; the
   `list indices must be integers` phrase exactly once; the T284
   paragraph still leads with `**Probe hygiene (T284` (the existing
   needle holds). RED-proven pre-merge by deletion/revert mutants.
4. No other file touched; the change is the paragraph extension + the
   pin legs (doctrine row, SOLO, kimi REQUIRED validation).

estimate: ~25 lines all-in (paragraph extension ~8 + pin legs ~15)

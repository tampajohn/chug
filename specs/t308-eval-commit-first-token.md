# T308 — eval-delta eval-commit marker: first-token extraction (prose-tolerant hash)

spec: LOOP-SPEC.md Phase 2 child spec (glm impl, 80/50 budgets).

## Finding (filed at the cycle-2091 eval, trip 486)

`scripts/eval-delta.sh`'s marker-reachability gate feeds the state file's
WHOLE `eval-commit:` field value to git:

```sh
MARKER=$(state_field "$STATE" eval-commit)
if ! g rev-parse --verify --quiet "${MARKER}^{commit}" >/dev/null; then
  VERDICT="FULL-READ REQUIRED"
  REASON="marker-unreachable (${MARKER} is not a commit in this repo)"
```

`state_field` prints everything after `eval-commit: ` — including any
provenance prose the wrap's state writer appends. The cycle-2087 wrap's
state FULL recompose (the 14th production exercise) wrote
`eval-commit: 7d3947c7ffd1d61fe32c619055c3334a0ecd7457 probed` — the bare
hash is a real commit (`git cat-file -t` OK), but the whole-field rev-parse
fails on the suffix, and every delta build since reports
`read-path: FULL-READ REQUIRED (marker-unreachable (… probed is not a
commit in this repo))` (observed at the cycle-2091 boot's 19:08:08Z build).
The cycle-2083 wrap's write was clean (the 2087 boot's delta read STATE-HIT,
marker 2796b0b reachable), so the suffix habit is new at the 2087 wrap.

Consequence to date is zero by luck: every build since the 2087 wrap was a
supervisor cheap-exit (never consults the verdict) or this TRIP eval
(full-reads regardless per the T260 TRIP clause). But the first non-trip
borderline launch under a prose-carrying `eval-commit` burns the full
~1.7M-token-class corpus read the STATE-HIT slim path exists to save
(fail-safe direction, pure budget), and the T260 STATE-HIT streak's reading
is polluted by a false flag — the T304 spec's consequence language verbatim.

This is the T304 sibling-class's second fire: T304 landed the state-drift
field's first-token compare (`[ "${drift%%[[:space:]]*}" != "none" ]`, with
the script's own comment documenting "The schema's token is the field's
FIRST word; the writers carry prose after it"). The writers' demonstrated
habit (provenance prose after marker values) is within-schema for
drift (T304/T307) but still breaks eval-commit, the LAST whole-field-compare
consumer in the script: `eval-at` is display-rendered only, the
STATE_FIELD_KEYS check is non-empty-only, `evals_behind` consumes `$MARKER`
post-extraction (covered by this fix). T306's facade leg
(`expected_eval_commit` probed-equality) guards a memory-falsified HASH,
not field hygiene — the 2087 writer passed the facade with the suffixed
value, so write-side equality alone does not cover this surface.

## Repo context

- `scripts/eval-delta.sh` — POSIX sh; the marker gate sits at ~line 192 in
  the verdict cascade (after the schema/missing-fields checks, before the
  ancestry and evals-behind legs). The drift field's first-token compare at
  ~line 207 (`${drift%%[[:space:]]*}`) is the in-file precedent to mirror.
- `tests/eval_state_delta.rs` — the Fixture-based pin suite; T304's
  `drift_none_with_prose_is_state_hit` / `drift_flagged_with_prose_is_full_read`
  (~line 518) are the test-shape precedents. A Fixture writes a state file
  against a real temp-repo commit; the suite runs the script and asserts the
  verdict line.
- The write-side discipline (the wrap writer keeps `eval-commit:` to the
  bare hash, provenance annotations riding prose fields) stays a wrap-side
  habit — this item is the reader-side robustness fix, not a writer gate.
- estimate: ~45 changed lines (script ~3 + tests ~35 + spec ~10)

## Requirements

1. The marker extraction in `scripts/eval-delta.sh` takes the field's FIRST
   whitespace-token before the rev-parse/merge-base/rev-list uses —
   `MARKER=$(state_field "$STATE" eval-commit); MARKER=${MARKER%%[[:space:]]*}`
   (or the inline equivalent at each use) — mirroring the drift field's
   documented first-token contract. A bare-hash field behaves byte-identically
   to today; a hash-plus-prose field resolves exactly as the bare hash.
2. Fail-closed preserved: a field whose FIRST token is garbage (not a
   commit, not an ancestor) still yields `FULL-READ REQUIRED
   (marker-unreachable …)` — the verdict only becomes prose-TOLERANT, never
   prose-LENIENT toward unresolvable markers.
3. RED-proven: the new prose-carrying eval-commit test goes red without the
   extraction (whole-field rev-parse fails) and green with it; the existing
   suite stays green unmodified.

## Tests

`tests/eval_state_delta.rs` (the existing Fixture suite, T304's shape):
- PASS path: a state fixture whose `eval-commit:` is `<real temp-repo commit
  hash> probed` (or equivalent prose) yields the SAME verdict the bare hash
  yields (STATE-HIT when the other gates pass) — assert the verdict line is
  NOT marker-unreachable.
- FAIL path: a state fixture whose eval-commit FIRST token is garbage
  (`eval-commit: zzzzzzz probed`) still yields
  `FULL-READ REQUIRED` with a marker-unreachable reason.
- The existing T304/T306-era legs stay green unmodified.

## Acceptance

- `cargo test --test eval_state_delta` green, including the two new legs.
- A by-hand repro: write `.chug/eval-state.md` with
  `eval-commit: <HEAD full hash> probed`, run `scripts/eval-delta.sh`, and
  the verdict is no longer marker-unreachable (restore the state file after).

check: cargo test --test eval_state_delta

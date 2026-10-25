# T304 — eval-delta state-drift verdict: first-token compare (prose-tolerant `none`)

spec: LOOP-SPEC.md Phase 2 child spec (glm impl, 80/50 budgets).

## Finding (filed at the cycle-1982 eval, trip 459)

`scripts/eval-delta.sh`'s state-drift gate compares the state's whole
`state-drift:` field value against the bare string `none`:

```sh
elif [ "$drift" != "none" ]; then
  VERDICT="FULL-READ REQUIRED"
  REASON="state-drift-flagged (state-drift: ${drift})"
```

But the state's writers (the wrap's eval-state recompose) legitimately write
prose after the schema token — the schema's token is the FIRST word
(`none` | `flagged`), and commentary after it is routine:

- `state-drift: none — first write (bootstrap; …)` — the cycle-229 bootstrap
  write: the FIRST mis-flag cluster, 8 FULL-READ builds
  (2026-10-08T11:43:50Z→13:20:01Z, loopd.log lines 6374-6629).
- `state-drift: none (the boot STATE-HIT: state+delta read mechanically,
  zero drift vs the committed eval body; T260 the 299th STATE-HIT)` — the
  cycle-1978 wrap's recompose: the SECOND cluster, 4 FULL-READ builds
  (2026-10-25T03:13:35Z / 03:17:28Z / 03:23:44Z / 03:33:35Z, loopd.log
  lines 67975/68044/68053/68067) — every delta build since that wrap,
  including the cycle-1982 boot's own.

Consequence to date is zero by luck: every mis-flagged boot so far was a
TRIP eval (full read regardless per the T260 TRIP clause) or a supervisor
cheap-exit (never consults the verdict). But the first non-trip borderline
launch under a prose-carrying state burns the full ~1.7M-token-class corpus
read the STATE-HIT slim path exists to save (fail-safe direction, pure
budget), and the STATE-HIT streak's reading is polluted by a false flag.

## Repo context

- `scripts/eval-delta.sh` — POSIX sh; the drift gate sits in the marker/
  ancestry cascade (~line 207, after the marker-reachable and evals-behind
  checks, before the splice check). The same `drift` variable is ALSO used
  display-only in `state_line` (keep that usage byte-identical — the full
  field prose is informative there).
- `tests/eval_state_delta.rs` — the live pin harness: builds a throwaway
  git repo (`Fixture`), writes state files via `state_text(eval_commit,
  drift, decisions)` + `set_state(text, also_prev)`, runs the real script
  via `run_delta`, and asserts on `verdict(&delta)` / `reason(&delta)`.
  The existing `drift_flag_is_full_read` pin (bare `flagged` → FULL-READ +
  reason) must keep passing UNCHANGED.
- estimate: ~45 changed lines all-in (script ~4, tests ~40). No other file
  moves — no src/, no doctrine, no workflow, no `check:` lines.

## Requirements

1. The drift gate compares the field's FIRST whitespace-delimited token
   against `none` — e.g. `case ${drift%%[[:space:]]*} in none) …` or an
   equivalent first-token extraction in the file's POSIX dialect. A field
   of exactly `none`, or `none` followed by any commentary, is NOT flagged.
   A field whose first token is anything else (`flagged`, with or without
   prose; empty handled by the schema-missing leg upstream) IS flagged,
   with the existing `state-drift-flagged (state-drift: ${drift})` reason
   shape byte-identical (the full field stays named in the reason).
2. The `state_line` display usage of `$drift` is untouched.
3. Pins (`tests/eval_state_delta.rs`, the live harness):
   - NEW `drift_none_with_prose_is_state_hit`: a state whose drift field is
     `none (the boot STATE-HIT: state+delta read mechanically; T260 the
     299th STATE-HIT)` (the prose shape the 1978 wrap wrote) yields
     `verdict == "STATE-HIT"` and an empty reason. RED before the fix,
     GREEN after.
   - NEW `drift_flagged_with_prose_is_full_read`: a state whose drift
     field is `flagged (my own drift note)` yields `FULL-READ REQUIRED`
     with the reason starting `state-drift-flagged` and containing
     `flagged`. Locks the flag direction symmetric to the prose tolerance.
4. Existing pins keep passing — in particular `drift_flag_is_full_read`
   (bare `flagged`) and every STATE-HIT pin (their bare `none` fields).

## Acceptance / check

The new pins plus the existing file green:

check: touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --test eval_state_delta

(worktree-relative; the T195 touch guard rides the shared slot; no pipe —
the filter is the whole `eval_state_delta` binary, broad enough to run
every test the change adds and every pin the change can break.)

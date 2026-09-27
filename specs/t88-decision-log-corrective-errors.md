# T88 — decision_log corrective validation errors

## Repo context

`src/decisions.rs` (T70, ~600 lines) implements the `decision_log` tool: six
required fields (`class`, `subject`, `inputs`, `options`, `choice`,
`confidence`), validated by `str_field` / `confidence_field`, then appended as
one JSON line to `<cwd>/.chug/decisions.jsonl`. The validation error today
names only the FIRST missing/invalid field: `missing or non-string field:
class`. The dispatch wrapper in `src/tools.rs` converts the `Err` into an
`is_error` tool result — the model sees the message and (is supposed to)
self-correct.

The failure data says one-field-at-a-time is not corrective enough:

- **FATAL — cycle-47 eval (kimi orchestrator, 2026-09-27 20:07Z,
  `.chug/events-20260927-201956.jsonl`)**: five consecutive identical
  `missing or non-string field: class` errors while logging its eval-triage
  records → the stuck tripwire aborted the run at 44/160 with the evaluation
  written but uncommitted and zero records logged. Cycle 48 had to land the
  artifacts verbatim and reconstruct 11 records from the eval's text
  (`89d1b5a`; EVALUATION.md cycle-48 Outcomes). The model announced "Logging
  the eval-triage records (5 filed + 4 weighed-and-rejected)" immediately
  before the five failures — consistent with a batched or mis-keyed call shape
  that the one-field error never diagnosed.
- **Non-fatal sightings**: t74-impl ×2 (`choice`, `options` — cycle-47 eval's
  "minor, no row" note), t85-impl ×1 (`options`), cycle-50 orchestrator ×1
  (`options`). All self-corrected, ~1 iteration each.

Nine sightings across BOTH model families and every role, one of them fatal.
The cycle-47 eval filed the class as "minor, rejected" on four sightings; the
same class then killed that very eval. That is the new data that flips the
call: the fix is not a wider contract (ONE record per call stays — batch ids
and outcome backfill subjects depend on it) but a corrective error that tells
the model everything wrong with its call in one shot.

## Requirements

1. **All-invalid-fields listing.** When validation fails, the error names
   EVERY missing or invalid field in one message (not just the first), each
   with its expected shape — `string` for the five text fields,
   `number 0..=1` for `confidence` — in the schema's field order
   (class, subject, inputs, options, choice, confidence).
2. **Unknown-shape diagnosis.** When the input object contains NONE of the
   six required keys (a batched wrapper like `{"records": [...]}`, or an
   aliased key like `{"type": ...}`), the error ALSO names the received
   top-level keys (first 8, sorted) and appends the contract reminder:
   one record per call; required: class, subject, inputs, options, choice,
   confidence.
3. **Non-object input.** A top-level JSON array / string / number / bool is a
   tool error naming the received JSON type plus the full required list.
4. **Success path byte-identical.** Record shape, key order, id format
   `d<ts>-<n>`, append semantics, and the `recorded <id>` return are
   unchanged. Existing pins stay green unmodified.
5. **`confidence` range message preserved** (`confidence must be a number in
   0..=1, got <n>`) — an out-of-range confidence is listed alongside any
   other invalid fields, same message text for that leg.

## Tests

All in the `src/decisions.rs` test module (production edits confined to
`src/decisions.rs`; `src/tools.rs` untouched):

- multi-missing: input with only `{class, subject}` → error names `inputs`,
  `options`, `choice`, `confidence` (all four, order pinned).
- wrapper leg (the cycle-47 shape): `{"records": [ ... ]}` → error names the
  received key `records` AND the one-record-per-call reminder AND the full
  required list.
- alias leg: `{"type": "eval-triage", <all five other fields valid>}` →
  error names `class` as missing AND the received keys include `type` (the
  plausible events.jsonl-alias mistake gets its corrective).
- array leg: top-level `[]` → error names the JSON type and the required
  list.
- single-missing regression pins: each of the six fields omitted alone is
  still named (no regression to silence).
- mixed-invalid leg: `confidence: "high"` plus `options` missing → both
  named.
- success leg unchanged: a fully valid call still returns `recorded d…` and
  appends exactly one line (existing tests stay green; do not weaken).

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green; the new error strings are exercised by the legs above.
- Behavior on the success path is byte-identical (existing corpus pins
  green); the change is error-text-only on failure legs.

check: cargo test --bin chug decisions

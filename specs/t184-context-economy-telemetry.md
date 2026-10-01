# T184 — Context-economy telemetry: cache tokens on iteration events + a Trim event + digest surfacing (F14 phase 1)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-a && cargo test

## Repo context

The loop is blind to its own context economy:

- T77's trim machinery (src/trim.rs — `TRIM_ABOVE_TOKENS=120_000`,
  `TRIM_TARGET_TOKENS=80_000`, 16k frozen segments collapsed to
  `[trimmed: …]` markers) FIRES in real runs (5 markers in the cycle-83
  kimi orchestrator transcript `.chug/transcript-20261001-102829.jsonl`;
  4 in the cycle-85 eval's own live transcript by iteration ~12) but
  leaves NO trace in `.chug/events.jsonl` — there is no Trim event.
  Both call sites are in src/driver.rs: `resume_messages` (~line 689)
  and the drive loop (~line 1498); each does
  `if trim::transcript_trim(messages) { transcript::rewrite(...) }`.
- `Event::Usage` (src/events.rs) carries only `{ input, output }`. The
  API layer ALREADY parses `cache_read_input_tokens` and
  `cache_creation_input_tokens` (src/api.rs ~line 1227), but the events
  stream drops them — the digest's "input context curve"
  (scripts/eval-digest.sh ~lines 193–211) reads `input_tokens` off
  serialized `{"type":"iteration", …}` lines, so it reports NON-CACHED
  input only and nobody can see per-call context size.
- src/eventlog.rs merges the Usage counters into the serialized
  iteration line (keys `input_tokens` / `output_tokens`; see the
  `sink_merges_iteration_and_usage_into_one_line` test ~line 801) — that
  merge seam is where the new cumulative cache keys ride.
- `.chug/events*.jsonl` files written before this change have NO cache
  keys and no trim lines; the digest and every consumer MUST tolerate
  their absence (jq `// 0` defaults already do).

F14 (FEATURES.md) was reframed at the cycle-85 eval: the driver-managed
context window EXISTS (T77); the missing phase-1 piece is observability.
Summarization-quality compaction is deferred (EVALUATION.md cycle-85 §4).

estimate: ~230 changed lines (src + tests + docs). Feature-row test/doc
density has run ~1.5–3x src; the filing estimate prices that in — do NOT
exceed ~500 landed lines; if the design grows past it, stop and narrow
the scope to the event+digest core.

## Requirements

1. **Trim event.** A new `Event::Trim`-style variant recorded (via the
   same best-effort, never-aborting events path as every other event)
   whenever `trim::transcript_trim` returns true — at BOTH call sites
   (resume path and loop path). The serialized line carries: estimated
   tokens before and after the trim, the number of segments collapsed in
   this pass, and the total trim-marker count after. Console and TUI
   sinks stay SILENT (events-sink JSONL only), matching how BudgetLow /
   OutputTruncated are telemetry-only. The event must fire under
   `chug run`, `--resume`, and chat alike (the call sites cover the
   first two; if chat shares the loop path, say so in the commit
   message).
2. **Cache token fields on iteration lines.** The serialized
   `{"type":"iteration", …}` line gains cumulative
   `cache_read_input_tokens` and `cache_creation_input_tokens` from the
   turn's response usage (the fields api.rs already parses), alongside
   the existing `input_tokens`/`output_tokens`. A turn with no cache
   fields serializes 0 for both. Pre-change events files are untouched.
3. **Digest surfacing.** scripts/eval-digest.sh's per-file block gains:
   (a) the tokens line extended with cumulative cache-read (and
   cache-creation) at the last iteration, `0` when absent; (b) a
   `trim fires: N` count line (0 when the file has none, including all
   pre-change files). Deterministic jq/awk only, LC_ALL=C, no new
   dependencies; the reader-staleness block and every existing line
   stay byte-stable for old-shape files except the two ADDITIONS.
4. **README.** The observability/events bullet names the trim event and
   the cache counters honestly (one compact edit, integrated into the
   existing events documentation — no new section).

## Tests

- eventlog unit legs: an iteration line with cache counters serializes
  the new keys (extend the existing merge test's family); a trim event
  serializes with all four fields; sinks that must be silent are silent.
- A unit leg around the trim-event fire seam: given a message set that
  crosses the trim threshold (build it synthetically — the trim
  machinery's own tests already construct oversized message sets), the
  caller-side helper emits exactly one Trim event per
  `transcript_trim`-true pass with before > after and correct counts.
  Do NOT run a 120k-token scripted driver run in tests.
- tests/eval_digest.rs fixture leg: a synthetic events file WITH cache
  keys and trim lines renders them in the block; a pre-change-shape
  fixture (no keys, no trim lines) renders `0` and `trim fires: 0`
  without breaking any existing assertion.
- Every pre-existing events/events-adjacent pin stays green unmodified
  unless a requirement above sanctions the change (name each amended
  pin in the commit message).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, and the
  full `cargo test` suite green.
- A real (manual, dev-machine) `chug run` that crosses the trim
  threshold shows `{"type":"trim", …}` lines in its `.chug/events.jsonl`
  and cache keys on iteration lines; `scripts/eval-digest.sh` over
  `.chug/` renders the new fields for it and `0`s for an old harvested
  file. (Manual verification noted in the commit message; the automated
  legs above are the gate.)
- RED-proof stated in the commit message for at least one leg (e.g.
  revert the serialization of the trim event → its eventlog leg fails).

## Out of scope

- Changing TRIM thresholds, segment size, or the marker format (T77's
  design is untouched).
- Summarization-quality compaction (F14's deferred half).
- Token-cost accounting/pricing.

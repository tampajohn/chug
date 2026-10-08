# T263 — loopd `laya_backfill` must mint a FRESH id for the outcome record (never the triage id)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && cargo test --release --test loopd_laya_triage

## Repo context

- `loopd.sh` is the loop supervisor. T259 added the Laya triage layer: every
  triage is recorded to `.chug/decisions.jsonl` (class `laya-triage`, id minted
  `d<epoch>-loopd<SEQ>` at `loopd.sh:479` via the shared `LAYA_SEQ` counter),
  and the triage id is PARKED to `.chug/loopd/triage-pending.json`; the next
  triaged cycle's `laya_record` first calls `laya_backfill`, which appends the
  outcome record (class `outcome`, choice landed-clean / fixed-up) labeling the
  parked triage id — the F13 distillation join.
- **The bug (cycle-237 eval §2.2, verified three ways):** `laya_backfill`
  (currently ~line 435) calls

  ```sh
  laya_record_write "$id" "$(date -u +%s)" "outcome" "$id" ...
  ```

  passing the parked TRIAGE id as the outcome record's OWN id (arg 1) as well
  as its subject (arg 4). `laya_record_write`'s signature is
  `(id, ts, class, subject, inputs, options, choice, confidence)`. The result
  in the live corpus: `.chug/decisions.jsonl` lines 1606 and 1618 both carry
  `"id":"d1791456953-loopd1"` — a laya-triage record and an outcome record
  with the SAME id, the outcome's subject naming itself.
- **Consequences (measured):** `scripts/decisions-audit.sh` on the live corpus
  moved duplicate ids 0 → 1 and malformed chain 1 → 3: the self-id'd outcome
  is malformed on its face, and the id collision secondarily mis-flags the
  CORRECTLY-targeted orchestrator outcome d1791459171-11 (the audit's id→class
  map resolves last-wins, so its subject now resolves to the duplicate
  outcome record). The class replicates once per triage: a pending park sits
  in `.chug/loopd/triage-pending.json` right now (id d1791461084-loopd2), and
  the next triaged cycle's backfill mints duplicate #2 unless this lands.
- estimate: ~150 changed lines all-in (shell ~10 + comment + pin family
  ~140 in `tests/loopd_laya_triage.rs`). Filed with the pin-family
  re-calibration applied (trip-26 eval §2.3: loopd.sh+pin-family rows have
  landed 3–3.8× their filing estimates; a naive "~30" would undershoot).
- **Append-only invariant (T70):** the existing malformed/duplicate records
  are immutable history. This item changes WRITER behavior only — never edit,
  move, or delete any existing `.chug/decisions.jsonl` line. Post-merge, the
  audit's standing counts (malformed 3, duplicates 1) are the grandfathered
  baseline every wrap names until retired.

## Requirements

1. **Fresh id for the outcome record.** `laya_backfill` mints its own unique
   id for the outcome record following the SAME convention as the triage
   writer — `d<epoch>-loopd<SEQ>` with the shared `LAYA_SEQ` counter
   incremented — and passes THAT as arg 1. The subject (arg 4) stays the
   parked triage id, unchanged. The outcome record's inputs/options/choice/
   confidence payload is byte-preserving (same `$what`/`$choice` text forms).
2. **Self-subject tripwire.** If the id about to be written ever equals the
   subject (a future regression of the same class), `laya_backfill` logs one
   WARN line to loopd.log naming both and SKIPS the write — the standing
   best-effort rule is unchanged: the corpus write never blocks the cycle.
3. **Exactly-once semantics preserved.** The pending park is still consumed
   exactly once (the `rm -f "$pending"` flow untouched); an intervening
   non-triage cycle still backfills exactly once (the existing pin
   `a_pending_surviving_an_intervening_non_triage_cycle_backfills_exactly_once`
   keeps passing unmodified).
4. **Pins (tests/loopd_laya_triage.rs, the existing real-loopd+fake-daemon
   harness).** New legs: (a) end-to-end — drive two consecutive triaged
   cycles against the fake daemon; assert the corpus holds an outcome record
   whose `subject` == the first cycle's triage id AND whose `id` != that
   triage id (and != every other record id in the written corpus — parse the
   written lines, assert uniqueness); (b) the tripwire — force the id==subject
   shape (e.g. a planted pending record whose id collides, or a direct
   function-level drive if the harness exposes one) and assert the WARN line
   lands in loopd.log and NO outcome line is appended; (c) id-convention pin —
   the minted outcome id matches the `^d[0-9]+-loopd[0-9]+$` shape, same as
   triage ids. RED-PROOF REQUIRED: leg (a) MUST fail against the pre-fix
   loopd.sh (the pre-fix writer emits the duplicate id) — demonstrate the red
   before the fix, green after (the impl child records both runs; the
   validator re-proves it).
5. **Comment honesty.** The `laya_backfill` header comment (the ONE-outcome-
   per-triage-id paragraph) keeps its meaning; adjust only what the fix makes
   stale (the id it writes is now its own, the subject is the parked id).

## Acceptance

- `cargo test --release --test loopd_laya_triage` green in the worktree (the
  check line above), including the new legs; the pre-existing pins untouched
  and green.
- The RED proof is recorded: the new end-to-end leg fails on the pre-fix
  loopd.sh and passes on the fixed one.
- Full gates at review: build + `cargo clippy --all-targets --release -- -D
  warnings` zero warnings + the release suite green.
- Post-merge, `scripts/decisions-audit.sh` on the live corpus still reports
  duplicate ids 1 and malformed chain 3 (the grandfathered baseline — the fix
  stops GROWTH, it does not rewrite history).

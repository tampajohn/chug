# T305 — wrap-corridor stream-datum helper (mechanical at-stamp generation)

spec: LOOP-SPEC.md Phase 2 child spec (glm impl, 80/50 budgets).

## Finding (filed at the cycle-1986 eval, trip 460)

The wrap corridor's stream-datum figures — the eval body's own-stream datum
at eval-write, the wrap-notes' T257 color, the state write's datum, and the
fill's FINAL datum — are hand-composed per wrap, and the composed figures
keep being carried from probes run at EARLIER stamps. The cycle-1986 eval's
mechanical re-probes found THREE committed corridor texts falsified at
their own named stamps within one window:

- the cycle-1910 wrap notes: `32 iters` vs TRUE 55 `type=="iteration"`
  events ≤ its own named stamp (2026-10-24T14:43:16.922Z) — the
  eval-write figure carried forward into the notes;
- the cycle-1910 wrap fill: `55 iters` vs TRUE 78 ≤ its own named stamp
  (2026-10-24T14:49:34.186Z) — the notes-stamp figure carried forward
  into the fill (the TRUE reconciliation is 78 + 3 post-fill legs == the
  whole-final 81);
- the cycle-1982 wrap fill (53af72d): `ok:false 4 == 4` vs TRUE 5 AND
  `trim 0 == 0` vs TRUE 1 at its own named stamp (04:42:12.713Z) — the
  fill's own datum-probe leg (04:41:56.033Z) was the 5th ok:false leg and
  the stream's trim event (04:41:19.331Z) predated the named stamp.

Every fire was caught one-or-more chains later by the eval-digest backstop
(zero casualty — the T272 designed net), but the T272 false-at-write
census moved 54 → 58 in this one window, the falsified iter figures rode
the re-arm riding list for 19 entries (surfacing at the 360th discharge,
the stale-carry census's 20th), and each fire costs correction prose at
the next eval/wrap. The T272 doctrine already mandates enumeration-by-flag
mechanical re-reads at write time; the fires persist because the
mechanical read is a multi-jq compose the orchestrator re-derives per
datum (four datums per corridor) under wrap time pressure. The T273/T295
precedent (`scripts/wrap_assert.py`) shows the loop adopts a helper when
the mechanical read is ONE command.

## Repo context

- `scripts/eval-digest.sh` reads the same events.jsonl schema with jq —
  the field names this helper reads are identical: `type=="iteration"`
  events for iters/anchors; `type=="tool_result" and .ok==false` for the
  ok:false legs; `type=="ctx_edit"` with `.accepted` bool; `type=="trim"`;
  `type=="goal"` with `.outcome` ("accepted"/"rejected"; the verdict field
  is `.outcome`, NOT `.verdict`).
- `tests/release_scripts.rs` + `tests/eval_state_delta.rs` — the pin
  harness pattern to mirror: fixtures inline in a tempdir, the script
  resolved from the RUNTIME checkout via `current_dir()` (the T48
  doctrine: a compile-time manifest-dir path is forbidden — the T47
  shared cache hands cached test binaries to other checkouts), run via
  `Command`, assert on stdout fields.
- `scripts/wrap_assert.py` is assert-shaped (functions raising
  AssertionError); a datum GENERATOR is a probe, not an assertion — a
  standalone script per the eval-digest.sh/eval-delta.sh shape, stdlib-
  only python3 (no jq dependency), mirroring wrap_assert.py's header
  comment convention (the finding + the classes it closes).
- estimate: ~270 changed lines all-in (script ~130, tests ~130, doctrine
  ~10). No src/, no workflow, no `check:`-line changes beyond this spec's
  own.

## Acceptance

1. `python3 scripts/stream_datum.py <events-file> [--at <ISO-8601 ts>]`
   emits the corridor datum as `key: value` lines, one per field:
   - `iters: N` — count of `type=="iteration"` events with ts ≤ the stamp;
   - `first-anchor: <ts>` / `last-anchor: <ts>` (the stamp) /
     `wall-secs: N.NNN` (the span between the two anchors);
   - `ok-false: N` + `ok-false-legs: <ts> [, <ts>…]` (stream order);
   - `ctx-edit: accepted A / rejected R`;
   - `trim: N` + `trim-legs: <ts> [, <ts>…]`;
   - `goal: accepted A / rejected R` or `goal: none` + `goal-ts: <ts>`
     when present (reads `.outcome`);
   - `bytes: N` + `mtime: <ISO ts>` (the file's own facts).
   The stamp defaults to the stream's last `type=="iteration"` anchor;
   `--at` overrides it (ISO-8601 `Z` form, string-compare scoping per the
   corpus convention).
2. At-stamp scoping: events with ts > the stamp are EXCLUDED from every
   count — the 1982-fill shape is the pin: a trim BEFORE the stamp is
   counted, one AFTER is excluded; an ok:false leg after the stamp is
   excluded from both the count and the leg list.
3. `tests/stream_datum.rs` pins (fixture streams built inline as jsonl):
   the empty stream (zero counts, `goal: none`); a bisecting `--at` stamp
   (every count scopes ≤ stamp); ok:false leg enumeration order; ctx_edit
   accepted+rejected counted separately; trim ts list; goal `.outcome`
   read in all three shapes (accepted / rejected / absent → none);
   `bytes:` == the fixture's written length; wall arithmetic on known
   anchors. Every assertion reads the helper's stdout fields, never a
   remembered constant (T270).
4. LOOP-SPEC.md Phase 3 gains ONE sentence at the fill-datum doctrine
   (near the T257 color rule): the wrap corridor's datum figures are
   GENERATED by `scripts/stream_datum.py` at their named stamp (or
   re-derived per T272) — never carried from an earlier probe's figure.
5. `cargo test --test stream_datum` green; the full suite + clippy
   `-D warnings` green in the worktree gates.

check: touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a cargo test --test stream_datum

# T131 — eval-digest reader staleness check: exclude the reader's own live stream

check: cargo test --test eval_digest && bash -n scripts/eval-digest.sh

## Repo context

META-META-SPEC corpus item 1: "If [the digest] reports itself stale,
regenerate it (`scripts/eval-digest.sh`) before evaluating." The
digest's rendered reader check
(scripts/eval-digest.sh:287-289) is:

```
find .chug -maxdepth 1 -name 'events*.jsonl' -newer .chug/eval-digest.md | grep -q . && echo STALE
```

This trips on EVERY evaluation cycle: loopd regenerates the digest
immediately before launching the cycle, then the cycle's own driver
rotates `.chug/events.jsonl` and appends continuously — the newest
events file is ALWAYS the evaluating cycle's own live stream, always
newer than the digest, and regeneration cannot change that (the
stream keeps appending). Observed live at the cycle-65 eval: digest
generated 17:47:52Z, the cycle's stream rotated at 17:48:12Z → the
mechanical check prints STALE within the first minute of the cycle.
The doctrine instruction ("regenerate") is literally unsatisfiable
mid-cycle; every evaluator burns an iteration diagnosing that the
flag is its own reflection.

The corpus question the check SHOULD answer is: "does some events
file OTHER than your own live stream postdate the digest?" — loopd's
immediate pre-launch regeneration makes the digest definitionally
fresh at cycle start, so the only staleness that matters is
foreign/new corpus (another process writing archives, a harvested
stream landing).

estimate: ~30 changed lines (script render + one pin leg)

## Requirements

1. scripts/eval-digest.sh's rendered reader staleness check excludes
   the single newest events file (the reader's own live stream) from
   the `-newer` test — shape:

   ```
   find .chug -maxdepth 1 -name 'events*.jsonl' -newer .chug/eval-digest.md \
     | grep -vx "$(ls -t .chug/events*.jsonl | head -1)" | grep -q . && echo STALE || echo FRESH
   ```

   (the `ls -t … | head -1` picks the newest events file — the live
   stream; `grep -vx` drops it from the candidate set). Render BOTH
   verdicts (`&& echo STALE || echo FRESH`) so the reader gets a
   definite answer, and render one explanatory sentence: the newest
   events file is excluded because it is the evaluating cycle's own
   live stream (loopd regenerates the digest immediately pre-launch,
   so it is fresh at cycle start); STALE now means a file OTHER than
   your own stream postdates the digest.
2. The pre-scan/post-scan machinery, the corpus-age field, and the
   four pinned staleness field labels (tests/eval_digest.rs
   GOLDEN_STALENESS_FIELDS) stay byte-identical — only the
   reader-check render lines change.
3. tests/eval_digest.rs gains ONE leg pinning the new check line's
   distinctive tokens (the `grep -vx`/`ls -t` exclusion and both
   `echo STALE` / `echo FRESH` verdicts) in the same shape-pinned
   style as the existing golden pins (labels/punctuation exact,
   volatile values wild) — and the leg RED-proves itself against the
   pre-T131 render (the old check line lacks the exclusion, so the
   leg dies on the parent tree; the child demonstrates this and says
   so in the commit message).
4. `bash -n scripts/eval-digest.sh` stays clean; no other script
   behavior changes.

## Tests

The eval_digest pin leg (req 3) plus a live demonstration the child
runs in a tempdir and records in the commit message: (a) digest
newest → FRESH; (b) only one events file newer than the digest →
FRESH (the own-stream leg); (c) a SECOND file newer than the digest
→ STALE (the foreign-corpus leg).

## Acceptance

- The rendered check no longer trips on the reader's own stream
  alone; foreign-corpus staleness still flags.
- `cargo test --test eval_digest` and `bash -n` green.
- META-META-SPEC's "regenerate if stale" doctrine needs no edit —
  it stays correct under the new check semantics (out of scope).

## Out of scope

Reworking the pre/post-scan or corpus-age machinery; digest content
sections; the budget_low render nuance (assessed and rejected at the
cycle-64 eval — below bar); META-META-SPEC doctrine edits.

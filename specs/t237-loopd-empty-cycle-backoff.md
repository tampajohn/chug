# T237 — loopd empty-cycle backoff: scale the cycle-OK sleep on consecutive empty-delta dispositions (doctrine, SOLO — loopd.sh)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test loopd_empty_backoff --test readme_layout --test todo_consistency

estimate: ~250 changed lines all-in (doctrine+pin carrier row — loopd.sh
~55 with comments, the new test file ~170, LOOP-SPEC clause ~8, README
clause ~4, spec)

## Concern

The loopd launch cadence now durably exceeds the work-arrival rate. With
the queue drained and no human in the loop, every cycle is an empty-delta
disposition: cycles 110, 111, 112, and the filing cycle 113 are FOUR
consecutive no-op cycles — the cycle-111/112 Outcomes entries armed the
threshold at ~4+ ("ONE more empty disposition trips the ~4+ threshold
re-filing a loopd pacing candidate"), and this filing is the trip.
Measured per-cycle cost from the digest (fresh 2026-10-05T13:12:51Z, 571
files, 35,179 iterations): cycle 110 = 15 iterations / 20m41s wall /
144.2k input tokens (565.2k cache-read) + the cold-by-construction
clippy leg + nextest 1651/1651; cycle 111 = 22i / 16m23s / 142.7k in
(1M cache-read); cycle 112 = 23i / 13m59s / 143.1k in (772.1k
cache-read). Cadence 25/20/18 min between launches (the flat 60s
LOOPD_SLEEP_OK default + the cycle wall). Sustained: ~3 cycles/hour ≈
70/day ≈ 10M input tokens/day of pure no-op burn — the disposition
doctrine absorbs each cycle's JUDGMENT at bounded cost, but nothing
throttles the LAUNCH RATE itself. Every no-op cycle also re-pays the
cold-by-construction clippy leg (its own wrap-notes commit invalidates
the build-script git-hash fingerprint) — pure waste when nothing landed.

The chain is self-sustaining by construction: the disposition
pre-authorization ("the same disposition applies while the delta stays
empty") means no future cycle spontaneously runs a real eval while the
delta stays empty — absent this row the ~20-minute no-op cadence runs
forever. The remedy is pacing, not doctrine change: keep the
disposition, make its cost match its value.

## Repo context

- loopd.sh:566: `sleep "${LOOPD_SLEEP_OK:-60}"` — the cycle-OK branch's
  only sleep. loopd.sh:561-563 carries the T137 comment pinning
  LOOPD_SLEEP_OK as a TEST SEAM ("production default 60, unchanged") —
  four test legs set it explicitly (`LOOPD_SLEEP_OK=1`:
  tests/loopd_daemon_ensure.rs:278, tests/loopd_stale_binary.rs:180+359,
  tests/loopd_orphan_reaper.rs:479), so explicit-set-wins semantics
  preserve every existing leg byte-identical. The string occurs exactly
  2× in loopd.sh (comment + sleep line);
  tests/loopd_stale_binary.rs:659 pins both `LOOPD_SLEEP_OK` and
  `LOOPD_SLEEP_FAIL` strings present — this row keeps both present.
- loopd.sh:127-131: the `routing)` subcommand — the proven test-surface
  pattern: tests/loopd_model_routing.rs runs `loopd.sh routing` against
  fixture files in a temp dir (the script cd's to its own directory, so
  a copied script + fixtures is a faithful harness; no repo file
  touched).
- The empty-disposition signal is already IN the git record,
  machine-greppable: every empty wrap-notes commit subject carries the
  literal token `empty-delta disposition` — verified via
  `git log --format=%s 886cb29..HEAD` = 4 commits: 3 matching
  (`eval: cycle-110/111/112 wrap notes (empty-delta disposition…`) and 1
  non-matching `eval:` bookkeeping commit (`eval: cycle-111 Outcomes
  compaction …` — the interleave case the streak walk must SKIP without
  stopping).
- The token is currently CONVENTION (eval-born, recorded in
  EVALUATION.md Outcomes), not doctrine — nothing pins the wrap-notes
  subject to keep carrying it. If a future orchestrator rewords the
  subject, the streak counter silently reads zero and the backoff never
  engages: req 5's LOOP-SPEC clause makes the token load-bearing
  doctrine.
- Streak RESETTING subjects, both verified in the record: any
  non-`eval:` commit (code/todo/spec/docs — work landing, e.g. every
  merge/flip commit) and any `eval:` wrap-notes commit WITHOUT the token
  (a real eval or item-landing wrap, e.g.
  `eval: cycle-109 wrap notes (eval-only cycle — ZERO rows filed…`).

## Requirements

1. **loopd.sh gains the streak counter.** An `empty_wrap_streak()` shell
   function: `git log --format=%s -30`, walk newest→oldest — an
   `eval:`-prefixed subject WITH the literal token
   `empty-delta disposition` → count++; an `eval:`-prefixed subject
   WITHOUT `wrap notes` → skip (pure bookkeeping: compaction /
   Outcomes-only); anything else (a wrap-notes subject without the
   token, or any non-`eval:` subject) → stop. Print the count.
   POSIX/macOS-safe awk only (no GNU-isms — the BSD-sed/awk doctrine),
   and a non-git cwd or empty log degrades to 0, never an error under
   `set -e`/`pipefail`.
2. **An `ok_sleep_seconds()` function and the cycle-OK branch uses it.**
   When LOOPD_SLEEP_OK is set non-empty → echo it verbatim and return
   (the T137 test seam, byte-identical). Else
   `s = min(60 * 2^n, ${LOOPD_EMPTY_SLEEP_CAP:-1800})` via a POSIX-safe
   doubling loop. Progression: n=0→60 (byte-identical to today's
   default), 1→120, 2→240, 3→480, 4→960, n≥5→1800. Replace the
   loopd.sh:566 literal sleep line with the computed value; the existing
   `cycle OK: $summary` log line stays BYTE-IDENTICAL (site-sync and log
   greps read it) and ONE new log line names the sleep/streak/cap, e.g.
   `cycle-OK sleep 240s (empty streak 2, cap 1800)`. Sweep tests/ for
   assumptions on the old line before landing (the req-context string
   pins above).
3. **A `sleep-ok` subcommand** (the `routing` pattern, loopd.sh:127)
   prints `<seconds> <streak>` for the cycle that WOULD sleep now,
   sleeping nothing — the behavioral test surface. The usage line gains
   the new subcommand.
4. **tests/loopd_empty_backoff.rs (new file, the loopd_model_routing.rs
   harness pattern).** BEHAVIORAL legs against fixture git repos in temp
   dirs (git init + `git commit --allow-empty -m <crafted subject>` + a
   copied loopd.sh): n=0 (HEAD = a code commit) → `60 0`; streaks 1/2/3
   → 120/240/480; the compaction interleave (empty wrap, then an `eval:`
   compaction commit, then an empty wrap) → counts 2, i.e.
   skip-does-not-stop; a non-empty `eval:` wrap-notes subject at HEAD
   breaks the streak → `60 0`; `LOOPD_SLEEP_OK=7` → `7` verbatim
   regardless of streak; `LOOPD_EMPTY_SLEEP_CAP=300` with streak 4 →
   `300`. STATIC pins (count_eq pattern): the literal token
   `empty-delta disposition` appears in BOTH loopd.sh (the awk needle)
   AND LOOP-SPEC.md (the req-5 clause) — cross-file equality; the cap
   default `1800` and base `60` wired inside `ok_sleep_seconds`; the
   explicit-set-wins wiring present. Every leg RED-proven (token removed
   → static RED; cap literal drifted → behavioral RED; override-wins
   removed → the override leg RED).
5. **LOOP-SPEC clause (the doctrine half).** Phase 3's wrap duties gain
   one sentence: an empty-delta disposition's wrap-notes commit subject
   MUST carry the literal token `empty-delta disposition` — the string
   is load-bearing for loopd.sh's cycle-OK backoff streak counter
   (T237); a disposition wrap without the token silently resets the
   streak and defeats the pacing.
6. **README loopd section gains ONE clause** (integrated where the
   cycle-cadence / LOOPD_SLEEP_OK behavior is documented, not an
   appended bullet): when consecutive cycles wrap with an empty delta,
   loopd backs off the success sleep exponentially — 60s doubling to a
   30-minute cap; LOOPD_SLEEP_OK still pins any fixed cadence,
   LOOPD_EMPTY_SLEEP_CAP retunes the ceiling.

## Tests

- The new tests/loopd_empty_backoff.rs legs above, each RED-proven per
  leg (behavioral mutants: cap literal, base literal, override-wins
  removal, skip-rule removal; static mutant: token removal).
- readme_layout + todo_consistency stay green (README clause integrated;
  no TODO row-format touch beyond the one row).
- The orchestrator runs the FULL gates per LOOP-SPEC step 3 (this row
  touches loopd.sh + LOOP-SPEC — doctrine, never the docs-only floor).

## Acceptance

- loopd.sh: streak function + scaled sleep + `sleep-ok` subcommand +
  usage line; the T137 seam (explicit LOOPD_SLEEP_OK wins) preserved;
  the `cycle OK:` log line byte-identical; the four existing
  `LOOPD_SLEEP_OK=1` test legs untouched and green.
- Behavioral + static pins green and RED-proven; the spec's named test
  targets green.
- LOOP-SPEC token clause landed; README clause integrated.
- Commit message names the progression (60→120→240→480→960→1800 cap) so
  the next eval reads the tuning surface without archaeology.

## Out of scope

- Any change to the FAIL-path sleep (LOOPD_SLEEP_FAIL stays 300 flat),
  the HALTED logic, the T81 routing predicate, or the disposition
  doctrine itself (the empty-delta disposition stays — the backoff makes
  its cost match its value); chat-mode or child-launch pacing;
  LOOP_ORCH_MODEL / LOOP_ROUTINE_MODEL knobs; any change to the
  wrap-notes format beyond the one token clause.

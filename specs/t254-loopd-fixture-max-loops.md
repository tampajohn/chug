# T254 — loopd test fixtures self-terminate: `LOOPD_MAX_LOOPS` honored by loopd.sh, exported by every fixture harness

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; cargo test --release

## repo-context

The cycle-168 trip evaluation found a leaked hermetic-test supervisor:
pid 81393, `bash loopd.sh run`, spawned 2026-10-01T00:38:52Z from the
`tests/loopd_stale_binary.rs` fixture at `$TMPDIR/.tmpjJMhXR`, alive
SIX DAYS with ppid 1 — its fixture log holding 4,465 identical
`driver probe FAILED (ps rc=7) — enumeration unknown; refusing to
risk a duplicate driver, skipping` lines at ~120s cadence (the T137
fail-closed loop working exactly as designed, forever). Evidence:
EVALUATION.md cycle-168 §2.1. The evaluator SIGTERM'd the instance
after full read-back.

Root structure, three facts:

1. The harness kill contract (`wait_for_any`,
   `tests/loopd_stale_binary.rs` lines ~202–213) kills the fixture
   child when its log needle appears — correct ONLY when the harness
   itself lives. An abrupt harness death (an outer bounded cap killing
   a nextest leg mid-test, a crash — the mechanism is a HYPOTHESIS per
   the indictment doctrine; the fixture is evidence, the death instant
   is not directly observed) orphans the grandchild to launchd.
2. The probe-fail skip path never reaches the build leg's 3-strike
   HALT, and the supervisor has no other self-termination path: a
   leaked fixture whose ps stub fails is immortal-but-inert.
3. The T152 orphan reaper's identity legs
   (`scripts/orphan-reaper.sh` line ~222) match argv needles
   `/tmp/chug-loop-t*` / `/tmp/chug-mut-*` only — a fixture
   supervisor's argv is plain `bash loopd.sh run` with cwd under
   `$TMPDIR/.tmp*`, structurally invisible to the reaper.

The fix is fixture-side self-termination (fail-safe by construction),
NOT a new reaper leg (a reaper needle broad enough to see
`bash loopd.sh run` risks the production supervisor — the reaper must
never kill an unresolved identity).

estimate: ~80 changed lines (loopd.sh +10–20, harness exports +1–2
per spawn site across 4–6 test files, one new behavioral test +40–60).

## requirements

1. **loopd.sh honors a new env knob `LOOPD_MAX_LOOPS`**: when set to a
   positive integer N, the supervisor counts main-loop iterations and,
   after N, logs one line naming the knob and exits 0 (exact wording
   is the impl's; the new test pins whatever it chooses). UNSET or
   empty = today's behavior, byte-identical — infinite loop,
   production operation unchanged. The T137 `LOOPD_SLEEP_OK` seam is
   the precedent: a knob production never sets.
2. **Every test harness that spawns `loopd.sh run` exports
   `LOOPD_MAX_LOOPS`** with a bound comfortably above its observed
   iteration need (suggested 50: each harness's test completes in
   ≤ a dozen iterations, so the bound only fires when the harness
   died and the fixture leaked). The sweep covers AT MINIMUM both
   `tests/loopd_stale_binary.rs` spawn sites (~lines 166 and 349) and
   every other `tests/*.rs` spawn site — as of filing the family
   includes `loopd_daemon_ensure.rs`, `loopd_orphan_reaper.rs`,
   `loopd_spoof_guard.rs`; the impl verifies the full set by grep
   (`loopd.sh run` / `run_loopd`) and names the swept files in the
   commit message.
3. **A RED-proven behavioral test**: a fixture supervisor launched
   with `LOOPD_MAX_LOOPS=3` (or a similarly small N) exits on its own
   within the test's poll window, exit status 0, and its log carries
   the exit line. Mutation leg: with the bound check removed from
   loopd.sh the new test must FAIL — the test therefore carries its
   own poll deadline so the RED leg fails fast instead of hanging
   (the existing `wait_for_any` timeout shape covers this).
4. **Existing pins stay green untouched**: the usage line, the
   routing subcommand, and the knob family (`LOOPD_SLEEP_FAIL` /
   `LOOPD_SLEEP_OK` / `LOOPD_EMPTY_SLEEP_CAP` / `CHUG_SITE_DIR`) are
   untouched; `tests/loopd_empty_backoff.rs`,
   `loopd_model_routing.rs`, `loopd_reexec.rs`, `loopd_spoof_guard.rs`,
   `loopd_stale_binary.rs`, `loopd_env_loader.rs`,
   `loopd_daemon_ensure.rs`, `loopd_orphan_reaper.rs` all stay green
   unmodified unless a pin's premise genuinely moves (then the pin
   update is part of the diff and named in the commit message).

## tests

- The new behavioral leg of requirement 3 (the impl chooses the file —
  extending `tests/loopd_stale_binary.rs`'s fixture harness is the
  natural home).
- Sweep verification: `grep -rn 'LOOPD_MAX_LOOPS' tests/` covers every
  spawn site.
- The check line runs the FULL release suite (loopd.sh pins live
  across many test binaries; warm wall ~45s, well under the 300s
  gate).

## acceptance

- The check line is green.
- Every `loopd.sh run` spawn site in `tests/` exports the knob
  (grep-verified).
- Unset-knob behavior unchanged: the existing loopd test family stays
  green unmodified (those tests exercise the unset path — a fixture
  launched WITHOUT the knob still loops past any bound).
- README: not user-visible (a test-only knob production never sets) —
  the merge-time README gate makes the final call.

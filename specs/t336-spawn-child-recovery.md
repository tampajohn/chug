# T336 — recovery recipe: the spawn-child families environment-bound red at the 3083 wrap

estimate: ~250


## Observed state (mechanically, 2026-11-07 ~20:15-21:00Z)

HEAD 4bf4353: P002 merged (248aca4, md-only) + flipped (4bf4353). The full
suite at HEAD = RED in the spawn-child families ONLY (1,900+ green):

- delegate::tests::launch: 9 reds under cargo-test, 2 reds under nextest
  ("stub never wrote argv.txt ... reported pid but no dump appeared";
  the T225 fence: "surface still advancing but the absolute backstop
  20s elapsed") — red ALONE with --test-threads=1, red with a CLEAN env
  (no CARGO_TARGET_DIR/CHUG_JUDGE/CHUG_DECISION_PROVIDER in the cycle env,
  env-grep rc-1 probed).
- loopd_spoof_guard failure_stamp_says_no_goal_complete_with_the_real_rc:
  3x red ALONE (the T158 machinery: attempt 1 invalidated "fixture
  deadline blow under load", attempts 2-3 panicked at the same fence:
  "loopd never reached the verdict ... no surface advance for 30.1s;
  backstop 120s"; the loopd.log: start -> gate runner -> daemon binary
  probe -> orphan-reaper examined=0 -> silence).
- loopd_dormant_mode / loopd_ideation_mode fixture spawns: WEDGED >180s
  in the pins battery (killed per the T178 bound; the SLOW markers read).

Verified immunities (all 0-REAL / zero-mutation probes):
- src byte-identical a11e5e9..HEAD (diff = LEDGER.md/TODO.md/docs/specs
  md-only) — a11e5e9's suite ran 1,942/1,942 GREEN 90 min earlier.
- no orphans (the test children died with the panics; ps read-back).
- the host daemon ALIVE (pid 1991, 25d uptime, `daemon --help` rc-0).
- the fleet loopd + caffeinate + daemon = the same shape as green eras.
- the T82 runner-inverse adjudication FAILS (red under BOTH runners).
- the dev binary REBUILT (cargo build --release, 1m36s, post-T332
  v0.19.0 linked 15:43) — the red persists 0-mutation.
- the T215 daemon binary = chug 0.16.0 installed (Oct 3) — served the
  green 3082 gates as-is.

Fleet history: ZERO recorded reds of these tests (the cycle logs grep).

## The delta

UNRESOLVED within the wrap budget (~35 reconstruction legs). The class:
the spawn-child machinery fails/wedges/writes-no-dump in the CURRENT
host state while byte-identical src ran green 90 min earlier.

## Recipe (the next cycle)

1. The fleet-state bisect: what content changed in the shared state
   between 18:30-20:21 (the .chug/ entries landed by the P002 cycle;
   the delegate/sandbox state dirs the spawn reads).
2. The one-leg trace per family: the stub's dump write (the delegate
   stub path), the loopd's post-reaper advance (the spoof_guard log),
   the dormant/ideation child's first write — trace each to the FIRST
   divergent leg.
3. The 44a3381-class fix if the delta is a derived fragment in the
   fixture; the fleet-state heal if the delta is shared state.
4. The re-run: the families + the pins battery v2 (the TRUE 12-target
   set, binary() grammar — the test() grammar composes wrong, 3-test
   0-REAL) before any wrap claims green.

Standing note: NO wrap may claim green gates over these families; the
fill battery gate = the pins battery v2 UNWEDGED.

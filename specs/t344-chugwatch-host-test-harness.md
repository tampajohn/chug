# T344 — chug-watch host-runnable test harness (unblocks T328's battery)

estimate: ~30 changed lines (investigation note + whichever sanctioned path lands: a package.json test script + dependency, a runner config, or a documented operator handoff)

check: set -o pipefail; npm --prefix ~/workspace/chug-watch test

## Why

T328's acceptance battery (hooks/parse.test.ts) imports `claude-code/testing`,
which cannot resolve on this host — the dispatch was refused at cycle-3155
(d1794189727-2) because a check that cannot execute fails every gate
iteration. The probe chain that established the block: npm `claude-code@1.0.0`
is a pointer stub to `@anthropic-ai/claude-code` with no `./testing` export;
the official installed package exposes no testing module; the `claude plugin`
subcommand set (details/disable/enable/eval/init/install/list/marketplace/
prune/tag/uninstall/update/validate) has no test runner; chug-watch is not in
installed_plugins.json. The chug-watch tree itself IS present (git d962b00,
2026-11-08, all three spec-named files) — the block is purely the harness.

## What

1. **Investigation (bounded, documented)**: the resolution paths are —
   (a) the claude-code SDK publishing the testing module to npm,
   (b) a fleet-provided runner (a `claude plugin test` subcommand or the
   installed-plugin runtime), (c) a sanctioned local shim that re-exports
   the REAL fleet module when present and refuses loudly when absent.
   A hand-rolled `expect` standing in for the fleet's is REJECTED —
   semantic drift between the local and fleet expect is a divergent-gate
   hazard (tests green here, red fleet-side).
2. **Land the sanctioned path** (or hand the decision to the operator with
   the investigation note filed in the mod repo).
3. **The check is the RED-proven unblock probe**: it is RED today (no
   package.json in chug-watch) and goes GREEN only when the real harness
   resolves through the mod's own test script. On GREEN, T328 dispatches
   with its original spec unchanged.

## Out of scope

- Rewriting fleet-owned test files' harness imports to another runner
  (vitest, node:test) — that breaks the fleet-side contract.
- A hand-rolled expect implementation standing in for the fleet's.
- Touching the live daemon, K7, or any fleet box.

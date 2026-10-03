# T214 — load-scaled liveness fences for the spawn-heavy loopd test families

## Repo context

The two through-loopd spawn-heavy integration families —
`tests/loopd_orphan_reaper.rs` (21 tests) and `tests/loopd_spoof_guard.rs`
(9 tests) — fence their child-verdict waits with a wall-clock deadline:
`Instant::now() + Duration::from_secs(30)` (reaper:436; spoof:163 and the
six T158 legs whose comments name the fence "a liveness fence, not a load
fence"). Solo the waits finish in ~3.4s; under full-suite parallel gate
load on a busy host they stretch past 30s (T152 measured 30.8s at 17-way
parallelism — the deadline's own margin gone). The flake signature fired
twice in cycle-96's gates: both post-merge gate runs needed the T82
family-isolation rule (reaper 21/21 + spoof 9/9 run isolated) at ~2-5
extra minutes per gate. T151/T158/T159 landed the timing-lock domain and
spawn-failure seams with an explicit zero-timeout-bump doctrine — the
fences stand because a hung child must still fail fast. What the fences
get wrong is only their BASIS: a wall-clock constant on a host whose
load the loop itself manufactures.

Mechanism, not a blanket bump: scale the fence by the host's measured
1-minute load average relative to its core count, clamped so a quiet
host sees byte-identical behavior and a melting host still fails a truly
hung child. `src/testsupport.rs` is the shared test-support module the
families already include verbatim (T159), so the helper lands in ONE
place both families share.

estimate: ~150 changed lines (testsupport helper + pure legs + adoption
in 2 families + pins; test-dense per the cycle-98 eval calibration)

## Requirements

1. `src/testsupport.rs` gains
   `pub fn load_scaled_deadline(base: std::time::Duration) -> std::time::Duration`:
   deadline = base × clamp(loadavg_1m / cores, 1.0, 4.0). The 1-minute
   load average is read through a thin seam
   (`fn read_loadavg_1m() -> Option<f64>` — `sysctl -n vm.loadavg` first
   field on macOS, `/proc/loadavg` first field on Linux; ANY failure →
   `None` → factor 1.0, i.e. fail-SAFE to today's behavior). Cores from
   `std::thread::available_parallelism` (failure → factor 1.0). The
   scaling computation itself is a pure function
   `fn scale_factor(load: f64, cores: u32) -> f64` so the clamp legs are
   testable without host state.
2. BASES UNCHANGED: every adoption site keeps its existing base constant
   (30s verdict fences, and any other family fence that adopts the
   helper) — zero timeout bumps; the helper only re-bases the fence on
   measured load. The comments at each adoption site are updated to name
   T214 and the load basis (replacing the "not a load fence" apology).
3. Adoption: BOTH families' verdict-deadline constructions route through
   `load_scaled_deadline`. A grep pin asserts no bare
   `Duration::from_secs(30)` deadline construction remains in either
   family file.
4. Fail-safe: when load or cores are unreadable the fence is exactly the
   base (byte-identical to today) — the helper never panics and never
   returns less than the base or more than 4× the base.
5. The families' T151/T159 timing-lock membership and T158 seams are
   untouched.

## Tests

- Pure `scale_factor` legs: (0.5, 8) → 1.0; (8.0, 8) → 1.0; (16.0, 8) →
  2.0; (32.0, 8) → 4.0 (upper clamp); (0.0, 8) → 1.0 (never below base).
- Seam legs: `read_loadavg_1m` returning `None` (or a garbage spawn
  result) → deadline == base exactly; available_parallelism failure path
  (via the pure seam) → factor 1.0.
- Adoption grep pin: `Duration::from_secs(30)` occurs ZERO times in the
  two family files (deadline constructions only — a comment naming the
  base is fine; assert on the code pattern, not comments, by grepping
  non-comment lines or pinning the helper-call count ≥ the old deadline
  count).
- Non-vacuousness RED-proof: temporarily make `scale_factor` always
  return 1.0 → the (16.0, 8) → 2.0 leg fails (proves the legs bind the
  scaling, not just the clamps).
- Both family suites green SOLO (baseline: reaper ~21, spoof ~9) and the
  full nextest release suite green — the acceptance that the isolation
  tax is gone: ONE full-suite gate run on a normally loaded host with
  NO family isolation passes both families (best-effort on host state;
  if the host is idle, say so in the commit message rather than
  simulating load).

## Acceptance

- All legs green; the RED-proof mutant demonstrated by the child (or
  orchestrator at review) and the tree restored byte-identical.
- `cargo test --bin chug testsupport` green (the helper's legs run in
  the bin unit tests — testsupport compiles into the bin).
- Both named integration families green; `t172_load_lock_semantics`
  green (it includes testsupport verbatim — the canary for a
  testsupport edit breaking an includer).
- clippy `--all-targets -- -D warnings` clean.

## Out of scope

- Raising any base constant (the zero-timeout-bump doctrine stands —
  30s/10s/5s bases unchanged).
- Touching the other timing families (tools.rs wall-clock legs stay on
  the T151 lock; no third family adopts the helper in this row).
- Loopd.sh or any production code.

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug testsupport && cargo test --test loopd_orphan_reaper --test loopd_spoof_guard --test t172_load_lock_semantics

# T195 — Gate source-touch guard: every shared-dir gate rebinds artifacts to the local checkout (T55-class closer)

check: touch src/*.rs tests/*.rs; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test nextest_gate_runner --test shared_target_dir --test todo_consistency && grep -qF "touch src/*.rs tests/*.rs" LOOP-SPEC.md && grep -qF "touch src" META-META-SPEC.md

## Repo context

The T55/T57 stale-artifact class: cargo's artifact filename excludes the
checkout path, and freshness is mtime-based (MEASURED this eval:
`touch src/main.rs` forces an 18s release bin rebuild — stable cargo has
no content-hash skip for local crates). Role-keyed dirs (T52/T57) bound
the class per role but do NOT close it: within one role dir, sequential
checkouts still poison each other whenever checkout A builds into the
dir and checkout B — created BEFORE A's build, so its sources are OLDER
— runs a gate afterwards: cargo reads A's artifacts as fresh and runs
them against B's sources. Three fires in the cycle-85 delta alone
(commit `ee2ff37` wrap note, "T55-class artifact events ×3 this cycle —
touch-before-gates guard to next eval"): the `74d3331` goal-gate
poisoning (t184's check built the 9-test eval_digest into
`target-shared`; t185's validator check then ran that foreign binary
against its 8-test sources — 2 false reds), the Sep-29 stale
eval_digest binary false-red on the t184 review gate (T55
touch-rebuild recovered), and a third caught by the t184 validator.
The false-GREEN leg (a stale PASSING binary masking a real failure) is
silent — this is a gate-integrity bug, pri 1.

Measured guard cost (this eval, target-shared-main, warm deps): full
`touch src/*.rs tests/*.rs` + `cargo build --release --all-targets` =
29.7s. Gate runs number ~10–15 per cycle → +5–8 min/cycle, against a
single false-red diagnosis costing ~30 min (the cycle-84 zombie-gate
precedent) or a silent false green.

This row is DOCTRINE-ONLY (LOOP-SPEC.md + META-SPEC.md +
META-META-SPEC.md + pin amendments) → SOLO: no other child in flight
while it runs.

estimate: ~110 changed lines (four LOOP-SPEC templates + two META-SPEC
templates + one META-META-SPEC convention sentence + pin
amendments/additions).

## Requirements

1. **Gate-time touch guard.** Every bounded-gate template that runs in
   a NON-MAIN checkout against a shared role-keyed target dir gains an
   immediate `touch src/*.rs tests/*.rs;` prefix on the cargo
   invocation (cwd-relative; `;` not `&&` so a touch hiccup never
   blocks the gate). The templates: LOOP-SPEC §2 step 3's nextest +
   fallback gate pair, step 4's validator-side gate references, the
   step-1 warm-build line (with the reason: a REUSED worktree — T63
   resume / next-cycle recovery — carries old mtimes and is the
   highest-risk case), and META-SPEC §5 review + §7 merge-gate
   templates.
2. **Main-exemption sentence, stated once at the first carrier:**
   post-merge main gates and Phase-3 final gates
   (`target-shared-main`) are EXEMPT — every builder in that dir is a
   main checkout and git refreshes mtimes on merge/checkout, so
   artifact identity holds by T57's construction (a touch there is
   dead cost, not safety).
3. **Inner-loop exemption, stated once:** the guard binds at GATE
   time, not in an impl child's iterative cargo loop — a child that
   already built its own checkout's artifacts into its slot rebuilds
   nothing foreign; re-touching every inner-loop `cargo test` would
   burn ~30s × dozens of runs for zero correctness.
4. **META-META-SPEC check-line convention** gains one sentence: a
   `check:` line that exports `CARGO_TARGET_DIR` into a shared slot
   begins its cargo leg with `touch src/*.rs tests/*.rs;` (the T195
   mechanism — a foreign checkout's artifacts in the slot are
   mtime-fresh against this checkout's older sources; the ~30s
   rebuild buys artifact identity). Spec authors dogfood it (this
   spec's own check line does).
5. **Pins amended in-commit, T187-style:** `tests/nextest_gate_runner.rs`
   and `tests/shared_target_dir.rs` pins that assert the pre-touch
   template forms are amended to assert the touched forms, each
   amendment justified in the commit message; ONE new structural pin
   (in `tests/nextest_gate_runner.rs`): every `perl -e 'alarm 280`
   gate line in LOOP-SPEC.md/META-SPEC.md whose env prefix names a
   `target-shared` dir OTHER than `target-shared-main` is immediately
   preceded by the touch guard — RED-proven by deleting one touch.

## Tests

- The new structural pin (req 5), RED-proven.
- Amended pins green against the new carriers.
- `cargo test --test nextest_gate_runner --test shared_target_dir --test todo_consistency` green.

## Acceptance

- Check line green; `cargo clippy --all-targets -- -D warnings` green
  (trivially — no src).
- Diff confined to LOOP-SPEC.md, META-SPEC.md, META-META-SPEC.md, and
  the two pin files.
- `grep -rn "alarm 600" LOOP-SPEC.md META-SPEC.md` stays empty (T187
  forms untouched except for the touch prefix).

## Out of scope

- Any change to the T52/T57 role-keyed dir layout or names.
- Cargo-version feature dependence (the guard assumes only stable
  mtime freshness).
- loopd.sh (its builds are main-checkout, T57-safe).

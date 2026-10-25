VERDICT: PASS

T305 validation — scripts/stream_datum.py + tests/stream_datum.rs (17 pins) + ONE LOOP-SPEC.md Phase-3 sentence, commit 6938fd1.

Gates (all run in this worktree, CARGO_TARGET_DIR=target-shared-validate-a):
1. Spec check `cargo test --test stream_datum` — 17/17 green.
2. `cargo clippy --all-targets -- -D warnings` — clean.
3. T82 gate runner `cargo nextest run --release` (nextest on PATH) — 1834/1834 green.
4. Live smoke on .chug/events-t302-validate-20261023-0100.jsonl: every figure cross-verified against jq (iters 36, ok-false 2 + legs, anchors, bytes all match); the default-stamp `goal: none` read confirmed CORRECT by design (the goal event at 00:58:43 lands after the last iteration anchor at 00:57:34; `--at <goal ts>` reads accepted 1) — exactly pin 8's shape.

Spec conformance: acceptance items 1–4 all met — the CLI shape and every named field; at-stamp string-compare scoping (the `<=` boundary pinned); all 8 required test-pin classes present, assertions reading stdout fields with fixture-derived (T270) expectations; the LOOP-SPEC.md sentence lands at the T257 bullet in Phase 3 (the fill-datum doctrine's home). Schema reads verified against src/eventlog.rs emitters (`ok` bool on tool_result, `accepted` bool on ctx_edit, `outcome` on goal — NOT `.verdict`, `trim`, `iteration`); the digest's own jq reads match.

Mutation testing: SERIAL (overlap judgment declared: all 10 mutants touch the single file scripts/stream_datum.py, so the goal's overlapping-files default applies; in-place mutate→test→revert→re-test in this worktree, tree verified byte-clean after, `git status` empty, script byte-identical to HEAD). 8/10 mutants killed:
- M-scope-lt (`<=`→`<`) CAUGHT by event_exactly_at_the_stamp_is_in_scope + 6 others
- M-scope-drop (stamp ignored) CAUGHT by at_stamp_scopes_every_count + 4 others
- M-goal-verdict (`.outcome`→`.verdict`) CAUGHT by goal_verdict_field_is_never_read_outcome_is + 3 others
- M-ctx-invert (accepted sense flipped) CAUGHT by ctx_edit_counts_accepted_and_rejected_separately + 1
- M-stamp-first (default stamp = first anchor) CAUGHT by default_stamp_excludes... + 6 others
- M-legs-reversed (leg order reversed) CAUGHT by ok_false_legs_enumerate_in_stream_order + 2 others
- M-wall-whole (wall from all-event min/max) CAUGHT by wall_arithmetic_derives_from_the_probed_anchors
- M-bad-at-exit0 (malformed --at accepted) CAUGHT by malformed_at_stamp_exits_2

Findings (both advisory weak-test gaps on shapes OUTSIDE the spec's required pin list; in both cases the SHIPPED code is the verifiably correct variant — no correctness bug shipped):
1. M-okf-truthy SURVIVED — `not ev.get("ok")` for `ev.get("ok") is False` goes uncaught: no fixture carries a tool_result with absent/non-bool `ok`. Manually confirmed the shipped form is jq-parity correct (jq `select(.type=="tool_result" and .ok==false)` matches 0 lines on an absent-ok fixture; the mutant counts 1). Unreachable on real streams — src/eventlog.rs:357 always emits `"ok": ok` as a bool — but a one-line fixture (tool_result with no `ok`, assert `ok-false: 0`) would pin the strict-equality parity the spec names.
2. M-goalts-first SURVIVED — `gl[0]` for `gl[-1]` in goal-ts goes uncaught: no fixture carries TWO goal events. The shipped last-event read matches its docstring and the corridor's final-verdict use; multi-goal streams are REAL in the corpus (≥5 main-repo events files carry 2 goal events — rejected-then-accepted within one run), where the mutant would name an earlier rejection's stamp as goal-ts. A fixture with a rejected+accepted pair pinning goal-ts to the LAST would kill it.

Notes (not findings): the mtime VALUE is unpinned (only its 20-char Z-form shape is asserted — a value assertion isn't portable); the `resolve_stamp` docstring says "last parsable event ts" where the code takes the last STRING ts — cosmetic, harmless under string-compare scoping.

No decisions.jsonl written in this worktree (the validator-side corpus stays empty per the goal); the verdict record is the orchestrator's write from this file.

VERDICT: PASS

Validation of T362 (orchestrator grind delegation) at commit 496f6155 (impl 3fb9e980 + spec re-key), worktree /private/tmp/chug-loop-t362.

Findings (numbered; #1 is the only actionable one and is NON-BLOCKING):

1. NON-BLOCKING weak fixture — the launch discriminator `startswith("launched:")` in scripts/shape_metric.sh is not pinned against a `contains("launched:")` mutation. mut-contains SURVIVED the battery (10/10 green). Demonstrated impact: a collect result whose embedded goal summary contains the literal substring "launched:" mid-text is miscounted as a launch (probe stream: clean script `delegate=1`, mutant `delegate=2`). Severity low: the metric is best-effort and never gates, and the inflation needs a child summary to quote the substring — but a collect preview DOES embed the child's full accepted summary verbatim, so the case is realistic. Strengthen by adding a non-launch line whose preview contains "launched:" mid-text to status_polls_and_collect_results_are_not_launches and asserting delegate=1.
2. Informational — equivalent mutant: mut-drop-q (`fromjson?` -> `fromjson`) survived because the installed jq 1.7.1's `//` operator already catches LHS errors, making the `?` redundant on this platform (it is jq-1.6 portability defense). The malformed-tolerance BEHAVIOR itself is asserted green against the real script (trailing-bad-line fixture; I additionally probed a MID-stream bad line by hand -> correct counts, exit 0). Not a test-strength gap.
3. Informational — estimate drift: spec estimate ~120 changed lines vs 645 landed (555 of them the test file). Eval-note material per the estimate-vs-actual convention, not a defect.

Gates (all run by me, CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a):
- Spec check: cargo test --release --test loop_spec_grind_delegation -> 10 passed, 0 failed.
- cargo clippy --release --all-targets -> clean, zero warnings.
- T82 gate: cargo nextest run --release (nextest on PATH) -> 2285 passed, 0 failed.

Mutation legs (6 mutants, parallel cap 3, one throwaway worktree + role-keyed target dir target-shared-mut-<k> per leg, worktrees removed after collection; overlap judgment: mutants touch only scripts/shape_metric.sh and LOOP-SPEC.md and each leg ran in its own checkout, so legs never interact — parallel safe):
- mut-launch-prefix (startswith("launched:")->startswith("state:")): CAUGHT — 3 delegate fixtures red for the right reason.
- mut-bash-name (.name=="bash"->.name=="read_file"): CAUGHT — 3 bash fixtures red.
- mut-kimi-carveout (verdict-remains-kimi's reworded): CAUGHT — doctrine once-pin + verdict_stays_kimis + window pin red.
- mut-budget-figure (fewer than 20 -> fewer than 25): CAUGHT (after re-keying my first sed, which missed the wrapped line — defective mutant, not a pass) — budget once-pin + window pin red.
- mut-drop-q: SURVIVED = equivalent (finding 2).
- mut-contains: SURVIVED = weak fixture (finding 1).

Spec coverage verified against the diff:
- Doctrine: Grind-delegation block placed between step 6's budget check and Trivial-row bundling; glm-children default + OUTPUT-review/no-per-fixture-bash; both carve-outs (kimi verdict w/ T81 reference; ~20-steps inline); <20 inline-bash budget + over-budget reason clause + named RED-proof delegation path; orchestrator-inline-grind eval-triage class w/ decision_log channel, >=20-and-no-delegate trigger, T261 tune channel.
- Shape metric: Phase-3 Outcomes bullet mandates exactly one GENERATED shape line per cycle via scripts/shape_metric.sh at the prep stamp (T305 discipline); the script is +x sh, best-effort (exit 0 on missing/empty/malformed streams), one exact line.
- Tests legs: synthetic below-budget delegated run (parses bash=5 <20, delegate=1), polls/collect not launches, empty/missing/malformed streams, doctrine once-pins, placement pins, validator-independence pin. The "appears in the eval note / count==1 per cycle" leg is pinned at the doctrine surface (no cycle has run under the doctrine yet), which is the testable reading.
- Ground-truthed against src: tool_result events carry type/name/preview (src/eventlog.rs); launch render begins "launched: pid" (src/delegate.rs:639), status "state: " (:1202), collect "verdict: " (render_collect) — the script's discriminators match the real stream; dogfooded on this run's own events -> `shape: bash=13 delegate=0 (impl/validate)`.
- Compat/out-of-scope respected: diff touches only LOOP-SPEC.md, scripts/shape_metric.sh, tests/loop_spec_grind_delegation.rs (+ 1-line spec re-key); no routing (T81), dormancy (T325), or release machinery changes; no battery slimming.

Tree state: byte-clean except the pre-existing harness-owned LEDGER.md modification (present at validation start, untouched by me); all throwaway mutant worktrees removed; this worktree's .chug/decisions.jsonl absent (I never called decision_log).

VERDICT: PASS

## Gates (all run by this validator, CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a)
1. Spec check line (11 targets, cargo test --release --test-threads=4): GREEN — 11/11 binaries, 167 tests, 0 failed; both new tests ran+passed (splice_all_records_carry0_with_matching_corpus_tail_is_corpus_tail_verified, splice_carry0_without_the_corpus_tail_identity_stays_full_read).
2. cargo clippy --all-targets --release -- -D warnings: GREEN (0 diagnostics).
3. T82 gate: cargo nextest run --release (full suite): 1839/1839 passed.

## Acceptance replay (shell, real scripts/eval-delta.sh, 16 probes — all correct)
- Matching corpus tail (26/12/13/40-record corpora, incl. the gap-record case) → STATE-HIT + `ok (carried 0, rotated 0, appended 12, corpus-tail-verified)`.
- FULL-READ with the violation verbatim on: corrupted ring id, missing corpus, malformed corpus (first line), empty corpus file, 8-record corpus vs 12-ring, trailing-space ring (byte-exactness), non-reversed ring. Torn-last-line corpus: valid prefix tail verifies (correct for append-only tail corruption).

## Mutation legs (throwaway worktrees /tmp/chug-mut-t315-*, role-keyed target dirs target-shared-mut-*, ≤3 in flight; all mutants touch scripts/eval-delta.sh — per-leg isolated checkouts, no on-disk overlap, parallel-safe)
- M1 (case pattern stubbed to NEVER-MATCH): killed — leg (a) red (FULL-READ), leg (b) green. [the spec's RED-proof, independently reproduced]
- M2 (identity check flipped `=` → `!=`): killed — legs (a)+(b) red.
- M3 (tail→head slice): killed — leg (a) red, leg (b) green.
- M4 (reversal dropped → cat): killed — leg (a) red.
- M5 (`- ` prefix dropped): killed — leg (a) red.
- M9 (case pattern widened to `"violated:"*`): SURVIVED — see finding 1.
- All throwaway worktrees removed; main worktree byte-clean (git status --porcelain empty).

## Findings
1. (weak-test note, non-blocking) M9 survivor: widening the acceptance leg's case pattern from the exact `violated: nothing carried verbatim from the previous ring (summarization?)` string to ANY `violated:*` string is caught by no test (32/32 green in the mutant run) — the leg's specificity to the nothing-carried violation is unpinned. Exploit window for a hypothetical over-broad leg is narrow: it requires corpus ≤ 11 records AND a <12-line ring AND ring == corpus tail reversed AND a genuine under-cap splice violation (rewritten/dropped/reordered), i.e. the early-repo window only; for k>12 (overfull) or corpus ≥ 12 (any k<12 ring) the identity can never match. The shipped code matches the spec letter exactly (the acceptance leg fires only on the nothing-carried violation), so this is a pin-gap observation, not a defect in the diff.
2. (robustness note, non-blocking) jq stops at the first parse error and emits only the valid prefix (verified on jq 1.7.1): a MID-file corpus corruption reads as a truncated corpus and the tail-12 is computed over the prefix. Fail-safe in practice — a green requires the ring to equal the prefix's tail-12 exactly (mismatch → FULL-READ), and append-only corruption realistically lands at the tail, where behavior is correct (probe: torn last line + valid prefix → correct STATE-HIT). The spec's fail-closed requirement (missing/unreadable corpus keeps the violation) is pinned by leg (b) and holds.
3. (nit) Spec-text imprecision: the spec locates the ring-discipline paragraph "in the Write-EVALUATION section"; it actually lives in the "Read first (the evaluation corpus)" section (META-META-SPEC.md lines 51–87). The rider landed in the correct paragraph (the rotate-at-most-11 sentence, line 65) — zero functional impact.
4. (nit) Estimate overshoot: spec estimated ~90 changed lines; the diff lands ~181 (script +52/-2, rider +12/-1, tests +116). No functional impact.

## Spec conformance
- Req 1 (script): caller-computed corpus-tail identity on the nothing-carried violation only; jq last-MAX_DECISIONS ids, `- `-prefixed, portable awk reversal (no tail -r/tac); exact match downgrades to the ok*-form `ok (carried 0, rotated 0, appended N, corpus-tail-verified)`; mismatch/unreadable/missing corpus keeps the violation verbatim (fail-closed). VERIFIED in code + replay.
- Req 2 (doctrine rider): rotate-at-most-11 sentence gains the all-records exception (corpus tail-12 reversed, append-only T70, delta corpus-tail leg, fail-closed). VERIFIED; zero pin drift (check line + full nextest green; `rotate at most` greps to META-META-SPEC.md only; the verbatim census window (Write-EVALUATION) untouched by the edit).
- Req 3 (pins): leg (a) STATE-HIT + exact splice-check string; leg (b) corrupted/missing/malformed all FULL-READ with the violation verbatim; `decisions_corpus` writer added in the existing fixture style (T70 shape, id first key). VERIFIED.
- Acceptance: the cycle-2730 case replays green; a corrupted carry-0 still reads FULL-READ. VERIFIED.

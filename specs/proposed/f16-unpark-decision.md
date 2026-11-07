# P002 — F16 unpark decision

- **User-visible value**: FEATURES.md stops carrying a parked F16 indefinitely — either it ships or it is formally killed, and the roadmap is honest either way.
- **First slice**: re-read the F16 spec and current harness state, write a keep/kill memo with the deciding facts; operator picks the branch.
- **Estimate class**: XS (one memo).
- **Source fact**: F16 has sat in the FEATURES.md `## Parked` section since 2026-10 with no re-examination.

## Dispatch (cycle-3083, glm routine cycle — the operator's 2026-11-06 blanket approval activates this row)

estimate: ~50 lines (docs-only; one new file `docs/f16-unpark-memo.md`, no production file, no FEATURES.md edit)

**The deliverable** — `docs/f16-unpark-memo.md`, the keep/kill memo the operator picks from:

1. **The question** — the P002 intent verbatim: FEATURES.md stops carrying a parked F16 indefinitely — ship, kill, or keep parked with a stated re-trigger.
2. **The deciding facts**, each citation-backed (file + the committed row/section; the T331 citation discipline):
   - the parked note's own unpark condition (`FEATURES.md` `## Parked` F16 entry: un-parks when F13's first distilled judge lands AND an alternate-runtime comparison has a consumer again);
   - F13 distillation rounds 1+2 both DECLINED the rollout (round 2 = `docs/distill-f13-2c.md` / T294: probe 72.3 pct vs majority 55.3 pct, no tau-95-at-50 point, held-out n=47 < 60) — the first distilled judge has NOT landed;
   - the alternate-runtime comparison lost its contestant at the T223 bake-off (LAYA STAYS — `docs/judge-parity.md`; kev DeltaNet-candle-blocked at impl; verdict-2.0 re-probed HTTP 401) — no consumer again yet;
   - the decision corpus is append-only and has grown since round 2's pinned 2,573 records — probe the live line count at memo-write time, cite it WITH its date label, and name a round-3 re-measure as the concrete step that could move the condition's first half (name it; do NOT run it);
   - T332's decision-provider abstraction (CHUG_DECISION_PROVIDER) landed 2026-11-07 — context the memo should carry: the judge layer is provider-pluggable now, which broadens what an alternate runtime could mean, but satisfies NEITHER precondition half by itself.
3. **The recommendation** (clearly labeled as such): KEEP PARKED with the re-trigger stated precisely (F13 round-3 verdict reaching the tau-95-at-50 point / a distilled judge landing, AND a benchmarked alternate-runtime contestant). Killing now would erase an option the operator's own parked note calls "real and near".
4. **What the operator picks between**: (a) keep parked with the stated re-trigger (recommended), (b) kill with this memo as the written reason, (c) amend the parked condition (e.g. drop the alternate-runtime half).

**Out of scope**: editing `FEATURES.md` (the operator's pick edits it), running distillation round 3, any code, any test file.

**Discipline**: public-repo-safe content only (the T249 internal-info lint runs in the review gates; F16/F13/T223/T294/T332 are public FEATURES/TODO/doctrine references — no org hosts, no internal-only ids); every fact cites its source; the memo is read-only toward the loop state (it probes the corpus count, it never writes `.chug/decisions.jsonl`).

## check:

test -s docs/f16-unpark-memo.md && grep -q '## The deciding facts' docs/f16-unpark-memo.md && grep -q '## Recommendation' docs/f16-unpark-memo.md && grep -q '## The operator picks' docs/f16-unpark-memo.md

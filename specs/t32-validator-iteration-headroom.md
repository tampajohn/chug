# T32 — LOOP-SPEC validation-child template --max-iters 40→50

check: grep -n '`max_iters: 50`, `max_minutes: 30`' LOOP-SPEC.md && ! grep -q '`max_iters: 40`, `max_minutes: 30`' LOOP-SPEC.md && grep -c 'max_iters: 50' LOOP-SPEC.md | grep -q '^2$'

## Repo context

LOOP-SPEC §2 step 4 (the adversarial-validation launch paragraph,
`LOOP-SPEC.md:90`) launches kimi validators at `max_iters: 40`,
`max_minutes: 30`. The last six substantive validators (recomputed
per-stream in the cycle-14 eval, EVALUATION.md §2 P2):

| validator | iters | budget_low@8 |
|---|---|---|
| t26-validate | 38/40 | YES |
| t27-validate | 16/40 | no |
| t28-validate | 34/40 | YES |
| t29-validate1 (FAIL verdict) | **39/40** | YES |
| t29-validate2 (PASS verdict) | 36/40 | YES |
| t30-validate | 19/40 | no |

4 of 6 at ≥34/40; t29-validate1 delivered its FAIL verdict **one
iteration from dying mid-verdict**. The suite grows ~10 tests/item
(403→415 across 3 items) and mutation count scales with diff surface.
This is the T21 failure class (impl children 40→50 after three 40/40
deaths) and the T27 class (orchestrator 80→120 after two near-deaths)
at the validator station — filed BEFORE the first died-mid-verdict,
which is the cheapest this class will ever be. Minutes are never
binding (t29-validate2 used ~13 of 30), so minutes stays 30.

## Requirements

1. In `LOOP-SPEC.md` §2 step 4 ONLY, change the validation launch
   budgets from `` `max_iters: 40`, `max_minutes: 30` `` to
   `` `max_iters: 50`, `max_minutes: 30` ``.
2. Keep the parenthetical truthful: it currently reads "(§6's budgets,
   passed explicitly — minutes is 30, not delegate's 35 default)". With
   iterations now 50≠§6's 40, adjust the wording minimally so it does
   not claim iterations are §6's (e.g. "(§6's budgets with T21-class
   widened iterations, passed explicitly — minutes is 30, not
   delegate's 35 default)"). Keep the minutes rationale verbatim.
3. Every other byte of LOOP-SPEC.md unchanged — step 2's impl template
   (`max_iters: 50`, `max_minutes: 35`) untouched, the §6-goal-verbatim
   sentence untouched, step numbering untouched.
4. **Do NOT edit META-SPEC.md** — human spec; LOOP-SPEC already
   overrides §6's launch mechanics (the T21/T24 pattern).
5. Commit message cites the evidence (the 39/40 FAIL verdict; 4-of-6
   ≥34/40; T21/T27 precedent).

## Tests

- Doctrine-only change: no code tests apply. The `check:` line above
  is the pin: exactly two `max_iters: 50` sites (step 2 impl + step 4
  validation), zero `` `max_iters: 40`, `max_minutes: 30` `` remaining.
- Run `cargo test -- --test-threads=4` once to confirm the tree is
  green (it must be — you changed no code).

## Acceptance

- `check:` passes verbatim from the worktree.
- `git diff` shows one hunk in LOOP-SPEC.md and nothing else.

## Boundaries

- Implement TODO item t32 ONLY. DO NOT touch TODO.md or LEDGER.md in
  the main tree — bookkeeping is the orchestrator's. Do not edit
  META-SPEC.md, META-META-SPEC.md, SPEC*.md, or README.md.

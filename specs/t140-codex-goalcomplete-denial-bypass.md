# T140 — goal_complete denial bypass (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md (operator-filed, pri 1).
false completion: denied/vetoed goal_complete still completes the run (driver.rs:967/1078/1164 — goal_summary populated from tool name, result.is_error never checked on that path unlike submit_plan)

## Requirements

1. Read the cited review section FIRST — it has the evidence and trigger.
2. Reproduce with a failing test (mutation-style: the bug as described must
   make the new test RED before the fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic: driver/api/loopd).

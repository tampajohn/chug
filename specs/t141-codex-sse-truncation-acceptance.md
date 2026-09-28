# T141 — SSE truncation acceptance (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md (operator-filed, pri 1).
api.rs:817/784/976 — clean EOF before content_block_stop/message_stop still returns success; partial tool calls execute (unfinished goal_complete can end a run without a check)

## Requirements

1. Read the cited review section FIRST — it has the evidence and trigger.
2. Reproduce with a failing test (mutation-style: the bug as described must
   make the new test RED before the fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic: driver/api/loopd).

# T142 — loopd grep spoofing (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md (operator-filed, pri 1).
model text containing 'chug: goal complete' satisfies the supervisor's log grep (loopd.sh:225 + events.rs:264) — failed cycle recorded as OK; check child exit status instead

## Requirements

1. Read the cited review section FIRST — it has the evidence and trigger.
2. Reproduce with a failing test (mutation-style: the bug as described must
   make the new test RED before the fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic: driver/api/loopd).

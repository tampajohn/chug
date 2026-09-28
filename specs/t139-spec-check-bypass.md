# T139 — Editable spec check bypasses bash deny + risk gate (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §2 HIGH — the model edits its own spec to 'check: <arbitrary>' and goal_completes next iteration (driver.rs:822/1390): run_shell executes outside permissions, hooks, and the risk gate. Replacing a failing check with 'check: true' also defeats verification integrity.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

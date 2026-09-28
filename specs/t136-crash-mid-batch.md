# T136 — Crash mid-tool-batch leaves unresumable transcript (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §1 HIGH — tool results persist only after the whole batch (driver.rs:930/943/1208): kill mid-bash and the resume hits unanswered tool_use blocks the endpoint rejects; earlier batch tools may already have changed files. transcript.rs:77 truncating rewrite can destroy the only active transcript on crash.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

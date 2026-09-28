# T138 — mcp.json executes before permission enforcement (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §2 HIGH — a repository-controlled mcp.json spawns its command before permissions load (mcp.rs:366/469, driver.rs:295/658): denying bash or mcp__* does not prevent startup execution in an untrusted checkout; the process inherits credentials unless overridden.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

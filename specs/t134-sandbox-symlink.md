# T134 — Sandbox escape: file tools follow symlinks; bash has no confinement (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §2 HIGH — resolve_safe is lexical-only (tools.rs:759/471/817, plan.rs:141): an in-tree symlink to an external path escapes read/write/edit; bash has zero confinement and inherits API credentials. README/SPEC 'all paths sandboxed' is FALSE (drift §3) — fix confinement AND the claims.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

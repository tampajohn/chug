# T135 — driver.lock acquisition race — two starters can both own it (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §1 HIGH — read-absent → write → verify is not atomic (driver_lock.rs:189/206/238): two starters both proceed into transcript housekeeping. Lock tests are sequential-successor only; add a simultaneous-acquisition leg.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

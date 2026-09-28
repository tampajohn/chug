# T137 — loopd launches stale binary after a failed build (codex adversarial review)

check: cargo test

Finding from reviews/CODEX-REVIEW-20260928.md. §1 HIGH — loopd.sh:15/182/221 never tests cargo build's exit status: a broken merge launches the previous release binary. set -euo pipefail + an explicit build gate before launch.

## Requirements

1. Read the cited review section FIRST — evidence and trigger are there.
2. Reproduce with a failing test (the described trigger must be RED pre-fix).
3. Fix minimally; sweep the class (sibling paths with the same shape).
4. Validator REQUIRED (core logic).

# T335 — goal-gate check extraction: first-match must not lose to markdown-wrapped prose

estimate: ~80 changed lines (driver.rs parse_check_command + bin unit tests)

## Why

First sighting, cycle 3077 (the t334 validator's goal gate): the spec's
Tests bullet wrapped so that a continuation line began with the token —
`  check:-contract sentence.` — two-space indent, then `check:`. The
goal gate's extractor (`parse_check_command`, src/driver.rs:2060) trims
the line and `strip_prefix("check:")`es it, so the prose continuation
line MATCHED, and `find_map` (first match wins) returned the garbage
command `-contract sentence.`. The gate ran `sh -c '-contract sentence.'`,
failed, and rejected the validator's `goal_complete` with "check command
failed" — even though the validation had already landed its written PASS
verdict. On an impl child without a written verdict the same collision
would mask a green gate behind a fake red and burn the remaining budget.

The real check line sat at spec line 73; the prose collision at line 53.
Any markdown-wrapped prose that wraps onto a line starting with `check:`
collides. The t334 repair (reworded both prose tokens so the only
`check:`-starting line is the real one) is the symptom fix; the
first-match-wins rule is the disease.

## What

1. Tighten `parse_check_command` (src/driver.rs) so a prose continuation
   line can no longer win. Implementer's choice among (evidence named,
   all RED-provable):
   - prefer the LAST line-anchored `check:`-prefixed occurrence (the
     corpus convention puts the real check line last; t309/t310's single
     occurrence is unchanged by last==first), or
   - when a `## check:` section header exists, take the first
     `check:`-prefixed line AFTER it, or
   - reject matches whose command cannot start a shell command (the
     `-contract sentence.` shape) — weakest, listed for completeness.
   Whatever the choice: the t334 pre-fix spec shape (prose continuation
   at line 53, real line at 73) must extract the REAL command.
2. Pin the `## check:` header behavior: a header line itself is never
   selected as the command (the t333 precedent — its gate ran green with
   a `## check:` header present — suggests headers are already skipped
   because `## check:` trims to `## check:` whose prefix is `#`; pin it).
3. Normal single-occurrence specs (t309/t310 shape: line-3 check, no
   header) extract byte-identically to today.

## Tests

Bin unit tests in src/driver.rs's test module beside
`parse_check_command` (the binary-only crate — the T67 no---lib rule):

- t334 pre-fix shape: prose continuation `  check:-contract sentence.`
  at an earlier line + real `check: echo ok` later → extracts `echo ok`
  (RED before the fix: extracts `-contract sentence.`).
- `## check:` header + real line → header never selected.
- t309/t310 shape: single line-3 occurrence → byte-identical extraction.
- Empty-command line (`check:` alone) → still None.

One RED-proven killing test for the class (the t334 pre-fix shape), the
pre-fix extraction named in the test comment.

## check:

check: cargo test --release --bin chug parse_check_command

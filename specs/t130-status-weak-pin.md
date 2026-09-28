# T130 — `chug_status` weak pin: the missing-`.chug/` leg must assert its distinctive phrase (T124 M3 survivor)

check: cargo test --bin chug mcp_serve

## Repo context

T124's kimi validator (verdict d1790615724-24, cycle 64) ran 6
mutants; M1/M2/M4/M5/M6 died to their targeted tests but **M3
SURVIVED**: deleting the `.chug/`-existence check
(src/mcp_serve.rs:228-234 — `let chug_dir = cwd.join(".chug"); if
!chug_dir.is_dir() { return ("…no .chug/ directory in…", true) }`)
goes undetected because the covering leg
`chug_status_missing_chug_dir_names_received_path`
(src/mcp_serve.rs:605) asserts only:

- `is_error` — the downstream events-unreadable leg also returns
  `isError: true`;
- `text.contains(<tmp path>)` — the unreadable message embeds
  `<tmp>/.chug/events.jsonl`, which CONTAINS the tmp path;
- `text.contains(".chug")` — that same embedded path contains the
  literal `.chug`.

Every assertion is substring-satisfied by the WRONG error message
(the events-unreadable message the mutant falls through to), so the
check can be deleted silently — the class of weak test the T119 row
was filed to sweep. The cycle-64 wrap carried it to this eval
("M3 weak-test, low severity — carried to next eval, T119-class
pin-strengthening"). Production behavior is correct today; the PIN
is what is broken.

estimate: ~20 changed lines (one src file, test module only)

## Requirements

1. Strengthen `chug_status_missing_chug_dir_names_received_path`
   (src/mcp_serve.rs test module) so it dies when the `.chug/`
   check is deleted: assert the DISTINCTIVE phrase of the real
   error (`no .chug/ directory in`) AND a negative assertion that
   the text does NOT contain `events file unreadable` (the shadow
   message). Keep the existing received-path-verbatim assertion.
2. Sweep-the-family (the cycle-33/T69 lesson — one class, one
   sweep): audit the OTHER `chug_status` fail-fast legs for the
   same substring-shadow class —
   `chug_status_relative_cwd_is_refused_naming_the_received_string`,
   `chug_status_nonexistent_cwd_is_refused_naming_the_path`,
   `chug_status_missing_events_file_names_the_path`, and the
   missing/non-string-argument legs. Each must assert the
   DISTINCTIVE phrase of its own error message (the phrase only
   that leg's `return` produces), with a negative assertion against
   the plausible shadow message(s) where one exists. Strengthen
   only where a shadow actually exists; do not churn legs that are
   already distinctive.
3. RED-prove EVERY touched leg: for each strengthened leg, apply
   its corresponding mutant (delete/neuter the production check it
   pins), show the leg dies, revert, show green. The commit message
   names each mutant and its killer leg.
4. Production code is NOT changed — the checks stay exactly as they
   are; only the test assertions sharpen. If the sweep finds a leg
   whose shadow reveals a REAL production ambiguity (two legs
   genuinely indistinguishable downstream), stop and note it in the
   ledger instead of editing production — that becomes a finding
   for the orchestrator.

## Tests

The strengthened legs themselves are the tests. The acceptance
evidence is the RED-proof table in the commit message: mutant →
killing leg → time-to-die.

## Acceptance

- The M3 mutant (delete the `.chug/`-existence check) dies to the
  strengthened leg.
- Every sibling leg in the sweep either asserts a distinctive
  phrase + negative shadow guard, or is explicitly named in the
  commit message as already-distinctive with the reason.
- `cargo test --bin chug mcp_serve` green (module-stem filter runs
  every mcp_serve leg; src-only change, no `tests/` integration
  surface touched).

## Out of scope

Production behavior changes; new tools; README (internal pin
hygiene, not user-visible); the delegate-side status tests
(`render_status` is byte-pinned there — untouched).

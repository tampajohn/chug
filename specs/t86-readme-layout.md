# T86 — README Development layout omits plan.rs + tgrep.rs

check: grep -q 'plan' README.md && grep -q 'tgrep' README.md && cargo test --test todo_consistency

## Repo context

- Cycle-47 eval §6(c): the README Development section's module list —
  `src/{api,archive,driver,driver_lock,eventlog,events,tools,tui,webfetch,chat,`
  `attach,complete,decisions,delegate,riskgate,mcp,mcp_http,sse,observ,auth,`
  `ledger,transcript,build_info}.rs` — predates T73 (plan mode →
  `src/plan.rs`) and T76 (tgrep → `src/tgrep.rs`). First staleness
  finding in four README audits; the only docs-vs-reality drift found.

## Requirements

1. Add `plan` and `tgrep` to the Development section's `src/{...}.rs`
   module list (sensible positions — the list is roughly grouped, not
   strictly alphabetical; the impl matches the existing grouping). Rest
   of the line and the README byte-identical.
2. One-line, one-file, pure-`.md` diff. One commit:
   `T86 — README Development layout gains plan.rs + tgrep.rs`.

## Tests

- `cargo test --test todo_consistency` green (the check: line above).
- Deletion hand-check: drop either name → the grep fails (non-vacuous).

## Acceptance

- The module list names every file in `src/` (verify with
  `ls src/*.rs | wc -l` against the list's count + main.rs).

## Out of scope

- Any other README edit; any code.

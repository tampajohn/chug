# T338 — README Tools headline enumeration gains `mcp_resource` (the W46 cold-read audit finding)

## Concern

The README's Tools chapter (README.md, `## Tools`, the opening enumeration) names 17
builtin tools — `read_file`, `write_file`, `edit_file`, `bash`, `grep`, `tgrep`,
`glob`, `list_dir`, `update_ledger`, `todo_add`, `todo_update`, `todo_list`,
`goal_complete`, `delegate`, `web_fetch`, `web_search`, `decision_log` — and omits
**`mcp_resource`**, the 18th builtin. `mcp_resource` IS a registered builtin (not an
`mcp__`-prefixed server tool): it is documented in the MCP-servers section ("The
model-facing surface is the builtin **`mcp_resource`** tool"), it is T90-denyable by
its plain name, and the plan-mode six-tool contract names it implicitly through the
registry. A newcomer reading the Tools chapter's headline list gets an incomplete
enumeration of the model's actual surface. Found by the cycle-3152 (740th)
evaluation's weekly README cold-read audit (2026-W46 trigger); the audit's layout,
links, and quickstart legs all verified EXACT — this is the audit's only finding.

## Scope

- README.md `## Tools` opening enumeration: add `mcp_resource` to the headline list
  (after `decision_log`, before the sandbox paragraph), with the same terse shape as
  its siblings — one clause naming it the MCP resource catalog/read surface and
  pointing at the MCP-servers section for the contract.
- NO other README edit. NO code change. NO doctrine edit.

## Acceptance

- `grep -n 'mcp_resource' README.md` shows the headline enumeration mention in the
  Tools chapter (previously the name appeared only in the MCP-servers section).
- `cargo test --release --test todo_consistency` green (row consistency).
- `cargo test --release --test docs_link_check` green (no link drift).
- The docs-only gate floor applies (every changed file ends `.md` — README.md only):
  the full cargo gates may shrink per LOOP-SPEC's T80 rule, but the implementer runs
  docs_link_check + todo_consistency explicitly.

## Estimate

~15 changed lines (README.md only, one enumeration entry + its clause).

check: CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared cargo test --release --test todo_consistency --test docs_link_check

# T93 — permissions `mcp__` matcher-fit canary accepts server-specific globs

check: cargo test --bin chug permission

## Repo context

T90 (F4 phase 1) landed `.chug/permissions.json` deny rules in
`src/permissions.rs`. Its load-time matcher-fit validation uses ONE canary
candidate — `src/permissions.rs:352–354`:

```rust
// MCP tools (`mcp__<server>__<tool>`) carry arbitrary args, so a glob
// that can match an mcp__ name fits any matcher.
if glob_matches(tool_glob, "mcp__server__tool") {
```

The t90 validator's non-blocking finding (1) (cycle-52 Outcomes): a rule
like `{"tool": "mcp__fs__*", "path": "*.env"}` never matches the canary
string (the glob's literal `mcp__fs__` prefix ≠ `mcp__server__…`), so the
rule is SKIPPED as matcher-that-cannot-fit — a valid operator deny never
enforces (fail-open, one warn + one `permission_error` line, so bounded —
but a policy-intent hole). The canary is wrong, not the glob.

## Requirements

1. Any `mcp__`-prefixed tool glob fits any arg matcher (MCP tools carry
   arbitrary args — the comment's own premise). Implement by treating a
   glob starting with `mcp__` as matcher-fit-valid (e.g.
   `tool_glob.starts_with("mcp__") || glob_matches(tool_glob,
   "mcp__server__tool")` — the second leg keeps non-prefixed globs like
   `mcp*` honest if they can match the canary; preserve today's behavior
   for non-MCP globs byte-for-byte).
2. A skipped-rule `permission_error` line is still emitted for a genuinely
   un-fittable matcher (e.g. `command` on `read_file`) — that leg is
   unchanged.
3. The doc-comment at the canary site is updated to state the prefix rule.
4. README's Permissions section: NO change needed unless the matcher-fit
   prose there contradicts the fix (check the "matcher that cannot fit"
   parenthetical; if it names the mcp__ behavior, align it — one clause
   max).

## Tests (new tests carry the `permission` stem — the check filter runs the whole family)

- RED-proven: `{"tool": "mcp__fs__*", "path": "*.env"}` loads as a VALID
  rule and `check("mcp__fs__read", {"path": "prod.env"})` DENIES while
  `check("mcp__fs__read", {"path": "ok.txt"})` allows. Prove RED by
  reverting the fix (rule is skipped → both calls allow → test fails).
- `mcp__*` whole-glob with an arg matcher still valid (pre-existing leg
  stays green unmodified).
- `command` on `read_file` still skipped with its `permission_error`
  line (fit rejection for builtins unchanged).
- Non-prefixed glob that cannot fit (e.g. `bash*` with a `path` matcher…
  verify against the existing builtin compatibility table) still skips —
  pick the shape the existing tests already pin and keep them green.

## Acceptance

- `cargo test --bin chug permission` green and demonstrably runs every
  permissions test (T96 lesson: confirm the run list covers the new legs).
- RED proof recorded in the commit message.
- Full suite + clippy green; diff confined to `src/permissions.rs` (+
  at most one README clause).

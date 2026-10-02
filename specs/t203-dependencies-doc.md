# T203 — DEPENDENCIES.md: audited external-dependency inventory

check: cargo test

estimate: ~120 lines (doc + README link)

## Concern

Operator 2026-10-02: "sans the LLM proxy, i want to audit the
dependencies of chug." The audit happened (operator session, F94
checkout a2f0df3) — capture it as a repo doc so the eval corpus and
future dependency reviews read it, and so new deps get checked against
it (reuse > extend > create applies to dependencies too).

## Repo context

Audit findings (2026-10-02, to be encoded in the doc):
- 9 runtime crates: anyhow, clap, crossterm, glob, ratatui,
  reqwest (blocking+rustls), serde, serde_json, sha2. No database, no
  docker, no gh CLI, no external config service.
- Runtime services (all env-optional, fire-and-forget): Langfuse
  (LANGFUSE_HOST/PUBLIC_KEY/SECRET_KEY); layad judge (LAYA_URL, default
  127.0.0.1:8420 — risk-gate judge + notify layad sink); DuckDuckGo HTML
  (CHUG_WEB_SEARCH_PROVIDER/BASE_URL, keyless default); web_fetch
  arbitrary URLs (capability); GitHub via plain git push/tag (delivery).
- Spawned processes: git, sh, ps, rg (core); osascript (macOS notify,
  optional); .chug/mcp.json servers (user-configured children).
- Build/release: crates.io at build; install.sh needs curl + shasum on
  the operator box; releases via v* tag -> GHA (macos-14 arm64, ubuntu
  x86_64 + aarch64 cross).
- Env surface: CHUG_* knobs, ANTHROPIC_* (LLM proxy — out of scope for
  the doc's audit table but listed), LANGFUSE_*, LAYA_URL, REMOTE_TOKEN.
- Failure semantics per dep (fail-open vs fail-closed vs blocking) are
  the doc's spine: telemetry drops, judge degrades logged, git blocks.

## Requirements

1. `DEPENDENCIES.md` at repo root: the inventory above as a table
   (dependency / kind / required-for / env knobs / failure mode /
   first-added-by row T###), plus a short "adding a dependency" section
   (check this doc first; fail-open unless it's a policy surface; record
   the row that adds it).
2. Every env var in `src/` referenced via std::env::var appears in the
   doc (the doc lists CHUG_* exhaustively — derive from source, not
   memory).
3. README gains one line linking it (no inline duplication — META-META
   §6 anti-append pattern).
4. The doc names the LLM proxy as the single hard dependency and states
   everything else degrades.

## Tests

- todo_consistency passes (this row + estimate line).
- Doc lint pin if a markdown checker exists in the suite; otherwise
  acceptance: eval cold-read finds the doc accurate against src/.

## Out of scope

- Changing any dependency (that's T204 et al.); SBOM tooling/cargo-deny
  automation (a later row if the operator wants CI enforcement).

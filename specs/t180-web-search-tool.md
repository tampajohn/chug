# T180 — `web_search` tool (F12 phase 1: provider seam + DuckDuckGo HTML provider)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test

## Repo context

F12 is the top wholly-unworked, non-deferred FEATURES.md roadmap item
(cycle-83 eval §4). `web_fetch` (T37, src/webfetch.rs) fetches a KNOWN
url — it cannot discover one; every benchmark (Claude Code WebSearch,
Codex web tool) ships search. The loop itself is a consumer: evaluation
and doctrine work regularly need "find the page" before "fetch the page".

Phase 1 scope (this row): the provider seam + ONE zero-config provider
(DuckDuckGo's HTML endpoint — no API key). Keyed providers (Brave,
Tavily) are a later phase; the seam is where they plug in. The full
multi-provider scope would blow the filing band, so the split is:
phase 1 = seam + duckduckgo (this row); phase 2 = keyed providers
(unfiled, named in README as later).

Existing shape to mirror (src/webfetch.rs, 1055 lines incl. tests):
module-level consts, `pub fn schema() -> Value`, `pub fn web_fetch(input)
-> anyhow::Result<ToolResult>`; registered in `src/tools.rs` schema list
( `crate::webfetch::schema()` at tools.rs:181) and dispatched
(`"web_fetch" => crate::webfetch::web_fetch(input)` at tools.rs:239).
HTTP plumbing is shared with `crate::mcp_http` (see webfetch.rs's test
imports at :581). Timeouts: connect 10s, total 30s; GET only; ≤5
redirects; body byte cap; non-2xx → tool error with a small preview.

estimate: ~420 changed lines (src/websearch.rs ~230 incl. parser +
provider seam, tools.rs schema+dispatch+pin update ~40, tests ~150
incl. local-stub leg, README ~12) — inside the ~400 soft band only
just; if the impl finds the parser+stub pushing past ~500, land the
tool + parser + unit tests first and the local-stub end-to-end leg in
the same commit only if it fits (it is req 6, not optional — growth
past the band means STOP and hand back with a note, per the T108
lesson).

## Requirements

1. **New builtin `web_search`** in a new `src/websearch.rs`, mirroring
   webfetch's module shape. Input: `{"query": string (required, non-empty),
   "max_results": integer (optional, default 5, clamped to 1..=10 — a
   larger request clamps, never errors)}`. Output (ok leg): numbered
   results, each `title`, `url`, one-line snippet, tag-stripped and
   whitespace-collapsed; total output char-capped (default 20_000,
   hard ceiling 100_000, same clamp semantics as web_fetch — a
   `max_chars` optional param is NOT required; a fixed 20_000 cap with
   a truncation note is acceptable and simpler).
2. **Provider seam**: an internal `SearchProvider` enum (phase 1:
   `DuckDuckGo` only) selected by env `CHUG_WEB_SEARCH_PROVIDER`
   (unset/empty → duckduckgo; `"duckduckgo"` → duckduckgo; anything
   else → tool error naming the received value AND the valid values).
   The env is read per call (no process-global latch), so tests can
   vary it without serialization hazards.
3. **DuckDuckGo leg**: GET `https://html.duckduckgo.com/html/?q=<url-
   encoded query>` (base url overridable per req 6). Parse result
   blocks: `<a class="result__a" href="HREF">TITLE</a>` and the
   sibling `<a class="result__snippet">SNIPPET</a>` (attribute ORDER
   and quoting may vary — match on the class token, not a byte-exact
   prefix). HREF may be a redirector `/l/?uddg=<urlencoded real url>&…`
   — decode `uddg` when present, else use HREF verbatim. Strip tags
   and collapse whitespace in title/snippet (reuse webfetch's
   tag-strip helper if it is factored for reuse; otherwise a small
   local stripper — do NOT pull an html crate).
4. **Honesty legs** (all surfaced as text, never a panic, never a
   hang): non-2xx → tool error with status + ≤500-char preview (mirror
   webfetch); connect/total timeouts mirror web_fetch (10s/30s) and
   produce tool errors naming the phase; zero parsed result blocks →
   an OK result whose text distinguishes the two cases: (a) the page
   itself says no results ( DuckDuckGo's no-results markup ) →
   "0 results for <query>"; (b) markup unrecognized (bytes arrived but
   no `result__a` blocks) → "search page returned N bytes but no
   result blocks parsed (the DuckDuckGo HTML shape may have changed)"
   — a silent empty list on markup drift is the failure mode this
   leg exists to prevent.
5. **Tool description** (in the schema, model-facing): names the
   provider (DuckDuckGo HTML, zero-config, no key), the env override
   + valid values, GET-only, the 10s/30s timeouts, the result cap,
   and one honest sentence that live HTML scraping can break or be
   rate-limited and surfaces those as tool errors. Network-not-
   filesystem wording mirrors web_fetch's description.
6. **Test seam**: the provider's base url is overridable in tests via
   env `CHUG_WEB_SEARCH_BASE_URL` (read per call, like req 2's env) so
   the end-to-end leg runs against a local stub (the T6 harness —
   bounded accept, no hangs); the env name is NOT in the tool
   description (test-only seam, CHUG_DELEGATE_BIN precedent).
7. **README**: Tools section gains a `web_search` bullet beside
   `web_fetch` (zero-config DuckDuckGo provider, env seam, phase-2
   keyed providers named as later; the honest scraping/rate-limit
   sentence). The Tools-intro "two documented exceptions" wording
   (README + the pin at src/tools.rs:2652-2690) becomes THREE
   exceptions (delegate, web_fetch, web_search) — update the README
   sentence AND the pin IN THE SAME COMMIT.

## Tests

- Parser unit tests over fixture HTML: two-result block; `uddg`
  redirector decoding; result without snippet; no-results page (case
  4a); unrecognized markup (case 4b); attribute-order variance.
- Provider selection: unset → duckduckgo; explicit `duckduckgo`;
  unknown value → error naming received + valid; per-call env read
  (two calls with different env in one test process).
- `max_results`: absent → 5; 0/negative/non-integer → tool error
  naming the bound; 99 → clamps to 10 (assert the clamp in the
  stubbed leg's output).
- Schema pins: `query` required; the description carries the req-5
  honesty tokens (assert substrings: provider name, env var name,
  rate-limit honesty).
- End-to-end stub leg (T6 harness): stub serves fixture HTML on
  127.0.0.1; `CHUG_WEB_SEARCH_BASE_URL` points at it; assert numbered
  output contains the fixture's decoded url + title; non-2xx stub →
  tool error leg.
- The updated tools-intro pin (three exceptions) passes; a revert of
  the README sentence fails it (RED-proof in the commit message).

## Acceptance

- `cargo test` green (plain — the row touches README.md + a
  src-carried pin, so the full suite runs, per the check-breadth
  doctrine); `cargo clippy --all-targets -- -D warnings` clean;
  build green.
- One commit on the branch; no TODO.md/LEDGER.md edits.

## Out of scope

- Keyed providers (Brave/Tavily), search-result caching, `max_chars`
  parameter, pagination, chat/UI surface changes. Phase 2 is unfiled.

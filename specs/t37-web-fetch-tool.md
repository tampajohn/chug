# T37 — `web_fetch` tool: read-only HTTP(S) GET, bounded

check: cargo test

## One concern

chug has no way to read a URL. LOOP-SPEC §2 names the gap outright
("a missing `delegate`/`web_fetch` tool") — `delegate` landed (T23);
`web_fetch` is the remaining half. Add a `web_fetch` tool: read-only
HTTP(S) GET with hard bounds, surfaced like every other builtin.

## Repo context

- Evidence (cycle-16 eval §4): six consecutive evals grep zero
  organic web attempts across every harvested stream — the loop's
  work is repo-local, so demand is latent, not measured. The filing
  argument is the capability-gap doctrine (META-META-SPEC §4: file a
  feature row when a credible gap exists; features are first-class)
  plus the harness-class standard (every peer harness ships a fetch
  tool). When a child DOES need external text today (an API doc, an
  error-message lookup), its only path is operator-wired MCP config —
  no self-serve.
- Power budget: `web_fetch` is strictly LESS powerful than the `bash`
  tool the model already has (`curl` exists). The value is a bounded,
  audited, token-safe surface: size caps, text extraction, and
  events.jsonl previews of every fetch — none of which a raw `curl`
  in bash gives the loop.
- Tools live in `src/tools.rs` (schema + dispatch, 3,037 lines) with
  the driver calling through one match arm; `src/mcp_http.rs` has a
  reqwest-based HTTP client pattern to crib from (connect 10s,
  first-byte 30s). Recommend a new module `src/webfetch.rs` to keep
  tools.rs from accreting; register in `main.rs`'s module list and
  add the schema in `tool_schemas()` alongside the builtins.
- Test discipline: T6's stub harness (accept deadlines, read/write
  timeouts) and T31's `dead_port()` probe (for the
  connection-refused leg) already exist in the suite — reuse them.
- The laya risk gate judges `bash` only; a read-only GET needs no
  gate leg (MCP tools bypass it too).

## Requirements

1. **Schema**: `web_fetch` with required `url` (string) and optional
   `max_chars` (integer, 1-based semantics N/A — a plain size cap).
   Default `max_chars` = 20,000; hard ceiling 100,000 even when
   requested higher (clamp, don't error). Description states: GET
   only, http/https only, size-capped, HTML is tag-stripped, binary
   content is refused.
2. **Transport**: GET only; `http://`/`https://` only (any other
   scheme → tool error naming the scheme). Follow redirects, cap 5
   (6th → tool error). Timeouts: connect 10s, total 30s. A response
   body larger than the cap is truncated (streaming or after read —
   implementer's choice) with a `[truncated at N chars]` marker.
3. **Content handling**: `text/html` → strip `<script>`/`<style>`
   blocks, then tags, collapse whitespace runs (a small hand-rolled
   state machine is fine — NO new heavyweight dependency; check
   Cargo.toml's existing tree first, and if an HTML-to-text crate is
   already present it may be used). `text/*` (other) and
   `application/json` → body verbatim (then cap). Any other
   Content-Type → tool error naming the type (binary refusal).
   Missing Content-Type → treat as text.
4. **Errors as tool errors, never aborts**: non-2xx status → tool
   error naming the status with a ≤500-char body preview;
   DNS/connect/timeout/redirect-overflow → tool error naming the
   class. No retry loop (one attempt; the model can re-call).
5. **Sandbox exemption honesty**: `web_fetch` reaches OUTSIDE the cwd
   sandbox by design (it is network, not filesystem) — the schema
   description must say so in one sentence, matching the delegate
   paragraph's candor.
6. **README**: the Tools bullet list gains `web_fetch`, plus one
   sentence in the Tools prose (bounds + the curl comparison).
7. Out of scope (say so in the impl commit): POST/PUT, headers
   beyond a fixed `User-Agent: chug/<version>`, cookies, caching,
   domain allowlists, auth.

## Tests

All behind the T6 stub-harness discipline (bounded accepts, socket
timeouts, `dead_port()` for the refused leg). Suite must not gain a
hang class.

1. Stub serves `text/plain` → body returned verbatim (under cap).
2. Stub serves `text/html` with `<script>`/`<style>`/tags → script
   and style content GONE, tags gone, visible text present, whitespace
   collapsed.
3. Stub serves 404 with a body → tool error names 404 (and the body
   preview survives).
4. Stub 302 chain (2 hops) → final body returned; 6-hop chain → tool
   error naming redirects.
5. Body larger than `max_chars` → truncated with the marker; explicit
   small `max_chars` honored; request above the hard ceiling clamps.
6. `file:///etc/passwd` (or `ftp://`) → tool error naming the scheme.
7. Stub serves `application/octet-stream` → tool error naming the
   type.
8. Dead port (T31's `dead_port()`) → tool error fast (well under the
   120s bash cap), no retry storm, no panic.
9. Non-vacuousness leg: one test asserts the default cap constant's
   value via a body just over 20,000 chars (kills a cap-removal
   mutant).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test -- --test-threads=4` all green in the worktree.
- The 9 test legs above exist and pass; stub harness discipline kept
  (no unbounded accept, no wall-clock assert tightened).
- README updated per req 6, integrated into the Tools section (not a
  bullet glued elsewhere).
- This spec's `check:` (`cargo test`) passes in the worktree — it is
  worktree-relative by construction (T30 doctrine).

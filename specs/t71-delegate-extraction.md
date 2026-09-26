# T71 — Extract the delegate surface from src/tools.rs into src/delegate.rs

check: cargo test --bin chug delegate && cargo build && cargo clippy --all-targets -- -D warnings && cargo test

## Repo context

src/tools.rs is 5,233 lines (2,048 at the T26 eval → 3,962 → 4,050 → 5,233;
T68 +283, T69 +863). The pre-declared watch trip line (~4,500, EVALUATION.md
watch item since cycle 30) is crossed. The delegate surface is the dominant
growth driver (T23 launch/status, T28 reap, T29 wait_secs, T39 max_tokens,
T58 resume/segments, T61 error suffix, T68 significant-wake, T69 collect —
eight items in one file) and it is self-contained: `DelegateSummary`,
`CollectSummary`, and every delegate helper are referenced ONLY inside
src/tools.rs (verified by grep at eval time).

The module precedent is src/webfetch.rs (T37): a standalone module,
registered in src/tools.rs with two one-line edits, `mod webfetch;` in
src/main.rs. webfetch.rs sits at 1,054 lines and has needed zero wayfinding
since. This is a PURE MOVE — byte-identical runtime behavior; the whole diff
is relocation plus import/visibility wiring.

## Requirements

1. **New module `src/delegate.rs`.** Move the COMPLETE delegate production
   surface out of src/tools.rs (current line numbers approximate — move by
   name, not by line): `delegate`, `delegate_cwd`, `delegate_spec`,
   `delegate_child_argv`, `delegate_max_tokens`, `delegate_resume`,
   `delegate_launch`, `delegate_status`, `parse_wait_secs`,
   `delegate_status_now`, `read_events`, `delegate_status_wait`,
   `summarize_events`, `DelegateSummary` (+ its `significant_ne` and any
   impls), `reap_and_alive`, `summarize_collect`, `CollectSummary`,
   `delegate_base`, `collect_git_commits`, `delegate_collect`,
   `render_collect`, and every delegate-only const/helper they use.
2. **Move the delegate tests with the code** — every `fn delegate_*`,
   `fn collect_*`, and the delegate-summary test cluster (incl. the T68
   significant-wake pins and the T69 collect pins) relocates into
   `src/delegate.rs`'s own `#[cfg(test)] mod tests`. Non-delegate tests stay
   put. Shared test helpers used by BOTH delegate and non-delegate tests
   stay in tools.rs and are imported (`pub(crate)` as needed).
3. **Registration stays exactly where it is**: the delegate schema json! in
   `tool_schemas()` and the `"delegate" => …` arm in `inner()` remain in
   src/tools.rs byte-identical; the arm now calls
   `crate::delegate::delegate(ctx, input)`. (The schema is the one
   cross-cutting surface other tools' pins reference — T41's sandbox pin
   among them — so it does not move.) `src/main.rs` gains `mod delegate;`.
4. **Visibility**: moved items become `pub(crate)` exactly as needed for
   tools.rs call sites and cross-module tests — no wider. No `pub use`
   re-export shim that would let tools.rs keep old paths (the point is the
   move); update the call sites instead.
5. **Byte-identical behavior**: no logic edits, no renames, no comment
   rewrites beyond fixing stale `src/tools.rs` path references IN comments
   (e.g. module-doc pointers); every moved function's body is unchanged.
   The README's Development layout brace list gains `delegate` (it pins the
   real tree — T60 keeps it honest).

## Tests

- The full pre-existing delegate suite passes from its new home
  (`cargo test --bin chug delegate` — the same filter the T69 spec check
  uses; count must match main's count exactly — record both in the commit
  message).
- `cargo test` whole-suite green; `cargo clippy --all-targets -- -D warnings`
  clean (dead_code warnings surface any helper left unreferenced — none may
  be silenced with `#[allow]`; either it moves with its users or it stays).
- Tool-count pin in tools.rs tests stays green untouched (schema surface
  unchanged).
- Hand-check in the commit message: revert one moved pin's target line back
  in delegate.rs (e.g. drop the wait_secs cap) → the moved pin goes RED —
  proves the tests exercise the moved code, not a stale copy.

## Acceptance

- `cargo build && cargo clippy --all-targets -- -D warnings && cargo test`
  green in the worktree (export
  `CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared` first).
- `check:` line passes verbatim from the worktree root.
- `git diff --stat` shows essentially two files (tools.rs shrink ≈
  delegate.rs growth; wiring hunks in main.rs/tools.rs/README.md small).
- Zero behavior change: the only edits outside the move are import/visibility
  lines, the one dispatch-arm call path, `mod delegate;`, and the README
  brace-list word.

## Out of scope

- Splitting delegate.rs further (collect vs status vs launch) — one module
  is the right grain today.
- Moving any non-delegate tool (web_fetch/bedit/etc.) — each is its own row
  if the monolith regrows.

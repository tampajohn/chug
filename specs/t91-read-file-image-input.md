# T91 — F5 phase 1: `read_file` image input (base64 blocks + endpoint-reject degrade)

check: cargo test --bin chug image

## Repo context

FEATURES.md F5 (Image input) is the top unworked roadmap row; the cycle-53
eval (EVALUATION.md §4) split it: this spec is phase 1, chat paste/drag is
deferred phase 2. Today `read_file` on a binary file returns text (lossy
UTF-8 mojibake for images) that silently poisons the conversation. The
Anthropic Messages API accepts image content blocks
(`{"type":"image","source":{"type":"base64","media_type":…,"data":…}}`)
inside user messages and inside tool_result content arrays.

Relevant code: `src/tools.rs` — `ToolResult { content: String, is_error:
bool }` (line ~32), `dispatch()` → `inner()` → `read_file()` (T26
pagination, `READ_MAX_LINES`, resolve_safe cwd sandbox). `src/api.rs` —
`ContentBlock`/`KnownBlock` enum (Text, ToolUse, ToolResult, Thinking,
RedactedThinking) + `Other` for forward-compat; `tool_result_block()` sets
tool_result `content` to a `Value::String`. `src/driver.rs` (~line 985) —
wraps each tool result with `ContentBlock::tool_result_block(id,
result.content.clone(), result.is_error)` into `user_blocks`. Events
previews: error legs keep a ≤2000-char tail window, ok legs ≤200-char head
(T25) — a base64 payload must NEVER ride either.

## Requirements

1. **Image detection.** `read_file` on a path whose extension (lowercased)
   is `png`, `jpg`, `jpeg`, `gif`, or `webp` reads the file as BYTES and
   returns an image instead of text. Media types: `image/png`,
   `image/jpeg`, `image/gif`, `image/webp`. All other files (including
   unknown binary extensions) keep today's exact behavior.
2. **Size guard.** An image file larger than 5 MiB (5 × 1024 × 1024 bytes)
   is a tool error naming the cap and the actual size — no partial read,
   no base64 in the error text.
3. **ToolResult image channel.** `ToolResult` gains a way to carry image
   payloads alongside the text `content` (e.g. an `images:
   Vec<ImageBlock>` field, default-empty; `ImageBlock` holds media_type +
   base64 data). The text content of an image result is a SHORT note like
   `[image: <path> (<n> bytes, image/png)]` — previews and events ride the
   text only. `is_error` semantics unchanged; the size-guard error is a
   normal text `ToolResult` with `is_error: true`.
4. **api.rs.** `KnownBlock` gains `Image { source: ImageSource }` with
   serde round-trip (`type: "base64"`, `media_type`, `data`), and
   `tool_result_block` gains a blocks-array variant (or a sibling
   constructor) so a tool result with images serializes as a tool_result
   whose `content` is an ARRAY: image block(s) first, then the text note.
   String-content results stay byte-identical (`Value::String`, not a
   one-element array).
5. **driver.rs.** The wrap site (and any other place tool results become
   content blocks — e.g. the T13/T38 steering/advisory paths if they wrap
   results) sends image blocks through. Plan mode: `read_file` is one of
   the five plan tools — image results work there too (read-only is
   unchanged; the response shape is the only delta). Permissions/hooks
   ordering untouched (the policy chain gates the CALL, not the result
   shape).
6. **Endpoint-rejection degrade (load-bearing safety leg).** If the API
   responds 400-class with a body indicating content/image rejection, the
   driver retries the request ONCE with every image block in the outgoing
   messages replaced by a text block `[image removed: endpoint rejected
   image content]`, records ONE events note (a new `image_degraded`-style
   line or the nearest existing event shape), and latches a per-run flag
   so later image results are downgraded at WRAP time (never sent) for the
   rest of the run. A non-vision endpoint degrades; it must never poison
   every subsequent request. Scope the detection narrowly (400 + body
   mentions image/content) so unrelated 400s keep today's behavior.
7. **Sandbox & non-image behavior byte-identical.** resolve_safe still
   gates the path BEFORE any read; `path escapes cwd` unchanged; text-file
   results, pagination notes, and error texts byte-identical (existing
   pins green unmodified).
8. **README.** The `read_file` mention in ## Tools gains the image clause
   (extensions, 5 MiB cap, degrade-on-reject) integrated into the existing
   sentence/paragraph — not a bolted-on bullet.

## Tests (all new tests carry the `image` stem so the check filter runs them)

- Extension → media-type map incl. uppercase extension (`X.PNG`).
- A tiny fixture PNG (hand-rolled minimal bytes under tests' fixtures or
  written by the test) returns: text note naming path+bytes+media type,
  one image block with the exact base64 of the bytes, `is_error: false`.
- Unknown binary extension (e.g. `.bin`) keeps the old text path
  (byte-identical vs pre-change behavior).
- Size guard: a >5 MiB image → tool error naming the cap; zero base64 in
  the error.
- tool_result serialization: image result → content ARRAY [image, text];
  plain result → `Value::String` byte-identical (non-regression pin).
- Degrade leg: stub API 400 with an image-rejection body → ONE retry with
  placeholder text blocks (assert the retried request carries ZERO image
  blocks + the placeholder text), events note emitted once, per-run latch
  downgrades a SECOND image result at wrap time. Stub 400 with an
  unrelated body → today's error path unchanged.
- Events preview hygiene: an image tool result's events line carries the
  short note and NEVER >100 chars of base64.
- Sandbox: an image path escaping cwd is refused before any read
  (`path escapes cwd` shape unchanged).
- Plan-mode leg: image read works in plan mode (five-tool contract
  untouched).

## Acceptance

- `cargo test --bin chug image` green (runs EVERY new test — verify the
  filter's run list against the test names, per the T96 lesson).
- Full suite + clippy green; every pre-existing read_file/tools/api pin
  green unmodified.
- RED proofs: (a) removing the image leg fails the fixture test;
  (b) reverting the degrade retry fails the stub-400 test; (c) widening a
  non-image file's behavior is caught by the byte-identical pin.

## Out of scope

Chat/TUI paste & drag (phase 2, deferred — EVALUATION.md cycle-53 §4),
PDFs and other media, image resizing/re-encoding, MCP tool results
carrying images.

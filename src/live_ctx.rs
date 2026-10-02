//! T192 live-context editing (F14 phase 2): the model curates its own
//! message list by editing a mirrored file with ordinary tools.
//!
//! Mechanism (ported from the "Context Language Models" harness shape —
//! mechanism only, no code; that repo is CC BY-NC 4.0): each iteration,
//! pre-LLM call, the driver mirrors the outgoing message list to
//! `.chug/LIVE_CTX.md` as `[[CTX_TURN i role=r]]` blocks with one-line
//! canonical-JSON bodies (post-system-prompt messages only — chug's system
//! prompt is assembled separately in [`crate::driver`] and never mirrored).
//! After the turn, if the file differs from the mirrored bytes, it is
//! parsed back into a candidate message list and gated:
//!
//! 1. *Whole turn blocks* — every block header must name an existing turn
//!    index with the matching role, strictly increasing, once each; the
//!    body must deserialize into a `Message` with the header's role.
//! 2. *Pinned content untouched* — turn 0 (the run's goal message) and
//!    every frozen marker (`[trimmed:` from T77, `[ctx-edit:` from this
//!    module) must stay present and byte-identical up to JSON
//!    normalization.
//! 3. *Pairing intact* — the assembled candidate (edited prefix + this
//!    turn's new tail messages) must keep every assistant `tool_use`
//!    answered by a `tool_result` in the immediately following user
//!    message, and vice versa, or the endpoint would reject the very next
//!    request.
//! 4. *Shrink gate* — the candidate must estimate strictly fewer tokens
//!    than the current list. Growing or equal-size edits are rejected.
//!
//! Non-pinned block bodies MAY be edited (a huge `tool_result` payload
//! shortened, a stale text block dropped) and whole blocks may be deleted;
//! the driver splices the result in at whole-turn granularity, inserting
//! one `[ctx-edit: …]` marker per contiguous run of deleted turns. An
//! accepted edit rewrites `transcript.jsonl` (T77's rewrite discipline) so
//! a `--resume` re-derives the identical context. A rejected edit touches
//! nothing; the driver surfaces a one-line reason to the model and the
//! file is re-mirrored at the next iteration.
//!
//! Honesty note: an accepted edit invalidates the prompt-cache prefix from
//! the edit point (one re-prefill of the surviving suffix). The bet — a
//! sustained smaller context beats a one-time re-prefill in long runs — is
//! measured by T184's cache-token telemetry, not asserted here.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::api::{ContentBlock, KnownBlock, Message};
use crate::driver::estimate_tokens;
use crate::tools;

/// Where the live mirror lives (inside the driver's `.chug/` session state
/// dir, next to `transcript.jsonl` — T55's driver lock covers writes to it).
pub(crate) fn live_ctx_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("LIVE_CTX.md")
}

/// The one-shot occupancy advisory the driver injects pre-call when the
/// estimated context crosses `--ctx-warn-at-tokens` (T13 one-shot latch;
/// the caller latches). Names LIVE_CTX editing as the remedy.
pub(crate) fn occupancy_notice(tokens: usize, threshold: u64) -> String {
    format!(
        "chug: context occupancy: {tokens} estimated tokens reached the \
         {threshold}-token warn threshold. Remedy: compact your own context — \
         edit .chug/LIVE_CTX.md (delete whole [[CTX_TURN i role=...]] blocks you \
         no longer need, or shorten a block's JSON payload) as a turn whose only \
         effect is that edit. An accepted edit-only turn is free: it does not \
         count against the iteration budget (hard cap: 3 consecutive), and the \
         token budget still binds."
    )
}

/// T192: the `[ctx-edit: …]` collapse marker — one message stands in for a
/// contiguous run of turns a LIVE_CTX edit deleted, in the T77 `[trimmed:]`
/// shape (`~Nk` = the deleted run's own token estimate rounded to the
/// nearest 1k, the same `(tokens + 500) / 1000` rounding).
fn ctx_edit_marker_text(tokens: usize, turns: usize) -> String {
    format!(
        "[ctx-edit: ~{}k tokens, {} turns]",
        (tokens + 500) / 1000,
        turns
    )
}

/// A T192 collapse marker written by an earlier accepted edit: same shape
/// rules as trim's `[trimmed:` detection — a user message whose single text
/// block starts with the prefix, so a resumed transcript re-derives the
/// pinned set without hidden state.
pub(crate) fn is_ctx_edit_marker(msg: &Message) -> bool {
    if msg.role != "user" || msg.content.len() != 1 {
        return false;
    }
    match &msg.content[0] {
        ContentBlock::Known(KnownBlock::Text { text }) => text.starts_with("[ctx-edit:"),
        _ => false,
    }
}

/// The pinned turns of a mirrored list: index 0 (the run's goal message —
/// the run contract, and the anchor every later turn hangs from) plus every
/// frozen marker (`[trimmed:` from T77, `[ctx-edit:` from here). A pinned
/// turn can never be deleted and its bytes can never change.
fn is_pinned(original: &[Message], index: usize) -> bool {
    index == 0
        || crate::trim::is_trim_marker(&original[index])
        || is_ctx_edit_marker(&original[index])
}

/// Serialized length of one message, 0 when it cannot serialize (mirrors
/// [`estimate_tokens`], which skips such messages — `Message` is plain JSON
/// data, so this never fires in practice).
fn message_chars(msg: &Message) -> usize {
    serde_json::to_string(msg).map(|s| s.len()).unwrap_or(0)
}

// ---------- mirroring ----------

/// The exact bytes the driver writes pre-call for `messages`: one
/// `[[CTX_TURN i role=r]]` header line plus a one-line canonical-JSON body
/// per message. Deterministic in the list alone, so a test can reproduce
/// the file the loop will mirror.
pub(crate) fn mirror_text(messages: &[Message]) -> String {
    let mut out = String::new();
    for (i, msg) in messages.iter().enumerate() {
        out.push_str("[[CTX_TURN ");
        out.push_str(&i.to_string());
        out.push_str(" role=");
        out.push_str(&msg.role);
        out.push_str("]]\n");
        // `Message` is plain JSON data; serialization cannot fail.
        let body = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());
        out.push_str(&body);
        out.push('\n');
    }
    out
}

/// Best-effort pre-call mirror write. A failure flips [`Mirror::written`]
/// off, which disables this iteration's end-of-turn check (the loop warns
/// once and retries next iteration — the mirror is advisory input, never a
/// run killer).
pub(crate) fn write_mirror(cwd: &Path, text: &str) -> std::io::Result<()> {
    let path = live_ctx_path(cwd);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, text)
}

/// Per-iteration mirror state: the bytes written pre-call, how many prefix
/// messages they covered, and whether the write landed.
pub(crate) struct Mirror {
    pub(crate) text: String,
    pub(crate) len: usize,
    pub(crate) written: bool,
}

impl Mirror {
    /// Write the mirror for `messages` (best-effort) and return the state
    /// the end-of-turn check consumes.
    pub(crate) fn write(cwd: &Path, messages: &[Message]) -> Mirror {
        let text = mirror_text(messages);
        let len = messages.len();
        let written = write_mirror(cwd, &text).is_ok();
        Mirror {
            text,
            len,
            written,
        }
    }
}

// ---------- parse-back ----------

/// One parsed `[[CTX_TURN i role=r]]` block: the named turn index, the
/// header's role, and the raw body text (one canonical-JSON line in the
/// mirrored shape; hand-pretty-printed multi-line JSON bodies are joined
/// and still parse).
struct Block {
    index: usize,
    role: String,
    body: String,
}

fn split_blocks(text: &str) -> Result<Vec<Block>, String> {
    const HEADER_PREFIX: &str = "[[CTX_TURN ";
    let mut blocks: Vec<Block> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue; // blank lines between blocks are tolerated
        }
        if let Some(inner) = line
            .strip_prefix(HEADER_PREFIX)
            .and_then(|s| s.strip_suffix("]]"))
        {
            let (index, role) = parse_header(inner)?;
            blocks.push(Block {
                index,
                role,
                body: String::new(),
            });
            continue;
        }
        match blocks.last_mut() {
            Some(block) => {
                if !block.body.is_empty() {
                    block.body.push('\n');
                }
                block.body.push_str(line);
            }
            None => {
                return Err(format!(
                    "content outside any [[CTX_TURN i role=...]] block: {:?}",
                    one_line(line)
                ))
            }
        }
    }
    Ok(blocks)
}

/// `"<i> role=<role>"` — the header interior. Strict: two fields, integer
/// index, non-empty role.
fn parse_header(inner: &str) -> Result<(usize, String), String> {
    let mut parts = inner.splitn(2, ' ');
    let index = parts
        .next()
        .unwrap_or_default()
        .parse::<usize>()
        .map_err(|_| format!("bad turn index in header [[CTX_TURN {inner}]]"))?;
    let role = parts
        .next()
        .and_then(|r| r.strip_prefix("role="))
        .filter(|r| !r.is_empty())
        .ok_or_else(|| format!("bad role in header [[CTX_TURN {inner}]]"))?;
    Ok((index, role.to_string()))
}

/// Flatten an arbitrary reason fragment to one bounded line (requirement:
/// the rejection reason is one line).
fn one_line(s: &str) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.chars().take(160).collect()
}

/// The parse-back result: the kept turns as `(original_index, parsed
/// message)` in file order (strictly increasing by header validation).
struct ParsedEdit {
    kept: Vec<(usize, Message)>,
}

/// Parse the edited file against the mirrored prefix of the context.
///
/// Enforces requirements 2(a)+(b): whole turn blocks (headers reference
/// existing turn indices, roles match, strictly increasing, once each;
/// bodies deserialize into `Message`s with the header's role) and pinned
/// content untouched (pinned turns present and byte-identical up to JSON
/// normalization). Deliberately NOT enforced here: which non-pinned turns
/// survive, and what a surviving non-pinned body says — that is the edit.
fn parse_edit(text: &str, original: &[Message]) -> Result<ParsedEdit, String> {
    let blocks = split_blocks(text)?;
    let mut kept: Vec<(usize, Message)> = Vec::with_capacity(blocks.len());
    let mut last_index: Option<usize> = None;
    for block in &blocks {
        if block.index >= original.len() {
            return Err(format!(
                "turn {} does not exist: the mirrored context has {} turns",
                block.index,
                original.len()
            ));
        }
        if let Some(prev) = last_index
            && block.index <= prev
        {
            return Err(format!(
                "turns must appear in context order, once each: turn {} follows turn {}",
                block.index, prev
            ));
        }
        last_index = Some(block.index);
        if block.role != original[block.index].role {
            return Err(format!(
                "turn {} role mismatch: header says {}, context has {}",
                block.index,
                block.role,
                original[block.index].role
            ));
        }
        let parsed: Message = serde_json::from_str(block.body.trim()).map_err(|e| {
            format!(
                "turn {} body is not a valid message: {}",
                block.index,
                one_line(&e.to_string())
            )
        })?;
        if parsed.role != block.role {
            return Err(format!(
                "turn {} body role {:?} does not match its header role {:?}",
                block.index, parsed.role, block.role
            ));
        }
        kept.push((block.index, parsed));
    }
    // Pinned content untouched: present AND byte-identical up to JSON
    // normalization (a body re-serialized with different whitespace or key
    // order parses to the same message and passes; any value change fails).
    for (i, original_msg) in original.iter().enumerate() {
        if !is_pinned(original, i) {
            continue;
        }
        let Some((_, parsed_msg)) = kept.iter().find(|(idx, _)| *idx == i) else {
            return Err(format!("pinned turn {i} was removed"));
        };
        let want = serde_json::to_string(original_msg).unwrap_or_default();
        let got = serde_json::to_string(parsed_msg).unwrap_or_default();
        if want != got {
            return Err(format!("pinned content of turn {i} was modified"));
        }
    }
    Ok(ParsedEdit { kept })
}

/// Assemble the candidate list for a parsed edit: the kept (possibly
/// body-edited) messages in context order, with one [`ctx_edit_marker_text`]
/// marker per contiguous run of deleted turns, then this turn's new tail
/// messages (appended after the mirror was written — never editable).
fn assemble(parsed: &ParsedEdit, original: &[Message], tail: &[Message]) -> Vec<Message> {
    let kept: std::collections::HashMap<usize, &Message> =
        parsed.kept.iter().map(|(i, m)| (*i, m)).collect();
    let mut out: Vec<Message> = Vec::with_capacity(original.len() + tail.len());
    let mut run_start: Option<usize> = None;
    for i in 0..original.len() {
        match kept.get(&i) {
            Some(msg) => {
                close_run(original, &mut out, &mut run_start, i);
                out.push((*msg).clone());
            }
            None => {
                run_start.get_or_insert(i);
            }
        }
    }
    close_run(original, &mut out, &mut run_start, original.len());
    out.extend_from_slice(tail);
    out
}

/// Close a deleted run ending just before `next_index`: one marker message
/// standing in for `original[start..next_index]`.
fn close_run(
    original: &[Message],
    out: &mut Vec<Message>,
    run_start: &mut Option<usize>,
    next_index: usize,
) {
    let Some(start) = run_start.take() else {
        return;
    };
    let chars: usize = original[start..next_index].iter().map(message_chars).sum();
    let marker = Message::user(vec![ContentBlock::text_block(ctx_edit_marker_text(
        chars / 4,
        next_index - start,
    ))]);
    out.push(marker);
}

/// The endpoint's pairing invariant over a whole candidate list: every
/// assistant `tool_use` id is answered by a `tool_result` in the
/// immediately following user message, and every `tool_result` answers the
/// immediately preceding message's `tool_use`. This is the shape the loop
/// itself writes (and T77's trim preserves); an edit that orphans either
/// half would make the next API call fail with a 400.
fn pairs_intact(messages: &[Message]) -> Result<(), String> {
    fn tool_use_ids(msg: &Message) -> Vec<&str> {
        msg.content
            .iter()
            .filter_map(ContentBlock::tool_use)
            .map(|(id, _, _)| id)
            .collect()
    }
    fn tool_result_ids(msg: &Message) -> Vec<&str> {
        msg.content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, .. }) => {
                    Some(tool_use_id.as_str())
                }
                _ => None,
            })
            .collect()
    }
    for i in 0..messages.len() {
        let uses = tool_use_ids(&messages[i]);
        let Some(next) = messages.get(i + 1) else {
            if let Some(id) = uses.first() {
                return Err(format!("tool_use {id} at turn {i} has no following tool_result message"));
            }
            continue;
        };
        let results = tool_result_ids(next);
        if !uses.is_empty() && next.role != "user" {
            return Err(format!(
                "tool_use at turn {i} is not immediately followed by a user tool_result message"
            ));
        }
        for id in &uses {
            if !results.contains(id) {
                return Err(format!("tool_use {id} at turn {i} would lose its tool_result"));
            }
        }
        for id in &results {
            if !uses.contains(id) {
                return Err(format!(
                    "tool_result {id} at turn {} would lose its tool_use",
                    i + 1
                ));
            }
        }
    }
    Ok(())
}

// ---------- the end-of-turn check ----------

/// The end-of-turn parse-back verdict. `after_tokens` is the candidate's
/// token estimate when one was parsed (so a gate rejection shows how much
/// the refused edit would have cost or saved); when nothing parseable was
/// proposed it equals `before_tokens` — the context is unchanged.
pub(crate) struct CtxEditCheck {
    pub(crate) accepted: bool,
    pub(crate) before_tokens: usize,
    pub(crate) after_tokens: usize,
    pub(crate) reason: Option<String>,
    pub(crate) new_messages: Option<Vec<Message>>,
}

fn rejected(before: usize, after: usize, reason: String) -> CtxEditCheck {
    CtxEditCheck {
        accepted: false,
        before_tokens: before,
        after_tokens: after,
        reason: Some(reason),
        new_messages: None,
    }
}

/// The end-of-turn LIVE_CTX check (requirement 2). Returns `None` when
/// nothing was attempted: the mirror never landed this iteration, or the
/// file on disk still holds the mirrored bytes (the model did not edit it).
pub(crate) fn check_after_turn(
    cwd: &Path,
    messages: &[Message],
    mirror: &Mirror,
) -> Option<CtxEditCheck> {
    if !mirror.written {
        return None;
    }
    let file = match fs::read_to_string(live_ctx_path(cwd)) {
        Ok(text) => text,
        // The mirror existed pre-call; an unreadable file means the model
        // deleted (or broke) it — a rejected edit, never a silent pass.
        Err(e) => {
            let before = estimate_tokens(messages);
            return Some(rejected(
                before,
                before,
                format!(
                    ".chug/LIVE_CTX.md is missing or unreadable: {}",
                    one_line(&e.to_string())
                ),
            ));
        }
    };
    if file == mirror.text {
        return None; // not edited this turn
    }
    let before = estimate_tokens(messages);
    let (prefix, tail) = messages.split_at(mirror.len.min(messages.len()));
    let parsed = match parse_edit(&file, prefix) {
        Ok(parsed) => parsed,
        Err(reason) => return Some(rejected(before, before, reason)),
    };
    let candidate = assemble(&parsed, prefix, tail);
    let after = estimate_tokens(&candidate);
    if let Err(reason) = pairs_intact(&candidate) {
        return Some(rejected(before, after, reason));
    }
    if after >= before {
        return Some(rejected(
            before,
            after,
            format!(
                "shrink gate: the edited context is not strictly smaller \
                 ({before} -> {after} tokens)"
            ),
        ));
    }
    Some(CtxEditCheck {
        accepted: true,
        before_tokens: before,
        after_tokens: after,
        reason: None,
        new_messages: Some(candidate),
    })
}

// ---------- free-edit-turn classification ----------

/// Per-call effect class for the free-edit-turn rule (requirement 4): a
/// turn's only effect may be an accepted LIVE_CTX edit, so every call in it
/// must be either a pure read or a write to the mirror itself.
pub(crate) enum ToolEffect {
    /// Pure read: no state changes.
    ReadOnly,
    /// A write whose target resolves to the live-context mirror.
    CtxEdit,
    /// Anything else (bash, ledger/todo/decision writes, delegate, MCP
    /// servers, a write anywhere else): disqualifies the free turn.
    Other,
}

/// Tools whose call can never change state, so they never disqualify an
/// edit-only turn. Deliberately tight: everything not listed here — bash
/// foremost, whose side effects are undecidable from a command string —
/// counts as [`ToolEffect::Other`].
const READ_ONLY_TOOLS: &[&str] = &[
    "read_file",
    "grep",
    "glob",
    "list_dir",
    "tgrep",
    "web_fetch",
    "web_search",
    "mcp_resource",
    "todo_list",
];

/// Classify one tool call. The write-path check is LEXICAL — the same
/// resolution [`tools::resolve_safe`] gives the actual `write_file`/
/// `edit_file` dispatch — so the class matches what the tool will really
/// touch.
pub(crate) fn classify_call(cwd: &Path, name: &str, input: &Value) -> ToolEffect {
    if READ_ONLY_TOOLS.contains(&name) {
        return ToolEffect::ReadOnly;
    }
    if name == "write_file" || name == "edit_file" {
        let path = input.get("path").and_then(Value::as_str);
        if let Some(path) = path
            && tools::resolve_safe(cwd, path) == Ok(live_ctx_path(cwd))
        {
            return ToolEffect::CtxEdit;
        }
    }
    ToolEffect::Other
}

// ---------- tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::KnownBlock;

    fn user_text(text: &str) -> Message {
        Message::user(vec![ContentBlock::text_block(text)])
    }

    fn assistant_text(text: &str) -> Message {
        Message::assistant(vec![ContentBlock::text_block(text)])
    }

    fn tool_use_msg(id: &str, name: &str, input: Value) -> Message {
        Message::assistant(vec![ContentBlock::Known(KnownBlock::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        })])
    }

    fn tool_result_msg(id: &str, content: &str) -> Message {
        Message::user(vec![ContentBlock::tool_result_block(
            id,
            content.into(),
            false,
        )])
    }

    /// goal, assistant text, one read_file pair, assistant text.
    fn sample_messages() -> Vec<Message> {
        vec![
            user_text("Goal: fix the thing"),
            assistant_text("Let me look at the repository layout first."),
            tool_use_msg("tu_1", "read_file", serde_json::json!({"path": "src/main.rs"})),
            tool_result_msg("tu_1", "fn main() { println!(\"hi\"); }"),
            assistant_text("Done looking at the repository layout."),
        ]
    }

    /// Remove whole turns (header line + following body lines) from mirror
    /// text — the scripted-edit shape the tests use.
    fn drop_turns(text: &str, turns: &[usize]) -> String {
        let mut out = String::new();
        let mut skipping = false;
        for line in text.lines() {
            if let Some(inner) = line.strip_prefix("[[CTX_TURN ").and_then(|s| s.strip_suffix("]]")) {
                let index = inner.split(' ').next().unwrap_or_default();
                skipping = index.parse::<usize>().is_ok_and(|i| turns.contains(&i));
            }
            if !skipping {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    // ----- mirror round-trip (test 1) -----

    /// messages -> LIVE_CTX.md on disk -> parse-back -> identical list.
    #[test]
    fn mirror_round_trip_through_the_file_is_identical() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        write_mirror(tmp.path(), &mirror_text(&messages)).unwrap();
        let from_disk = fs::read_to_string(live_ctx_path(tmp.path())).unwrap();
        assert_eq!(from_disk, mirror_text(&messages));
        let parsed = parse_edit(&from_disk, &messages).expect("mirror parses");
        assert_eq!(assemble(&parsed, &messages, &[]), messages);
    }

    /// The mirror header shape is the documented one, bodies single lines.
    #[test]
    fn mirror_headers_carry_index_and_role() {
        let text = mirror_text(&sample_messages());
        assert!(text.starts_with("[[CTX_TURN 0 role=user]]\n"));
        assert!(text.contains("\n[[CTX_TURN 1 role=assistant]]\n"));
        for (n, line) in text.lines().enumerate() {
            if n % 2 == 0 {
                assert!(line.starts_with("[[CTX_TURN "), "{line}");
            } else {
                assert!(line.starts_with('{'), "{line}");
                assert!(serde_json::from_str::<Message>(line).is_ok(), "{line}");
            }
        }
    }

    // ----- gates (tests 2-5) -----

    /// Growth is rejected by the shrink gate through the real end-of-turn
    /// path: a padded non-pinned body re-estimates larger than the original.
    #[test]
    fn gate_growth_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let grown = text.replace(
            "Done looking at the repository layout.",
            "Done looking at the repository layout, and now padded with a great \
             deal of extra commentary so the candidate context estimates strictly \
             more tokens than the original list did.",
        );
        let mirror = Mirror {
            text,
            len: messages.len(),
            written: true,
        };
        write_mirror(tmp.path(), &grown).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        assert!(
            check
                .reason
                .as_deref()
                .unwrap_or_default()
                .starts_with("shrink gate"),
            "{:?}",
            check.reason
        );
        assert!(check.new_messages.is_none());
        // Telemetry: the refused candidate's size is what the gate saw.
        let parsed = parse_edit(&grown, &messages).unwrap();
        assert_eq!(
            check.after_tokens,
            estimate_tokens(&assemble(&parsed, &messages, &[]))
        );
        assert!(check.after_tokens > check.before_tokens);
    }

    /// Equal size is rejected: a token-neutral body rewrite re-estimates
    /// exactly the original count, which is not strictly smaller.
    #[test]
    fn gate_equal_size_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        // Same byte length, different bytes: '.' traded for an extra letter.
        let equal = text.replace(
            "Done looking at the repository layout.",
            "Done looking at the reposeetory layout",
        );
        assert_eq!(equal.len(), text.len(), "test setup: equal-size rewrite");
        let parsed = parse_edit(&equal, &messages).unwrap();
        assert_eq!(
            estimate_tokens(&assemble(&parsed, &messages, &[])),
            estimate_tokens(&messages),
            "test setup: the rewrite must be token-neutral"
        );
        let mirror = Mirror {
            text,
            len: messages.len(),
            written: true,
        };
        write_mirror(tmp.path(), &equal).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        assert!(
            check
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("not strictly smaller"),
            "{:?}",
            check.reason
        );
        assert_eq!(check.after_tokens, check.before_tokens);
    }

    /// A shrinking edit (delete the read_file pair, drop whole turns) is
    /// accepted; the candidate carries one marker for the deleted run.
    #[test]
    fn gate_shrink_accepted_with_marker() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let edited = drop_turns(&text, &[2, 3]);
        let mirror = Mirror {
            text: text.clone(),
            len: messages.len(),
            written: true,
        };
        write_mirror(tmp.path(), &edited).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(check.accepted, "{:?}", check.reason);
        assert!(check.reason.is_none());
        let new_messages = check.new_messages.expect("accepted carries the list");
        // Kept turns 0, 1, 4 plus one marker for the deleted 2..4 run.
        assert_eq!(new_messages.len(), 4);
        assert_eq!(new_messages[0], messages[0]);
        assert_eq!(new_messages[1], messages[1]);
        assert_eq!(new_messages[3], messages[4]);
        let marker = new_messages[2].content[0].text().unwrap();
        assert!(marker.starts_with("[ctx-edit:"), "{marker}");
        assert!(marker.contains("2 turns"), "{marker}");
        assert!(is_ctx_edit_marker(&new_messages[2]));
        assert!(check.after_tokens < check.before_tokens);
    }

    /// A malformed file is rejected with a one-line reason naming the leg:
    /// garbage outside blocks, broken JSON, unknown index, role mismatch,
    /// duplicates, out-of-order headers.
    #[test]
    fn gate_malformed_rejected_with_reason() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let mirror = Mirror {
            text: text.clone(),
            len: messages.len(),
            written: true,
        };
        let body = |i: usize| serde_json::to_string(&messages[i]).unwrap();
        let cases: Vec<(String, &str)> = vec![
            (
                "not a mirror at all\n".to_string(),
                "content outside any [[CTX_TURN",
            ),
            ("[[CTX_TURN 0 role=user]]\n{broken json\n".to_string(), "body is not a valid message"),
            (
                "[[CTX_TURN 99 role=user]]\n{}\n".to_string(),
                "does not exist",
            ),
            (
                "[[CTX_TURN 1 role=user]]\n{\"role\":\"user\",\"content\":[]}\n".to_string(),
                "role mismatch",
            ),
            (
                "[[CTX_TURN 1 role=assistant]]\n".to_string()
                    + &body(1)
                    + "\n[[CTX_TURN 1 role=assistant]]\n"
                    + &body(1)
                    + "\n",
                "once each",
            ),
            (
                "[[CTX_TURN 2 role=assistant]]\n".to_string()
                    + &body(2)
                    + "\n[[CTX_TURN 1 role=assistant]]\n"
                    + &body(1)
                    + "\n",
                "context order",
            ),
            (
                "[[CTX_TURN 2 role=assistant]]\n{\"role\":\"user\",\"content\":[]}\n".to_string(),
                "does not match its header role",
            ),
        ];
        for (file, needle) in cases {
            write_mirror(tmp.path(), &file).unwrap();
            let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
            assert!(!check.accepted, "{file}");
            let reason = check.reason.as_deref().unwrap_or_default();
            assert!(reason.contains(needle), "{reason} ~ {needle}");
            assert!(!reason.contains('\n'), "reason is one line: {reason}");
            assert!(check.new_messages.is_none());
            assert_eq!(check.after_tokens, check.before_tokens);
        }
    }

    /// Pinned turns — turn 0 and frozen markers — cannot be deleted or
    /// rewritten (requirement 2b: "no pinned content is touched").
    #[test]
    fn gate_pinned_tamper_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let mut messages = sample_messages();
        // Plant a frozen ctx-edit marker in the middle.
        messages.insert(
            2,
            Message::user(vec![ContentBlock::text_block(
                "[ctx-edit: ~2k tokens, 3 turns]".to_string(),
            )]),
        );
        let text = mirror_text(&messages);
        let mirror = Mirror {
            text: text.clone(),
            len: messages.len(),
            written: true,
        };

        // Drop the pinned marker's whole block.
        let dropped = drop_turns(&text, &[2]);
        write_mirror(tmp.path(), &dropped).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        assert!(
            check
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("pinned turn 2 was removed"),
            "{:?}",
            check.reason
        );

        // Rewrite the goal message (turn 0) body.
        let tampered = text.replace("Goal: fix the thing", "Goal: fix the OTHER thing");
        write_mirror(tmp.path(), &tampered).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        assert!(
            check
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("pinned content of turn 0 was modified"),
            "{:?}",
            check.reason
        );
    }

    /// A non-pinned body may be rewritten (shrunk) while pinned bytes stay
    /// byte-identical: the acceptance path for real curation.
    #[test]
    fn non_pinned_body_edit_accepted() {
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let edited = text.replace(
            &serde_json::to_string(&messages[4]).unwrap(),
            &serde_json::to_string(&assistant_text("Summary: looked, done.")).unwrap(),
        );
        let parsed = parse_edit(&edited, &messages).expect("parses");
        let candidate = assemble(&parsed, &messages, &[]);
        assert_eq!(candidate[4], assistant_text("Summary: looked, done."));
        assert!(estimate_tokens(&candidate) < estimate_tokens(&messages));
    }

    /// A normalized pinned body (same message, different serialization
    /// whitespace/key order) still passes the pinned check.
    #[test]
    fn pinned_body_normalized_comparison() {
        let messages = vec![user_text("Goal: keep me"), assistant_text("ok")];
        let text = mirror_text(&messages);
        // Pretty-print turn 0's body: same value, different bytes.
        let pretty = serde_json::to_string_pretty(&messages[0]).unwrap();
        let edited = text.replace(&serde_json::to_string(&messages[0]).unwrap(), &pretty);
        let parsed = parse_edit(&edited, &messages).expect("normalized pinned body passes");
        assert_eq!(assemble(&parsed, &messages, &[]), messages);
    }

    /// An edit that orphans a tool_use (drop only the tool_result turn) is
    /// rejected before the shrink gate, with a pairing reason.
    #[test]
    fn pairing_orphan_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let dropped = drop_turns(&text, &[3]);
        let mirror = Mirror {
            text: text.clone(),
            len: messages.len(),
            written: true,
        };
        write_mirror(tmp.path(), &dropped).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        let reason = check.reason.as_deref().unwrap_or_default();
        assert!(
            reason.contains("tool_use") && reason.contains("tool_result"),
            "{reason}"
        );
        // Both halves dropped together is FINE (pair intact, both gone).
        let both = drop_turns(&text, &[2, 3]);
        write_mirror(tmp.path(), &both).unwrap();
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(check.accepted, "{:?}", check.reason);
    }

    /// No edit attempted: byte-identical file and an unwritten mirror both
    /// yield `None` (no check, no event, no transcript churn).
    #[test]
    fn no_change_and_unwritten_mirror_yield_none() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let text = mirror_text(&messages);
        write_mirror(tmp.path(), &text).unwrap();
        let mirror = Mirror {
            text: text.clone(),
            len: messages.len(),
            written: true,
        };
        assert!(check_after_turn(tmp.path(), &messages, &mirror).is_none());

        let unwritten = Mirror {
            text,
            len: messages.len(),
            written: false,
        };
        write_mirror(tmp.path(), "edited\n").unwrap();
        assert!(check_after_turn(tmp.path(), &messages, &unwritten).is_none());
    }

    /// A deleted mirror file is a rejected edit, never a silent pass; the
    /// context is unchanged (after == before).
    #[test]
    fn deleted_mirror_file_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let messages = sample_messages();
        let mirror = Mirror {
            text: mirror_text(&messages),
            len: messages.len(),
            written: true,
        };
        assert!(!live_ctx_path(tmp.path()).exists());
        let check = check_after_turn(tmp.path(), &messages, &mirror).unwrap();
        assert!(!check.accepted);
        assert!(
            check
                .reason
                .as_deref()
                .unwrap_or_default()
                .contains("missing or unreadable")
        );
        assert_eq!(check.after_tokens, check.before_tokens);
    }

    /// The tail (messages appended after the mirror) survives an accepted
    /// edit untouched, and pairing holds across the prefix/tail seam.
    #[test]
    fn tail_is_preserved_across_the_seam() {
        let tmp = tempfile::tempdir().unwrap();
        let prefix = sample_messages();
        let tail = vec![
            tool_use_msg("tu_9", "bash", serde_json::json!({"command": "true"})),
            tool_result_msg("tu_9", "ok"),
        ];
        let mut all = prefix.clone();
        all.extend(tail.clone());
        let text = mirror_text(&prefix);
        let mirror = Mirror {
            text: text.clone(),
            len: prefix.len(),
            written: true,
        };
        let edited = drop_turns(&text, &[2, 3]);
        write_mirror(tmp.path(), &edited).unwrap();
        let check = check_after_turn(tmp.path(), &all, &mirror).unwrap();
        assert!(check.accepted, "{:?}", check.reason);
        let new_messages = check.new_messages.unwrap();
        let tail_start = new_messages.len() - tail.len();
        assert_eq!(&new_messages[tail_start..], &tail[..]);
        assert!(pairs_intact(&new_messages).is_ok());
    }

    /// Marker text matches the T77 `~Nk` rounding shape.
    #[test]
    fn marker_text_matches_t77_rounding() {
        assert_eq!(
            ctx_edit_marker_text(1_640, 3),
            "[ctx-edit: ~2k tokens, 3 turns]"
        );
        assert_eq!(ctx_edit_marker_text(1_499, 1), "[ctx-edit: ~1k tokens, 1 turns]");
        assert_eq!(ctx_edit_marker_text(0, 1), "[ctx-edit: ~0k tokens, 1 turns]");
    }

    /// Blank lines between blocks are tolerated; block detection is
    /// line-anchored so JSON bodies never read as headers.
    #[test]
    fn blank_lines_tolerated() {
        let messages = sample_messages();
        let text = mirror_text(&messages);
        let spaced = text.replace("]]\n{", "]]\n\n   \n{");
        let parsed = parse_edit(&spaced, &messages).expect("blank lines are not content");
        assert_eq!(assemble(&parsed, &messages, &[]), messages);
    }

    // ----- free-edit-turn classification -----

    #[test]
    fn read_only_tools_classify_read_only() {
        let cwd = Path::new("/w");
        for name in READ_ONLY_TOOLS {
            assert!(
                matches!(
                    classify_call(cwd, name, &serde_json::json!({})),
                    ToolEffect::ReadOnly
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn ctx_writes_classify_by_resolved_path() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path();
        let ctx_file = live_ctx_path(cwd);
        let ctx_rel = ctx_file.strip_prefix(cwd).unwrap().to_str().unwrap();
        // The mirror, three spellings: relative, ./-prefixed, absolute.
        for path in [
            ctx_rel.to_string(),
            format!("./{ctx_rel}"),
            ctx_file.to_string_lossy().to_string(),
        ] {
            assert!(
                matches!(
                    classify_call(
                        cwd,
                        "write_file",
                        &serde_json::json!({"path": path, "content": "x"})
                    ),
                    ToolEffect::CtxEdit
                ),
                "{path}"
            );
            assert!(
                matches!(
                    classify_call(
                        cwd,
                        "edit_file",
                        &serde_json::json!({"path": path, "old": "a", "new": "b"})
                    ),
                    ToolEffect::CtxEdit
                ),
                "{path}"
            );
        }
        // Any other write target is Other.
        assert!(matches!(
            classify_call(
                cwd,
                "write_file",
                &serde_json::json!({"path": "src/x.rs", "content": "x"})
            ),
            ToolEffect::Other
        ));
        assert!(matches!(
            classify_call(
                cwd,
                "write_file",
                &serde_json::json!({"path": "../escape", "content": "x"})
            ),
            ToolEffect::Other
        ));
        assert!(matches!(
            classify_call(
                cwd,
                "edit_file",
                &serde_json::json!({"path": "LEDGER.md", "old": "a", "new": "b"})
            ),
            ToolEffect::Other
        ));
    }

    #[test]
    fn everything_else_classifies_other() {
        let cwd = Path::new("/w");
        let cases = [
            ("bash", serde_json::json!({"command": "echo hi"})),
            ("update_ledger", serde_json::json!({"content": "# L"})),
            ("todo_add", serde_json::json!({"title": "t"})),
            ("todo_update", serde_json::json!({"id": "t1", "status": "done"})),
            (
                "decision_log",
                serde_json::json!({"class": "outcome", "subject": "s", "inputs": "i", "options": "o", "choice": "landed-clean", "confidence": 1}),
            ),
            ("delegate", serde_json::json!({"action": "launch"})),
            ("goal_complete", serde_json::json!({"summary": "s"})),
            ("mcp__fake__echo", serde_json::json!({"text": "x"})),
            ("submit_plan", serde_json::json!({"plan": "p"})),
            ("write_file", serde_json::json!({})),
        ];
        for (name, input) in cases {
            assert!(
                matches!(classify_call(cwd, name, &input), ToolEffect::Other),
                "{name}"
            );
        }
    }

    /// The occupancy advisory names the remedy and the numbers at fire time,
    /// on one line.
    #[test]
    fn occupancy_notice_names_remedy_and_counts() {
        let notice = occupancy_notice(142_000, 120_000);
        assert!(notice.contains("142000"), "{notice}");
        assert!(notice.contains("120000"), "{notice}");
        assert!(notice.contains(".chug/LIVE_CTX.md"), "{notice}");
        assert!(notice.contains("[[CTX_TURN"), "{notice}");
        assert!(!notice.contains('\n'), "one line: {notice}");
    }
}

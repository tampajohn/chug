//! T77 transcript-trim machinery, extracted from driver.rs (T84) on the T71
//! delegate.rs precedent: a byte-identical move, no logic edits, no renames.
//! Fixed 16k-token segments collapse to frozen `[trimmed: ...]` markers once
//! and never change again, keeping the assembled request prefix byte-stable
//! (prompt-cache friendly). driver.rs keeps only the two assembly call sites,
//! which call `trim::transcript_trim`; token estimation stays in driver.rs
//! (`estimate_tokens`, mirrored by [`message_chars`] here).

use crate::api::{ContentBlock, KnownBlock, Message};
use crate::driver::estimate_tokens;

const TRIM_ABOVE_TOKENS: usize = 120_000;
const TRIM_TARGET_TOKENS: usize = 80_000;
const KEEP_LAST_MESSAGES: usize = 20;

/// T77: fixed 16k-token trim segments. Once a segment this size is collapsed
/// its bytes never change again — only whole oldest-complete segments are
/// ever edited, so each trim event appends one collapse at the frozen
/// boundary and the request prefix stays byte-stable (prompt-cache friendly).
const SEGMENT_TOKENS: usize = 16_000;

/// Serialized length of one message, 0 when it cannot serialize (mirrors
/// [`estimate_tokens`], which skips such messages).
fn message_chars(msg: &Message) -> usize {
    serde_json::to_string(msg).map(|s| s.len()).unwrap_or(0)
}

fn count_tool_results(msg: &Message) -> usize {
    msg.content
        .iter()
        .filter(|b| matches!(b, ContentBlock::Known(KnownBlock::ToolResult { .. })))
        .count()
}

/// A user message carrying tool_result blocks. In the loop's message flow
/// such a message immediately follows the assistant message holding the
/// matching tool_use, so a segment boundary must never fall right before it.
fn is_tool_result_user(msg: &Message) -> bool {
    msg.role == "user" && count_tool_results(msg) > 0
}

/// A T77 collapse marker written by an earlier trim: a user message whose
/// single text block starts with `[trimmed:`. Assistant text can never match
/// (role differs) and operator notes are prefixed `[operator]`, so detection
/// needs no hidden state — a resumed transcript re-segments identically.
fn is_trim_marker(msg: &Message) -> bool {
    if msg.role != "user" || msg.content.len() != 1 {
        return false;
    }
    match &msg.content[0] {
        ContentBlock::Known(KnownBlock::Text { text }) => text.starts_with("[trimmed:"),
        _ => false,
    }
}

/// T77: the segment-level collapse marker. One message stands in for the
/// whole segment; `~Nk` is the segment's own token estimate rounded to the
/// nearest 1k (canonical fixed 16k segments render exactly `~16k`), and the
/// tool-result count is what the segment held.
fn trim_marker_text(tokens: usize, tool_results: usize) -> String {
    format!(
        "[trimmed: ~{}k tokens, {} tool results]",
        (tokens + 500) / 1000,
        tool_results
    )
}

/// One trimmable segment: the half-open message range `[start..end)` inside
/// the trimmable window (never message 0, never the last
/// [`KEEP_LAST_MESSAGES`] messages), its token estimate and tool-result
/// count.
struct TrimSegment {
    start: usize,
    end: usize, // exclusive
    tokens: usize,
    tool_results: usize,
    /// Accumulated to a full [`SEGMENT_TOKENS`] block — the only kind T77
    /// collapses. A segment younger than the threshold stays verbatim.
    complete: bool,
    /// Already collapsed to a marker on an earlier trim: frozen bytes, kept
    /// as its own singleton segment so later segments chain from the same
    /// position forever.
    collapsed: bool,
    /// Collapsing would strand a tool_use/tool_result pair across the
    /// segment boundary.
    pairing_unsafe: bool,
}

/// T77: partition the trimmable window into FIXED ~16k-token segments with a
/// deterministic front-to-back walk, so the same message prefix always
/// partitions identically and boundaries never move once written. Markers
/// from earlier trims are singleton frozen segments; a segment completes
/// when its accumulation reaches [`SEGMENT_TOKENS`], then absorbs the
/// immediately following tool_result user message(s) so a collapse never
/// separates an assistant tool_use from its result.
fn plan_trim_segments(messages: &[Message], window_end: usize) -> Vec<TrimSegment> {
    let mut segments = Vec::new();
    let mut i = 1; // message 0 is never trimmed
    while i < window_end {
        if is_trim_marker(&messages[i]) {
            segments.push(TrimSegment {
                start: i,
                end: i + 1,
                tokens: 0,
                tool_results: 0,
                complete: false,
                collapsed: true,
                pairing_unsafe: false,
            });
            i += 1;
            continue;
        }
        let start = i;
        let (mut chars, mut tool_results) = (0usize, 0usize);
        let mut end = i;
        while end < window_end {
            chars += message_chars(&messages[end]);
            tool_results += count_tool_results(&messages[end]);
            end += 1;
            if chars / 4 >= SEGMENT_TOKENS {
                break;
            }
        }
        let complete = chars / 4 >= SEGMENT_TOKENS;
        // Pairing extension: a complete segment must not end right before a
        // tool_result message — the tool_use it answers sits inside the
        // segment. Absorb the result while it is still inside the window.
        while complete && end < window_end && is_tool_result_user(&messages[end]) {
            chars += message_chars(&messages[end]);
            tool_results += count_tool_results(&messages[end]);
            end += 1;
        }
        // Pairing safety: a tool_result in the segment's FIRST message pairs
        // with a tool_use before the segment, and a segment ending at the
        // window edge must not leave the first protected message a stranded
        // tool_result. Either way the segment is not safe to collapse.
        let pairing_unsafe = count_tool_results(&messages[start]) > 0
            || (complete && messages.get(end).is_some_and(is_tool_result_user));
        segments.push(TrimSegment {
            start,
            end,
            tokens: chars / 4,
            tool_results,
            complete,
            collapsed: false,
            pairing_unsafe,
        });
        i = end;
    }
    segments
}

/// T184: the telemetry one trim pass hands the driver's `Event::Trim` —
/// estimated tokens before/after the pass, how many segments THIS pass
/// collapsed, and the total `[trimmed: …]` marker count after.
pub(crate) struct TrimStats {
    pub before_tokens: usize,
    pub after_tokens: usize,
    /// Collapsed in THIS pass: every collapse splices in exactly one marker,
    /// so this is the marker-count delta across the pass (frozen markers from
    /// earlier passes sit in both counts and cancel).
    pub segments_collapsed: usize,
    pub marker_count: usize,
}

fn count_trim_markers(messages: &[Message]) -> usize {
    messages.iter().filter(|m| is_trim_marker(m)).count()
}

/// T184 caller-side seam: one [`transcript_trim`] pass plus the telemetry the
/// driver records as a Trim event. `None` = the pass was a no-op (nothing
/// collapsed) — no event, exactly as `transcript_trim` returning false means
/// no transcript rewrite.
pub(crate) fn transcript_trim_stats(messages: &mut Vec<Message>) -> Option<TrimStats> {
    let before_tokens = estimate_tokens(messages);
    let markers_before = count_trim_markers(messages);
    if !transcript_trim(messages) {
        return None;
    }
    let after_tokens = estimate_tokens(messages);
    let marker_count = count_trim_markers(messages);
    Some(TrimStats {
        before_tokens,
        after_tokens,
        segments_collapsed: marker_count - markers_before,
        marker_count,
    })
}

/// Transcript trimming (T77 — cache-stable, segment-frozen): above
/// [`TRIM_ABOVE_TOKENS`] estimated tokens, collapse whole oldest-complete
/// [`SEGMENT_TOKENS`] segments until under [`TRIM_TARGET_TOKENS`]. Each
/// collapse replaces the ENTIRE segment with one marker message —
/// `[trimmed: ~16k tokens, N tool results]` — written once and never edited
/// again: every trim event only appends collapses at the frozen boundary, so
/// the request prefix up to the last marker stays byte-stable across
/// assemblies (prompt-cache friendly, unlike the old mid-history `[trimmed]`
/// gutting which re-edited bytes at an arbitrary message boundary). Message
/// 0 and the last [`KEEP_LAST_MESSAGES`] messages are never touched; a
/// segment younger than the threshold stays verbatim. Ledger, resume and
/// rotation semantics are unchanged — the caller still rewrites the
/// transcript file only when something collapsed.
pub(crate) fn transcript_trim(messages: &mut Vec<Message>) -> bool {
    if estimate_tokens(messages) <= TRIM_ABOVE_TOKENS {
        return false;
    }
    let window_end = messages.len().saturating_sub(KEEP_LAST_MESSAGES);
    if window_end <= 1 {
        return false; // nothing outside message 0 + the protected tail
    }
    let segments = plan_trim_segments(messages, window_end);
    // Choose oldest-first (the frozen boundary only ever moves forward),
    // tracking the estimated total as each segment shrinks to its marker.
    let mut total = estimate_tokens(messages);
    let mut chosen: Vec<(usize, usize, Message)> = Vec::new();
    for seg in &segments {
        if total <= TRIM_TARGET_TOKENS {
            break;
        }
        if seg.collapsed || !seg.complete || seg.pairing_unsafe {
            continue;
        }
        let marker = Message::user(vec![ContentBlock::text_block(trim_marker_text(
            seg.tokens, seg.tool_results,
        ))]);
        total = total
            .saturating_sub(seg.tokens)
            .saturating_add(message_chars(&marker) / 4);
        chosen.push((seg.start, seg.end, marker));
    }
    if chosen.is_empty() {
        return false;
    }
    // Splice back-to-front so earlier ranges stay valid.
    chosen.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
    for (start, end, marker) in chosen {
        messages.splice(start..end, std::iter::once(marker));
    }
    true
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ScriptedLlm;
    use crate::driver::tests::{ctx_for, knobs_with, tool_use_response, RecordingSink};
    use crate::driver::{Controls, DriveOutcome, Mode, SlashUpdate, drive_loop};
    use crate::mcp::McpRegistry;
    use crate::observ;
    use serde_json::json;
    use std::sync::mpsc;

    fn tool_result_msg(text: &str) -> Message {
        Message::user(vec![ContentBlock::tool_result_block("t1", text.to_string(), false)])
    }

    /// T77 fixture: one realistic exchange — assistant(tool_use) followed by
    /// user(tool_result) — the shape every trimmable transcript region has.
    fn use_result_pair(id: &str, payload: &str) -> Vec<Message> {
        vec![
            Message::assistant(vec![ContentBlock::Known(KnownBlock::ToolUse {
                id: id.to_string(),
                name: "bash".into(),
                input: json!({ "command": payload }),
            })]),
            Message::user(vec![ContentBlock::tool_result_block(
                id,
                payload.to_string(),
                false,
            )]),
        ]
    }
    #[test]
    fn trimming_preserves_last_20_and_first_message() {
        let long = "x".repeat(25_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("first message")])];
        for i in 0..15 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages));

        // Message 0 is never touched.
        assert_eq!(messages[0].content[0].text(), Some("first message"));
        // The trimmable pool collapses into segment markers; the protected
        // tail keeps its bytes.
        let markers = marker_texts(&messages);
        assert!(!markers.is_empty(), "expected at least one segment marker");
        // No gutted `"[trimmed]"` payloads anywhere — old-style edits are
        // gone; whole segments become markers.
        for (i, msg) in messages.iter().enumerate() {
            if i == 0 || is_trim_marker(msg) {
                if is_trim_marker(msg) {
                    assert_eq!(msg.role, "user");
                }
                continue;
            }
            match &msg.content[0] {
                ContentBlock::Known(KnownBlock::ToolResult { content, .. }) => {
                    assert_eq!(content, &long, "verbatim tool_result");
                }
                ContentBlock::Known(KnownBlock::ToolUse { input, .. }) => {
                    assert_eq!(input["command"], json!(long), "verbatim tool_use");
                }
                other => panic!("unexpected block {other:?}"),
            }
        }
        // The protected tail is untouched.
        for msg in &messages[messages.len() - KEEP_LAST_MESSAGES..] {
            assert!(!is_trim_marker(msg), "tail must never carry a marker");
        }
    }

    #[test]
    fn trimming_noop_under_threshold() {
        let mut messages = vec![tool_result_msg("small"), tool_result_msg("tiny")];
        assert!(!transcript_trim(&mut messages));
        let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &messages[0].content[0]
        else {
            panic!("expected tool_result");
        };
        assert_eq!(content, "small");
    }

    #[test]
    fn trimming_respects_target_or_exhausts() {
        let long = "y".repeat(10_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("first")])];
        for i in 0..30 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        assert!(transcript_trim(&mut messages));
        // 61 messages of ~10k-char payloads start above the 120k-token
        // threshold; the trimmable pool (everything older than the last 20)
        // is big enough to reach the 80k-token target.
        assert!(estimate_tokens(&messages) <= TRIM_TARGET_TOKENS);
        // The last 20 are untouched.
        for msg in &messages[messages.len() - KEEP_LAST_MESSAGES..] {
            assert!(!is_trim_marker(msg), "tail never carries a marker");
        }
    }

    /// T184: the caller-side trim-event seam — `transcript_trim_stats`
    /// reports exactly one stats set per collapsing pass (`None` on a no-op
    /// pass: no event), with before > after, this pass's collapses as the
    /// marker delta, and the total marker count after. The driver's Trim
    /// event serializes straight from this.
    #[test]
    fn trim_stats_seam_reports_before_after_segments_and_markers() {
        let long = "x".repeat(25_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("first message")])];
        for i in 0..15 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);

        let stats = transcript_trim_stats(&mut messages).expect("a collapsing pass reports stats");
        assert!(
            stats.before_tokens > stats.after_tokens,
            "before {} > after {}",
            stats.before_tokens,
            stats.after_tokens
        );
        assert!(stats.segments_collapsed >= 1, "the pass collapsed something");
        assert_eq!(
            stats.marker_count, stats.segments_collapsed,
            "no frozen markers before the pass: the delta IS the collapse count"
        );
        // Re-running on the same transcript cannot collapse anything new
        // (frozen markers + the protected tail only) → a no-op → no event.
        assert!(
            transcript_trim_stats(&mut messages).is_none(),
            "a no-op pass reports None: exactly one Trim event per collapsing pass"
        );

        // Grow past the next threshold: the second pass's delta counts only
        // the NEW collapses; the first pass's frozen markers ride the total.
        for i in 15..27 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        let stats2 = transcript_trim_stats(&mut messages).expect("second collapsing pass");
        assert!(stats2.before_tokens > stats2.after_tokens);
        assert!(stats2.segments_collapsed >= 1);
        assert_eq!(
            stats2.marker_count,
            stats.marker_count + stats2.segments_collapsed,
            "total markers = frozen markers + this pass's collapses"
        );
    }

    /// T77: a frozen segment is byte-identical after later trims — the
    /// marker sequence only ever grows by appendage, and the request prefix
    /// through the last frozen marker never changes.
    #[test]
    fn trim_frozen_segments_byte_identical_after_later_trims() {
        let long = "z".repeat(25_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        let mut pid = 0usize;
        let add_pairs = |messages: &mut Vec<Message>, n: usize, pid: &mut usize| {
            for _ in 0..n {
                let payload = format!("{pid} {long}");
                messages.extend(use_result_pair(&format!("tu_{pid}"), &payload));
                *pid += 1;
            }
        };
        add_pairs(&mut messages, 12, &mut pid);
        assert!(transcript_trim(&mut messages), "first trim collapses");
        let markers_after_1 = marker_texts(&messages);
        assert!(!markers_after_1.is_empty());
        // Frozen prefix: everything through the last marker, serialized.
        let frozen_end = last_marker_index(&messages);
        let frozen_1 = prefix_bytes(&messages, frozen_end);

        // Grow past the next threshold and trim again.
        add_pairs(&mut messages, 12, &mut pid);
        assert!(transcript_trim(&mut messages), "second trim collapses");
        let markers_after_2 = marker_texts(&messages);
        assert!(
            markers_after_2.len() > markers_after_1.len(),
            "markers only grow: {} -> {}",
            markers_after_1.len(),
            markers_after_2.len()
        );
        assert_eq!(
            &markers_after_2[..markers_after_1.len()],
            &markers_after_1[..],
            "old markers are frozen bytes, never rewritten"
        );
        assert_eq!(
            prefix_bytes(&messages, frozen_end),
            frozen_1,
            "prefix up to the last frozen boundary is byte-stable"
        );
    }

    /// T77: marker format — the segment-level marker `[trimmed: ~16k tokens,
    /// N tool results]` stands in for the WHOLE segment (one edit per
    /// segment, once ever). A pair of ~32k-char payloads is one canonical
    /// ~16k segment, so every marker renders the exact spec format.
    #[test]
    fn trim_marker_format_is_canonical_16k() {
        let long = "q".repeat(32_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        for i in 0..10 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        // 21 messages: the 20-message protected tail swallows the whole pool
        // — nothing trimmable yet, no partial edits either.
        assert!(!transcript_trim(&mut messages), "protected tail swallows all");
        assert!(marker_texts(&messages).is_empty());
        assert!(messages.iter().all(|m| !is_trim_marker(m)));

        for i in 10..20 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        assert!(transcript_trim(&mut messages));
        let markers = marker_texts(&messages);
        assert_eq!(markers.len(), 10, "one marker per collapsed pair-segment");
        for m in &markers {
            assert_eq!(
                m, "[trimmed: ~16k tokens, 1 tool results]",
                "canonical 16k segment marker format"
            );
        }
        // Message 0 never touched.
        assert_eq!(messages[0].content[0].text(), Some("kick"));
    }

    /// T77: threshold math at segment granularity — only whole
    /// oldest-complete segments collapse until the target; complete segments
    /// YOUNGER than the point where the target is met stay verbatim.
    #[test]
    fn trim_young_complete_segments_stay_verbatim() {
        let long = "w".repeat(4_400);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        for i in 0..60 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &long));
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages));

        let markers = marker_texts(&messages);
        assert!(!markers.is_empty());
        // The target was met before the pool ran out: young complete
        // segments survive verbatim between the last marker and the tail.
        let young: Vec<&Message> = messages[frozen_or_marker_end(&messages)..]
            .iter()
            .take(messages.len() - KEEP_LAST_MESSAGES - frozen_or_marker_end(&messages))
            .collect();
        assert!(
            !young.is_empty(),
            "target met with pool to spare leaves young segments"
        );
        for msg in &young {
            assert!(!is_trim_marker(msg));
            match &msg.content[0] {
                ContentBlock::Known(KnownBlock::ToolResult { content, .. }) => {
                    assert!(content.as_str().unwrap_or_default().contains(&long));
                }
                ContentBlock::Known(KnownBlock::ToolUse { .. }) => {}
                other => panic!("unexpected block {other:?}"),
            }
        }
        // The estimate landed at or under the target: trimming stops once
        // under 80k, whole segments only.
        assert!(estimate_tokens(&messages) <= TRIM_TARGET_TOKENS);
    }

    /// T77 integration: a scripted long run past two trim thresholds keeps
    /// the request prefix up to the last frozen boundary byte-stable across
    /// three consecutive API request assemblies, and the marker list only
    /// ever grows by appendage.
    #[test]
    fn trim_requests_byte_stable_across_consecutive_assemblies() {
        let tmp = tempfile::tempdir().unwrap();
        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Autonomous, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(30);
        let loud = format!("printf '{}'", "x".repeat(25_000));
        let mut responses = Vec::new();
        for _ in 0..18 {
            responses.push(tool_use_response("bash", json!({ "command": loud.clone() })));
        }
        responses.push(tool_use_response("goal_complete", json!({ "summary": "done" })));
        let mut llm = ScriptedLlm::new(responses);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            Some("check: true".to_string()),
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::RunFinished(0)));

        // Per-assembly marker counts; each increase is one trim event.
        let assemblies: Vec<Vec<Message>> =
            llm.calls.iter().map(|(_, m)| m.clone()).collect();
        let counts: Vec<usize> = assemblies
            .iter()
            .map(|m| m.iter().filter(|x| is_trim_marker(x)).count())
            .collect();
        assert!(
            counts.iter().max().copied().unwrap_or(0) >= 3,
            "run went past two trim thresholds: {counts:?}"
        );
        let increases: Vec<usize> = (1..counts.len())
            .filter(|&i| counts[i] > counts[i - 1])
            .collect();
        assert!(increases.len() >= 2, "two trim events expected: {counts:?}");

        // Across the assemblies straddling a trim event that extends an
        // already-frozen prefix — the last pre-trim assembly and the two
        // after it — the prefix through that assembly's last frozen marker
        // is byte-identical.
        let t = *increases
            .iter()
            .find(|&&i| counts[i - 1] > 0)
            .expect("a trim extending existing frozen markers");
        assert!(t + 2 < assemblies.len());
        let frozen = assemblies[t - 1]
            .iter()
            .rposition(is_trim_marker)
            .expect("pre-trim assembly carries frozen markers");
        let want = prefix_bytes(&assemblies[t - 1], frozen);
        assert_eq!(prefix_bytes(&assemblies[t], frozen), want);
        assert_eq!(prefix_bytes(&assemblies[t + 1], frozen), want);
        // Marker bytes only ever grow by appendage — never rewritten.
        let m1 = marker_texts(&assemblies[t - 1]);
        let m2 = marker_texts(&assemblies[t]);
        let m3 = marker_texts(&assemblies[t + 1]);
        assert_eq!(&m2[..m1.len()], &m1[..]);
        assert_eq!(&m3[..m1.len()], &m1[..]);
    }

    /// T77: a collapse never splits a tool_use/tool_result pair — every
    /// remaining tool_use id has its tool_result and vice versa.
    #[test]
    fn trim_never_splits_tool_pair() {
        let long = "w".repeat(30_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        // Paired exchanges: assistant(tool_use) + user(tool_result).
        for i in 0..24 {
            messages.push(Message::assistant(vec![ContentBlock::Known(
                KnownBlock::ToolUse {
                    id: format!("tu_{i}"),
                    name: "bash".into(),
                    input: json!({ "command": format!("run {i} {}", long) }),
                },
            )]));
            messages.push(Message::user(vec![ContentBlock::tool_result_block(
                &format!("tu_{i}"),
                long.clone(),
                false,
            )]));
        }
        assert!(transcript_trim(&mut messages));
        let mut uses = std::collections::BTreeSet::new();
        let mut results = std::collections::BTreeSet::new();
        for msg in messages.iter() {
            for block in &msg.content {
                match block {
                    ContentBlock::Known(KnownBlock::ToolUse { id, .. }) => {
                        uses.insert(id.clone());
                    }
                    ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, .. }) => {
                        results.insert(tool_use_id.clone());
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(uses, results, "every remaining pair is intact");
        assert!(!uses.is_empty(), "the tail keeps real pairs");
    }

    /// T77 fix-up killer (surviving mutant M1, clause 2): a transcript whose
    /// trimmable window ends ON an assistant tool_use leaves that use's
    /// tool_result as the first PROTECTED message. The window's last segment
    /// is complete, but collapsing it would orphan the protected
    /// tool_result — production 400. `pairing_unsafe` must refuse it while
    /// the older safe segments still collapse.
    #[test]
    fn trim_refuses_pairing_unsafe_window_edge_segment() {
        fn tool_use_msg(id: &str, payload: &str) -> Message {
            Message::assistant(vec![ContentBlock::Known(KnownBlock::ToolUse {
                id: id.to_string(),
                name: "bash".into(),
                input: json!({ "command": payload }),
            })])
        }
        let big = "u".repeat(70_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        for i in 0..20 {
            messages.push(tool_use_msg(&format!("tu_{i}"), &big));
            messages.push(Message::user(vec![ContentBlock::tool_result_block(
                &format!("tu_{i}"),
                "done".to_string(),
                false,
            )]));
        }
        // Mid-turn transcript tail: the last assistant tool_use has not been
        // answered yet, which makes the window end on a tool_use.
        messages.push(tool_use_msg("tu_20", &big));
        let w = messages.len() - KEEP_LAST_MESSAGES;
        assert_eq!(w % 2, 0, "window ends on an even index = a tool_use");
        assert_eq!(
            messages[w].content[0].text(),
            None,
            "window's last in-pool boundary message is the bare tool_use"
        );

        assert!(transcript_trim(&mut messages), "safe segments collapse");
        // The pairing-unsafe segment was refused: tu_10's tool_use (the
        // window's last message) and its protected tool_result both survive.
        let mut uses = std::collections::BTreeSet::new();
        let mut results = std::collections::BTreeSet::new();
        for msg in messages.iter() {
            for block in &msg.content {
                match block {
                    ContentBlock::Known(KnownBlock::ToolUse { id, .. }) => {
                        uses.insert(id.clone());
                    }
                    ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, .. }) => {
                        results.insert(tool_use_id.clone());
                    }
                    _ => {}
                }
            }
        }
        assert!(
            results.iter().all(|r| uses.contains(r)),
            "no orphaned tool_result: {results:?} vs {uses:?}"
        );
        assert!(uses.contains("tu_10"), "window-edge tool_use kept verbatim");
        assert_eq!(
            marker_texts(&messages).len(),
            10,
            "exactly the 10 safe pair-segments collapsed, not the unsafe one"
        );
    }

    /// T77 fix-up killer (surviving mutant M1, clause 1): a segment whose
    /// FIRST message carries a tool_result pairs with a tool_use BEFORE the
    /// segment — collapsing it orphans that use. The walk must refuse the
    /// segment (and still collapse the safe younger ones).
    #[test]
    fn trim_refuses_segment_starting_on_tool_result() {
        let big = "v".repeat(70_000);
        let mut messages = vec![Message::assistant(vec![ContentBlock::Known(
            KnownBlock::ToolUse {
                id: "tu_a".into(),
                name: "bash".into(),
                input: json!({ "command": "kick" }),
            },
        )])];
        // A leading tool_result whose tool_use sits in message 0.
        messages.push(Message::user(vec![ContentBlock::tool_result_block(
            "tu_a",
            big.clone(),
            false,
        )]));
        for i in 0..15 {
            messages.push(Message::assistant(vec![ContentBlock::Known(
                KnownBlock::ToolUse {
                    id: format!("tu_{i}"),
                    name: "bash".into(),
                    input: json!({ "command": big.clone() }),
                },
            )]));
            messages.push(Message::user(vec![ContentBlock::tool_result_block(
                &format!("tu_{i}"),
                "done".to_string(),
                false,
            )]));
        }
        assert!(transcript_trim(&mut messages), "safe segments collapse");
        // The leading tool_result survived verbatim: its segment was refused.
        let first = &messages[1].content[0];
        match first {
            ContentBlock::Known(KnownBlock::ToolResult { tool_use_id, content, .. }) => {
                assert_eq!(tool_use_id, "tu_a");
                assert_eq!(content, &big, "leading tool_result kept verbatim");
            }
            other => panic!("expected the surviving leading tool_result, got {other:?}"),
        }
        let mut uses = std::collections::BTreeSet::new();
        for msg in messages.iter() {
            for block in &msg.content {
                if let ContentBlock::Known(KnownBlock::ToolUse { id, .. }) = block {
                    uses.insert(id.clone());
                }
            }
        }
        assert!(uses.contains("tu_a"), "message-0 tool_use still answered");
        assert!(!marker_texts(&messages).is_empty(), "safe segments did collapse");
    }

    /// T77 fix-up killer (surviving mutant M2): the FIXED segment size is
    /// 16k tokens. With ~8k-token plain text messages a completed segment is
    /// exactly TWO messages — 16k pins the boundary there (8k would make
    /// every single message a segment, 32k would need four). The marker
    /// count and the canonical `~16k` text both move if the constant moves.
    #[test]
    fn trim_segment_boundaries_form_at_16k_not_8k_or_32k() {
        let big = "t".repeat(32_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        for _ in 0..44 {
            messages.push(Message::assistant(vec![ContentBlock::text_block(
                big.clone(),
            )]));
        }
        assert!(transcript_trim(&mut messages));
        let markers = marker_texts(&messages);
        assert_eq!(
            markers.len(),
            12,
            "24 pool messages of ~8k tokens = 12 fixed 16k segments"
        );
        for m in &markers {
            assert_eq!(
                m, "[trimmed: ~16k tokens, 0 tool results]",
                "canonical marker pins the 16k segment size"
            );
        }
    }

    /// T77 fix-up killer (surviving mutant M3): a segment YOUNGER than the
    /// 16k threshold (incomplete) must stay verbatim even when the chooser
    /// still wants more collapses (pool exhausted, estimate above target).
    /// Only whole complete segments advance the marker count, and a second
    /// trim with nothing new complete is a no-op.
    #[test]
    fn trim_young_incomplete_segment_stays_verbatim_under_pool_exhaustion() {
        let big = "s".repeat(32_000);
        let small = format!("{}{}", "note ".repeat(20), "{i}");
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        // One complete 16k segment (two ~8k-token messages)...
        for i in 0..2 {
            messages.push(Message::assistant(vec![ContentBlock::text_block(
                format!("{i} {}", big),
            )]));
        }
        // ...then a young, far-below-threshold remainder...
        for i in 0..10 {
            messages.push(Message::user(vec![ContentBlock::text_block(
                small.replace("{i}", &i.to_string()),
            )]));
        }
        // ...and a big protected tail that keeps the estimate above target.
        for i in 0..20 {
            messages.push(Message::assistant(vec![ContentBlock::text_block(
                format!("tail {i} {}", big),
            )]));
        }
        let w = messages.len() - KEEP_LAST_MESSAGES;
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages), "the complete segment collapses");
        assert_eq!(
            marker_texts(&messages),
            vec!["[trimmed: ~16k tokens, 0 tool results]".to_string()],
            "only the one complete segment collapsed"
        );
        let young: Vec<String> = messages[2..w]
            .iter()
            .map(|m| serde_json::to_string(m).unwrap())
            .collect();
        assert_eq!(young.len(), 11, "the young remainder is still there");

        // Pool exhausted: another trim must be a no-op — the young segment
        // never collapses, the marker count never advances.
        assert!(!transcript_trim(&mut messages), "young segment refuses to collapse");
        assert_eq!(marker_texts(&messages).len(), 1, "marker count frozen");
        let young_after: Vec<String> = messages[2..w]
            .iter()
            .map(|m| serde_json::to_string(m).unwrap())
            .collect();
        assert_eq!(young_after, young, "young region byte-verbatim across trims");
    }

    /// T77 fix-up round 2, killer for the chooser's STOP-AT-TARGET break
    /// (`if total <= TRIM_TARGET_TOKENS { break; }`). Pins the EXACT extent
    /// of collapse: for 60 distinct 4.4k-char pairs the trim must collapse
    /// exactly the first four complete 16-message segments and nothing more
    /// — exactly 4 canonical markers, a post-trim length of 61, the young
    /// complete segments tu_32..tu_49 byte-verbatim at their mapped
    /// positions, and the 20-message tail byte-verbatim. Deleting the break
    /// over-collapses the pool (6 markers, young pool destroyed) and trips
    /// every pin here; merely asserting `<= target` (the old
    /// `trimming_respects_target_or_exhausts`) did not, because
    /// over-collapse still lands under 80k.
    #[test]
    fn trim_stops_at_target_exact_collapse_extent() {
        let long = "w".repeat(4_400);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        let mut pre: Vec<String> = Vec::new();
        let snap = |m: &Message| serde_json::to_string(m).unwrap();
        pre.push(snap(&messages[0]));
        for i in 0..60 {
            let pair = use_result_pair(&format!("tu_{i}"), &format!("payload {i} {}", long));
            for m in &pair {
                pre.push(snap(m));
            }
            messages.extend(pair);
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages), "trim engages");

        // Exactly four oldest segments collapsed — one canonical marker
        // each, in order, nothing else.
        assert_eq!(
            marker_texts(&messages),
            vec!["[trimmed: ~18k tokens, 8 tool results]".to_string(); 4],
            "exactly the 4 oldest complete segments collapsed, no more"
        );
        // Post-trim layout: 1 (msg0) + 4 markers + 36 young + 20 tail.
        assert_eq!(messages.len(), 61, "4 segments of 16 messages became 4 markers");
        for m in &messages[1..5] {
            assert!(is_trim_marker(m), "markers occupy exactly indices 1..=4");
        }
        assert!(!is_trim_marker(&messages[5]), "marker run ends at index 4");
        // The collapsed ranges were exactly pre[1..65]: young pool
        // pre[65..101] -> post[5..41], byte-verbatim at the mapped index.
        for k in 0..36 {
            assert_eq!(
                snap(&messages[5 + k]),
                pre[65 + k],
                "young pool message {k} (tu_{}) byte-verbatim at its mapped slot",
                32 + k / 2
            );
        }
        // And the protected tail pre[101..121] -> post[41..61], untouched.
        for k in 0..20 {
            assert_eq!(snap(&messages[41 + k]), pre[101 + k], "tail message {k} byte-verbatim");
        }
        // The target semantics still hold: under 80k after stopping.
        assert!(estimate_tokens(&messages) <= TRIM_TARGET_TOKENS);
    }

    /// T77 fix-up round 2, killer for the chooser's POOL-EXHAUSTION no-op
    /// (`if chosen.is_empty() { return false; }`). A resumed transcript
    /// whose trimmable window holds ONLY frozen markers (nothing left to
    /// collapse) while the estimate is still above the engage threshold
    /// must be a `false` no-op — not a `true` "trimmed" that would make the
    /// caller rewrite the transcript file for nothing. Deleting the guard
    /// flips the return value (the splice loop over an empty `chosen` is
    /// invisible), so the bool itself is pinned here, alongside byte
    /// identity of the whole message vec.
    #[test]
    fn trim_pool_of_only_frozen_markers_is_a_noop_false() {
        let tail_payload = "t".repeat(32_000);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        // Two frozen markers already in the pool (a resumed transcript).
        for _ in 0..2 {
            messages.push(Message::user(vec![ContentBlock::text_block(trim_marker_text(
                16_000, 0,
            ))]));
        }
        // A 20-message protected tail big enough to keep the estimate above
        // the engage threshold: 20 x ~8k tokens > 120k.
        for i in 0..20 {
            messages.push(Message::assistant(vec![ContentBlock::Known(KnownBlock::Text {
                text: format!("tail {i} {}", tail_payload),
            })]));
        }
        assert_eq!(messages.len(), 23);
        assert_eq!(messages.len() - KEEP_LAST_MESSAGES, 3, "pool is exactly indices 1..=2");
        assert!(is_trim_marker(&messages[1]) && is_trim_marker(&messages[2]));
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS, "trim engages");

        let before: Vec<String> = messages.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
        assert!(
            !transcript_trim(&mut messages),
            "pool of only frozen markers: false no-op, not a true rewrite"
        );
        let after: Vec<String> = messages.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
        assert_eq!(after, before, "no bytes moved");
        assert!(!transcript_trim(&mut messages), "idempotent: still a no-op");
    }

    /// T77 fix-up round 2, killer for the KEEP_LAST window bound
    /// (`messages.len().saturating_sub(KEEP_LAST_MESSAGES)`). Every message
    /// here is a complete 16k singleton segment, so the chooser collapses
    /// the ENTIRE pool (the 160k-token tail keeps the estimate above target
    /// no matter how much collapses) — the collapse stops exactly at the
    /// protected-tail edge. The count 20 is HARDCODED, not read from the
    /// production constant, so shrinking KEEP_LAST intrudes a marker into
    /// the pinned tail byte-verbatim region and growing it changes the
    /// exact marker count.
    #[test]
    fn trim_protected_tail_is_exactly_20_messages_byte_verbatim() {
        let big = "t".repeat(65_000); // ~16.2k tokens: one complete segment per message
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        let mut pre: Vec<String> = vec![serde_json::to_string(&messages[0]).unwrap()];
        for i in 0..40 {
            let m = Message::assistant(vec![ContentBlock::Known(KnownBlock::Text {
                text: format!("seg {i} {}", big),
            })]);
            pre.push(serde_json::to_string(&m).unwrap());
            messages.push(m);
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages), "the whole collapsible pool collapses");
        // Exactly the 20 pool messages (indices 1..=20 of 41) collapsed.
        assert_eq!(
            marker_texts(&messages),
            vec!["[trimmed: ~16k tokens, 0 tool results]".to_string(); 20],
            "exactly the 20 pool singletons collapsed"
        );
        assert_eq!(messages.len(), 41, "20 messages became 20 markers in place");
        assert!(is_trim_marker(&messages[20]), "collapse stops flush against the tail");
        assert!(!is_trim_marker(&messages[21]), "first tail message is real bytes");
        // Hardcoded 20: the last 20 messages are byte-verbatim at the SAME
        // indices (20-for-20 swap keeps positions stable).
        for k in 21..41 {
            assert_eq!(
                serde_json::to_string(&messages[k]).unwrap(),
                pre[k],
                "protected tail message {k} byte-verbatim (hardcoded 20-message tail)"
            );
        }
        assert_eq!(messages[0].content[0].text(), Some("kick"), "message 0 untouched");
    }

    /// T77 fix-up round 2, killer for the segment-start advance (`i = end`).
    /// The walk must consume each segment whole: the next segment starts
    /// where the previous ended, never re-slicing consumed messages.
    /// Advancing by one instead overlaps every segment with its predecessor
    /// — the chooser then picks overlapping ranges and the back-to-front
    /// splice corrupts the transcript (shifted markers, duplicated or
    /// destroyed payload regions). The 160k-token protected tail dominates
    /// the estimate, so the target never binds and the marker vec is set
    /// purely by the walk: two pure-pair segments (~18k/8 results), one
    /// MIXED segment straddling the pair/tail seam (~2 pair remainder + 2
    /// tail messages), then 2-message tail segments up to the window edge.
    #[test]
    fn trim_segment_walk_advances_to_end_not_re_sliced() {
        let long = "w".repeat(4_400);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        let mut pre: Vec<String> = vec![serde_json::to_string(&messages[0]).unwrap()];
        for i in 0..18 {
            let pair = use_result_pair(&format!("tu_{i}"), &format!("payload {i} {}", long));
            for m in &pair {
                pre.push(serde_json::to_string(m).unwrap());
            }
            messages.extend(pair);
        }
        for i in 0..24 {
            let m = Message::assistant(vec![ContentBlock::Known(KnownBlock::Text {
                text: format!("tail {i} {}", "t".repeat(32_000)),
            })]);
            pre.push(serde_json::to_string(&m).unwrap());
            messages.push(m);
        }
        // len 61, window_end 41: pool = 1..41 = two 16-message pair
        // segments, one mixed seam segment (4 pair msgs + 2 tail msgs),
        // then one 2-message tail segment ending flush at the window edge.
        assert_eq!(messages.len(), 61);
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages));
        assert_eq!(
            marker_texts(&messages),
            vec![
                "[trimmed: ~18k tokens, 8 tool results]".to_string(),
                "[trimmed: ~18k tokens, 8 tool results]".to_string(),
                "[trimmed: ~21k tokens, 2 tool results]".to_string(),
                "[trimmed: ~16k tokens, 0 tool results]".to_string(),
            ],
            "walk shape: pair, pair, mixed seam, tail — nothing re-sliced"
        );
        assert_eq!(messages.len(), 25, "40 pool messages became 4 markers");
        for m in &messages[1..5] {
            assert!(is_trim_marker(m), "markers occupy exactly indices 1..=4");
        }
        assert!(!is_trim_marker(&messages[5]), "marker run ends at index 4");
        // Collapsed ranges were exactly pre[1..41] — the ENTIRE pool: the
        // 20-message tail pre[41..61] survives verbatim at post[5..25].
        for k in 0..20 {
            assert_eq!(
                serde_json::to_string(&messages[5 + k]).unwrap(),
                pre[41 + k],
                "tail message {k} byte-verbatim"
            );
        }
    }

    /// T77 fix-up round 2, killer for the ABOVE-THRESHOLD engage gate
    /// (`if estimate_tokens(messages) <= TRIM_ABOVE_TOKENS { return false; }`).
    /// The estimate sits strictly BETWEEN the two thresholds — above the
    /// 80k target (so the chooser would act if engaged) but under the 120k
    /// engage threshold — with a pool holding complete collapsible
    /// segments. The gate is the ONLY thing returning false here; deleting
    /// it collapses two segments and re-collapses on every call in this
    /// band. (A pool under 80k total cannot kill the gate: the
    /// stop-at-target break fires first — the band is the point.)
    #[test]
    fn trim_below_threshold_even_with_completable_pool_is_noop() {
        let long = "w".repeat(4_400);
        let mut messages = vec![Message::user(vec![ContentBlock::text_block("kick")])];
        for i in 0..46 {
            messages.extend(use_result_pair(&format!("tu_{i}"), &format!("payload {i} {}", long)));
        }
        assert_eq!(messages.len(), 93, "pool holds indices 1..=72");
        let est = estimate_tokens(&messages);
        assert!(est > TRIM_TARGET_TOKENS, "above target: the chooser would act: {est}");
        assert!(est <= TRIM_ABOVE_TOKENS, "under the engage threshold: {est}");

        let before: Vec<String> = messages.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
        assert!(
            !transcript_trim(&mut messages),
            "below TRIM_ABOVE_TOKENS: false no-op despite a collapsible pool"
        );
        assert!(marker_texts(&messages).is_empty(), "no marker may appear");
        let after: Vec<String> = messages.iter().map(|m| serde_json::to_string(m).unwrap()).collect();
        assert_eq!(after, before, "no bytes moved");
    }

    fn marker_texts(messages: &[Message]) -> Vec<String> {
        messages
            .iter()
            .filter(|m| is_trim_marker(m))
            .filter_map(|m| m.content[0].text().map(str::to_string))
            .collect()
    }

    fn last_marker_index(messages: &[Message]) -> usize {
        messages
            .iter()
            .rposition(is_trim_marker)
            .expect("at least one marker")
    }

    fn prefix_bytes(messages: &[Message], through: usize) -> Vec<u8> {
        let mut out = Vec::new();
        for msg in &messages[..=through] {
            out.extend_from_slice(serde_json::to_string(msg).unwrap().as_bytes());
        }
        out
    }

    fn frozen_or_marker_end(messages: &[Message]) -> usize {
        last_marker_index(messages) + 1
    }

}

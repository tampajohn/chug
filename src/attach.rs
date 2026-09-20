//! SPEC-5 §1 foundation: `@file` attachments in chat input.
//!
//! Tokenizes `@<path>` mentions out of a submitted chat message and expands
//! them into `<file path="...">` blocks appended to the LLM-bound message.
//! Nothing here ever errors: a mention that does not resolve (missing,
//! unreadable, or escaping the sandbox) degrades to an inline
//! `[file not found: <path>]` note in the text.
//!
//! Tokenizer rules:
//! - A mention starts at an `@` that is at the start of the text or directly
//!   after whitespace, so email-like `a@b` is NOT a mention.
//! - Unquoted form: `@` followed by a run of non-whitespace chars.
//! - Quoted form: `@"path with spaces"` (a closing quote is required).
//! - Trailing punctuation `, . ; : ! ? ) ]` is stripped from unquoted
//!   tokens, so `see @a.rs, please` references `a.rs` and keeps the comma.
//!   (Decision: the stripped punctuation is NOT part of the path; it stays in
//!   the surrounding text untouched.)
//! - `@` alone (or followed only by strippable punctuation) is not a mention.
//!
//! Wired into the chat submit path in a later round.
#![allow(dead_code)]

use std::fs;
use std::path::Path;

use crate::tools;

/// Line cap for a single attached file's contents inside the `<file>` block.
pub const MAX_FILE_LINES: usize = 2000;
/// Char cap for a single attached file's contents inside the `<file>` block.
pub const MAX_FILE_CHARS: usize = 50_000;
/// Cap for directory listings (mirrors the `list_dir` tool's 500).
const DIR_LISTING_CAP: usize = 500;

/// Result of expanding the `@mentions` in a submitted chat message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedMessage {
    /// The typed text with unresolvable mentions replaced by
    /// `[file not found: <path>]` notes; resolvable mentions stay inline.
    /// This is what the activity stream shows.
    pub text_with_notes: String,
    /// `text_with_notes` plus one `<file path="...">` block per expanded
    /// mention, appended in mention order. Equals `text_with_notes` when
    /// nothing expanded. This is what the model and transcript see.
    pub llm_message: String,
    /// Paths (as typed) that expanded successfully, in order — feeds the
    /// later `attached: a.rs, b.py` indicator line.
    pub attached: Vec<String>,
}

/// One `@mention` found in the submitted text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Mention {
    /// Byte offset of the `@`.
    start: usize,
    /// Byte offset just past the mention (excludes stripped trailing
    /// punctuation; includes both quotes of the quoted form).
    end: usize,
    /// The path as typed: quotes removed, trailing punctuation stripped.
    path: String,
}

/// Expand every `@mention` in `text` against `cwd` (same path-safety as the
/// tools: `..` escapes rejected). Never fails; see module docs.
pub fn expand_message(cwd: &Path, text: &str) -> ExpandedMessage {
    let mentions = tokenize(text);
    if mentions.is_empty() {
        return ExpandedMessage {
            text_with_notes: text.to_string(),
            llm_message: text.to_string(),
            attached: Vec::new(),
        };
    }
    let mut text_with_notes = String::with_capacity(text.len());
    let mut blocks = String::new();
    let mut attached = Vec::new();
    let mut last = 0;
    for mention in &mentions {
        match expand_one(cwd, &mention.path) {
            Some(body) => {
                // The mention itself stays inline; its contents ride along as
                // a trailing block.
                attached.push(mention.path.clone());
                blocks.push_str(&format!(
                    "\n\n<file path=\"{}\">\n{body}\n</file>",
                    mention.path
                ));
            }
            None => {
                text_with_notes.push_str(&text[last..mention.start]);
                text_with_notes.push_str(&format!("[file not found: {}]", mention.path));
                last = mention.end;
            }
        }
    }
    text_with_notes.push_str(&text[last..]);
    let llm_message = if attached.is_empty() {
        text_with_notes.clone()
    } else {
        format!("{text_with_notes}{blocks}")
    };
    ExpandedMessage {
        text_with_notes,
        llm_message,
        attached,
    }
}

/// Expand one mention to its block body: capped file contents, or a
/// `list_dir`-style listing for a directory. `None` covers missing,
/// unreadable, and sandbox-escaping paths alike — the caller substitutes the
/// not-found note.
fn expand_one(cwd: &Path, typed: &str) -> Option<String> {
    let path = tools::resolve_safe(cwd, typed).ok()?;
    let meta = fs::metadata(&path).ok()?;
    if meta.is_dir() {
        dir_listing(&path)
    } else if meta.is_file() {
        let data = fs::read_to_string(&path).ok()?;
        Some(cap_contents(&data, MAX_FILE_LINES, MAX_FILE_CHARS))
    } else {
        None
    }
}

/// Immediate-entries listing in the `list_dir` tool's style (non-recursive,
/// directories suffixed with `/`), sorted and capped via the shared helper.
fn dir_listing(path: &Path) -> Option<String> {
    let mut entries: Vec<String> = Vec::new();
    for entry in fs::read_dir(path).ok()?.flatten() {
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let name = entry.file_name().to_string_lossy().into_owned();
        entries.push(if is_dir { format!("{name}/") } else { name });
    }
    Some(tools::format_sorted_capped(entries, DIR_LISTING_CAP, "entries"))
}

/// Cap file contents at `max_lines` lines and `max_chars` chars, appending a
/// truncation note line for each cap that fired. Caps are applied in
/// line-then-char order; the char-cap count is measured after line-capping.
fn cap_contents(text: &str, max_lines: usize, max_chars: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut notes = Vec::new();
    let mut body = if lines.len() > max_lines {
        notes.push(format!(
            "[truncated: showing lines 1-{max_lines} of {}]",
            lines.len()
        ));
        lines[..max_lines].join("\n")
    } else {
        // Joining the lines back drops only the final newline, so the block
        // wraps as `<file...>\n<contents>\n</file>` exactly.
        lines.join("\n")
    };
    let total_chars = body.chars().count();
    if total_chars > max_chars {
        body = body.chars().take(max_chars).collect();
        notes.push(format!(
            "[truncated: showing first {max_chars} of {total_chars} chars]"
        ));
    }
    for note in notes {
        body.push('\n');
        body.push_str(&note);
    }
    body
}

/// Scan `text` for `@mentions`, in order. See module docs for the rules.
fn tokenize(text: &str) -> Vec<Mention> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    // The char immediately before the current one (for the email-like check).
    let mut prev: Option<char> = None;
    while let Some((i, c)) = chars.next() {
        if c != '@' {
            prev = Some(c);
            continue;
        }
        // Email-like `a@b`: `@` must be at the start or after whitespace.
        if prev.is_some_and(|p| !p.is_whitespace()) {
            prev = Some(c);
            continue;
        }
        match chars.peek().copied() {
            Some((_, '"')) => {
                chars.next(); // consume the opening quote
                let path_start = i + 2;
                let mut closed_at = None;
                for (j, c2) in chars.by_ref() {
                    if c2 == '"' {
                        closed_at = Some(j);
                        break;
                    }
                }
                if let Some(j) = closed_at {
                    let path = &text[path_start..j];
                    if !path.is_empty() {
                        out.push(Mention {
                            start: i,
                            end: j + 1,
                            path: path.to_string(),
                        });
                    }
                    prev = Some('"');
                } else {
                    // Unterminated quote: not a mention. The loop above ran
                    // to the end of the text, so we're done.
                    prev = None;
                }
            }
            Some(_) => {
                // Unquoted: a run of non-whitespace chars.
                let tok_start = i + 1;
                let mut tok_end = text.len();
                let mut last_char = None;
                for (j, c2) in chars.by_ref() {
                    if c2.is_whitespace() {
                        tok_end = j;
                        last_char = Some(c2);
                        break;
                    }
                    last_char = Some(c2);
                }
                prev = last_char;
                let token = &text[tok_start..tok_end];
                let path = token.trim_end_matches(strippable_trailing);
                if !path.is_empty() {
                    out.push(Mention {
                        start: i,
                        end: tok_start + path.len(),
                        path: path.to_string(),
                    });
                }
            }
            // `@` at the very end of the text: not a mention.
            None => prev = Some(c),
        }
    }
    out
}

/// Trailing punctuation stripped from unquoted mention tokens (documented in
/// the module docs): sentence punctuation and closing brackets that almost
/// always belong to the prose, not the path.
fn strippable_trailing(c: char) -> bool {
    matches!(c, ',' | '.' | ';' | ':' | '!' | '?' | ')' | ']')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(mentions: &[Mention]) -> Vec<&str> {
        mentions.iter().map(|m| m.path.as_str()).collect()
    }

    // --- Tokenizer ---

    #[test]
    fn tokenize_inline_mention() {
        let mentions = tokenize("please read @src/main.rs now");
        assert_eq!(paths(&mentions), vec!["src/main.rs"]);
        assert_eq!(mentions[0].start, 12);
        assert_eq!(mentions[0].end, 24);
    }

    #[test]
    fn tokenize_mention_at_start_and_after_newline() {
        let mentions = tokenize("@a.rs\nthen @b.rs");
        assert_eq!(paths(&mentions), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn tokenize_quoted_path_with_spaces() {
        let mentions = tokenize("read @\"my docs/a b.txt\" please");
        assert_eq!(paths(&mentions), vec!["my docs/a b.txt"]);
        // The span covers both quotes, so a not-found note replaces all of it.
        let m = &mentions[0];
        assert_eq!(&"read @\"my docs/a b.txt\" please"[m.start..m.end], "@\"my docs/a b.txt\"");
    }

    #[test]
    fn tokenize_unterminated_quote_not_a_mention() {
        assert!(tokenize("read @\"my docs/a.txt please").is_empty());
        // Empty quoted path is not a mention either.
        assert!(tokenize("read @\"\" please").is_empty());
    }

    #[test]
    fn tokenize_strips_trailing_punctuation() {
        let mentions = tokenize("@a.rs, @b.rs. @c.rs; @d.rs: @e.rs! @f.rs? @g.rs) @h.rs]");
        assert_eq!(
            paths(&mentions),
            vec!["a.rs", "b.rs", "c.rs", "d.rs", "e.rs", "f.rs", "g.rs", "h.rs"]
        );
        // The punctuation is not part of the span, so it survives substitution.
        let m = &mentions[0];
        assert_eq!(m.end - m.start, "@a.rs".len());
        // Interior punctuation is kept: only the trailing run is stripped.
        let mentions = tokenize("see @foo(v1).rs,");
        assert_eq!(paths(&mentions), vec!["foo(v1).rs"]);
    }

    #[test]
    fn tokenize_at_alone_not_a_mention() {
        assert!(tokenize("a @ b").is_empty());
        assert!(tokenize("@").is_empty());
        assert!(tokenize("@ ").is_empty());
        // `@` followed only by strippable punctuation is not a mention.
        assert!(tokenize("wait @, really").is_empty());
    }

    #[test]
    fn tokenize_email_like_not_a_mention() {
        assert!(tokenize("mail a@b.com now").is_empty());
        assert!(tokenize("user@host").is_empty());
    }

    #[test]
    fn tokenize_no_mentions() {
        assert!(tokenize("plain text, nothing here").is_empty());
    }

    // --- cap_contents ---

    #[test]
    fn cap_contents_under_caps_unchanged() {
        assert_eq!(cap_contents("a\nb\nc", 10, 100), "a\nb\nc");
        // A single trailing newline is normalized away for clean wrapping.
        assert_eq!(cap_contents("a\nb\n", 10, 100), "a\nb");
    }

    #[test]
    fn cap_contents_line_cap_notes() {
        let text = (1..=10).map(|n| format!("line{n}")).collect::<Vec<_>>().join("\n");
        let out = cap_contents(&text, 3, 1_000);
        assert_eq!(
            out,
            "line1\nline2\nline3\n[truncated: showing lines 1-3 of 10]"
        );
    }

    #[test]
    fn cap_contents_char_cap_notes() {
        let text = "x".repeat(100);
        let out = cap_contents(&text, 10, 10);
        assert_eq!(
            out,
            format!("{}\n[truncated: showing first 10 of 100 chars]", "x".repeat(10))
        );
    }

    #[test]
    fn cap_contents_both_caps_apply() {
        // 20 lines of 10 chars: line cap fires first, char cap sees the rest.
        let text = (0..20).map(|_| "x".repeat(10)).collect::<Vec<_>>().join("\n");
        let out = cap_contents(&text, 5, 20);
        assert!(out.contains("[truncated: showing lines 1-5 of 20]"));
        assert!(out.contains("[truncated: showing first 20 of 54 chars]"));
    }

    // --- Expansion ---

    #[test]
    fn expand_no_mentions_is_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let out = expand_message(tmp.path(), "plain message, no mentions");
        assert_eq!(out.text_with_notes, "plain message, no mentions");
        assert_eq!(out.llm_message, out.text_with_notes);
        assert!(out.attached.is_empty());
    }

    #[test]
    fn expand_file_contents_block() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), "hello\nworld\n").unwrap();
        let out = expand_message(tmp.path(), "look at @a.txt please");
        // The typed text is untouched; contents ride along in a block.
        assert_eq!(out.text_with_notes, "look at @a.txt please");
        assert_eq!(
            out.llm_message,
            "look at @a.txt please\n\n<file path=\"a.txt\">\nhello\nworld\n</file>"
        );
        assert_eq!(out.attached, vec!["a.txt"]);
    }

    #[test]
    fn expand_quoted_path_with_spaces() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("my docs")).unwrap();
        fs::write(tmp.path().join("my docs/a b.txt"), "spaced").unwrap();
        let out = expand_message(tmp.path(), "see @\"my docs/a b.txt\"");
        assert_eq!(out.text_with_notes, "see @\"my docs/a b.txt\"");
        assert_eq!(
            out.llm_message,
            "see @\"my docs/a b.txt\"\n\n<file path=\"my docs/a b.txt\">\nspaced\n</file>"
        );
        assert_eq!(out.attached, vec!["my docs/a b.txt"]);
    }

    #[test]
    fn expand_file_cap_note_with_real_constants() {
        let tmp = tempfile::tempdir().unwrap();
        // Exceed the real 2000-line cap; the note must say so.
        let big = (1..=2001).map(|n| format!("l{n}\n")).collect::<String>();
        fs::write(tmp.path().join("big.txt"), big).unwrap();
        let out = expand_message(tmp.path(), "@big.txt");
        assert!(out.llm_message.contains("<file path=\"big.txt\">\nl1\nl2\n"));
        assert!(
            out.llm_message
                .contains("[truncated: showing lines 1-2000 of 2001]"),
            "{}",
            &out.llm_message[out.llm_message.len() - 200..]
        );
        assert!(!out.llm_message.contains("l2001"));
    }

    #[test]
    fn expand_directory_listing() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src/deep")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        let out = expand_message(tmp.path(), "check @src");
        assert_eq!(
            out.llm_message,
            "check @src\n\n<file path=\"src\">\ndeep/\nlib.rs\nmain.rs\n</file>"
        );
        assert_eq!(out.attached, vec!["src"]);
    }

    #[test]
    fn expand_empty_directory_lists_no_entries() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("empty")).unwrap();
        let out = expand_message(tmp.path(), "@empty");
        assert_eq!(
            out.llm_message,
            "@empty\n\n<file path=\"empty\">\nno entries\n</file>"
        );
    }

    #[test]
    fn expand_missing_becomes_inline_note() {
        let tmp = tempfile::tempdir().unwrap();
        let out = expand_message(tmp.path(), "read @nope.txt please");
        assert_eq!(out.text_with_notes, "read [file not found: nope.txt] please");
        // No expansions: the LLM message is exactly the noted text.
        assert_eq!(out.llm_message, out.text_with_notes);
        assert!(out.attached.is_empty());
    }

    #[test]
    fn expand_missing_keeps_stripped_punctuation_in_text() {
        let tmp = tempfile::tempdir().unwrap();
        let out = expand_message(tmp.path(), "see @nope.txt, thanks");
        assert_eq!(out.text_with_notes, "see [file not found: nope.txt], thanks");
    }

    #[test]
    fn expand_multiple_files_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), "AAA").unwrap();
        fs::write(tmp.path().join("b.txt"), "BBB").unwrap();
        let out = expand_message(tmp.path(), "compare @a.txt and @b.txt");
        assert_eq!(out.text_with_notes, "compare @a.txt and @b.txt");
        assert_eq!(
            out.llm_message,
            "compare @a.txt and @b.txt\n\n<file path=\"a.txt\">\nAAA\n</file>\n\n<file path=\"b.txt\">\nBBB\n</file>"
        );
        assert_eq!(out.attached, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn expand_mixed_found_and_missing() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.txt"), "AAA").unwrap();
        let out = expand_message(tmp.path(), "@a.txt then @gone.txt");
        assert_eq!(out.text_with_notes, "@a.txt then [file not found: gone.txt]");
        assert_eq!(
            out.llm_message,
            "@a.txt then [file not found: gone.txt]\n\n<file path=\"a.txt\">\nAAA\n</file>"
        );
        assert_eq!(out.attached, vec!["a.txt"]);
    }

    #[test]
    fn expand_path_escape_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        // Even if something exists outside the sandbox, `..` never resolves.
        let out = expand_message(tmp.path(), "@../secret.txt");
        assert_eq!(out.text_with_notes, "[file not found: ../secret.txt]");
        assert_eq!(out.llm_message, out.text_with_notes);
        assert!(out.attached.is_empty());
        // Quoted escapes are rejected the same way.
        let out = expand_message(tmp.path(), "@\"../secret.txt\"");
        assert_eq!(out.text_with_notes, "[file not found: ../secret.txt]");
        // Absolute paths outside the cwd are rejected too.
        let out = expand_message(tmp.path(), "@/etc/passwd");
        assert_eq!(out.text_with_notes, "[file not found: /etc/passwd]");
    }

    #[test]
    fn expand_nested_relative_path() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        let out = expand_message(tmp.path(), "fix @src/main.rs");
        assert_eq!(
            out.llm_message,
            "fix @src/main.rs\n\n<file path=\"src/main.rs\">\nfn main() {}\n</file>"
        );
        assert_eq!(out.attached, vec!["src/main.rs"]);
    }

    #[test]
    fn expand_unreadable_file_becomes_inline_note() {
        // A non-UTF8 file cannot be read as text: inline note, not an error.
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("bin.dat"), [0xff, 0xfe, 0x00]).unwrap();
        let out = expand_message(tmp.path(), "@bin.dat");
        assert_eq!(out.text_with_notes, "[file not found: bin.dat]");
        assert!(out.attached.is_empty());
    }
}

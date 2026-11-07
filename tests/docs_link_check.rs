//! T331 — the risk-gate story page's citation link-check.
//!
//! `docs/risk-gate.md` is written under a tone contract: every claim cites
//! the mechanism (a file path, a feature id, or an item id) — no
//! adjectives without a citation. This test is the mechanical half of that
//! contract: a scripted grep over the repo that FAILS when a citation
//! dangles. Three citation grammars are checked:
//!
//!   backticked paths — every whitespace-separated word inside a `...`
//!     span that looks like a repo path (ends in a known source
//!     extension, or carries a `*` glob) must resolve to a TRACKED file;
//!     a glob must match at least one. Words under `.chug/` or `~/` name
//!     RUNTIME artifacts no fresh clone has, so those must instead be
//!     documented verbatim in some OTHER tracked file.
//!   item ids — every `T<n>` (uppercase T, word-bounded) must name a
//!     TODO.md row; every `F<n>` a FEATURES.md row. The boundary rule is
//!     the same shape the site-sync in-flight classifier uses: the letter
//!     must not sit inside a longer alphanumeric run (`F1` never matches
//!     `F13`).
//!   relative links — a markdown link target that is not http(s)/`#`
//!     resolves relative to docs/ and must be tracked.
//!
//! Plus the cross-page pin: the adopt FAQ's safety entry must link the
//! rendered section (`https://chug.sh/docs.html#risk-gate`) — the T331
//! "adopt.html cross-link" deliverable, pinned statically here and
//! end-to-end by the t331 fixtures in tests/site_sync.rs.
//!
//! T48 doctrine: the repo root is resolved at RUNTIME
//! (`std::env::current_dir()`; cargo runs test binaries with cwd = the
//! package root), never via the compile-time manifest-dir macro — under
//! the T47 shared cache a compile-time path can point at a since-removed
//! worktree.

#![cfg(unix)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn tracked_files(root: &std::path::Path) -> BTreeSet<String> {
    let out = Command::new("git")
        .args(["ls-files"])
        .current_dir(root)
        .output()
        .expect("git ls-files runs");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// `git grep -F -e NEEDLE` over tracked files, with the story page itself
/// excluded — a runtime-artifact citation must be documented somewhere a
/// reader can reach, never only by the page claiming it.
fn grep_tracked(root: &std::path::Path, needle: &str) -> bool {
    let out = Command::new("git")
        .args(["grep", "-F", "-e", needle, "--", ":!docs/risk-gate.md"])
        .current_dir(root)
        .output()
        .expect("git grep runs");
    out.status.success()
}

const DOC: &str = "docs/risk-gate.md";

/// Extensions that mark a backticked word as a repo-path citation. The
/// story page cites sources; it does not cite binaries or flags, so a word
/// outside this list is prose, not a citation.
const PATH_EXTS: [&str; 10] = [
    "md", "rs", "sh", "py", "json", "jsonl", "toml", "yml", "yaml", "txt",
];

/// Trailing punctuation that rides along when a citation sits next to a
/// sentence boundary (`scripts/site-sync.sh),` and the like).
fn strip_trailing_punct(word: &str) -> &str {
    word.trim_end_matches(|c: char| ".,;:!?'\")]}".contains(c))
}

fn looks_like_path(word: &str) -> bool {
    if word.is_empty()
        || word.starts_with('-')
        || word.contains(['<', '>', '(', ')', '\\', '$', '"'])
    {
        return false;
    }
    if word.contains('*') {
        return true; // a glob citation
    }
    PATH_EXTS.iter().any(|e| word.ends_with(&format!(".{e}")))
}

fn is_runtime_artifact(word: &str) -> bool {
    word.starts_with(".chug/") || word.starts_with("~/")
}

/// The backtick spans of `text` — the odd segments of a '`' split. An odd
/// backtick COUNT is a broken doc (a stray or unclosed code span), so it
/// fails loudly here before any citation is parsed.
fn backtick_spans<'a>(text: &'a str, doc: &str) -> Vec<&'a str> {
    let count = text.matches('`').count();
    assert_eq!(
        count % 2,
        0,
        "{doc}: an odd number of backticks ({count}) — a stray or unclosed code span breaks every citation parse"
    );
    text.split('`')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, s)| s)
        .collect()
}

/// `T<n>` / `F<n>` ids in raw text, word-bounded BOTH ways: the letter must
/// sit at a non-alphanumeric boundary and the digits must not run on
/// (`F13/F15` yields F13 and F15; `F1` inside `F15` is never a match;
/// `specs/t331-...` is lowercase and never matches the uppercase scan).
fn id_tokens(text: &str, letter: char) -> Vec<(String, usize)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == letter as u8
            && (i == 0 || !bytes[i - 1].is_ascii_alphanumeric())
            && i + 1 < bytes.len()
            && bytes[i + 1].is_ascii_digit()
        {
            let start = i;
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            // the digits must not run on into another digit context — they
            // already cannot (the loop stops at the first non-digit), but
            // the token itself is [start, j)
            out.push((text[start..j].to_string(), start));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Row ids declared in a table corpus file: lines of the `| T6 | ... |`
/// shape open with `| <id> |`.
fn declared_ids(corpus: &str, letter: char) -> BTreeSet<String> {
    corpus
        .lines()
        .filter_map(|l| l.strip_prefix("| "))
        .filter_map(|rest| rest.split_once(" |"))
        .map(|(id, _)| id.to_string())
        .filter(|id| {
            id.len() > 1
                && id.starts_with(letter)
                && id[1..].bytes().all(|b| b.is_ascii_digit())
        })
        .collect()
}

/// One assertion pass over both citation corpora; shared by the path/id leg
/// and the relative-link leg so each failure names every dangling citation
/// at once (a fix-up round sweeps the class, not one instance).
fn check_citations(root: &std::path::Path) {
    let tracked = tracked_files(root);
    let doc_path = root.join(DOC);
    let text = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| panic!("{DOC} must exist and read: {e}"));

    // --- backticked path citations -----------------------------------------
    let mut dangling: Vec<String> = Vec::new();
    for span in backtick_spans(&text, DOC) {
        for raw in span.split_whitespace() {
            let word = strip_trailing_punct(raw);
            if !looks_like_path(word) {
                continue;
            }
            if is_runtime_artifact(word) {
                // a runtime artifact: not in a fresh clone — it must be
                // documented verbatim in some OTHER tracked file
                if !grep_tracked(root, word) {
                    dangling.push(format!(
                        "{word} (runtime artifact documented in no other tracked file)"
                    ));
                }
            } else if word.contains('*') {
                // a glob citation: at least one tracked file must match
                // (simple split-on-* matching; the page cites `dir/stem*.ext`
                // shapes, not recursive globs)
                let parts: Vec<&str> = word.split('*').collect();
                let hit = tracked.iter().any(|f| {
                    if parts.len() != 2 {
                        return false;
                    }
                    f.starts_with(parts[0]) && f.ends_with(parts[1])
                });
                if !hit {
                    dangling.push(format!("{word} (glob matches no tracked file)"));
                }
            } else if !tracked.contains(word) {
                dangling.push(format!("{word} (not a tracked file)"));
            }
        }
    }

    // --- item ids -----------------------------------------------------------
    let todo = std::fs::read_to_string(root.join("TODO.md"))
        .unwrap_or_else(|e| panic!("TODO.md must exist and read: {e}"));
    let features = std::fs::read_to_string(root.join("FEATURES.md"))
        .unwrap_or_else(|e| panic!("FEATURES.md must exist and read: {e}"));
    let todo_ids = declared_ids(&todo, 'T');
    let feature_ids = declared_ids(&features, 'F');
    for (id, at) in id_tokens(&text, 'T') {
        if !todo_ids.contains(&id) {
            dangling.push(format!("{id} (at byte {at}: no TODO.md row)"));
        }
    }
    for (id, at) in id_tokens(&text, 'F') {
        if !feature_ids.contains(&id) {
            dangling.push(format!("{id} (at byte {at}: no FEATURES.md row)"));
        }
    }

    // --- relative markdown links -------------------------------------------
    let mut rest = &text[..];
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let close = after.find(')').expect("every ]( has a closing )");
        let target = &after[..close];
        rest = &after[close + 1..];
        if target.starts_with("http://")
            || target.starts_with("https://")
            || target.starts_with('#')
            || target.contains("://")
            || target.is_empty()
        {
            continue;
        }
        let file = target.split('#').next().unwrap_or(target);
        let resolved = format!("docs/{}", file.trim_start_matches("./"));
        if !tracked.contains(&resolved) {
            dangling.push(format!(
                "{target} (relative link resolves to {resolved}, not a tracked file)"
            ));
        }
    }

    assert!(
        dangling.is_empty(),
        "{DOC}: dangling mechanism citations (every citation must resolve to a \
         tracked path, a documented runtime artifact, a TODO.md row, or a \
         FEATURES.md row):\n  - {}",
        dangling.join("\n  - ")
    );
}

/// The T331 gate: every mechanism citation in the risk-gate story resolves.
#[test]
fn t331_risk_gate_story_citations_all_resolve() {
    let root = repo_root();
    let tracked = tracked_files(&root);
    assert!(
        tracked.contains(DOC),
        "{DOC} must be tracked — the story page is the deliverable"
    );
    check_citations(&root);
}

/// The adopt page's safety FAQ must link the rendered risk-gate section —
/// the T331 cross-page deliverable ("linked from adopt.html's safety FAQ
/// entry"). The anchor side of the link is pinned dynamically by the t331
/// fixtures in tests/site_sync.rs (the section id exists in BOTH shapes —
/// rendered body or placeholder).
#[test]
fn t331_adopt_faq_links_the_rendered_risk_gate_section() {
    let root = repo_root();
    let faq = std::fs::read_to_string(root.join("docs/adopt/faq.md"))
        .unwrap_or_else(|e| panic!("docs/adopt/faq.md must exist and read: {e}"));
    assert!(
        faq.contains("https://chug.sh/docs.html#risk-gate"),
        "the safety FAQ entry must link https://chug.sh/docs.html#risk-gate (the \
         rendered section), not only the README:\n{faq}"
    );
}

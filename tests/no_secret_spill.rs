//! T205 — the no-secret regression pin: every `HF_TOKEN` occurrence in the
//! repo is the env VAR NAME, never a VALUE.
//!
//! Operator history: tokens were once spilled by an env dump, and the T205
//! fine-tune publish path adds a private-repo token to the operator's
//! workflow (loopd env file, runbooks). This test walks the working tree
//! (the whole repo — specs, runbooks, scripts, loopd.sh, src) and fails on
//! any line that assigns a literal token-shaped value to HF_TOKEN.
//! Documented placeholder forms pass: `$(` (vault one-liners), `<...>`
//! templates, `"$HF_TOKEN"` indirection, empty, and prose that only NAMES
//! the variable.
//!
//! This is the "test or pre-commit check" pin the T205 spec asks for — as a
//! test, it rides the same `cargo test` gate as everything else.

use std::path::{Path, PathBuf};

/// The needles the whole pin hangs on: env var NAMES whose VALUES must
/// never be committed. `HF_TOKEN` is the original (T205); T330 adds
/// `CHUG_LLM_API_KEY` — the profile contract's literal-credential key —
/// with the same rule (the var NAME may appear; a committed VALUE fails).
/// The bool is dash-tolerance: HF tokens are `hf_` + `[A-Za-z0-9]`, while
/// `sk-ant-…` proxy keys carry dashes INSIDE the value, so the value run
/// for CHUG_LLM_API_KEY must consume `-` too (placeholders like
/// `sk-REPLACE-ME` stay under the length bar and still pass).
const NEEDLES: &[(&str, bool)] = &[("HF_TOKEN", false), ("CHUG_LLM_API_KEY", true)];

/// Skip the machinery: VCS internals, build outputs, runtime state, and
/// this test itself (its matching logic necessarily contains assignment
/// SHAPES, and it has nothing to spill).
fn skip_dir(dir: &Path) -> bool {
    matches!(
        dir.file_name().and_then(|n| n.to_str()),
        Some(".git") | Some(".chug") | Some("node_modules")
    ) || dir
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with("target"))
        .unwrap_or(false)
}

/// Collect every text file under `root` (UTF-8 readable; binaries and
/// unreadable files are skipped — they cannot be prose spills).
fn collect(root: &Path, out: &mut Vec<PathBuf>, depth: u8) {
    if depth > 8 {
        return;
    }
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        // DirEntry::file_type does NOT follow symlinks and never blocks on
        // FIFOs/sockets — only real dirs recurse, only real files are read.
        let Ok(ft) = entry.file_type() else { continue };
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "no_secret_spill.rs" {
            continue; // this file — the matcher itself, no token content
        }
        if ft.is_dir() {
            if !skip_dir(&path) {
                collect(&path, out, depth + 1);
            }
        } else if ft.is_file() && std::fs::read_to_string(&path).is_ok() {
            out.push(path);
        }
    }
}

/// The line's needle mention is a VALUE ASSIGNMENT if it reads
/// `<NEEDLE>` (or `"NEEDLE"` / `'NEEDLE'`) followed by `=` or `:`,
/// then an optional quote, then a literal token-shaped value: at least 12
/// chars of `[A-Za-z0-9_+]` (plus `-` for the dash-tolerant needles)
/// starting with `hf_` OR a bare run of 24+ such chars. Placeholder/
/// documentational starts — `$` (vault one-liners), `<` (templates), `(` —
/// pass, as does anything that is not an assignment at all (prose, the var
/// name in a flag list, the events note's boolean `token_set` key, which
/// does not contain this needle). The `*_CMD` companion form never matches:
/// after the needle comes `_CMD=…`, and `_` is not a `=`/`:` separator.
fn is_token_value_assignment(line: &str) -> bool {
    NEEDLES
        .iter()
        .any(|(needle, dashes)| is_secret_value_assignment(line, needle, *dashes))
}

fn is_secret_value_assignment(line: &str, needle: &str, dashes: bool) -> bool {
    let needle_pos = match line.find(needle) {
        Some(p) => p,
        None => return false,
    };
    let after_name = &line[needle_pos + needle.len()..];
    let quoted = after_name.starts_with('"') || after_name.starts_with('\'');
    let after_name = if quoted { &after_name[1..] } else { after_name };
    let after_sep = after_name.trim_start().strip_prefix(['=', ':']);
    let Some(after_sep) = after_sep else {
        return false; // not an assignment — a name mention (allowed)
    };
    let after_sep = after_sep.trim_start();
    let after_sep = after_sep
        .strip_prefix('"')
        .or_else(|| after_sep.strip_prefix('\''))
        .unwrap_or(after_sep);
    let value: String = after_sep
        .chars()
        .take_while(|c| {
            c.is_ascii_alphanumeric() || *c == '_' || *c == '+' || (dashes && *c == '-')
        })
        .collect();
    if value.len() < 12 {
        return false; // empty / placeholder words / short prose
    }
    // template + indirection forms are documentation, not spilled values
    if after_sep.starts_with('$') || after_sep.starts_with('<') || after_sep.starts_with('(') {
        return false;
    }
    value.starts_with("hf_") || value.len() >= 24
}

#[test]
fn repo_never_contains_an_hf_token_value() {
    // runtime root, never the compile-time macro (the T48 pin bans
    // CARGO_MANIFEST_DIR in Rust source: cargo runs test binaries with
    // cwd = the package root)
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let mut files = Vec::new();
    collect(&root, &mut files, 0);
    assert!(
        files.len() > 50,
        "the walk found only {} files — the pin must cover the real tree",
        files.len()
    );
    let mut spills: Vec<String> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        for (i, line) in text.lines().enumerate() {
            if NEEDLES.iter().any(|(n, _)| line.contains(n))
                && is_token_value_assignment(line)
            {
                // report the location, NEVER the matching line content
                spills.push(format!(
                    "{}:{}",
                    file.strip_prefix(&root).unwrap_or(file).display(),
                    i + 1
                ));
            }
        }
    }
    assert!(
        spills.is_empty(),
        "HF_TOKEN VALUE(s) committed at {} — the repo carries the var NAME only",
        spills.join(", ")
    );
}

#[test]
fn the_matcher_itself_is_pinned() {
    // spilled shapes MUST fail
    assert!(is_token_value_assignment("export HF_TOKEN=hf_ABCDEF0123456789abcdef"));
    assert!(is_token_value_assignment("HF_TOKEN=\"hf_XYZ0123456789abcdef\""));
    assert!(is_token_value_assignment(
        r#"{"token": "HF_TOKEN=0123456789abcdef0123456789abcdef"}"#
    ));
    // documented / safe shapes MUST pass
    assert!(!is_token_value_assignment("export HF_TOKEN=$(vault kv get -field=token secret/chug)"));
    assert!(!is_token_value_assignment("HF_TOKEN=<read-scoped-token>"));
    assert!(!is_token_value_assignment("export HF_TOKEN=\"$HF_TOKEN\""));
    assert!(!is_token_value_assignment("HF_TOKEN="));
    assert!(!is_token_value_assignment("set HF_TOKEN to a read-scoped token with access"));
    assert!(!is_token_value_assignment("HF_TOKEN (private/gated repos) — read-scoped"));
    assert!(!is_token_value_assignment("HF_TOKEN is set but was rejected"));
    assert!(!is_token_value_assignment("\"token_set\": false"));
    // the needle's absence is trivially safe
    assert!(!is_token_value_assignment("CHUG_LAYA_CHECKPOINT=org/model@rev"));
}

// --- T330 — the profile contract's key rides the same pin ---------------------

/// The T330 profile contract (docs/install-profiles.md): the repo never
/// carries endpoints or keys in `profiles/`. The walk covers the directory
/// by construction (it scans every readable file), and THIS test makes the
/// scope load-bearing: the committed example profile must be among the
/// walked files, and a CHUG_LLM_API_KEY value — the contract's literal
/// credential — must fail the matcher exactly like an HF_TOKEN value would.
#[test]
fn profile_files_are_walked_and_the_llm_key_value_is_caught() {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let mut files = Vec::new();
    collect(&root, &mut files, 0);
    assert!(
        files
            .iter()
            .any(|p| p.ends_with("profiles/local-openai-compat.env")),
        "the committed example profile must be in the walk's scope — profiles/ \
         is where a spilled company key would land"
    );
    // spilled shapes MUST fail (literal CHUG_LLM_API_KEY, quoted, and the
    // dash-carrying sk-ant-… shape the HF matcher alone would miss)
    assert!(is_token_value_assignment(
        r#"CHUG_LLM_API_KEY="sk-ant-api03-ABCDEFGHIJKLMNOPQRSTUVWXYZ123456""#
    ));
    assert!(is_token_value_assignment("CHUG_LLM_API_KEY=0123456789abcdef0123456789abcdef"));
    // documented / safe shapes MUST pass: the CMD escape hatch (the needle
    // is a PREFIX of CHUG_LLM_API_KEY_CMD — the separator check must not
    // fire on `_CMD=`), the short placeholder, and the commented form in the
    // committed example
    assert!(!is_token_value_assignment(
        "CHUG_LLM_API_KEY_CMD=$(vault kv get -field=token secret/chug/llm)"
    ));
    assert!(!is_token_value_assignment("export CHUG_LLM_API_KEY_CMD='op read token'"));
    assert!(!is_token_value_assignment(r#"CHUG_LLM_API_KEY="sk-REPLACE-ME""#));
    assert!(!is_token_value_assignment("# export CHUG_LLM_API_KEY=\"sk-REPLACE-ME\""));
}

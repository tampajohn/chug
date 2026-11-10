//! T353 — the profile contract's dead-key audit: a documented key with no
//! consumer is worse than an absent feature — it lies.
//!
//! docs/install-profiles.md's contract table is the promise an operator
//! profiles against: every `CHUG_*` key it documents must actually be
//! consumed by the loop. This test is the mechanical half of that promise:
//! parse the `## The contract` section's table, and for EVERY documented
//! `CHUG_*` key assert a consumer exists, in one of exactly two shapes:
//!
//!   the literal key appears in src/ or scripts/ (exact token —
//!     `CHUG_LLM_API_KEY` must not be satisfied by a
//!     `CHUG_LLM_API_KEY_CMD` hit), or
//!   the loader's generic `CHUG_LANGFUSE_*` translation arm
//!     (scripts/loopd_profile_loader.sh, prefix-strip to `LANGFUSE_*`)
//!     exists AND the translated native name is consumed in src/
//!     (`src/observ.rs` reads LANGFUSE_HOST / LANGFUSE_PUBLIC_KEY /
//!     LANGFUSE_SECRET_KEY).
//!
//! Only intentionally-reserved keys may skip the audit, via `ALLOWLIST` —
//! and an allowlist entry that no longer names a documented key fails as
//! stale (exemptions rot). RED-proof of the class: plant a dead key row in
//! the contract table and the audit fails naming it; remove the row and it
//! passes. A dead documented key can never silently re-enter the contract.
//!
//! T48 doctrine: the repo root is resolved at RUNTIME
//! (std::env::current_dir(); cargo runs test binaries with cwd = the
//! package root), never via the compile-time manifest-dir macro — under
//! the T47 shared cache a compile-time path can point at a since-removed
//! worktree.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_else(|e| panic!("reading {rel}: {e}")))
        .into_owned()
}

/// Keys exempt from the consumer audit — ONLY intentionally-reserved ones
/// (a documented key that is deliberately not wired yet), each entry
/// carrying a comment naming the reservation. Empty today: every key the
/// contract documents is consumed. An entry that no longer names a
/// documented key is itself an audit failure (see the stale leg below).
const ALLOWLIST: &[&str] = &[];

// --- parsing docs/install-profiles.md ---------------------------------------

/// The byte span of one markdown section: from its `heading` line to the
/// next line opening another `## ` section (EOF if none).
fn section<'a>(docs: &'a str, heading: &str) -> &'a str {
    let mut offset = 0usize;
    let mut start: Option<usize> = None;
    let mut end = docs.len();
    for chunk in docs.split_inclusive('\n') {
        let line = chunk.trim_end(); // strips '\n' (and any '\r')
        if start.is_some() {
            if line.starts_with("## ") {
                end = offset;
                break;
            }
        } else if line == heading {
            start = Some(offset + chunk.len());
        }
        offset += chunk.len();
    }
    let s =
        start.unwrap_or_else(|| panic!("docs/install-profiles.md must carry a `{heading}` section"));
    &docs[s..end]
}

/// The backticked spans of one table cell (odd indices of a '`'-split).
fn backticked(cell: &str) -> Vec<String> {
    cell.split('`')
        .skip(1)
        .step_by(2)
        .map(|span| span.trim().to_string())
        .collect()
}

/// A well-formed profile-owned key: `CHUG_` + uppercase/digits/underscores.
fn is_chug_key(token: &str) -> bool {
    token.starts_with("CHUG_")
        && token.len() > "CHUG_".len()
        && token
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Every `CHUG_*` key the contract table documents: the `## The contract`
/// section's table rows, all backticked `CHUG_*` tokens in ANY cell (the
/// Langfuse row carries three keys in one `<br>`-joined cell, and a
/// description cell that backticks a related key documents it too — the
/// audit is deliberately inclusive: more keys audited, fail closed).
fn documented_keys(docs: &str) -> Vec<String> {
    let contract = section(docs, "## The contract");
    let mut keys: Vec<String> = Vec::new();
    for line in contract.lines().filter(|l| l.starts_with('|')) {
        for cell in line.split('|') {
            for token in backticked(cell) {
                if is_chug_key(&token) && !keys.contains(&token) {
                    keys.push(token);
                }
            }
        }
    }
    keys
}

// --- consumer search ---------------------------------------------------------

fn is_key_byte(b: u8) -> bool {
    b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'
}

/// True when `key` appears as an EXACT token in `haystack`: neither the
/// preceding nor the following byte is another [A-Z0-9_], so
/// `CHUG_LLM_API_KEY` is NOT satisfied by a `CHUG_LLM_API_KEY_CMD`
/// occurrence (and `LANGFUSE_PUBLIC_KEY` not by a longer LANGFUSE_* name).
/// Keys are ASCII, so every match boundary is a char boundary.
fn contains_token(haystack: &str, key: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = haystack[from..].find(key) {
        let at = from + pos;
        let after = at + key.len();
        let before_ok = at == 0 || !is_key_byte(bytes[at - 1]);
        let after_ok = after >= bytes.len() || !is_key_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        from = at + 1;
    }
    false
}

/// (rel-path, contents) of every file under src/ and scripts/ — the two
/// trees a contract key's consumer may live in (src/ reads env, scripts/
/// carries the loader that translates profile keys to native env).
fn source_texts(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for tree in ["src", "scripts"] {
        let tree_root = root.join(tree);
        collect_texts(tree, &tree_root, &tree_root, &mut out);
    }
    out.sort();
    out
}

fn collect_texts(tree: &str, tree_root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
    for entry in entries {
        let path = entry.expect("dir entry readable").path();
        if path.is_dir() {
            collect_texts(tree, tree_root, &path, out);
        } else {
            let rel = path
                .strip_prefix(tree_root)
                .expect("walk stays under the tree root")
                .to_string_lossy()
                .into_owned();
            let text = String::from_utf8_lossy(
                &std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display())),
            )
            .into_owned();
            out.push((format!("{tree}/{rel}"), text));
        }
    }
}

// --- the audit ---------------------------------------------------------------

/// Parser self-check: the audit reads a real table, not an empty set. If
/// docs/install-profiles.md restructures, this fails with a parser-shaped
/// message instead of the audit passing vacuously.
#[test]
fn contract_table_parses_to_documented_keys() {
    let keys = documented_keys(&read("docs/install-profiles.md"));
    assert!(
        !keys.is_empty(),
        "the `## The contract` table yielded ZERO CHUG_* keys — the parser broke (section split, row shape, or backtick extraction)"
    );
    for canary in ["CHUG_LLM_BASE_URL", "CHUG_AUTO_UPDATE"] {
        assert!(
            keys.iter().any(|k| k == canary),
            "the parsed key set is missing `{canary}` — a structural anchor of the contract table; the parser broke"
        );
    }
}

#[test]
fn every_documented_contract_key_has_a_consumer() {
    let keys = documented_keys(&read("docs/install-profiles.md"));
    assert!(
        !keys.is_empty(),
        "no documented keys parsed — run contract_table_parses_to_documented_keys first"
    );

    // Stale-exemption leg: an allowlist entry that no longer names a
    // documented key is rot — remove it.
    for allowed in ALLOWLIST {
        assert!(
            keys.iter().any(|k| k == allowed),
            "ALLOWLIST entry `{allowed}` no longer names a documented contract key — the exemption is stale, remove it"
        );
    }

    let sources = source_texts(&repo_root());
    let loader = sources
        .iter()
        .find(|(path, _)| path == "scripts/loopd_profile_loader.sh")
        .map(|(_, text)| text.as_str())
        .unwrap_or_else(|| {
            panic!(
                "scripts/loopd_profile_loader.sh is missing — it is the translation half of the profile contract"
            )
        });

    let mut failures: Vec<String> = Vec::new();
    for key in &keys {
        if ALLOWLIST.contains(&key.as_str()) {
            continue; // intentionally reserved — the allowlist comment says why
        }
        // Shape 1: the literal key, consumed somewhere in src/ or scripts/.
        if sources.iter().any(|(_, text)| contains_token(text, key)) {
            continue;
        }
        // Shape 2 (the only non-literal one): the loader's generic
        // CHUG_LANGFUSE_* translation arm — the profile key itself appears
        // in no consumer, but the prefix-stripped native name must be read
        // in src/. Confined to the CHUG_LANGFUSE_ family: that is the arm
        // the loader actually carries.
        if key.starts_with("CHUG_LANGFUSE_") {
            let native = key.strip_prefix("CHUG_").expect("checked above");
            let arm = loader.contains("CHUG_LANGFUSE_*)");
            let native_consumed = sources
                .iter()
                .filter(|(path, _)| path.starts_with("src/"))
                .any(|(_, text)| contains_token(text, native));
            if !arm {
                failures.push(format!(
                    "{key}: no literal consumer in src/ or scripts/, and the loader's generic \
                     CHUG_LANGFUSE_* translation arm (`CHUG_LANGFUSE_*)` in \
                     scripts/loopd_profile_loader.sh) is gone — restore the arm or wire the key \
                     literally"
                ));
            } else if !native_consumed {
                failures.push(format!(
                    "{key}: no literal consumer in src/ or scripts/; the loader arm translates it \
                     to {native}, but {native} is read nowhere in src/ — a documented no-op"
                ));
            }
        } else {
            failures.push(format!(
                "{key}: no consumer — no exact-token occurrence in src/ or scripts/ (a longer \
                 key's prefix does not count); either wire the key or drop it from the contract"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "documented contract keys with no consumer (docs/install-profiles.md `## The contract`):\n  {}",
        failures.join("\n  ")
    );
}

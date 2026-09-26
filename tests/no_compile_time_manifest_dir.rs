//! T48 — static pin: no compile-time CARGO_MANIFEST_DIR anywhere in Rust code.
//!
//! T47's shared `CARGO_TARGET_DIR` (`target-shared/`) serves one build cache
//! to every worktree, and cargo's incremental cache can hand a CACHED test
//! binary to a later `cargo test` in a different checkout without
//! recompiling. Any compile-time `env!` of CARGO_MANIFEST_DIR in that binary
//! then holds the checkout it was BUILT in — a path that may be a
//! since-removed worktree. That fired for real in cycle 21: post-merge gates
//! in main false-redded 2/452 because the cached binary read files under the
//! deleted `/tmp/chug-loop-t43` worktree.
//!
//! The five test sites known at T48 time were converted to runtime
//! resolution: cargo executes test binaries with the current directory set
//! to the package root (unit tests and integration tests alike), so
//! `std::env::current_dir()` resolves the checkout the binary RUNS against.
//! This pin bans the pattern from re-entering ANY `.rs` file under `src/`
//! or `tests/` — test or production — so a regression forces a deliberate
//! re-review instead of a silent stale-path landmine.

use std::path::{Path, PathBuf};

/// The banned byte pattern, assembled by concatenation so THIS file's own
/// source never contains it contiguously. The scan below covers this file
/// too (no self-skip): with the needle split across parts there is no
/// self-match on a clean tree, and if the pattern ever re-enters even here
/// the pin must go red — no self-exemption.
const NEEDLE: &str = concat!("env!(", "\"CARGO_MANIFEST_DIR\"", ")");

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("dir entry under {}: {e}", dir.display()))
            .path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_compile_time_manifest_dir_in_any_rust_source() {
    // The repo root the binary RUNS against — the same runtime resolution
    // the five converted sites use (cargo sets the test binary's cwd to the
    // package root).
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    // Sanity: the concatenation above really assembled the banned pattern —
    // a "fixed" pin that mangles its own needle must not pass silently.
    assert_eq!(NEEDLE, format!("env!({:?})", "CARGO_MANIFEST_DIR"));

    let mut files = Vec::new();
    collect_rs_files(&root.join("src"), &mut files);
    collect_rs_files(&root.join("tests"), &mut files);
    assert!(
        !files.is_empty(),
        "the walk found no .rs files — an empty scan would make this pin vacuously green"
    );

    let offenders: Vec<String> = files
        .iter()
        .filter(|file| {
            let bytes =
                std::fs::read(file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
            // Byte-level substring search: comments and string literals count
            // as occurrences — the pattern must not appear anywhere at all.
            bytes
                .windows(NEEDLE.len())
                .any(|window| window == NEEDLE.as_bytes())
        })
        .map(|file| file.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "compile-time CARGO_MANIFEST_DIR re-entered Rust code (T48 pin) in \
         {offenders:?} — resolve the repo root at runtime \
         (std::env::current_dir; cargo runs test binaries with cwd = the \
         package root); the compile-time macro is what false-redded the \
         cycle-21 post-merge gates under the T47 shared cache"
    );
}

//! T204 phase 1 (F15) — static pin: the feature-off build graph carries
//! ZERO inference deps (candle-*, hf-hub, tokenizers).
//!
//! The T204 doctrine (Cargo.toml's `daemon` feature comment) is that every
//! `chug run` keeps its lean 9-crate-compile hot path with no candle in it:
//! the ~650MB judge model loads ONCE per host in the `chug daemon` process,
//! never in a per-run process. Nothing enforces "off by default" at the
//! dependency level — an accidental non-optional candle dep or a feature
//! leak would compile silently. This test runs `cargo tree` over the
//! default (feature-off) graph and fails the moment any inference crate
//! appears, so the regression forces a deliberate re-review instead of a
//! slow hot-path compile landing unnoticed.
//!
//! Runs the real cargo (no reimplementation: the tests guard the manifest,
//! they do not duplicate cargo's resolution). `cargo test` executes test
//! binaries with the package root as cwd (the T48 doctrine), so the plain
//! invocation resolves this checkout; `--offline` keeps it read-only and
//! deterministic once the registry cache is warm (it is by test time — the
//! parent `cargo test` just built against it).

use std::process::Command;

const BANNED: [&str; 5] = ["candle-core", "candle-nn", "candle-transformers", "hf-hub", "tokenizers"];

#[test]
fn feature_off_tree_has_zero_inference_deps() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(&cargo)
        .args(["tree", "--offline", "-e", "normal", "--no-default-features"])
        .output()
        .unwrap_or_else(|e| panic!("spawning {cargo} tree: {e}"));
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    for dep in BANNED {
        assert!(
            !tree.contains(dep),
            "feature-off dependency graph contains {dep} — the daemon feature \
             must keep candle/hf-hub/tokenizers out of the default build\n{tree}"
        );
    }
}

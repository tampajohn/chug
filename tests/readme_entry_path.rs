//! T351 — the README's entry path: install-first quickstart, no docs drift.
//!
//! The docs audit (2026-11-09, the T349/T350 lens) found the README ~95%
//! current with three gaps, all in the ENTRY PATH a new reader takes: the
//! Quickstart opened with the from-source path (`cargo build && cargo
//! install --path .`) instead of the one-command install (the T330 story),
//! the continuous-mode section led with the clone-era `nohup ./loopd.sh`
//! instead of the `chug loop` subcommand flow, and the first `chug run`
//! example carried an internal-only serving name
//! (`anthropic-system.ai.kimi-k3`) that a public reader copy-pasting gets
//! a provider error from. This fixture pins the repaired entry path:
//!
//!   entry-path parity — the quickstart commands (the install one-liner,
//!     `chug loop init`, `chug loop run`) appear VERBATIM in both
//!     README's `## Quickstart` section and docs/adopt/quickstart.md: the
//!     two entry paths a new reader can take (the README, or the public
//!     site's adopt page, machine-rendered from docs/adopt/) must never
//!     drift apart on the commands themselves.
//!   install-first — if a from-source instruction appears in the
//!     quickstart section at all, it comes AFTER the install one-liner;
//!     the quickstart leads with the binary install, and the from-source
//!     variant lives under Development.
//!   hygiene — no `anthropic-system.ai` serving name in the README's
//!     Quickstart section (public readers copy-paste it; internals
//!     sections keep theirs, where the audience is operators).
//!
//! The README-side checks scope to the `## Quickstart` SECTION — the
//! slice scripts/site-sync.sh renders onto docs.html — not the whole
//! file, so a drift in the quickstart itself is caught even while the
//! same string still survives elsewhere.
//!
//! T48 doctrine: the repo root is resolved at RUNTIME
//! (`std::env::current_dir()`; cargo runs test binaries with cwd = the
//! package root), never via the compile-time manifest-dir macro — under
//! the T47 shared cache a compile-time path can point at a since-removed
//! worktree (`tests/no_compile_time_manifest_dir.rs` pins that).

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(root: &std::path::Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel))
        .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"))
}

/// The quickstart commands the two entry paths share, verbatim. The
/// install one-liner is the T330 one-command install; the loop pair is
/// the post-T333 subcommand flow (docs/adopt/quickstart.md carries the
/// detached-start variant, which contains `chug loop run` verbatim).
const SHARED_COMMANDS: [&str; 3] = [
    "curl -fsSL https://chug.sh/install.sh | sh",
    "chug loop init",
    "chug loop run",
];

/// The README's `## Quickstart` section body (heading dropped; ends at
/// the next top-level heading) — the slice scripts/site-sync.sh renders
/// onto docs.html, so the pins below scope to exactly what a new reader
/// sees first. A fence line never starts with `## ` in this section, so
/// the fence-blind cut matches the renderer's fence-aware one.
fn readme_quickstart_section(readme: &str) -> &str {
    const HEAD: &str = "## Quickstart";
    let start = readme
        .find(HEAD)
        .unwrap_or_else(|| panic!("`{HEAD}` section not found in README.md"));
    let body = &readme[start + HEAD.len()..];
    match body.find("\n## ") {
        Some(end) => &body[..end],
        None => body,
    }
}

/// No drift between the two entry paths: every quickstart command README
/// readers see must exist verbatim in docs/adopt/quickstart.md (the
/// public site's adopt page renders from the docs/ fragments), and the
/// README quickstart must still carry all of them.
#[test]
fn quickstart_commands_match_docs_adopt_quickstart() {
    let root = repo_root();
    let readme = read(&root, "README.md");
    let qs = readme_quickstart_section(&readme);
    let docs_qs = read(&root, "docs/adopt/quickstart.md");
    for cmd in SHARED_COMMANDS {
        assert!(
            qs.contains(cmd),
            "README's `## Quickstart` section must carry the entry-path command verbatim: {cmd}"
        );
        assert!(
            docs_qs.contains(cmd),
            "docs/adopt/quickstart.md must carry the same entry-path command verbatim \
             (no drift between the two entry paths): {cmd}"
        );
    }
}

/// Install-first: the quickstart's first install instruction is the
/// binary one-liner. The guard is vacuous when clean — a from-source
/// instruction may not appear in the section at all (it lives under
/// Development); if one ever returns, it must at least come after the
/// one-command install, never before it.
#[test]
fn quickstart_is_install_first() {
    let root = repo_root();
    let readme = read(&root, "README.md");
    let qs = readme_quickstart_section(&readme);
    const INSTALL: &str = "curl -fsSL https://chug.sh/install.sh | sh";
    let install = qs.find(INSTALL).unwrap_or_else(|| {
        panic!("README quickstart must lead with the one-command install: {INSTALL}")
    });
    for from_source in ["cargo build", "cargo install --path ."] {
        if let Some(off) = qs.find(from_source) {
            assert!(
                off > install,
                "README quickstart must be install-first: `{from_source}` appears before \
                 the install one-liner — the from-source path belongs under Development"
            );
        }
    }
}

/// Hygiene: the internal-only serving name never appears in the README
/// quickstart — it resolves only inside one company's proxy, and the
/// example is the first thing a public reader copy-pastes. The generic
/// model name (or the profile indirection) takes its place; internals
/// sections keep their operator-facing names.
#[test]
fn quickstart_section_has_no_internal_serving_name() {
    let root = repo_root();
    let readme = read(&root, "README.md");
    let qs = readme_quickstart_section(&readme);
    assert!(
        !qs.contains("anthropic-system.ai"),
        "README quickstart must not carry the internal-only serving name \
         `anthropic-system.ai` — a public reader copy-pasting it gets a provider \
         error; use a generic resolvable model (see docs/install-profiles.md)"
    );
}

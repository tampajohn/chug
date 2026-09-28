//! T95 — README Development layout: module set-equality guard.
//!
//! The Development section's layout line lists the crate's modules as
//! `src/{...}.rs`. It drifts silently whenever a module is added by hand:
//! T86 added tgrep/plan/hooks to `src/` without touching the line (caught
//! by two humans counting 27/27), and T90 added `src/permissions.rs` plus
//! a README `## Permissions` section but skipped the layout line again —
//! the cycle-53 eval caught it at 27 listed vs 28 on disk. Second sighting
//! of the class, so the class earned this automation: the README list must
//! be SET-EQUAL to the filesystem set (every `src/*.rs` stem except
//! `main`), in either direction, so the next module addition cannot land
//! with a stale layout line.
//!
//! The list wraps across a newline in the README (the braces list breaks
//! mid-list onto a second line), so the parser is newline-tolerant: it
//! reads the raw text between `src/{` and the closing `}` and trims
//! whitespace from every comma-separated token.
//!
//! T48 doctrine: the repo root is resolved at RUNTIME
//! (`std::env::current_dir()`; cargo runs test binaries with cwd = the
//! package root), never via the compile-time manifest-dir macro — under
//! the T47 shared cache a compile-time path can point at a since-removed
//! worktree (`tests/no_compile_time_manifest_dir.rs` pins that).

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

/// The module stems named by the README Development section's `src/{...}`
/// braces list, comma-split and whitespace-trimmed (the list wraps across
/// a newline mid-list, so tokens can carry the break's `\n`). Returns an
/// error naming the section when the parse finds nothing — a missing or
/// mangled list must fail loudly, never pass vacuously.
fn readme_layout_modules(readme: &str) -> Result<Vec<String>, String> {
    let dev = readme
        .find("## Development")
        .ok_or("`## Development` section not found in README.md".to_string())?;
    let body = &readme[dev..];
    const OPEN: &str = "src/{";
    let open = body
        .find(OPEN)
        .ok_or("`src/{...}` layout list not found in README's Development section")?
        + OPEN.len();
    let close = open
        + body[open..]
            .find('}')
            .ok_or("README layout list has no closing `}`")?;
    Ok(body[open..close]
        .split(',')
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
        .collect())
}

/// The filesystem set: every `src/*.rs` file stem except `main` (the
/// binary entry point the layout line names separately in its
/// `(+ \`main.rs\`; ...)` parenthetical).
fn src_module_stems(root: &Path) -> Vec<String> {
    let src = root.join("src");
    let entries = std::fs::read_dir(&src)
        .unwrap_or_else(|e| panic!("reading {}: {e}", src.display()));
    let mut stems = Vec::new();
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("dir entry under {}: {e}", src.display()))
            .path();
        let is_rs_file =
            path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("rs");
        if is_rs_file {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_else(|| panic!("non-UTF-8 stem: {}", path.display()))
                .to_string();
            if stem != "main" {
                stems.push(stem);
            }
        }
    }
    stems.sort();
    stems.dedup();
    assert!(
        !stems.is_empty(),
        "the src/ walk found no module files — an empty filesystem set would make this guard vacuously green"
    );
    stems
}

/// (T95) The README Development layout list is SET-EQUAL to the filesystem
/// set of `src/*.rs` module stems (everything except `main`). The failure
/// names BOTH drift directions so the next T86-class hand-add diagnoses
/// itself: modules missing from the README, and README entries with no
/// backing file.
#[test]
fn readme_development_layout_matches_src_modules() {
    let root = repo_root();
    let readme = std::fs::read_to_string(root.join("README.md"))
        .unwrap_or_else(|e| panic!("reading README.md from the runtime checkout: {e}"));
    let readme_modules =
        readme_layout_modules(&readme).expect("README Development layout parse failed");
    assert!(
        !readme_modules.is_empty(),
        "the README layout list parsed to zero modules — a vacuous list would make this guard green while drifting"
    );
    let fs_modules = src_module_stems(&root);

    let missing_from_readme: Vec<&String> = fs_modules
        .iter()
        .filter(|stem| !readme_modules.contains(stem))
        .collect();
    let missing_from_filesystem: Vec<&String> = readme_modules
        .iter()
        .filter(|stem| !fs_modules.contains(stem))
        .collect();

    assert!(
        missing_from_readme.is_empty() && missing_from_filesystem.is_empty(),
        "README Development layout `src/{{...}}` list is not set-equal to the \
         src/*.rs module files (T95 guard)\n\
         - modules missing from README (present as src/<name>.rs): \
         {missing_from_readme:?}\n\
         - README entries with no src/<name>.rs file: {missing_from_filesystem:?}\n\
         update the README layout line (or add the missing module file); \
         the T86 class is a module added by hand without updating the line"
    );
}

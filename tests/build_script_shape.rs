//! T179 — build.rs's worktree always-stale rerun hint: the source-shape pin.
//!
//! `build.rs::emit_git_rerun_hints` used to emit
//! `cargo:rerun-if-changed=.git/HEAD` BEFORE reading it. In a worktree
//! `.git` is a FILE (pointing at the main repo's worktree gitdir), so
//! `.git/HEAD` never exists as a path — and cargo treats a missing
//! rerun-if-changed target as ALWAYS stale. Result: the build script, and
//! with it every dependent (the whole chug crate, ~40-95s release),
//! rebuilt on EVERY cargo invocation in ANY worktree — the cycle-81
//! discovery behind slow worktree gates, eaten gate windows, and child
//! budget deaths (`CARGO_LOG=cargo::core::compiler::fingerprint=trace
//! cargo build` logs `StaleItem(MissingFile { path: ".../.git/HEAD" })`).
//! The T179 fix emits the hint only after the read succeeded; this file
//! pins the fixed shape so a revert (the read back to front) goes RED.
//!
//! Source-text pins by construction: build.rs is not importable from an
//! integration test (binary-only crate), so the established idiom is a
//! runtime file read of the source the binary is built from.
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn build_rs() -> String {
    std::fs::read_to_string(repo_root().join("build.rs"))
        .expect("reading build.rs from the runtime checkout")
}

// ---- (a) the ordering pin: the read BEFORE the rerun-if-changed emission ----

/// The read the fix orders first: `emit_git_rerun_hints` reads `.git/HEAD`
/// to decide whether the hints can be emitted at all.
const READ_NEEDLE: &str = "fs::read_to_string(\".git/HEAD\")";

/// The read's else arm — the early-out that must sit BETWEEN the read and
/// the emission (read → `return;` → emit), the cycle-81 gate-enablement
/// shape.
const ELSE_RETURN_NEEDLE: &str = "return;";

/// The emission the fix gates: the `.git/HEAD` rerun hint, moved INSIDE the
/// read-OK path.
const EMIT_NEEDLE: &str = "println!(\"cargo:rerun-if-changed=.git/HEAD\");";

/// The window opener: the function whose body is pinned. Exactly one
/// occurrence expected in build.rs.
const FN_NEEDLE: &str = "fn emit_git_rerun_hints";

/// (a) `emit_git_rerun_hints` must order the `.git/HEAD` read BEFORE the
/// rerun-if-changed emission, with the read's else-return arm between
/// them: read → `return;` → emit. Emitting before the read (the pre-T179
/// shape) is the cycle-81 always-stale bug — in a worktree `.git` is a
/// file so `.git/HEAD` is a missing path, cargo treats missing
/// rerun-if-changed targets as always stale, and every cargo invocation
/// in a worktree rebuilt the whole crate. Reverting the hint move puts
/// the println first and trips this pin's ordering leg; deleting the
/// emission, the read, or the else arm trips an exactly-once leg.
#[test]
fn emit_git_rerun_hints_reads_head_before_emitting_the_hint() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        READ_NEEDLE.starts_with("fs::") && READ_NEEDLE.contains(".git/HEAD"),
        "the read needle must be the .git/HEAD read verbatim"
    );
    assert_eq!(ELSE_RETURN_NEEDLE, "return;");
    assert!(
        EMIT_NEEDLE.starts_with("println!")
            && EMIT_NEEDLE.contains("rerun-if-changed=.git/HEAD"),
        "the emission needle must be the .git/HEAD rerun hint verbatim"
    );
    let build = build_rs();
    assert_eq!(
        build.matches(FN_NEEDLE).count(),
        1,
        "build.rs must declare emit_git_rerun_hints exactly once — zero means \
         it was renamed or removed (the main tree loses its commit-move \
         rebuild hints), more than one means a second declaration drifted in"
    );
    let start = build.find(FN_NEEDLE).unwrap();
    // The body window: this fn through the next `\nfn ` (or EOF — it is the
    // file's last fn today). Scoped so an unrelated println/return elsewhere
    // in build.rs cannot satisfy the counts.
    let window_end = match build[start..].find("\nfn ") {
        Some(rel) => start + rel,
        None => build.len(),
    };
    let window = &build[start..window_end];
    assert_eq!(
        window.matches(READ_NEEDLE).count(),
        1,
        "emit_git_rerun_hints must read .git/HEAD exactly once ({READ_NEEDLE:?} \
         in its body) — zero means the read was removed, and with it the \
         branch-switch and ref hints the main tree watches"
    );
    assert_eq!(
        window.matches(EMIT_NEEDLE).count(),
        1,
        "emit_git_rerun_hints must emit {EMIT_NEEDLE:?} exactly once — zero \
         means the .git/HEAD hint was deleted (the main tree loses its \
         commit-move rebuilds), more than one means a duplicate drifted in"
    );
    assert_eq!(
        window.matches(ELSE_RETURN_NEEDLE).count(),
        1,
        "the read's else arm must be the function's only early-out \
         ({ELSE_RETURN_NEEDLE:?} once in its body) — zero means the skip-\
         silently arm was dropped (a worktree build would then fall through \
         to hints it cannot honor), more than one means a second return \
         drifted in and this pin's betweenness can no longer name the arm"
    );
    let read = window.find(READ_NEEDLE).unwrap();
    let else_arm = window.find(ELSE_RETURN_NEEDLE).unwrap();
    let emit = window.find(EMIT_NEEDLE).unwrap();
    assert!(
        read < else_arm && else_arm < emit,
        "the .git/HEAD read ({read}) must come BEFORE its else-return arm \
         ({else_arm}) and the emission LAST ({emit}) — the pre-T179 shape \
         (emit before the read) is the cycle-81 always-stale bug: in a \
         worktree .git is a file, .git/HEAD is a missing path, and cargo \
         treats missing rerun-if-changed targets as always stale, so every \
         cargo invocation in a worktree rebuilt the whole crate"
    );
}

// ---- (b) the doc-comment evidence carriers ----

/// The doc comment's first line — the window opener for the carriers pin.
const DOC_START_NEEDLE: &str = "/// Rebuild when the checked-out commit moves.";

/// The worktree-file clause: the doc comment must name the mechanism (in a
/// worktree `.git` is a FILE, so `.git/HEAD` is a missing path).
const FILE_CLAUSE_NEEDLE: &str = "is a FILE";

/// The cargo semantics: a missing rerun-if-changed target is ALWAYS stale
/// (the per-invocation full-rebuild cause).
const STALE_CLAUSE_NEEDLE: &str = "ALWAYS stale";

/// The CARGO_LOG evidence command the comment must name (the cycle-81
/// confirmation).
const CARGO_LOG_NEEDLE: &str = "CARGO_LOG=cargo::core::compiler::fingerprint=trace";

/// The trace signature the evidence command surfaces.
const STALE_ITEM_NEEDLE: &str = "StaleItem(MissingFile";

/// The discovery's naming token.
const CYCLE_NEEDLE: &str = "cycle-81";

/// The CHUG_GIT_HASH stub-test clause — documented, deliberately not
/// behaviorally changed (T179 requirement 4).
const STUB_CLAUSE_NEEDLE: &str = "CHUG_GIT_HASH";

/// (b) The expanded doc comment carries its evidence: the worktree-file
/// clause, the always-stale semantics, the CARGO_LOG evidence command with
/// the `StaleItem(MissingFile …)` signature it surfaces, the cycle-81
/// naming, and the CHUG_GIT_HASH stub-test clause (one clause, no behavior
/// change) — each exactly once inside the comment's window. Deleting or
/// rewording a clause drops its count to zero; duplicating one doubles it.
#[test]
fn emit_git_rerun_hints_doc_comment_carries_the_t179_evidence() {
    // Needle self-checks (T48 idiom).
    assert!(
        DOC_START_NEEDLE.starts_with("/// Rebuild") && DOC_START_NEEDLE.ends_with("moves."),
        "the doc-start needle must be the comment's first line verbatim"
    );
    assert!(
        CARGO_LOG_NEEDLE.contains("CARGO_LOG=")
            && CARGO_LOG_NEEDLE.ends_with("fingerprint=trace"),
        "the CARGO_LOG needle must be the evidence command verbatim"
    );
    assert!(
        STALE_ITEM_NEEDLE.starts_with("StaleItem(") && STALE_ITEM_NEEDLE.ends_with("MissingFile"),
        "the trace-signature needle must name the StaleItem variant verbatim"
    );
    let build = build_rs();
    assert_eq!(
        build.matches(FN_NEEDLE).count(),
        1,
        "build.rs must declare emit_git_rerun_hints exactly once (shared with \
         the ordering pin — the doc window resolves against it)"
    );
    let doc_start = build
        .find(DOC_START_NEEDLE)
        .expect("the emit_git_rerun_hints doc comment's first line present");
    let fn_pos = build.find(FN_NEEDLE).unwrap();
    assert!(
        doc_start < fn_pos,
        "the doc comment ({doc_start}) must sit directly before \
         emit_git_rerun_hints ({fn_pos}) — the window is the comment alone"
    );
    let window = &build[doc_start..fn_pos];
    for (needle, what) in [
        (
            FILE_CLAUSE_NEEDLE,
            "the worktree .git-is-a-file clause (why .git/HEAD is a missing path)",
        ),
        (
            STALE_CLAUSE_NEEDLE,
            "the always-stale semantics (cargo's treatment of missing targets)",
        ),
        (CARGO_LOG_NEEDLE, "the CARGO_LOG evidence command"),
        (STALE_ITEM_NEEDLE, "the StaleItem(MissingFile) signature"),
        (CYCLE_NEEDLE, "the cycle-81 discovery naming"),
        (
            STUB_CLAUSE_NEEDLE,
            "the CHUG_GIT_HASH stub-test clause (documented, not changed)",
        ),
    ] {
        assert_eq!(
            window.matches(needle).count(),
            1,
            "the emit_git_rerun_hints doc comment must state {what} \
             ({needle:?}) exactly once — zero means the clause was deleted \
             or reworded away (the next reader relearns the cycle-81 tax \
             the hard way), more than one means it is stated twice"
        );
    }
}

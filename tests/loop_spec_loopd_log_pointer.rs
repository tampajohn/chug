//! T267 — loopd-log pointer pins: LOOP-SPEC's first bare "loopd.log"
//! mention names the real supervisor-log path `.chug/loopd/loopd.log`.
//!
//! The `loopd.log-path-miss` census class: eval and orchestrator streams
//! probing the supervisor log read LOOP-SPEC's bare "loopd.log", guess the
//! literal `.chug/loopd.log`, miss (the daemon writes
//! `.chug/loopd/loopd.log` — `loopd.sh`'s `LOG=$STATE/loopd.log` under
//! `STATE=.chug/loopd/`; the cheap-exit fixture writes the same nested
//! path), and self-correct via discovery one command later — 0→2 fires at
//! the cycle-241 eval, 2→3 at trip 52, 3→4 at trip 53. Every fire masked,
//! every fire self-corrected, zero casualty — but ~1 iteration per fire,
//! so the trigger's remedy is a ONE-CLAUSE pointer at the first bare
//! mention: "in loopd.log (`.chug/loopd/loopd.log`)". The other two bare
//! mentions ("git/loopd.log record" in the same sentence; "one note per
//! cycle in loopd.log" in the T259 triage paragraph) stay byte-identical —
//! they read unambiguously once the first mention names the path.
//!
//! These pins keep the pointer from silently drifting back out in a future
//! doctrine edit: (a) the pointer clause sits in the Phase-1 window
//! exactly once (deletion-proof — delete the clause, the count goes 0, RED;
//! a duplicated copy goes >1, RED), (b) the FIRST bare "in loopd.log"
//! occurrence in Phase 1 is the pointer-bearing one (a later mention
//! gaining the path while the first stays bare re-opens the cold-probe
//! miss the clause exists to close), and (c) the literal path is present
//! file-wide (the acceptance grep, presence-only so a future doctrine edit
//! naming the path a second time does not spuriously red). No production
//! code changes — docs+pin only.
//!
//! T48 doctrine: the pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test
//! binaries with cwd = the package root), never via the compile-time
//! manifest-dir macro — under the T47 shared cache a compile-time path can
//! point at a since-removed worktree. T78 idiom: multi-word needles are
//! matched against whitespace-collapsed text because the doctrine prose
//! wraps mid-phrase. Deletion-proof convention: every count leg doubles as
//! the deletion check (delete the clause → count 0 → RED); the
//! GREEN → delete → RED → restore → GREEN hand-check was run before
//! committing and is stated in the commit message.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 flat idiom): the doctrine prose wraps
/// mid-phrase, so a multi-word needle must match whitespace-collapsed text
/// or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- windows (the T64 loose-heading pattern) ----

const PHASE1_HEADING: &str = "## Phase 1";
const PHASE2_HEADING: &str = "## Phase 2";

fn window<'a>(spec: &'a str, start_marker: &str, end_marker: &str, what: &str) -> &'a str {
    let start = spec
        .find(start_marker)
        .unwrap_or_else(|| panic!("the {what} window's start marker {start_marker:?} is present"));
    let end = start
        + spec[start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("the {what} window's end marker {end_marker:?} follows its start"));
    &spec[start..end]
}

// ---- the pointer clause (T267 req 1) ----

/// The pointer clause as the spec writes it — the real path in backticks,
/// parenthesized, immediately after the first bare "loopd.log".
const POINTER_CLAUSE: &str = "in loopd.log (`.chug/loopd/loopd.log`)";

/// The real path, bare — the literal a cold probe must be able to read out
/// of the doctrine.
const REAL_PATH: &str = ".chug/loopd/loopd.log";

/// (a) The pointer clause lives in the Phase-1 window EXACTLY once,
/// flat-matched. Zero means the T267 clause was deleted or rewrapped (the
/// cold-probe miss class re-opens); more than one means the pointer was
/// duplicated.
#[test]
fn loopd_log_pointer_clause_lives_in_phase1_exactly_once() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        POINTER_CLAUSE.starts_with("in loopd.log (") && POINTER_CLAUSE.ends_with("`)"),
        "the pointer needle must be the parenthesized-path clause shape"
    );
    assert!(
        POINTER_CLAUSE.contains(REAL_PATH),
        "the pointer needle must carry the real path {REAL_PATH:?} verbatim"
    );
    let spec = flat(&loop_spec());
    let p1 = window(
        &spec,
        &flat(PHASE1_HEADING),
        &flat(PHASE2_HEADING),
        "Phase-1",
    );
    let needle = flat(POINTER_CLAUSE);
    assert_eq!(
        p1.matches(&needle).count(),
        1,
        "LOOP-SPEC Phase 1 must name the supervisor log's real path at the \
         first bare loopd.log mention exactly once ({needle:?}) — zero means \
         the T267 pointer clause was deleted or rewrapped, more than one \
         means it is stated twice"
    );
}

/// (b) Req 1's "first bare mention" semantics: the FIRST "in loopd.log"
/// occurrence in the flat Phase-1 window is the pointer-bearing one — a
/// cold probe reading Phase 1 top-down hits the real path at the first
/// mention, not three paragraphs later. A later mention gaining the path
/// while the first stays bare re-opens the miss class this clause closes.
#[test]
fn first_bare_loopd_log_mention_in_phase1_is_the_pointer_bearing_one() {
    let spec = flat(&loop_spec());
    let p1 = window(
        &spec,
        &flat(PHASE1_HEADING),
        &flat(PHASE2_HEADING),
        "Phase-1",
    );
    let first = p1
        .find("in loopd.log")
        .unwrap_or_else(|| panic!("Phase 1 must still mention loopd.log at all"));
    assert!(
        p1[first..].starts_with(&flat(POINTER_CLAUSE)),
        "Phase 1's FIRST bare \"in loopd.log\" must be immediately followed \
         by the real-path parenthetical ({:?}) — a pointer parked on a later \
         mention leaves the first bare mention guessing \
         `.chug/loopd.log` (the T267 miss class)",
        flat(POINTER_CLAUSE)
    );
}

/// (c) The literal path is present file-wide (raw text, unflattened) — the
/// acceptance grep's direct pin. Presence-only: a future doctrine edit that
/// names the path a second time (e.g. the T259 triage paragraph gaining its
/// own pointer) must not spuriously red this leg; legs (a)/(b) carry the
/// positional and duplication guards.
#[test]
fn real_loopd_log_path_literal_is_present_file_wide() {
    let spec = loop_spec();
    assert!(
        spec.contains(REAL_PATH),
        "LOOP-SPEC.md must contain the literal supervisor-log path \
         {REAL_PATH:?} — the T267 pointer clause drifted out of doctrine"
    );
}

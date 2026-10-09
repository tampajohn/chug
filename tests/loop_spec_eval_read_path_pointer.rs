//! T268 — eval-read-path pointer pins: LOOP-SPEC's Phase-1 opening
//! paragraph names the T260 evaluation read-path pair by its real paths,
//! `.chug/eval-delta.md` and `.chug/eval-state.md`.
//!
//! The `guarded-path-probe` census's state-file sub-variant: eval and
//! orchestrator streams probing the evaluation state guess a literal and
//! miss — `.json`-variant state-path guesses fired at trips 47/48, again at
//! trip 53 (`.chug/eval-state.json`, d1791549131-2), and trip 54 added the
//! wrong-dir variant (`.chug/loopd/eval-state.json`, d1791554509-2). Every
//! fire masked, every fire self-corrected one probe later via discovery,
//! zero casualty — but ~1 iteration per fire, and LOOP-SPEC (the surface a
//! cold eval stream reads first) carried ZERO mentions of either real path
//! at filing, while META-META-SPEC named them correctly (4 mentions) and
//! still the misses fired. The remedy mirrors T267 exactly (the loopd.log
//! pointer, landed 00bc596 — its first live exercise passed: the trip-54
//! stream's loopd.log probes hit first-try): a ONE-SENTENCE pointer in the
//! Phase-1 opening paragraph naming both real paths with the honesty
//! half-clause ("no `.json` variant of either file exists"), riding the
//! same sentence block as the `.chug/events.jsonl` preference — the two
//! other ingress probes (events stream, read-path pair) now both named.
//!
//! These pins keep the pointer from silently drifting out in a future
//! doctrine edit: (a) the pointer sentence sits in the Phase-1 window
//! exactly once, flat-matched (deletion-proof — delete the sentence, the
//! count goes 0, RED; a reword goes 0, RED; a duplicated copy goes 2, RED),
//! (b) both real-path literals are present file-wide raw (the acceptance
//! grep's direct pin, presence-only so a future doctrine edit naming a path
//! a second time does not spuriously red — legs (a)/(c) carry the
//! positional and duplication guards), and (c) each literal's FIRST
//! occurrence inside Phase 1 is the pointer-bearing one (a cold probe
//! reading Phase 1 top-down must hit the real path at the pair's first
//! mention, not meet a bare literal the pointer was parked after — the
//! T267 first-mention semantics, symmetric here).
//!
//! T48 doctrine: the pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test
//! binaries with cwd = the package root), never via the compile-time
//! manifest-dir macro — under the T47 shared cache a compile-time path can
//! point at a since-removed worktree. T78 idiom: the multi-word pointer
//! sentence is matched against whitespace-collapsed text because the
//! doctrine prose wraps mid-phrase. Deletion-proof convention: every count
//! leg doubles as the deletion check (delete the sentence → count 0 →
//! RED); the GREEN → delete → RED → restore → GREEN hand-check was run
//! before committing and is stated in the commit message. No production
//! code changes — docs+pin only.

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

// ---- the pointer sentence (T268 req 1) ----

/// The pointer sentence as the spec writes it — both real paths, the
/// loopd-rebuilds fact, and the no-json honesty half-clause, in one
/// sentence.
const POINTER_SENTENCE: &str = "The T260 read-path pair is named, not guessed: the \
delta you read is `.chug/eval-delta.md` (loopd rebuilds it before every \
cycle) and the state you maintain is `.chug/eval-state.md` — no `.json` \
variant of either file exists.";

/// The real paths, bare — the literals a cold probe must be able to read
/// out of the doctrine (the acceptance grep greps exactly these).
const DELTA_PATH: &str = ".chug/eval-delta.md";
const STATE_PATH: &str = ".chug/eval-state.md";

/// (a) The pointer sentence lives in the Phase-1 window EXACTLY once,
/// flat-matched. Zero means the T268 sentence was deleted, rewrapped, or
/// reworded out of shape (the cold-probe miss class re-opens); more than
/// one means the pointer was duplicated.
#[test]
fn eval_read_path_pointer_sentence_lives_in_phase1_exactly_once() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    let needle = flat(POINTER_SENTENCE);
    assert!(
        needle.contains(DELTA_PATH) && needle.contains(STATE_PATH),
        "the pointer needle must carry both real paths ({DELTA_PATH:?}, \
         {STATE_PATH:?}) verbatim"
    );
    assert!(
        needle.contains("no `.json` variant"),
        "the pointer needle must carry the no-json honesty half-clause — \
         the .json-variant guess is the census's most-fired shape"
    );
    let spec = flat(&loop_spec());
    let p1 = window(
        &spec,
        &flat(PHASE1_HEADING),
        &flat(PHASE2_HEADING),
        "Phase-1",
    );
    assert_eq!(
        p1.matches(&needle).count(),
        1,
        "LOOP-SPEC Phase 1 must name the T260 read-path pair's real paths \
         in one pointer sentence exactly once ({needle:?}) — zero means the \
         T268 pointer sentence was deleted, rewrapped, or reworded, more \
         than one means it is stated twice"
    );
}

/// (b) Both real-path literals are present file-wide (raw text,
/// unflattened) — the acceptance grep's direct pin. Presence-only: a
/// future doctrine edit that names a path a second time (e.g. the Phase-3
/// fallback paragraph gaining its own pointer) must not spuriously red
/// this leg; legs (a)/(c) carry the positional and duplication guards.
#[test]
fn real_eval_read_path_literals_are_present_file_wide() {
    let spec = loop_spec();
    assert!(
        spec.contains(DELTA_PATH),
        "LOOP-SPEC.md must contain the literal eval-delta path {DELTA_PATH:?} \
         — the T268 pointer sentence drifted out of doctrine"
    );
    assert!(
        spec.contains(STATE_PATH),
        "LOOP-SPEC.md must contain the literal eval-state path {STATE_PATH:?} \
         — the T268 pointer sentence drifted out of doctrine"
    );
}

/// (c) First-mention semantics, symmetric to T267's leg (b): within the
/// flat Phase-1 window, the FIRST occurrence of each real-path literal is
/// the pointer-bearing one — a cold probe reading Phase 1 top-down hits
/// the real path inside the full pointer (with its no-json half-clause) at
/// the pair's first mention, never at a bare literal parked after it.
#[test]
fn first_eval_read_path_mentions_in_phase1_are_the_pointer_bearing_ones() {
    let spec = flat(&loop_spec());
    let p1 = window(
        &spec,
        &flat(PHASE1_HEADING),
        &flat(PHASE2_HEADING),
        "Phase-1",
    );
    let needle = flat(POINTER_SENTENCE);
    let sentence_start = p1
        .find(&needle)
        .unwrap_or_else(|| panic!("the T268 pointer sentence must sit in the Phase-1 window"));
    let sentence_end = sentence_start + needle.len();
    for (what, literal) in [("delta", DELTA_PATH), ("state", STATE_PATH)] {
        let first = p1
            .find(literal)
            .unwrap_or_else(|| panic!("Phase 1 must still name the {what} path {literal:?} at all"));
        assert!(
            first >= sentence_start && first < sentence_end,
            "Phase 1's FIRST mention of the {what} path ({literal:?}) must be \
             inside the T268 pointer sentence — a bare mention parked before \
             the pointer leaves the cold probe reading the path with no \
             no-json guidance at the pair's first sighting"
        );
    }
}

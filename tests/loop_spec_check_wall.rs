//! T163 — the check-wall: goal-gate check timeout vs real check walls.
//!
//! Every spec `check:` line runs under [`tools::CHECK_TIMEOUT_SECS`] (the
//! goal gate's verification budget, `src/tools.rs`). Cycle-76 eval §2.1
//! recorded the class this row closes: t153-fixup's spec check embedded
//! `cargo clippy --release --all-targets` + release tests (warm wall over
//! 600s) and the child was rejected TWICE with `check command failed` on
//! GREEN work (`.chug/events-t153-fixup-20260929-185044.jsonl`, `goal:
//! rejected 2`); t152-fixup took one more. The warm full debug suite
//! runs ~800–1000s against the old 600s cap — each false rejection burns
//! child budget and can cost a wrap (named the top next-eval candidate by
//! the cycle-73 AND cycle-75 wraps).
//!
//! Three legs pin the remedy:
//! (a) the cap is 1200 (20 min: covers the observed 800–1000s warm wall
//!     with headroom, still bounded inside an 80-iter/35-min child);
//! (b) META-META-SPEC's spec-quality bar carries the check-wall doctrine
//!     sentence (budget the warm wall; ~300s warm preferred; >~600s must
//!     state its measured warm wall in the spec's repo-context), exactly
//!     once, near the `check:`-line rules;
//! (c) the goal-rejection message still interpolates the const (never a
//!     hardcoded number) — the T9 environment-honesty channel cannot
//!     drift from reality.
//!
//! These are source-text pins by construction: the crate is binary-only
//! (`src/main.rs`, no lib target), so an integration test cannot import
//! the const or call `goal_rejected_message`; the established idiom is a
//! runtime file read of the source the binary is built from.
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

use std::path::{PathBuf};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn tools_rs() -> String {
    std::fs::read_to_string(repo_root().join("src/tools.rs"))
        .expect("reading src/tools.rs from the runtime checkout")
}

fn driver_rs() -> String {
    std::fs::read_to_string(repo_root().join("src/driver.rs"))
        .expect("reading src/driver.rs from the runtime checkout")
}

fn meta_meta_spec() -> String {
    std::fs::read_to_string(repo_root().join("META-META-SPEC.md"))
        .expect("reading META-META-SPEC.md from the runtime checkout")
}

// ---- (a) the const value pin ----

/// The const's declaration prefix — exactly one occurrence expected in
/// src/tools.rs; the digits after it are parsed, so a reformat cannot
/// silently unpin the value.
const DECL_PREFIX: &str = "pub const CHECK_TIMEOUT_SECS: u64 = ";

/// (a) The goal-gate check budget is 1200s. Reverting to the old 600 cap
/// (or any other value) goes red here; renaming or moving the const away
/// from src/tools.rs also goes red (count 0). The value is parsed from
/// the declaration, so a whitespace reformat cannot silently unpin it.
#[test]
fn check_timeout_const_is_1200() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        DECL_PREFIX.starts_with("pub const")
            && DECL_PREFIX.contains("CHECK_TIMEOUT_SECS")
            && DECL_PREFIX.ends_with("= "),
        "the declaration needle must name the const and its type verbatim"
    );
    let tools = tools_rs();
    assert_eq!(
        tools.matches(DECL_PREFIX).count(),
        1,
        "src/tools.rs must declare CHECK_TIMEOUT_SECS exactly once — zero \
         means it was renamed or moved (every reference interpolates it), \
         more than one means a second declaration drifted in"
    );
    let start = tools.find(DECL_PREFIX).unwrap() + DECL_PREFIX.len();
    let digits_end = start
        + tools[start..]
            .find(';')
            .expect("the const declaration is terminated by a semicolon");
    let value: u64 = tools[start..digits_end]
        .trim()
        .parse()
        .expect("the declared value parses as u64");
    assert_eq!(
        value, 1200,
        "CHECK_TIMEOUT_SECS must be 1200 (T163) — the old 600 cap false-\
         rejected green work whenever a spec check's warm wall passed 10 \
         minutes (t153-fixup rejected twice, t152-fixup once, cycle-76 \
         eval §2.1); 1200 covers the observed 800-1000s warm full-suite \
         wall with headroom"
    );
}

// ---- (b) the META-META-SPEC check-wall doctrine sentence ----

/// The doctrine sentence's repo-context token — the load-bearing phrase
/// that forces a long-wall spec to STATE its measured warm wall. Must
/// occur EXACTLY once in META-META-SPEC.md.
const MEASURED_NEEDLE: &str = "measured warm wall";

/// The doctrine sentence's phrase token. NOTE the overlap: the required
/// sentence uses "warm wall" twice standalone ("budget the check line's
/// warm wall", "whose warm wall exceeds") plus once inside the
/// `measured warm wall` token — so the census is 3 total, 1 of them
/// inside [`MEASURED_NEEDLE`]. Deleting the sentence drops both counts to
/// 0; duplicating the sentence doubles both; rewording any one instance
/// breaks the relation.
const WARM_WALL_NEEDLE: &str = "warm wall";

/// The doctrine sentence's half-gate token — the threshold (half the
/// 1200s gate) above which a spec must state its measured warm wall.
/// Must occur EXACTLY once in META-META-SPEC.md.
const HALF_GATE_NEEDLE: &str = "~600s";

/// The sentence sits in the spec-quality-bar paragraph, AFTER the T114
/// BREAK-side rule it extends (both are `check:`-line cost rules) and
/// BEFORE the T110 estimate-ceiling sentence — pinned in the T64
/// loose-heading window pattern (the "## Extend `TODO.md`" section).
const BREAK_SIDE_RULE: &str = "every test the change can BREAK";
const EXTEND_TODO_HEADING: &str = "## Extend `TODO.md`";
const HANDOFF_HEADING: &str = "## Handoff section in EVALUATION.md";

/// (b) The check-wall doctrine sentence's load-bearing tokens occur
/// EXACTLY once each in META-META-SPEC.md: `measured warm wall` once,
/// `~600s` once, and `warm wall` with the census its required sentence
/// dictates (3 total: two standalone + one inside `measured warm wall`).
/// Delete the sentence and every token goes red (count 0 — which also
/// breaks the spec's own line-wise grep); duplicate it and the counts
/// double; reword one instance and the census relation breaks.
#[test]
fn check_wall_budget_tokens_occur_exactly_once() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        WARM_WALL_NEEDLE.contains("warm") && WARM_WALL_NEEDLE.contains("wall"),
        "the warm-wall needle must be the phrase verbatim"
    );
    assert!(
        MEASURED_NEEDLE.starts_with("measured") && MEASURED_NEEDLE.contains(WARM_WALL_NEEDLE),
        "the measured needle must contain the warm-wall phrase (the overlap \
         the census below accounts for)"
    );
    assert!(
        HALF_GATE_NEEDLE.starts_with('~') && HALF_GATE_NEEDLE.ends_with('s'),
        "the half-gate needle must be the ~600s token verbatim"
    );
    let spec = meta_meta_spec();
    let measured = spec.matches(MEASURED_NEEDLE).count();
    assert_eq!(
        measured, 1,
        "META-META-SPEC must state {MEASURED_NEEDLE:?} exactly once — zero \
         means the check-wall doctrine sentence was deleted (or the needle \
         was rewrapped across a line break, which also breaks the spec's \
         own line-wise grep), more than one means it is stated twice"
    );
    let warm_wall_total = spec.matches(WARM_WALL_NEEDLE).count();
    assert_eq!(
        warm_wall_total, 3,
        "META-META-SPEC must state {WARM_WALL_NEEDLE:?} exactly three times \
         — the required sentence uses it twice standalone (\"budget the \
         check line's warm wall\", \"whose warm wall exceeds\") plus once \
         inside {MEASURED_NEEDLE:?}; zero means the sentence was deleted, \
         more than three means it is stated twice, fewer means an instance \
         was reworded away"
    );
    assert_eq!(
        spec.matches(HALF_GATE_NEEDLE).count(),
        1,
        "META-META-SPEC must state the {HALF_GATE_NEEDLE:?} half-gate \
         threshold exactly once — zero means the doctrine sentence was \
         deleted (or the token rewrapped across a line break), more than \
         one means it is stated twice"
    );
}

/// (b placement) The doctrine sentence sits INSIDE the spec-quality-bar
/// window (the "## Extend `TODO.md`" section through the "## Handoff"
/// heading), AFTER the T114 BREAK-side rule it extends and BEFORE the
/// T110 estimate-ceiling sentence — near the `check:`-line rules where
/// spec authors read the bar top-to-bottom at filing time, not a bolt-on
/// in another section.
#[test]
fn check_wall_sentence_sits_in_the_spec_quality_bar_near_the_check_rules() {
    let spec = meta_meta_spec();
    let start = spec
        .find(EXTEND_TODO_HEADING)
        .expect("the Extend-TODO heading present");
    let end = start
        + spec[start..]
            .find(HANDOFF_HEADING)
            .expect("the Handoff heading present after the Extend-TODO heading");
    let window = &spec[start..end];
    let break_rule = window
        .find(BREAK_SIDE_RULE)
        .expect("the spec-quality-bar window must carry the T114 BREAK-side rule");
    let measured = window.find(MEASURED_NEEDLE).unwrap_or_else(|| {
        panic!(
            "the spec-quality-bar window must carry {MEASURED_NEEDLE:?} — \
             the check-wall doctrine sentence was deleted, or moved out of \
             the Extend-TODO section"
        )
    });
    let half_gate = window.find(HALF_GATE_NEEDLE).unwrap_or_else(|| {
        panic!(
            "the spec-quality-bar window must carry {HALF_GATE_NEEDLE:?} — \
             the check-wall doctrine sentence was deleted, or moved out of \
             the Extend-TODO section"
        )
    });
    // The T110 estimate-ceiling sentence the doctrine sentence sits before
    // (its needle must stay contiguous on one line — the T125 pin counts
    // it file-wide).
    let estimate = window
        .find("Every spec carries an `estimate: ~N changed lines` line")
        .expect("the spec-quality-bar window must carry the estimate-ceiling sentence (T110)");
    assert!(
        break_rule < half_gate && half_gate < measured && measured < estimate,
        "the check-wall doctrine sentence must sit in the spec-quality-bar \
         paragraph after the BREAK-side rule it extends ({break_rule}) and \
         BEFORE the estimate-ceiling sentence ({estimate}) — half-gate \
         token at {half_gate}, measured-warm-wall token at {measured}"
    );
}

// ---- (c) the rejection message still interpolates the const ----

/// The function whose format string renders the check's environment note
/// (the T9 honesty channel) and the next `\nfn ` that closes the window.
const FN_NEEDLE: &str = "fn goal_rejected_message";
/// The interpolation token as written in the format string.
const INTERP_NEEDLE: &str = "{CHECK_TIMEOUT_SECS}s timeout";
/// The named-argument binding that sources the token from the const.
const BINDING_NEEDLE: &str = "CHECK_TIMEOUT_SECS = tools::CHECK_TIMEOUT_SECS";

/// (c) The goal-rejection message's check-timeout figure is INTERPOLATED
/// from [`tools::CHECK_TIMEOUT_SECS`], never hardcoded. The T9 doc comment
/// on the function promises the note "cannot drift from reality" because
/// its literals are derived from the same constants the shell wrapper
/// uses — hardcoding `600s timeout` (or `1200s timeout`) in the format
/// string breaks exactly that promise the next time the const moves, and
/// kills this pin (the interpolation token count drops to 0). Scoped to
/// the `goal_rejected_message` window, not a file-wide grep, so an
/// unrelated literal elsewhere cannot satisfy it.
#[test]
fn goal_rejection_message_interpolates_the_const_not_a_hardcoded_number() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        FN_NEEDLE.starts_with("fn goal_rejected") && FN_NEEDLE.ends_with("message"),
        "the window needle must name the rejection-message function"
    );
    assert!(
        INTERP_NEEDLE.starts_with("{CHECK_TIMEOUT_SECS}")
            && INTERP_NEEDLE.ends_with("s timeout"),
        "the interpolation needle must be the format-string token verbatim"
    );
    assert!(
        BINDING_NEEDLE.starts_with("CHECK_TIMEOUT_SECS =")
            && BINDING_NEEDLE.ends_with("tools::CHECK_TIMEOUT_SECS"),
        "the binding needle must source the token from the const"
    );
    let driver = driver_rs();
    let start = driver
        .find(FN_NEEDLE)
        .unwrap_or_else(|| panic!("{FN_NEEDLE:?} present (the T9 function was renamed?)"));
    let end = start
        + driver[start..]
            .find("\nfn ")
            .expect("a following fn exists after goal_rejected_message");
    let window = &driver[start..end];
    assert_eq!(
        window.matches(INTERP_NEEDLE).count(),
        1,
        "the rejection message must render its check timeout by \
         interpolating {INTERP_NEEDLE:?} exactly once — zero means the \
         number was hardcoded (a 600s or 1200s literal), which lets the \
         T9 environment note drift from the real budget the next time the \
         const moves"
    );
    assert_eq!(
        window.matches(BINDING_NEEDLE).count(),
        1,
        "the interpolated token must be bound to \
         tools::CHECK_TIMEOUT_SECS — the rendered figure comes from the \
         same constant the verify() path passes to run_shell, never from \
         a second literal"
    );
}

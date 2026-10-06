//! T242 — LOOP-SPEC cold-scale gate-leg pins: the FIRST cargo leg in
//! `target-shared-main` after any main commit runs as a bounded background
//! window, never inline under the bash cap.
//!
//! `build.rs` watches `.git/HEAD` and the loose ref it points at (the T11
//! banner-hash feature — the binary reports its checkout's commit), so
//! EVERY main commit — including md-only wrap-notes and eval commits —
//! invalidates the chug-crate fingerprint in the T57 main-dedicated
//! `target-shared-main`, and the first cargo leg after any wrap commit
//! re-lints/rebuilds the whole crate: measured 4.5–15+ min depending on
//! host load, against the `CHUG_BASH_TIMEOUT=300` bash cap that no inline
//! bash call can span. The measured cost of leaving the doctrine unwritten:
//! FIVE 300s kills across cycles 113–114 (~25 orchestrator wall minutes) —
//! the worst shape an INLINE POLL, the cycle-114 kill where the nextest
//! suite itself went 1670/1670 in 43s and the poll outlived the cap.
//! Cycles 110–113 absorbed the same leg with an ad-hoc background window,
//! but the pattern lived in wrap notes, not in LOOP-SPEC, so each
//! orchestrator re-discovered it (or didn't). The doctrine now states the
//! rule ONCE, at its first carrier (step 5's post-merge gates — the first
//! main-dedicated surface a cycle reaches), and references it from the
//! second surface (Phase 3's final gates).
//!
//! These pins assert the rule sits at BOTH gate surfaces: each surface's
//! window carries the rule name, the cold-scale mechanism (`.git/HEAD` +
//! the chug-crate fingerprint), the bounded-background-window verdict, the
//! never-inline / never-inline-polled ban, and the known-warm inline
//! exemption — while the single-statement details (the exact nohup
//! template, the measured band, the poll protocol, the iteration-budget
//! bound) stay exactly-once at step 5 so the pattern cannot fork into
//! per-surface variants. Every token is asserted so that removing its
//! clause flips the pin RED: the two-surface tokens drop to count 1 (or 0),
//! the step-5-only tokens drop to 0, and every window leg loses its
//! `contains`. Doctrine-only file: no production code changes.
//!
//! T48 doctrine: LOOP-SPEC.md is resolved from the checkout the binary RUNS
//! against (`std::env::current_dir()`), never the compile-time manifest-dir
//! macro; T78 idiom: multi-word needles are matched against
//! whitespace-collapsed text because the doctrine prose wraps mid-phrase.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Step-5 window (T64 loose-heading scope pattern, the docs-only/recovery
/// anchors): the step-5 post-merge gates clause — the shared statement's
/// carrier.
fn step5_window(spec: &str) -> &str {
    let start = spec
        .find("5. **Harvest")
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry step 5's heading (`5. **Harvest`)"));
    let end = start
        + spec[start..]
            .find("6. **Budget check")
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry step 6's heading after step 5's"));
    &spec[start..end]
}

/// Phase-3 window (the tag-doctrine anchors): `## Phase 3 — Wrap` through
/// `## Hard rules` — the final-gates surface.
fn phase3_window(spec: &str) -> &str {
    let start = spec
        .find("## Phase 3")
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry `## Phase 3`"));
    let end = start
        + spec[start..]
            .find("## Hard rules")
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry `## Hard rules` after Phase 3"));
    &spec[start..end]
}

/// (a) The rule's name at each surface, each case-exact so the two cannot
/// stand in for one another: the step-5 carrier's capitalized name and the
/// Phase-3 reference's in-sentence form. Each occurs EXACTLY once in the
/// whole flat spec, inside its own window.
const S5_NAME: &str = "Cold-scale gate-leg rule (T242)";
const P3_NAME: &str = "cold-scale gate-leg rule (T242)";

/// (b) The rule statement's tokens, carried at BOTH surfaces (exactly twice
/// whole-file, once per window): the subject leg (FIRST cargo leg in the
/// main-dedicated dir after ANY main commit), the cold-scale-by-
/// construction verdict, the mechanism (`build.rs` watches `.git/HEAD` —
/// the T11 banner-hash watcher — so every commit invalidates the chug-crate
/// fingerprint), the bounded-background-window disposition, the
/// never-inline / never-inline-polled ban, and the known-warm inline
/// exemption. The needles are matched against FLAT text (T78): both
/// clauses wrap mid-phrase.
const FIRST_LEG: &str =
    "FIRST cargo leg (build / clippy / nextest) in `target-shared-main` after ANY main commit";
const COLD_BY_CONSTRUCTION: &str = "cold-scale by construction";
const MECHANISM: &str = "`build.rs` watches `.git/HEAD`";
const GIT_HEAD: &str = ".git/HEAD";
const FINGERPRINT: &str = "chug-crate fingerprint";
const BG_WINDOW: &str = "bounded background window";
const NEVER_INLINE: &str = "inline under the bash cap";
const NEVER_POLLED: &str = "inline-polled";
const KNOWN_WARM: &str = "known-warm";
/// The same-HEAD qualifier appears three times (step 5's exemption names it
/// twice — the completed leg and the incremental later leg — Phase 3's once),
/// so it is window-pinned without a whole-file count.
const SAME_HEAD: &str = "same-HEAD";

/// (c) The shared statement's single-statement details — exactly once in
/// the whole spec, inside step 5's window, so the pattern cannot fork into
/// per-surface variants: the measured band (4.5–15+ min cold, the en-dash
/// spelled as an escape so an editor normalization cannot silently unpin
/// it), the 300s bash cap it is measured against, the exact background-
/// window template (raw-text pinned: the template must stay on ONE line to
/// stay copy-pasteable), the LATER-calls poll protocol, the `kill -0`
/// liveness check with the zombie/defunct exclusion (worded to avoid the
/// recovery pins' exactly-once `kill -0 <pid>` / `defunct-zombie` tokens —
/// this pin references step 5's ps rule instead of restating it), the
/// unbounded-window / iteration-budget bound, the T11 mechanism citation,
/// and the two cross-references that make the statement SHARED (step 5
/// names Phase 3 as the referencing surface; Phase 3 names step 5's shared
/// statement as its governor).
const BAND: &str = "4.5\u{2013}15+ min";
const BASH_CAP: &str = "300s bash cap";
const TEMPLATE: &str =
    "nohup sh -c '<cargo legs>' > /tmp/wrap-gates-<ts>.log 2>&1 & echo $!";
const LOG_PATH: &str = "/tmp/wrap-gates-<ts>.log";
const POLL: &str = "poll the log in LATER bash calls";
const KILL_ZERO: &str = "`kill -0` on the recorded pid";
const ZOMBIE_EXCLUSION: &str = "zombie/defunct exclusion";
const UNBOUNDED: &str = "window itself is unbounded";
const ITER_BOUND: &str = "iteration budget is the bound";
const T11_WATCHER: &str = "T11 banner-hash";
const S5_CARRIER_NOTE: &str = "referenced from Phase 3's final gates";
const P3_POINTER: &str = "step 5's shared statement governs";

/// (d) The known-warm exemption's worked example: the same-HEAD leg this
/// step already runs (the todo_consistency guard run) — the token that
/// makes the exemption concrete instead of a judgment call. Exactly once.
const GUARD_RUN_EXAMPLE: &str = "todo_consistency guard run";

/// (a) The rule is NAMED once at each surface, inside that surface's
/// window. Removing either clause flips this red (count 0 for its name);
/// duplicating a statement flips the count leg.
#[test]
fn rule_named_once_at_each_surface() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        S5_NAME.starts_with("Cold-scale") && S5_NAME.ends_with("(T242)"),
        "the step-5 name needle must carry the rule's capitalized name + item id"
    );
    assert!(
        P3_NAME.starts_with("cold-scale") && P3_NAME.ends_with("(T242)"),
        "the Phase-3 name needle must carry the rule's in-sentence name + item id"
    );
    let spec = flat(&loop_spec());
    assert_eq!(
        spec.matches(S5_NAME).count(),
        1,
        "LOOP-SPEC must name the cold-scale gate-leg rule at its step-5 \
         carrier exactly once — zero means the shared statement was dropped, \
         more than one means it is stated twice"
    );
    assert_eq!(
        spec.matches(P3_NAME).count(),
        1,
        "LOOP-SPEC must name the cold-scale gate-leg rule at the Phase-3 \
         final-gates surface exactly once — zero means the final-gates \
         reference was dropped, more than one means it is stated twice"
    );
    let raw = loop_spec();
    let s5 = flat(step5_window(&raw));
    let p3 = flat(phase3_window(&raw));
    assert!(
        s5.contains(S5_NAME),
        "the rule's step-5 name must live inside step 5's window — the \
         shared statement sits at the post-merge gates surface"
    );
    assert!(
        p3.contains(P3_NAME),
        "the rule's Phase-3 name must live inside the Phase-3 window — the \
         final-gates surface carries the reference"
    );
}

/// (b) The rule statement's tokens are carried at BOTH gate surfaces —
/// exactly twice whole-file (one per window), and each window contains
/// every token. Removing the token from either clause drops its whole-file
/// count to 1 AND empties that window's contains — both legs red.
#[test]
fn rule_statement_tokens_carry_at_both_surfaces() {
    assert!(
        FIRST_LEG.contains("FIRST cargo leg")
            && FIRST_LEG.contains("target-shared-main")
            && COLD_BY_CONSTRUCTION.contains("cold-scale")
            && MECHANISM.contains(".git/HEAD")
            && FINGERPRINT.contains("fingerprint")
            && BG_WINDOW.contains("background window")
            && NEVER_INLINE.contains("inline")
            && NEVER_POLLED.contains("polled")
            && KNOWN_WARM.contains("warm"),
        "the needles must carry the rule statement's tokens verbatim"
    );
    let raw = loop_spec();
    let spec = flat(&raw);
    let s5 = flat(step5_window(&raw));
    let p3 = flat(phase3_window(&raw));
    for (needle, what) in [
        (FIRST_LEG, "the FIRST-cargo-leg subject leg"),
        (COLD_BY_CONSTRUCTION, "the cold-scale-by-construction verdict"),
        (MECHANISM, "the build.rs-watches-HEAD mechanism"),
        (FINGERPRINT, "the chug-crate-fingerprint invalidation"),
        (BG_WINDOW, "the bounded-background-window disposition"),
        (NEVER_INLINE, "the never-inline-under-the-cap ban"),
        (NEVER_POLLED, "the never-inline-polled ban"),
        (KNOWN_WARM, "the known-warm inline exemption"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            2,
            "LOOP-SPEC must state {what} exactly twice — once per gate \
             surface (step 5 + Phase 3); fewer means a surface lost the \
             rule, more means it is stated beyond the two surfaces"
        );
        assert!(
            s5.contains(needle),
            "step 5's post-merge gates window must carry {what} ({needle:?})"
        );
        assert!(
            p3.contains(needle),
            "Phase 3's final-gates window must carry {what} ({needle:?})"
        );
    }
    // The mechanism token the spec names verbatim, pinned on its own so a
    // reword of the surrounding sentence cannot drop it silently.
    assert_eq!(
        spec.matches(GIT_HEAD).count(),
        2,
        "LOOP-SPEC must name the `.git/HEAD` watcher token exactly twice — \
         once per gate surface (the T11 banner-hash mechanism is WHY the \
         rule exists; a surface without it invites deletion as superstition)"
    );
    assert!(s5.contains(GIT_HEAD) && p3.contains(GIT_HEAD));
    // The same-HEAD qualifier at both surfaces (count 3 whole-file — step 5
    // names it twice — so window-pinned only).
    assert!(
        s5.contains(SAME_HEAD) && p3.contains(SAME_HEAD),
        "both gate surfaces must carry the same-HEAD qualifier of the \
         known-warm exemption"
    );
}

/// (c) The shared statement's details stay exactly-once at step 5: the
/// measured band against the 300s cap, the exact nohup template (raw-text
/// pinned — it must stay on one line), the log path, the LATER-calls poll
/// protocol, the kill -0 liveness with the zombie/defunct exclusion, the
/// unbounded-window/iteration-budget bound, the T11 citation, and BOTH
/// cross-references (step 5 → Phase 3's reference; Phase 3 → step 5's
/// statement). Removing any of them from step 5 drops its count to 0.
#[test]
fn shared_statement_details_stated_once_inside_step_5() {
    assert!(
        BAND.starts_with("4.5") && BAND.ends_with("min"),
        "the band needle must carry the measured 4.5–15+ min range"
    );
    assert!(
        TEMPLATE.starts_with("nohup sh -c")
            && TEMPLATE.contains("/tmp/wrap-gates-")
            && TEMPLATE.ends_with("& echo $!"),
        "the template needle must carry the exact background-window command"
    );
    let raw = loop_spec();
    let spec = flat(&raw);
    let s5_raw = step5_window(&raw);
    let s5 = flat(s5_raw);
    for (needle, what) in [
        (BAND, "the measured 4.5–15+ min cold band"),
        (BASH_CAP, "the 300s bash cap the band is measured against"),
        (POLL, "the poll-the-log-in-LATER-calls protocol"),
        (KILL_ZERO, "the kill -0 liveness check on the recorded pid"),
        (ZOMBIE_EXCLUSION, "the zombie/defunct exclusion in the poll"),
        (UNBOUNDED, "the window-is-unbounded statement"),
        (ITER_BOUND, "the iteration-budget-is-the-bound statement"),
        (T11_WATCHER, "the T11 banner-hash mechanism citation"),
        (S5_CARRIER_NOTE, "the step-5 → Phase-3 cross-reference"),
        (LOG_PATH, "the /tmp/wrap-gates-<ts>.log path"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once, at the step-5 carrier \
             — zero means the detail was dropped, more than one means the \
             pattern forked into a second variant"
        );
        assert!(
            s5.contains(needle),
            "step 5's window must carry {what} ({needle:?})"
        );
    }
    // The template is pinned against the RAW spec: flat text would forgive
    // a re-wrap, and a wrapped template is no longer copy-pasteable — the
    // one-line form is part of "specified exactly".
    assert_eq!(
        raw.matches(TEMPLATE).count(),
        1,
        "LOOP-SPEC must carry the exact background-window template exactly \
         once, on one line — zero means the pattern was dropped or reworded, \
         more than one means it forked"
    );
    assert!(
        s5_raw.contains(TEMPLATE),
        "the background-window template must live inside step 5's window"
    );
    // Phase 3's side of the shared statement: the pointer at step 5's
    // governor and the per-step-5-pattern reference.
    let p3 = flat(phase3_window(&raw));
    assert!(
        p3.contains(P3_POINTER),
        "Phase 3's final-gates clause must point at step 5's shared \
         statement ({P3_POINTER:?}) — a reference-less restatement would \
         fork the doctrine"
    );
    assert_eq!(spec.matches(P3_POINTER).count(), 1);
    assert!(
        p3.contains("per step 5's pattern"),
        "Phase 3's clause must run the window per step 5's pattern"
    );
}

/// (d) The known-warm exemption names its worked example — the
/// todo_consistency guard run this step already completes — and the
/// THIS-cycle qualifier that bounds it (step 5's capitalized THIS cycle;
/// Phase 3's in-sentence form). Deleting the example leaves the exemption
/// a judgment call; this leg holds it concrete.
#[test]
fn known_warm_exemption_names_the_guard_run_example() {
    assert!(
        GUARD_RUN_EXAMPLE.contains("todo_consistency"),
        "the example needle must name the guard run"
    );
    let raw = loop_spec();
    let spec = flat(&raw);
    let s5 = flat(step5_window(&raw));
    let p3 = flat(phase3_window(&raw));
    assert_eq!(
        spec.matches(GUARD_RUN_EXAMPLE).count(),
        1,
        "LOOP-SPEC must name the todo_consistency guard run as the \
         known-warm example exactly once — zero means the exemption lost \
         its worked example"
    );
    assert!(s5.contains(GUARD_RUN_EXAMPLE));
    assert!(
        s5.contains("already completed THIS cycle"),
        "step 5's exemption must bound the warm leg to THIS cycle's \
         same-HEAD completion"
    );
    assert!(
        p3.contains("already completed this cycle"),
        "Phase 3's exemption must bound the warm leg to this cycle's \
         same-HEAD completion"
    );
}

// ---- T253 — the known-warm exemption is PROFILE-exact; the guard floor names --release ----
//
// Cycle 130 fired the profile-blind warmth doctrine twice in one delta
// (the standing inline-cold-scale census's third and fourth fires): (1)
// the orchestrator's post-merge main-dedicated guard leg (bare `cargo
// test --test todo_consistency`, DEBUG) was killed at the 300s bash cap
// 300,136ms in, minutes after the same dir's release gates ran green
// same-HEAD — T242's letter PERMITTED the inline run ("a same-HEAD cargo
// leg already completed THIS cycle"; its example even names the guard
// run, which is itself a cross-profile counterexample) — and the
// identical legs re-run WITH `--release` passed instantly; (2)
// t251-impl's bare `cargo clippy --all-targets -- -D warnings` (dev) was
// killed inline at the cap; windowed, dev clippy took 7m44s cold vs 51s
// release in the same worktree (the T253 amendment keys the clippy
// surfaces' form to --release — T235's needle pins that form). Root
// cause is doctrinal, not cargo: T242's known-warm clause and the
// guard/clippy templates never named a profile, so a leg's warmth was
// left to inference — and the inference is wrong whenever the leg's
// profile differs from the dir's warm one. The doctrine now names the
// profile at every surface: the known-warm exemption is profile-exact at
// BOTH gate surfaces, the per-profile warmth principle is stated once at
// the step-5 carrier with the fire named, the (seconds) tie-back
// qualifies the warmth claim with the profile the gates run, and the
// main-dedicated guard invocation runs `--release` (the docs-only floor's
// FLOOR_CMD/FLOOR_CMD_T8 needles in loop_spec_docs_only_gates.rs pin the
// floor commands; the T231 leg in loop_spec_recovery.rs pins the
// invocation beside this one). This leg pins the profile-exact phrasing,
// the per-profile principle, the release guard invocation, and the fire's
// duration token in the file's pattern.

/// Step 5's profile-exact statement of the completed leg — same-HEAD AND
/// same-profile (T253; flat-matched, the clause wraps mid-phrase). Must
/// occur EXACTLY once, inside step 5's window.
const S5_PROFILE_EXACT: &str = "same-HEAD, SAME-PROFILE cargo leg already completed THIS cycle";

/// Step 5's profile-exact statement of the later leg the exemption
/// licenses — the incremental leg must match BOTH qualifiers too.
const S5_PROFILE_LATER: &str = "later same-HEAD, same-profile leg incremental";

/// Phase 3's profile-exact restatement (the in-sentence lowercase form,
/// distinct from step 5's capitalized statement, so the two surfaces
/// cannot stand in for one another). Must occur EXACTLY once, inside the
/// Phase-3 window.
const P3_PROFILE_EXACT: &str = "same-HEAD, same-profile cargo leg already completed this cycle";

/// The per-profile warmth principle — stated once at the step-5 carrier,
/// with the cycle-130 fire named beside it (both flat-matched).
const PER_PROFILE: &str = "Warmth is per-PROFILE, never per-dir";
const FIRE_PRINCIPLE: &str = "a release leg does not warm a debug leg";

/// The fire clause in the known-warm amendment — the same-HEAD debug
/// guard leg that died at the cap minutes after the same dir's release
/// gates ran green (the cross-profile counterexample).
const FIRE_NAMED: &str = "a same-HEAD debug guard leg died at the bash cap";

/// The (seconds) tie-back's profile qualifier (the T231 sentence, amended
/// by T253): the dir is warm across cycles IN THE PROFILE THE GATES RUN.
const SECONDS_PROFILE_QUALIFIER: &str = "warm across cycles IN THE PROFILE THE GATES RUN";

/// The cycle-130 fire's measured duration — the load-bearing evidence
/// token (the orchestrator's guard leg killed at 300,136ms; the identical
/// `--release` re-run passed instantly).
const FIRE_DURATION: &str = "300,136ms";

/// The release-profile main-dedicated guard invocation (the T231
/// invocation as amended by T253) — raw-text pinned: it must stay on ONE
/// line to stay copy-pasteable. Must occur EXACTLY once, inside step 5's
/// window.
const GUARD_RELEASE_INVOCATION: &str =
    "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main cargo test --release --test todo_consistency";

/// (e) T253 — the known-warm exemption is profile-exact at BOTH gate
/// surfaces, the per-profile warmth principle is stated once at the
/// step-5 carrier naming the cycle-130 fire, the (seconds) tie-back
/// qualifies the warmth claim with the profile the gates run, and the
/// main-dedicated guard invocation runs `--release`. Reverting the
/// profile-exact phrasing at either surface, reverting the guard's
/// `--release`, or dropping the fire's evidence token flips this leg red
/// (count 0, or a window miss); duplicating any needle fires its count
/// leg; rewrapping the raw-text invocation across a line break dies on
/// the raw count/window legs.
#[test]
fn known_warm_exemption_is_profile_exact_and_guard_floor_names_release() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        S5_PROFILE_EXACT.contains("SAME-PROFILE")
            && S5_PROFILE_EXACT.ends_with("THIS cycle"),
        "the step-5 profile-exact needle must carry the SAME-PROFILE \
         qualifier + the THIS-cycle bound verbatim"
    );
    assert!(
        S5_PROFILE_LATER.starts_with("later same-HEAD")
            && S5_PROFILE_LATER.ends_with("leg incremental"),
        "the step-5 later-leg needle must carry the same-profile \
         incremental-leg language verbatim"
    );
    assert!(
        P3_PROFILE_EXACT.contains("same-profile")
            && P3_PROFILE_EXACT.ends_with("this cycle"),
        "the Phase-3 profile-exact needle must carry the in-sentence \
         same-profile form verbatim (lowercase, distinct from step 5's)"
    );
    assert!(
        PER_PROFILE.starts_with("Warmth is per-PROFILE")
            && PER_PROFILE.ends_with("never per-dir"),
        "the principle needle must carry the per-PROFILE warmth statement \
         verbatim"
    );
    assert!(
        FIRE_NAMED.contains("debug guard leg")
            && FIRE_NAMED.ends_with("bash cap"),
        "the fire needle must name the cross-profile casualty verbatim"
    );
    assert!(
        SECONDS_PROFILE_QUALIFIER.starts_with("warm across cycles")
            && SECONDS_PROFILE_QUALIFIER.ends_with("THE GATES RUN"),
        "the tie-back qualifier needle must carry the profile-the-gates-run \
         language verbatim"
    );
    assert_eq!(FIRE_DURATION, "300,136ms");
    assert!(
        GUARD_RELEASE_INVOCATION.starts_with("CARGO_TARGET_DIR=")
            && GUARD_RELEASE_INVOCATION.contains("target-shared-main")
            && GUARD_RELEASE_INVOCATION.ends_with("--release --test todo_consistency"),
        "the invocation needle must carry the T57 env prefix + the \
         release-profile guard selection verbatim"
    );

    let raw = loop_spec();
    let spec = flat(&raw);
    let s5 = flat(step5_window(&raw));
    let p3 = flat(phase3_window(&raw));

    // Profile-exact phrasing at BOTH T242 surfaces: each needle occurs
    // EXACTLY once whole-file and inside its own window — a revert at
    // either surface drops its count to 0, a duplicate fires the count
    // leg, a move out of the window dies on the contains.
    for (needle, what, window, wname) in [
        (
            S5_PROFILE_EXACT,
            "step 5's same-HEAD, SAME-PROFILE completed-leg statement",
            &s5,
            "step 5",
        ),
        (
            S5_PROFILE_LATER,
            "step 5's same-profile later-leg statement",
            &s5,
            "step 5",
        ),
        (
            P3_PROFILE_EXACT,
            "Phase 3's same-HEAD, same-profile restatement",
            &p3,
            "Phase 3",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             profile-exact amendment was reverted, more than one means it \
             is stated twice"
        );
        assert!(
            window.contains(needle),
            "{wname}'s window must carry {what} ({needle:?})"
        );
    }

    // The per-profile principle + the fire's two clauses: exactly once,
    // at the step-5 carrier (the single-statement discipline — Phase 3
    // references the shared statement, it does not restate the
    // principle).
    for (needle, what) in [
        (PER_PROFILE, "the warmth-is-per-PROFILE principle"),
        (FIRE_PRINCIPLE, "the release-leg-does-not-warm-debug principle"),
        (FIRE_NAMED, "the cycle-130 fire's cross-profile casualty clause"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once, at the step-5 \
             carrier — zero means the amendment was dropped, more than \
             one means the principle forked into a second variant"
        );
        assert!(
            s5.contains(needle),
            "step 5's window must carry {what} ({needle:?})"
        );
    }

    // The (seconds) tie-back's profile qualifier and the fire's measured
    // duration: exactly once, inside step 5's window.
    assert_eq!(
        spec.matches(SECONDS_PROFILE_QUALIFIER).count(),
        1,
        "LOOP-SPEC must qualify the (seconds) warmth claim with the \
         profile the gates run exactly once — zero means the T253 \
         qualifier was reverted, more than one means it is stated twice"
    );
    assert!(s5.contains(SECONDS_PROFILE_QUALIFIER));
    assert_eq!(
        spec.matches(FIRE_DURATION).count(),
        1,
        "LOOP-SPEC must carry the cycle-130 fire's 300,136ms duration \
         exactly once — zero means the evidence token was dropped, more \
         than one means it leaked beyond the tie-back"
    );
    assert!(s5.contains(FIRE_DURATION));

    // The release-profile guard invocation: raw-text pinned (one line,
    // copy-pasteable), exactly once, inside step 5's window. Reverting
    // the `--release` drops the count to 0 — the same flip T231's leg in
    // loop_spec_recovery.rs rides.
    assert_eq!(
        raw.matches(GUARD_RELEASE_INVOCATION).count(),
        1,
        "LOOP-SPEC must carry the release-profile main-dedicated guard \
         invocation exactly once, on one line — zero means the `--release` \
         was reverted (a bare debug-profile guard leg in target-shared-main \
         is cold-scale, the cycle-130 fire), more than one means it forked"
    );
    assert!(
        step5_window(&raw).contains(GUARD_RELEASE_INVOCATION),
        "the release-profile guard invocation must live inside step 5's \
         window (the TODO.md-edit guard paragraph)"
    );
}

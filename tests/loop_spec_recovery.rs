//! T63 — doctrine pins: LOOP-SPEC §2 step 2 names the `delegate resume:true`
//! budget-death recovery leg.
//!
//! The loop's most common child failure is BUDGET death with the work
//! complete-but-uncommitted or unwrapped — t15/t17/t20 at 40/40, t28, t47,
//! t55, and t58 at 50/50 (the T58 row itself records the fifth occurrence
//! of the pattern). T58 shipped the in-tool recovery primitive (launch's
//! optional `resume: true`, plus `delegate status` reading the LATEST run
//! segment), but the DOCTRINE never named it — an orchestrator mid-arc
//! reads LOOP-SPEC, not the tool schema history, and cycle 18's actual
//! recovery was a hand-rolled bash `chug run --resume` under nohup (the
//! exact launch pattern T24 replaced) precisely because the doctrine had
//! no resume leg to follow.
//!
//! These pins assert the leg exists and sits where the spec requires:
//! inside §2 step 2's polling paragraph, directly after "Exit of the pid =
//! child done; then review." and BEFORE the hand-rolled-nohup
//! tool-failure fallback — the resume leg supplements that fallback, never
//! replaces it. No production code changes; this file is doctrine-only.
//!
//! T48 doctrine: every pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test binaries
//! with cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.
//!
//! T107 doctrine: the same step-2 template now also carries the
//! worktree-discipline commitment clause (the cycle-58 INCIDENT — the T104
//! impl child cd'd from its worktree to the main checkout for read-only
//! checks and ran `git add` + `git commit` THERE, "here" having resolved to
//! its CURRENT directory once it cd'd out). Legs (e)–(g) pin the clause's
//! two check: needles exactly-once inside step 2's window (the T64
//! loose-heading scope-leg pattern — the clause sits in the launch block,
//! BEFORE the polling paragraph legs (b)/(d) anchor on), extending — never
//! replacing — the pre-existing `Commit your work here.` sentence, with the
//! template's untouched sentences byte-identical.
//!
//! T110 doctrine: step 2's `(80, not 65: ...)` parenthetical carried a
//! written measure clause (T102) — "if >1 of the next 6 impl children
//! still dies at 80/80 with the work done, the next eval considers a
//! spec-size cap instead of further iteration raises." The census TRIPPED
//! in cycle 59: 2 of the last 4 impl children died 80/80 (t108-impl
//! mid-impl at 141 total iterations, t108-fixup post-commit), both on the
//! one row the cycle-59 eval sized at ~700–900 lines — a sizing that lived
//! in EVALUATION.md prose, not in the spec, with no doctrine forcing a
//! split. Leg (h) pins the resolution now recorded in the parenthetical:
//! the remedy is the filing-time ~500-line estimate ceiling in
//! META-META-SPEC's spec quality bar, not further iteration raises.

/// The leg's signature phrase: "resume" + the one-attempt cap language in
/// one contiguous run. Must occur EXACTLY once in LOOP-SPEC.md.
const RESUME_CAP: &str = "ONE resume attempt per child";

/// Step 2's polling anchor — the leg must sit directly after this sentence.
const STEP2_ANCHOR: &str = "Exit of the pid = child done";

/// Step 3's heading — the leg must not cross the step boundary (step
/// numbering is unchanged; the leg folds INTO step 2).
const STEP3_HEADING: &str = "3. **Review.**";

/// The pre-existing hand-rolled-nohup fallback sentence, byte-identical
/// INCLUDING its wrapped line breaks (the two non-ASCII bytes are spelled
/// as escapes so an editor normalization cannot silently unpin them:
/// \u{a7} = section sign, \u{2014} = em dash). The resume leg is inserted
/// BEFORE it; these bytes must survive untouched.
const NOHUP_FALLBACK: &str = concat!(
    "If\n",
    "   `delegate` itself errors persistently (the tool, not the child),\n",
    "   META-SPEC \u{a7}4's hand-rolled nohup launch template remains the fallback\n",
    "   launch path \u{2014} note the fallback in your ledger."
);

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// (a) The leg's signature needle — "resume" + the one-attempt cap language
/// — occurs in LOOP-SPEC.md exactly once. Delete the leg and this goes red
/// (count 0); a duplicate statement of the cap elsewhere also goes red.
#[test]
fn resume_cap_needle_occurs_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        RESUME_CAP.contains("ONE")
            && RESUME_CAP.contains("resume")
            && RESUME_CAP.contains("attempt"),
        "the needle must carry both the one-attempt cap language and \"resume\""
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(RESUME_CAP).count(),
        1,
        "LOOP-SPEC must state the ONE-resume-attempt cap exactly once — \
         zero means the recovery leg was deleted, more than one means it is \
         stated twice"
    );
}

/// (b) The leg sits INSIDE step 2: after the polling anchor, before the
/// hand-rolled-nohup fallback sentences, and before step 3's heading —
/// step numbering unchanged, the fallback supplemented not replaced.
#[test]
fn recovery_leg_sits_inside_step_2_before_the_nohup_fallback() {
    let spec = loop_spec();
    let anchor = spec
        .find(STEP2_ANCHOR)
        .expect("step-2 polling anchor (\"Exit of the pid = child done\") present");
    let leg = spec
        .find(RESUME_CAP)
        .expect("the recovery leg's cap needle present (leg deleted?)");
    let fallback = spec
        .find(NOHUP_FALLBACK)
        .expect("the hand-rolled-nohup fallback sentence present (pin (c) covers its bytes)");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading (\"3. **Review.**\") present");
    assert!(
        anchor < leg && leg < fallback && fallback < step3,
        "the recovery leg must sit inside step 2's polling paragraph, after \
         \"Exit of the pid = child done\" and BEFORE the nohup fallback and \
         step 3's heading (anchor {anchor}, leg {leg}, fallback {fallback}, \
         step3 {step3})"
    );
}

/// (c) The pre-existing nohup-fallback sentence survives BYTE-IDENTICAL
/// (its wrapped line breaks included) — the resume leg supplements the
/// tool-failure fallback, never replaces or rewraps it.
#[test]
fn nohup_fallback_sentence_survives_byte_identical() {
    let spec = loop_spec();
    assert_eq!(
        spec.matches(NOHUP_FALLBACK).count(),
        1,
        "the hand-rolled-nohup fallback sentence must survive byte-identical \
         (wrapping included) exactly once — the resume leg supplements it, \
         never replaces it"
    );
}

/// (d) The leg names its mechanics and its scope guards, inside step 2:
/// the `resume: true` relaunch shape (same worktree / spec / goal / model /
/// budgets), why it works (T19 — the worktree is never removed pre-harvest
/// so the untracked `.chug/` transcript persists; T58 — status reads the
/// LATEST run segment), and the fix-up-children exclusion (step 4's FAIL
/// arc starts fresh by design).
#[test]
fn leg_names_mechanics_and_scope_guards_inside_step_2() {
    let spec = loop_spec();
    let anchor = spec
        .find(STEP2_ANCHOR)
        .expect("step-2 polling anchor present");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading present");
    let window = &spec[anchor..step3];
    for needle in [
        "`resume: true`",
        "SAME worktree",
        "re-carries the T47",
        "80/35 impl, 50/30 validate",
        "continues the child's",
        "never removed pre-harvest (T19)",
        "LATEST run segment (T58)",
        "start fresh by design",
    ] {
        assert!(
            window.contains(needle),
            "the step-2 recovery leg must name {needle:?} — the leg names its \
             mechanics (same worktree/spec/goal/model/budgets; T19 transcript \
             persistence; T58 latest-segment status) and its scope guards \
             (fix-up children start fresh by design)"
        );
    }
}

// ---- T107 — the worktree-discipline commitment clause ----
//
// The step-2 goal template's `Commit your work here.` was ambiguous once a
// child cd'd out of its worktree (the cycle-58 INCIDENT). The template now
// carries an explicit clause re-anchoring "here" to the worktree cwd the
// child was LAUNCHED in. These legs assert the clause exists, sits inside
// step 2's launch block extending (never replacing) the pre-existing
// commitment sentence, and that the template's untouched sentences survive
// byte-identical.

/// The clause's commitment needle — in one contiguous run, LOWERCASE
/// `commit` exactly as the spec's check: greps it. The check: line is grep
/// (line-wise, case-sensitive), so the goal template must be wrapped so
/// the needle never crosses a line break. Must occur EXACTLY once in
/// LOOP-SPEC.md.
const WORKTREE_CWD_COMMIT: &str = "commit ONLY from your worktree cwd";

/// The clause's enforcement needle — the main-repo-cwd prohibition in one
/// contiguous run, verbatim per the spec's check:. Must occur EXACTLY once.
const MAIN_REPO_CWD_BAN: &str =
    "never run git add or git commit with the main repo as cwd";

/// The pre-existing commitment sentence the clause EXTENDS, never replaces
/// (a replacement mutant that drops it dies here).
const COMMIT_HERE: &str = "Commit your work here.";

/// Step 2's heading, matched LOOSELY — number + bold marker only (the T64
/// heading-scope pattern; unique in the file today). The clause lives in
/// step 2's delegate-launch block, BEFORE the polling paragraph the legs
/// above anchor on, so the clause's scope leg must window from the step
/// heading itself.
const STEP2_HEADING_LOOSE: &str = "2. **";

/// The goal template's T47 export prefix, byte-identical INCLUDING its
/// wrapped line breaks (the em dash is spelled as an escape so an editor
/// normalization cannot silently unpin it: \u{2014} = em dash). The
/// discipline clause is inserted AFTER this prefix; these bytes must
/// survive untouched.
const T47_EXPORT_PREFIX: &str = concat!(
    "Implement TODO item t<N> ONLY. export\n",
    "             CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared before\n",
    "             every cargo command (T47 shared build cache \u{2014} delegate has no env\n",
    "             parameter, so the goal carries the export)."
);

/// The goal template's closing DO-NOT sentence, byte-identical INCLUDING
/// its wrapped line break and the goal string's closing quote — the clause
/// sits BEFORE it, so these bytes must survive untouched and still close
/// the template.
const DO_NOT_TOUCH_SENTENCE: &str = concat!(
    "DO NOT touch TODO.md\n",
    "             or LEDGER.md \u{2014} bookkeeping is the orchestrator's.\""
);

/// (e) T107 — the clause's two check: needles and the pre-existing
/// commitment sentence each occur EXACTLY once in LOOP-SPEC.md. Delete the
/// inserted clause and both clause needles go red (count 0, which also
/// breaks the spec check's line-wise grep); a replacement mutant that swaps
/// `Commit your work here.` for the clause dies on the third count.
#[test]
fn worktree_discipline_needles_occur_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        WORKTREE_CWD_COMMIT.starts_with("commit ONLY")
            && WORKTREE_CWD_COMMIT.ends_with("worktree cwd"),
        "the worktree-cwd needle must carry the clause's commitment language \
         verbatim (lowercase `commit`, per the spec's case-sensitive check:)"
    );
    assert!(
        MAIN_REPO_CWD_BAN.contains("git add") && MAIN_REPO_CWD_BAN.contains("main repo as cwd"),
        "the main-repo-cwd needle must carry the prohibition language verbatim"
    );
    assert!(
        COMMIT_HERE.starts_with("Commit your work") && COMMIT_HERE.ends_with("here."),
        "the commitment needle must be the pre-existing sentence, period included"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (WORKTREE_CWD_COMMIT, "the worktree-cwd commitment clause"),
        (MAIN_REPO_CWD_BAN, "the main-repo-cwd prohibition"),
        (COMMIT_HERE, "the pre-existing `Commit your work here.` sentence"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means it was \
             deleted (or rewrapped across a line break, which also breaks \
             the spec check's line-wise grep), more than one means it is \
             stated twice"
        );
    }
}

/// (f) T107 — the clause sits INSIDE step 2 (the T64 loose-heading window:
/// step 2's heading through step 3's heading) and AFTER the sentence it
/// extends: `Commit your work here.` first, then the worktree-cwd
/// commitment, then the main-repo-cwd prohibition — the clause supplements
/// the commitment sentence, never replaces or precedes it.
#[test]
fn worktree_discipline_clause_sits_inside_step_2_after_commit_here() {
    let spec = loop_spec();
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let here = window.find(COMMIT_HERE).unwrap_or_else(|| {
        panic!("step-2 window must carry {COMMIT_HERE:?} (clause replaced it?)")
    });
    let worktree = window.find(WORKTREE_CWD_COMMIT).unwrap_or_else(|| {
        panic!("step-2 window must carry {WORKTREE_CWD_COMMIT:?} (clause deleted?)")
    });
    let ban = window.find(MAIN_REPO_CWD_BAN).unwrap_or_else(|| {
        panic!("step-2 window must carry {MAIN_REPO_CWD_BAN:?} (clause deleted?)")
    });
    assert!(
        here < worktree && worktree < ban,
        "the clause must EXTEND the commitment sentence — `Commit your work \
         here.` first ({here}), then the worktree-cwd commitment ({worktree}), \
         then the main-repo-cwd prohibition ({ban})"
    );
}

/// (g) T107 — the goal template's untouched sentences survive BYTE-IDENTICAL
/// (wrapped line breaks and em dashes included): the T47 export prefix the
/// clause is inserted after, and the DO-NOT sentence the clause is inserted
/// before (still the template's last sentence, goal string's closing quote
/// included) — req 2's byte-identity made load-bearing.
#[test]
fn goal_template_untouched_sentences_survive_byte_identical() {
    let spec = loop_spec();
    assert_eq!(
        spec.matches(T47_EXPORT_PREFIX).count(),
        1,
        "the goal template's T47 export prefix must survive byte-identical \
         (wrapping included) exactly once — the discipline clause is \
         inserted after it, never rewraps it"
    );
    assert_eq!(
        spec.matches(DO_NOT_TOUCH_SENTENCE).count(),
        1,
        "the goal template's DO-NOT sentence must survive byte-identical \
         (wrapping and the goal string's closing quote included) exactly \
         once — the discipline clause is inserted before it, never after it"
    );
}

// ---- T110 — the measure clause's resolution: the filing-time ceiling ----
//
// The `(80, not 65: ...)` parenthetical's Measure sentence (T102) named the
// remedy it would consider if the census tripped; the census DID trip in
// cycle 59, and the parenthetical now carries the resolution — the remedy
// chosen is the filing-time ~500-line estimate ceiling in META-META-SPEC's
// spec quality bar, NOT further iteration raises. These bytes must survive
// so an orchestrator reading step 2 sees both the history and how it
// resolved.

/// The resolution's needle — the remedy phrase the spec check greps, in one
/// contiguous run (the check's grep is line-wise, so the LOOP-SPEC text
/// must keep it on a single line). Must occur EXACTLY once in LOOP-SPEC.md.
const ESTIMATE_CEILING: &str = "filing-time ~500-line estimate ceiling";

/// The Measure sentence the resolution FOLLOWS — the parenthetical keeps
/// its history and gains the resolution, never replaces it.
const MEASURE_CENSUS: &str = "Measure: if >1 of";

/// (h) T110 — the resolution needle occurs EXACTLY once in LOOP-SPEC.md,
/// inside step 2's window (the T64 loose-heading scope pattern) and AFTER
/// the `Measure: if >1 of` census sentence it resolves. Delete the
/// resolution and this goes red at count 0 (the RED leg recorded in the
/// T110 commit); a duplicate statement of the remedy elsewhere also goes
/// red; moving it out of step 2 dies on the window find.
#[test]
fn measure_clause_resolution_needle_exactly_once_inside_step_2() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        ESTIMATE_CEILING.starts_with("filing-time")
            && ESTIMATE_CEILING.contains("~500-line")
            && ESTIMATE_CEILING.ends_with("estimate ceiling"),
        "the needle must carry the filing-time ~500-line estimate ceiling \
         language verbatim (per the spec's case-sensitive, line-wise check:)"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(ESTIMATE_CEILING).count(),
        1,
        "LOOP-SPEC must state the filing-time ~500-line estimate ceiling \
         exactly once — zero means the measure clause's resolution was \
         deleted (or rewrapped across a line break, which also breaks the \
         spec check's line-wise grep), more than one means it is stated \
         twice"
    );
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let measure = window
        .find(MEASURE_CENSUS)
        .expect("step-2 window must carry the `Measure: if >1 of` census sentence");
    let resolution = window
        .find(ESTIMATE_CEILING)
        .expect("step-2 window must carry the resolution needle (deleted or moved out of step 2?)");
    assert!(
        measure < resolution,
        "the resolution must FOLLOW the measure clause it resolves — \
         `Measure: if >1 of` first ({measure}), then the filing-time \
         ~500-line estimate ceiling remedy ({resolution})"
    );
}

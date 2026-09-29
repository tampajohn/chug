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
//!
//! T114 doctrine: META-META-SPEC's spec quality bar covered the ADD side
//! (T96: "broad enough to run every test the change adds") but nothing
//! about the tests a change can BREAK without adding — the `tests/`
//! integration pins over files the change touches. The T111 arc is the
//! evidence: a spec's own `check: cargo test --bin chug` ran the bin unit
//! tests only, so its goal gate never executed the `tests/readme_layout.rs`
//! pin (T95) the spec's README edit broke — the REAL RED was caught by the
//! ORCHESTRATOR's review gates instead. The bar now carries the BREAK-side
//! rule, and legs (i)–(j) pin it: both check: needles exactly-once, inside
//! the spec-quality-bar window (the T64 loose-heading scope pattern over
//! the "## Extend `TODO.md`" section), AFTER the ADD-side sentence it
//! extends and BEFORE the estimate-ceiling sentence — ordering pinned so
//! the rule reads as an extension of the T96 bar, never a replacement.
//!
//! T120 doctrine: cycle-61 produced two orchestrator SELF-inflicted
//! incidents, both discipline gaps LOOP-SPEC never wrote down. (1) The
//! healthy-validator SIGKILL — a RENDER-ONLY garble in the orchestrator's
//! own console view (a duplicated-tail rendering artifact in the delegate
//! status output) was read as a corrupt launch goal, and the kill landed
//! in the SAME breath as the check, before any read-back; transcript
//! read-back afterwards proved the 2166-byte payload intact, and a full
//! validator spin-up burned on the relaunch. (2) The anchor-typo silent
//! no-op — a `sed -i` bookkeeping edit with a typo'd anchor exited 0
//! changing nothing, because sed never fails on no-match (`edit_file`
//! does). The lessons lived only in EVALUATION.md; orchestrators read
//! LOOP-SPEC every cycle. Legs (k)–(l) pin the two rules where they are
//! read: the kill rule inside step 2 (beside the poll-posture and
//! budget-death-recovery paragraphs) and the sed-assertion rule inside
//! step 5 (beside the TODO.md bookkeeping-edit discipline), both
//! exactly-once + windowed in the T64/T114 pattern.
//!
//! T125 doctrine: the T110 estimate ceiling is only as good as the
//! estimate under it, and the era's landed actuals show estimates
//! undershooting SYSTEMATICALLY on feature rows, where test + doc density
//! multiplies the src diff (T113 ~455→583, T115 ~130→398, T116 ~30→116,
//! T117 ~280→603 — the last died mid-impl at 80/80, uncommitted, costing
//! a T63 resume; its TRUE size was over the ~500 ceiling the ~280
//! estimate claimed to be under). The failure mode is estimate ERROR, not
//! ceiling value — a ~500 ceiling cannot catch a row filed at "~280" that
//! lands at 603. META-META-SPEC's bar now carries the calibration rule
//! beside the ceiling sentence it qualifies: estimates count ALL changed
//! lines (src + tests + docs) at the observed ~1.5–3x test/doc density, a
//! novel-logic row whose all-in estimate exceeds ~400 SHOULD split at
//! filing time (the hard ~500 ceiling and the byte-identical move-row
//! exemption are unchanged), and each evaluation re-calibrates from the
//! Outcomes records instead of editing the threshold in passing. Legs
//! (m)–(n) pin it in the T64/T114 pattern: the three load-bearing tokens
//! exactly-once in META-META-SPEC.md, inside the spec-quality-bar window,
//! after the estimate-ceiling sentence and before the Priority doctrine
//! sentence — the same paragraph region evaluators read top-to-bottom at
//! filing time.
//!
//! T126 doctrine: META-SPEC §6's validator goal template taught the
//! cross-tree READ rule but said nothing about WRITES — validators kept
//! discovering the write half live. The cycle-61/62 streams carry TEN
//! `path escapes cwd … cross-tree paths go through bash` tool errors
//! across three validators, all the same shape: writing the mutation
//! helper script to /tmp with write_file/edit_file (t112-validate ×7,
//! t113-validate ×2, t115-validate ×1). T79's parallel-mutant legs
//! (routine since cycle 61) multiply exactly this write — one apply/run
//! script pair per leg, cap 3 — so the class is growing, not shrinking,
//! and each fire costs a validator iteration. The template now carries the
//! write half: /tmp helper scripts (the mutant apply/run legs) go through
//! bash heredocs, because write_file and edit_file are cwd-confined the
//! same way. Leg (o) pins it — this file's FIRST META-SPEC needle, with
//! the loader and pattern taken from `meta_meta_spec()`: both
//! load-bearing tokens exactly-once in META-SPEC.md, inside §6's
//! Validate-step window, the write sentence immediately after the
//! byte-identical cross-tree-READ sentence it extends. LOOP-SPEC's
//! §6-override paragraph is launch mechanics only and is NOT edited; §6's
//! budgets, model, verdict shape, and T79 parallel-mutant mandate stay
//! byte-identical.
//!
//! T155 doctrine: T149 (8699a70) raised the validator-child budgets
//! 50/30 → 60/40 on three surfaces and pinned only the bare numbers — the
//! step-2 T63-resume echo's "80/35 impl, 60/40 validate" (leg (d) above).
//! Its kimi validator PASSed with two non-blocking unpinned-text
//! observations (verdict d1790688874-9): text-revert mutants on (a)
//! LOOP-SPEC step 4's budget RATIONALE — the census sentence ("4 of the
//! last 4 validator children died at budget in cycles 66–69") and the
//! measure clause (">1 of the next 8 validator runs still dies at 60/40 …
//! trim default mutation-leg counts") — and (b) META-SPEC §6's
//! validator-template budget argv, stayed full-suite GREEN. The rationale
//! is the teeth that make the budgets self-governing (the census is the
//! evidence; the measure clause is the pre-committed remedy), and §6's
//! argv is the template line every validator actually launches from — a
//! future edit can currently drop either with every gate green. Legs
//! (p)–(q) pin both texts in place, extending — never replacing — the
//! T149 numbers pin: the step-4 tokens are asserted in the SAME paragraph
//! region as step 4's own `max_iters: 60`, `max_minutes: 40` pair
//! (adjacency, not two disjoint file-wide greps — the T24 whole-file-grep
//! non-localizing observation), and the §6 argv is one contiguous
//! fragment on the kimi launch line. No doctrine text is edited by this
//! row: LOOP-SPEC.md and META-SPEC.md must stay byte-identical
//! before/after (any wording drift the pin exposes is reported, not
//! fixed).

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
        "80/35 impl, 60/40 validate",
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

// ---- T114 — the spec-quality bar's BREAK-side rule ----
//
// The T96 ADD-side rule ("broad enough to run every test the change adds")
// says nothing about the tests a change can BREAK without adding — the
// `tests/` integration pins over the files the change touches. The T111
// arc is the escape: a spec's own `check: cargo test --bin chug` ran the
// bin unit tests only, so its goal gate never executed the
// `tests/readme_layout.rs` pin (T95) the spec's README edit broke — the
// REAL RED was caught by the ORCHESTRATOR's review gates instead. The bar
// now carries the BREAK-side rule; these legs pin it in place, extending —
// never replacing — the T96 bar and the T110 estimate-ceiling sentence.

fn meta_meta_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading META-META-SPEC.md from the runtime checkout: {e}"))
}

/// The BREAK-side rule's commitment needle — the rule's core claim, in one
/// contiguous run, verbatim per the spec's check: grep (line-wise, so the
/// bar's wrapping must keep it on a single line). Must occur EXACTLY once
/// in META-META-SPEC.md.
const BREAK_NEEDLE: &str = "every test the change can BREAK";

/// The BREAK-side rule's mechanics needle — the bin-only blindness that
/// makes a README/doctrine-touching spec's `--bin chug` check blind to
/// exactly the pins most likely to break, in one contiguous run, verbatim
/// per the spec's check: grep. Must occur EXACTLY once in
/// META-META-SPEC.md.
const BIN_ONLY_NEEDLE: &str = "never the `tests/` integration binaries";

/// The ADD-side sentence (T96) the BREAK-side rule anchors AFTER — the
/// insertion extends it, never replaces or precedes it (req 2: this is an
/// insertion, not a rewrite).
const ADD_SIDE_RULE: &str = "every test the change adds";

/// The estimate-ceiling sentence (T110) the BREAK-side rule sits BEFORE —
/// the bar keeps its history: ADD-side, then BREAK-side, then the ceiling.
const ESTIMATE_SENTENCE: &str =
    "Every spec carries an `estimate: ~N changed lines` line";

/// The spec-quality-bar section's opening heading (loose, the T64 pattern)
/// and the next heading that closes the window — the rule must live inside
/// the "Extend `TODO.md`" section's spec-quality-bar paragraph, not drift
/// into another section of META-META-SPEC.
const EXTEND_TODO_HEADING: &str = "## Extend `TODO.md`";
const HANDOFF_HEADING: &str = "## Handoff section in EVALUATION.md";

/// (i) T114 — both BREAK-side check: needles occur EXACTLY once in
/// META-META-SPEC.md. Delete the rule and both go red (count 0, which also
/// breaks the spec's own line-wise grep); rewrap either needle across a
/// line break and it goes red the same way; a duplicate statement of
/// either needle elsewhere also goes red (count 2).
#[test]
fn break_side_rule_needles_occur_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        BREAK_NEEDLE.starts_with("every test")
            && BREAK_NEEDLE.contains("the change can")
            && BREAK_NEEDLE.ends_with("BREAK"),
        "the BREAK needle must carry the can-BREAK language verbatim \
         (per the spec's case-sensitive, line-wise check:)"
    );
    assert!(
        BIN_ONLY_NEEDLE.starts_with("never the `tests/`")
            && BIN_ONLY_NEEDLE.ends_with("integration binaries"),
        "the bin-only needle must carry the `tests/` integration-binaries \
         language verbatim, backticks included (per the spec's check:)"
    );
    let spec = meta_meta_spec();
    for (needle, what) in [
        (BREAK_NEEDLE, "the can-BREAK rule"),
        (
            BIN_ONLY_NEEDLE,
            "the bin-only-runs-no-`tests/`-binaries clause",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-META-SPEC must state {what} exactly once — zero means the \
             BREAK-side rule was deleted (or rewrapped across a line break, \
             which also breaks the spec's own line-wise grep), more than one \
             means it is stated twice"
        );
    }
}

/// (j) T114 — the insertion sits INSIDE the spec-quality-bar window (the
/// T64 loose-heading scope pattern: the "## Extend `TODO.md`" heading
/// through the next "## Handoff" heading), AFTER the ADD-side sentence it
/// extends (T96's `every test the change adds`) and BEFORE the
/// estimate-ceiling sentence (T110) — ordering pinned: the BREAK-side rule
/// reads as an extension of the T96 bar, never a replacement, a promotion
/// above the bar, or a move out of the section.
#[test]
fn break_side_rule_sits_after_add_side_rule_and_before_estimate_sentence() {
    let spec = meta_meta_spec();
    let start = spec
        .find(EXTEND_TODO_HEADING)
        .expect("the Extend-TODO heading present");
    let end = start
        + spec[start..]
            .find(HANDOFF_HEADING)
            .expect("the Handoff heading present after the Extend-TODO heading");
    let window = &spec[start..end];
    let add = window
        .find(ADD_SIDE_RULE)
        .expect("the spec-quality-bar window must carry the ADD-side rule (T96)");
    let break_rule = window
        .find(BREAK_NEEDLE)
        .expect("the spec-quality-bar window must carry the BREAK-side rule \
                 (deleted, or moved out of the Extend-TODO section?)");
    let estimate = window
        .find(ESTIMATE_SENTENCE)
        .expect("the spec-quality-bar window must carry the estimate-ceiling \
                 sentence (T110)");
    assert!(
        add < break_rule && break_rule < estimate,
        "the BREAK-side rule must sit INSIDE the spec-quality-bar window, \
         after the ADD-side rule it extends ({add}) and BEFORE the \
         estimate-ceiling sentence ({estimate}) — the rule was found at \
         offset {break_rule}"
    );
}

// ---- T120 — verify-then-kill SEQUENTIAL (step 2) + assert-after-sed (step 5) ----
//
// The two cycle-61 orchestrator self-inflicted incidents, now doctrine:
// the kill rule (verify-then-kill is SEQUENTIAL — read the payload back
// from the child's on-disk artifacts in a SEPARATE completed step, kill
// only on proof) lives in step 2 beside the poll-posture and
// budget-death-recovery paragraphs; the edit-assertion rule (sed exits 0
// on no-match — grep-verify the needle after any in-place bash edit)
// lives in step 5 beside the TODO.md bookkeeping-edit discipline. These
// legs pin both rules exactly-once + windowed, in the T64/T114 pattern.

/// The kill rule's commitment needle — verbatim per the spec's check:
/// grep (line-wise, so the LOOP-SPEC wrapping must keep it on a single
/// line). Must occur EXACTLY once in LOOP-SPEC.md.
const VERIFY_THEN_KILL_SEQUENTIAL: &str = "verify-then-kill is SEQUENTIAL";

/// The kill rule's enforcement needle — the same-breath prohibition,
/// verbatim per the spec's check: grep (capital N per the check's
/// case-sensitivity). Must occur EXACTLY once in LOOP-SPEC.md.
const NEVER_KILL_SAME_BREATH: &str = "Never kill in the same breath";

/// The sed rule's needle — the silent-success fact the assertion
/// discipline exists for, verbatim per the spec's check: grep. Must occur
/// EXACTLY once in LOOP-SPEC.md.
const SED_NO_MATCH: &str = "sed exits 0 on no-match";

/// Step 5's heading, matched loosely (the T64 pattern; unique in the file
/// today) and step 6's heading that closes the window — the sed rule must
/// live inside step 5's bookkeeping window, not drift into another step.
const STEP5_HEADING_LOOSE: &str = "5. **";
const STEP6_HEADING_LOOSE: &str = "6. **";

/// (k) T120 — both kill-rule needles occur EXACTLY once in LOOP-SPEC.md,
/// inside step 2's window (the T64 loose-heading scope pattern: step 2's
/// heading through step 3's heading), read-first before kill-after
/// (the SEQUENTIAL commitment needle precedes the same-breath prohibition,
/// mirroring the rule's own order). Delete the rule and both needles go
/// red (count 0, which also breaks the spec's own line-wise grep); a
/// duplicate statement elsewhere goes red (count 2); moving the rule out
/// of step 2 dies on the window find.
#[test]
fn kill_rule_needles_occur_exactly_once_inside_step_2() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        VERIFY_THEN_KILL_SEQUENTIAL.starts_with("verify-then-kill")
            && VERIFY_THEN_KILL_SEQUENTIAL.ends_with("SEQUENTIAL"),
        "the kill-rule needle must carry the verify-then-kill language \
         verbatim (per the spec's case-sensitive, line-wise check:)"
    );
    assert!(
        NEVER_KILL_SAME_BREATH.starts_with("Never kill")
            && NEVER_KILL_SAME_BREATH.ends_with("same breath"),
        "the same-breath needle must carry the prohibition language \
         verbatim (capital N, per the spec's check:)"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (
            VERIFY_THEN_KILL_SEQUENTIAL,
            "the verify-then-kill is SEQUENTIAL rule",
        ),
        (
            NEVER_KILL_SAME_BREATH,
            "the Never-kill-in-the-same-breath prohibition",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the kill \
             rule was deleted (or a needle was rewrapped across a line \
             break, which also breaks the spec's own line-wise grep), more \
             than one means it is stated twice"
        );
    }
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let sequential = window.find(VERIFY_THEN_KILL_SEQUENTIAL).unwrap_or_else(|| {
        panic!(
            "step-2 window must carry {VERIFY_THEN_KILL_SEQUENTIAL:?} \
             (kill rule deleted, or moved out of step 2?)"
        )
    });
    let same_breath = window.find(NEVER_KILL_SAME_BREATH).unwrap_or_else(|| {
        panic!(
            "step-2 window must carry {NEVER_KILL_SAME_BREATH:?} \
             (kill rule deleted, or moved out of step 2?)"
        )
    });
    assert!(
        sequential < same_breath,
        "the kill rule must read first, kill after — the verify-then-kill \
         is SEQUENTIAL commitment ({sequential}) precedes the Never kill \
         in the same breath prohibition ({same_breath})"
    );
}

/// (l) T120 — the sed needle occurs EXACTLY once in LOOP-SPEC.md, inside
/// step 5's window (the T64 loose-heading scope pattern: step 5's heading
/// through step 6's heading). Delete the rule and the needle goes red
/// (count 0, which also breaks the spec's own line-wise grep); a
/// duplicate statement elsewhere goes red (count 2); moving the rule out
/// of step 5 dies on the window find.
#[test]
fn sed_no_match_needle_occurs_exactly_once_inside_step_5() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        SED_NO_MATCH.starts_with("sed exits 0")
            && SED_NO_MATCH.ends_with("no-match"),
        "the sed needle must carry the no-match silent-success language \
         verbatim (per the spec's case-sensitive, line-wise check:)"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(SED_NO_MATCH).count(),
        1,
        "LOOP-SPEC must state the sed no-match silent-success fact exactly \
         once — zero means the edit-assertion rule was deleted (or the \
         needle was rewrapped across a line break, which also breaks the \
         spec's own line-wise grep), more than one means it is stated twice"
    );
    let start = spec
        .find(STEP5_HEADING_LOOSE)
        .expect("step-5 heading (`5. **`) present");
    let end = start
        + spec[start..]
            .find(STEP6_HEADING_LOOSE)
            .expect("step-6 heading present after step 5's");
    let window = &spec[start..end];
    assert!(
        window.contains(SED_NO_MATCH),
        "step 5's window must carry {SED_NO_MATCH:?} — the \
         edit-assertion rule was deleted, or moved out of step 5's \
         bookkeeping window"
    );
}

// ---- T125 — the estimate-calibration rule (the ~400 should-split band) ----
//
// The T110 ceiling is only as good as the estimate under it, and the era's
// landed actuals show estimates undershooting SYSTEMATICALLY on feature
// rows, where test + doc density multiplies the src diff — T117's ~280
// estimate landed at +603 and its impl child died mid-impl at 80/80, the
// named cost of undershoot. The bar now carries the calibration rule
// beside the ceiling sentence it qualifies; these legs pin it in the
// T64/T114 pattern: the three load-bearing tokens exactly-once in
// META-META-SPEC.md, inside the spec-quality-bar window, after the
// estimate-ceiling sentence and before the Priority doctrine sentence —
// the same paragraph region evaluators read top-to-bottom at filing time.

/// The should-split band's threshold needle — the all-in estimate above
/// which a novel-logic row SHOULD split at filing time even though the
/// hard ~500 ceiling is unchanged. Must occur EXACTLY once in
/// META-META-SPEC.md.
const SPLIT_BAND_NEEDLE: &str = "~400";

/// The evidence token — T117, the row whose ~280 estimate landed at +603
/// and whose impl child died mid-impl at 80/80 with the work uncommitted:
/// the named cost of undershoot. Must occur EXACTLY once in
/// META-META-SPEC.md.
const EVIDENCE_TOKEN_NEEDLE: &str = "T117";

/// The density claim's needle — the observed test/doc density range the
/// src diff multiplies by, in one contiguous run (the en dash is spelled
/// as an escape so an editor normalization cannot silently unpin it:
/// \u{2013} = en dash, matching the bar's existing `2–3 rows` dash).
/// Must occur EXACTLY once in META-META-SPEC.md.
const DENSITY_NEEDLE: &str = "1.5\u{2013}3x";

/// The Priority doctrine sentence that opens the bar's next claim — the
/// calibration rule must sit BEFORE it, i.e. in the same paragraph region
/// as the estimate-ceiling sentence, not a new section.
const PRIORITY_DOCTRINE: &str = "Priority doctrine:";

/// (m) T125 — the calibration rule's three load-bearing tokens occur
/// EXACTLY once each in META-META-SPEC.md. Delete the rule and all three
/// go red (count 0); a duplicate statement of any token elsewhere (or a
/// needle rewrapped across a line break) also goes red.
#[test]
fn estimate_calibration_tokens_occur_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        SPLIT_BAND_NEEDLE.starts_with('~') && SPLIT_BAND_NEEDLE.contains("400"),
        "the band needle must be the ~400 should-split threshold verbatim"
    );
    assert!(
        EVIDENCE_TOKEN_NEEDLE.starts_with('T') && EVIDENCE_TOKEN_NEEDLE.ends_with("117"),
        "the evidence needle must be the T117 token verbatim"
    );
    assert!(
        DENSITY_NEEDLE.starts_with("1.5") && DENSITY_NEEDLE.ends_with("3x"),
        "the density needle must be the 1.5–3x range verbatim, en dash included"
    );
    let spec = meta_meta_spec();
    for (needle, what) in [
        (SPLIT_BAND_NEEDLE, "the ~400 should-split band threshold"),
        (EVIDENCE_TOKEN_NEEDLE, "the T117 undershoot-cost evidence token"),
        (DENSITY_NEEDLE, "the ~1.5–3x test/doc density claim"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-META-SPEC must state {what} exactly once — zero means the \
             calibration rule was deleted (or the needle was rewrapped \
             across a line break), more than one means it is stated twice"
        );
    }
}

/// (n) T125 — the calibration rule sits INSIDE the spec-quality-bar window
/// (the T64 loose-heading scope pattern: the "## Extend `TODO.md`" heading
/// through the next "## Handoff" heading), AFTER the estimate-ceiling
/// sentence (T110) it qualifies and BEFORE the Priority doctrine sentence
/// that opens the bar's next claim — the same paragraph region, not a new
/// section. The rule reads top-to-bottom as the bar demands: the density
/// claim, then its evidence token, then the remedy band.
#[test]
fn estimate_calibration_rule_sits_after_ceiling_sentence_inside_quality_bar() {
    let spec = meta_meta_spec();
    let start = spec
        .find(EXTEND_TODO_HEADING)
        .expect("the Extend-TODO heading present");
    let end = start
        + spec[start..]
            .find(HANDOFF_HEADING)
            .expect("the Handoff heading present after the Extend-TODO heading");
    let window = &spec[start..end];
    let ceiling = window
        .find(ESTIMATE_SENTENCE)
        .expect("the spec-quality-bar window must carry the estimate-ceiling \
                 sentence (T110)");
    let density = window.find(DENSITY_NEEDLE).unwrap_or_else(|| {
        panic!(
            "the spec-quality-bar window must carry {DENSITY_NEEDLE:?} \
             (density claim deleted, or moved out of the quality bar?)"
        )
    });
    let evidence = window.find(EVIDENCE_TOKEN_NEEDLE).unwrap_or_else(|| {
        panic!(
            "the spec-quality-bar window must carry {EVIDENCE_TOKEN_NEEDLE:?} \
             (evidence token deleted, or moved out of the quality bar?)"
        )
    });
    let band = window.find(SPLIT_BAND_NEEDLE).unwrap_or_else(|| {
        panic!(
            "the spec-quality-bar window must carry {SPLIT_BAND_NEEDLE:?} \
             (should-split band deleted, or moved out of the quality bar?)"
        )
    });
    let priority = window
        .find(PRIORITY_DOCTRINE)
        .expect("the spec-quality-bar window must carry the Priority doctrine \
                 sentence (bar reordered?)");
    assert!(
        ceiling < density && density < evidence && evidence < band && band < priority,
        "the calibration rule must sit in the same paragraph region as the \
         estimate ceiling, after it and before the Priority doctrine \
         sentence — ceiling ({ceiling}), density claim ({density}), \
         evidence token ({evidence}), remedy band ({band}), priority \
         doctrine ({priority})"
    );
}

// ---- T126 — §6's validator template gains the cross-tree WRITE rule ----
//
// META-SPEC §6's validator goal template taught the cross-tree READ rule
// ("Read worktree files via bash — ... refuse cross-tree paths with `path
// escapes cwd`; cross-tree reads go through bash") but said nothing about
// WRITES. Validators kept discovering the write half live: the cycle-61/62
// streams carry TEN `path escapes cwd ... cross-tree paths go through
// bash` tool errors across three validators, all the same shape — writing
// the mutation helper script to /tmp with write_file/edit_file (t112 ×7,
// t113 ×2, t115 ×1). T79's parallel-mutant legs (routine since cycle 61)
// multiply exactly this write — one apply/run script pair per leg, cap 3 —
// so the class is growing, not shrinking. The remedy is one sentence in
// the template every validator already reads: /tmp helper scripts go
// through bash heredocs, because write_file and edit_file are cwd-confined
// the same way. This is the file's FIRST META-SPEC pin (the loader below
// is the `meta_meta_spec()` pattern); LOOP-SPEC's §6-override paragraph is
// launch mechanics only and is NOT edited — the template text lives in
// META-SPEC.

/// The cross-tree READ sentence the write rule extends, byte-identical
/// INCLUDING its wrapped line breaks (the non-ASCII byte is spelled as an
/// escape so an editor normalization cannot silently unpin it:
/// \u{2014} = em dash). The write sentence is inserted immediately after
/// it; these bytes must survive untouched (req: the existing read sentence
/// stays byte-identical).
const CROSS_TREE_READ_SENTENCE: &str = concat!(
    "Read worktree files via bash \u{2014}\n",
    "             read_file/grep/glob/list_dir/edit_file are cwd-confined and\n",
    "             refuse cross-tree paths with `path escapes cwd`; cross-tree\n",
    "             reads go through bash."
);

/// The write rule's verb needle — the /tmp helper-scripts clause as
/// written. Must occur EXACTLY once in META-SPEC.md.
const TMP_HELPER_SCRIPTS: &str = "Write /tmp helper scripts";

/// The write rule's mechanism needle — the bash heredoc remedy, the tool
/// pair's working alternative for cross-tree writes. Must occur EXACTLY
/// once in META-SPEC.md.
const HEREDOC_NEEDLE: &str = "heredoc";

/// The sentence that FOLLOWS the insertion point in §6's goal text — the
/// write sentence must sit BEFORE it (the insertion is additive; the read
/// sentence's successor must not be displaced or reordered).
const RUN_GATES_SENTENCE: &str = "Run cargo build + clippy";

/// §6's Validate step opens the window and step 7 closes it — the write
/// rule must live inside the validator goal template (the T64
/// loose-heading scope pattern), not drift into another step of META-SPEC.
const STEP6_VALIDATE_HEADING: &str = "6. **Validate";
const STEP7_HEADING: &str = "7. **";

/// META-SPEC.md loader — the `meta_meta_spec()` pattern, reading the file
/// from the checkout the binary RUNS against (`std::env::current_dir()`;
/// cargo runs test binaries with cwd = the package root), never via the
/// compile-time manifest-dir macro (the T48 doctrine).
fn meta_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("META-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading META-SPEC.md from the runtime checkout: {e}"))
}

/// (o) T126 — the §6 validator template's cross-tree WRITE rule: both
/// load-bearing tokens occur EXACTLY once in META-SPEC.md, inside the
/// Validate-step window, and the write sentence sits IMMEDIATELY after the
/// byte-identical cross-tree READ sentence it extends (whitespace-only
/// gap, before the Run-cargo-gates sentence). Delete the write sentence
/// and both needles go red (count 0, which also breaks the pin's own
/// reads); a duplicate statement elsewhere goes red (count 2); moving the
/// rule out of §6 dies on the window find; rewriting the read sentence the
/// rule extends dies on the byte-identical guard.
#[test]
fn cross_tree_write_rule_needles_occur_exactly_once_after_read_sentence() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert_eq!(
        HEREDOC_NEEDLE, "heredoc",
        "the mechanism needle must be the bare heredoc token verbatim"
    );
    assert!(
        TMP_HELPER_SCRIPTS.starts_with("Write /tmp helper")
            && TMP_HELPER_SCRIPTS.ends_with("scripts"),
        "the /tmp helper-scripts needle must carry the write-verb clause \
         verbatim"
    );
    assert!(
        RUN_GATES_SENTENCE.starts_with("Run cargo build"),
        "the successor anchor must be the Run-cargo-gates sentence"
    );
    let spec = meta_spec();
    for (needle, what) in [
        (TMP_HELPER_SCRIPTS, "the write-/tmp-helper-scripts clause"),
        (HEREDOC_NEEDLE, "the bash-heredoc remedy"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-SPEC must state {what} exactly once — zero means the \
             write rule was deleted (or the needle was rewrapped across a \
             line break), more than one means it is stated twice"
        );
    }
    // The read sentence the write rule extends must be byte-identical —
    // the insertion is additive, never a rewrite.
    let read_pos = spec.find(CROSS_TREE_READ_SENTENCE).unwrap_or_else(|| {
        panic!(
            "the cross-tree READ sentence must survive byte-identical — the \
             write rule EXTENDS it, never rewrites it"
        )
    });
    let read_end = read_pos + CROSS_TREE_READ_SENTENCE.len();
    // Window: §6's Validate step through step 7 — the rule must live in
    // the validator goal template (T64 loose-heading scope pattern).
    let start = spec
        .find(STEP6_VALIDATE_HEADING)
        .expect("step-6 Validate heading present");
    let end = start
        + spec[start..]
            .find(STEP7_HEADING)
            .expect("step-7 heading present after step 6's");
    let write = start
        + spec[start..end]
            .find(TMP_HELPER_SCRIPTS)
            .unwrap_or_else(|| {
                panic!(
                    "the Validate-step window must carry \
                     {TMP_HELPER_SCRIPTS:?} (write rule deleted, or moved \
                     out of §6?)"
                )
            });
    let heredoc = start
        + spec[start..end].find(HEREDOC_NEEDLE).unwrap_or_else(|| {
            panic!(
                "the Validate-step window must carry {HEREDOC_NEEDLE:?} \
                 (heredoc remedy deleted, or moved out of §6?)"
            )
        });
    let run_gates = start
        + spec[start..end]
            .find(RUN_GATES_SENTENCE)
            .expect("the Validate-step window must carry the Run-cargo-gates \
                     sentence (§6 template reordered?)");
    assert!(
        write < heredoc && heredoc < run_gates,
        "the write rule must read as one sentence — the /tmp helper-scripts \
         clause ({write}) before its heredoc remedy ({heredoc}) — and sit \
         before the Run-cargo-gates sentence ({run_gates})"
    );
    // Adjacency: the read sentence lives in the same window, and the write
    // sentence IMMEDIATELY follows it (whitespace-only gap) — the write
    // rule extends the read rule in the same breath, where the validator
    // reads it, not a bolt-on elsewhere in the template.
    assert!(
        read_pos >= start && read_end <= end && read_end < write,
        "the read sentence must sit inside the Validate-step window, before \
         the write rule (read {read_pos}..{read_end}, write {write}, window \
         {start}..{end})"
    );
    let between = &spec[read_end..write];
    assert!(
        between.chars().all(char::is_whitespace),
        "the write sentence must sit IMMEDIATELY after the cross-tree read \
         sentence (whitespace-only gap) — found in between: {:?}",
        between.trim()
    );
}

// ---- T155 — the T149 unpinned-text gaps: step-4 rationale + §6 argv ----
//
// T149's kimi verdict (d1790688874-9) carried two non-blocking
// unpinned-text observations: text-revert mutants on (a) LOOP-SPEC step
// 4's budget RATIONALE and (b) META-SPEC §6's validator-template budget
// argv stayed full-suite GREEN, because only the bare numbers were pinned
// (leg (d), the step-2 T63-resume echo). These legs pin both texts.

/// Step 4's budget numbers — the pair the rationale hangs on, contiguous
/// as written (backticks, comma-space). Leg (d) pins the step-2
/// T63-resume echo's budgets ("80/35 impl, 60/40 validate"); THIS needle
/// pins step 4's own launch bytes as the adjacency anchor for the
/// census/measure tokens — the first step-4 numbers pin, not a duplicate
/// of leg (d). Presence is asserted via the in-window find (below), not a
/// separate file-wide count leg — the numbers are already pinned once and
/// this row does not duplicate that pin.
const STEP4_BUDGET_NUMBERS: &str = "`max_iters: 60`, `max_minutes: 40`";

/// The census clause's stable core tokens — the evidence the 60/40 budgets
/// stand on (four budget deaths in four cycles, 66–69). Contiguous as
/// written; must occur EXACTLY once in LOOP-SPEC.md.
const STEP4_CENSUS_NEEDLE: &str = "4 of the last 4 validator children died at budget";

/// The measure clause's census-threshold token — the tripwire that keeps
/// the budgets self-governing. Contiguous as written; EXACTLY once.
const STEP4_MEASURE_THRESHOLD_NEEDLE: &str = ">1 of the next 8 validator runs";

/// The measure clause's remedy token — trim default mutation-leg counts.
/// Byte-identical INCLUDING its wrapped line break (the file wraps between
/// "mutation-leg" and "counts"; the concat! spells the wrap so a rewrap
/// cannot silently unpin it — the T47_EXPORT_PREFIX idiom).
const STEP4_MEASURE_REMEDY_NEEDLE: &str = concat!(
    "trimming default mutation-leg\n",
    "   counts"
);

/// Step 4's heading, matched loosely (the T64 pattern; unique in the file
/// today) — the rationale must live inside step 4's window, closed by the
/// pre-existing STEP5_HEADING_LOOSE.
const STEP4_HEADING_LOOSE: &str = "4. **";

/// (p) T155 — step 4's budget RATIONALE survives beside the numbers it
/// rationalizes: the census clause and both measure-clause tokens occur
/// EXACTLY once each in LOOP-SPEC.md, inside step 4's window (the T64
/// loose-heading scope pattern), AFTER the `max_iters: 60`,
/// `max_minutes: 40` numbers pair — the same paragraph region (the budget
/// parenthetical), not a disjoint section (the T24 non-localizing
/// observation). Delete the rationale text — T149's observed text-revert
/// mutant — and every needle goes red at count 0 / window-miss; move the
/// rationale out of step 4 and it dies on the window find; reorder
/// census/measure and it dies on the ordering assert. Leg (d)'s step-2
/// echo pin is untouched and stays green under this row's mutants — that
/// co-green gap is exactly what T149's verdict flagged.
#[test]
fn step4_budget_rationale_tokens_occur_once_beside_the_budget_numbers() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        STEP4_BUDGET_NUMBERS.starts_with("`max_iters: 60`")
            && STEP4_BUDGET_NUMBERS.ends_with("`max_minutes: 40`"),
        "the numbers needle must be step 4's contiguous 60/40 pair verbatim"
    );
    assert!(
        STEP4_CENSUS_NEEDLE.starts_with("4 of the last 4")
            && STEP4_CENSUS_NEEDLE.ends_with("died at budget"),
        "the census needle must carry the four-deaths evidence verbatim"
    );
    assert!(
        STEP4_MEASURE_THRESHOLD_NEEDLE.starts_with(">1 of the next 8")
            && STEP4_MEASURE_THRESHOLD_NEEDLE.ends_with("validator runs"),
        "the measure threshold needle must carry the >1-of-8 tripwire verbatim"
    );
    assert!(
        STEP4_MEASURE_REMEDY_NEEDLE.starts_with("trimming default")
            && STEP4_MEASURE_REMEDY_NEEDLE.ends_with("counts"),
        "the measure remedy needle must carry the mutation-leg-counts \
         remedy verbatim, wrapped line break included"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (STEP4_CENSUS_NEEDLE, "the step-4 budget census clause"),
        (
            STEP4_MEASURE_THRESHOLD_NEEDLE,
            "the measure clause's >1-of-8 tripwire",
        ),
        (
            STEP4_MEASURE_REMEDY_NEEDLE,
            "the measure clause's trim-mutation-leg-counts remedy",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             budget rationale was deleted (or a needle was rewrapped \
             across a line break), more than one means it is stated twice"
        );
    }
    // Window: step 4 through step 5 (the T64 loose-heading scope
    // pattern) — the rationale must live in step 4's validation
    // paragraph, not drift into another step.
    let start = spec
        .find(STEP4_HEADING_LOOSE)
        .expect("step-4 heading (`4. **`) present");
    let end = start
        + spec[start..]
            .find(STEP5_HEADING_LOOSE)
            .expect("step-5 heading present after step 4's");
    let window = &spec[start..end];
    let numbers = window.find(STEP4_BUDGET_NUMBERS).unwrap_or_else(|| {
        panic!(
            "step-4 window must carry {STEP4_BUDGET_NUMBERS:?} — the 60/40 \
             numbers pair was deleted or moved out of step 4 (leg (d)'s \
             step-2 echo pin is unaffected either way)"
        )
    });
    let census = window.find(STEP4_CENSUS_NEEDLE).unwrap_or_else(|| {
        panic!(
            "step-4 window must carry {STEP4_CENSUS_NEEDLE:?} — the census \
             clause was deleted, or moved out of step 4"
        )
    });
    let threshold = window
        .find(STEP4_MEASURE_THRESHOLD_NEEDLE)
        .unwrap_or_else(|| {
            panic!(
                "step-4 window must carry {STEP4_MEASURE_THRESHOLD_NEEDLE:?} \
                 — the measure tripwire was deleted, or moved out of step 4"
            )
        });
    let remedy = window.find(STEP4_MEASURE_REMEDY_NEEDLE).unwrap_or_else(|| {
        panic!(
            "step-4 window must carry {STEP4_MEASURE_REMEDY_NEEDLE:?} \
             (wrapped line break included) — the measure remedy was \
             deleted, rewrapped, or moved out of step 4"
        )
    });
    assert!(
        numbers < census && census < threshold && threshold < remedy,
        "the rationale must sit in the SAME paragraph region as the 60/40 \
         numbers, reading in the written order — numbers ({numbers}), \
         census ({census}), measure tripwire ({threshold}), measure remedy \
         ({remedy})"
    );
}

/// (q) T155 — §6's validator launch template keeps its budget argv as ONE
/// contiguous fragment on the kimi line: `--max-iters 60 --max-minutes
/// 40`. T149 raised §6's template 40/30 → 60/40 in the same commit as the
/// LOOP-SPEC numbers, but nothing pinned the template's bytes — reverting
/// the argv to the pre-T149 `--max-iters 40 --max-minutes 30` stayed
/// full-suite GREEN. The fragment is asserted contiguously (one
/// substring, not two separate greps), exactly once in META-SPEC.md,
/// inside §6's Validate-step window (the T64 loose-heading pattern over
/// the existing STEP6_VALIDATE_HEADING / STEP7_HEADING bounds), on the
/// line that carries the kimi model id — the validator's launch line, not
/// some other template's.
const META_SPEC_VALIDATOR_ARGV: &str = "--max-iters 60 --max-minutes 40";

/// The kimi model id that must share the argv's line — the fragment pins
/// the VALIDATOR launch, so it must sit beside the kimi model flag.
const KIMI_MODEL_ID: &str = "anthropic-system.ai.kimi-k3";

/// (q) T155 — see [`META_SPEC_VALIDATOR_ARGV`].
#[test]
fn meta_spec_validator_template_budget_argv_fragment_exactly_once_in_step6() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        META_SPEC_VALIDATOR_ARGV.starts_with("--max-iters 60")
            && META_SPEC_VALIDATOR_ARGV.ends_with("--max-minutes 40"),
        "the argv needle must be the contiguous 60/40 fragment verbatim"
    );
    assert!(
        KIMI_MODEL_ID.starts_with("anthropic-system.ai.")
            && KIMI_MODEL_ID.ends_with("kimi-k3"),
        "the model-id anchor must be the kimi validator model verbatim"
    );
    let spec = meta_spec();
    assert_eq!(
        spec.matches(META_SPEC_VALIDATOR_ARGV).count(),
        1,
        "META-SPEC §6's validator launch template must state \
         {META_SPEC_VALIDATOR_ARGV:?} exactly once as one contiguous \
         fragment — zero means the argv was reverted, split, or rewrapped \
         (which also breaks the fragment's contiguity), more than one \
         means it is stated twice"
    );
    // Window: §6's Validate step through step 7 (T64 loose-heading scope
    // pattern, reusing the T126 bounds).
    let start = spec
        .find(STEP6_VALIDATE_HEADING)
        .expect("step-6 Validate heading present");
    let end = start
        + spec[start..]
            .find(STEP7_HEADING)
            .expect("step-7 heading present after step 6's");
    let argv = start
        + spec[start..end]
            .find(META_SPEC_VALIDATOR_ARGV)
            .unwrap_or_else(|| {
                panic!(
                    "the Validate-step window must carry \
                     {META_SPEC_VALIDATOR_ARGV:?} (argv reverted to the \
                     pre-T149 40/30, or moved out of §6?)"
                )
            });
    // Line-scoped localization: the fragment's line must carry the kimi
    // model id — it pins the VALIDATOR launch line, not another
    // template's.
    let line_start = spec[..argv].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = argv
        + spec[argv..]
            .find('\n')
            .unwrap_or(spec.len() - argv);
    let line = &spec[line_start..line_end];
    assert!(
        line.contains(KIMI_MODEL_ID),
        "the budget argv must sit on the kimi validator launch line — the \
         line carrying {META_SPEC_VALIDATOR_ARGV:?} reads {line:?}, with \
         no {KIMI_MODEL_ID:?} on it"
    );
}

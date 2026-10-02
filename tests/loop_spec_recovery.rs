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
//! step-2 T63-resume echo's "80/50 impl, 60/50 validate" (leg (d) above;
//! the echo read "80/35 impl, 60/40 validate" when T149 pinned it —
//! T173's minutes raise re-keyed it, see the T173 block). Its kimi
//! validator PASSed with two non-blocking unpinned-text
//! observations (verdict d1790688874-9): text-revert mutants on (a)
//! LOOP-SPEC step 4's budget RATIONALE — the census sentence ("4 of the
//! last 4 validator children died at budget in cycles 66–69") and the
//! measure clause (">1 of the next 8 validator runs still dies at 60/50 …
//! trim default mutation-leg counts" — "60/40" pre-T173) — and (b)
//! META-SPEC §6's validator-template budget argv, stayed full-suite
//! GREEN. The rationale
//! is the teeth that make the budgets self-governing (the census is the
//! evidence; the measure clause is the pre-committed remedy), and §6's
//! argv is the template line every validator actually launches from — a
//! future edit can currently drop either with every gate green. Legs
//! (p)–(q) pin both texts in place, extending — never replacing — the
//! T149 numbers pin: the step-4 tokens are asserted in the SAME paragraph
//! region as step 4's own `max_iters: 60`, `max_minutes: 50` pair
//! (pre-T173 40; adjacency, not two disjoint file-wide greps — the T24
//! whole-file-grep non-localizing observation), and the §6 argv is one
//! contiguous fragment on the kimi launch line. No doctrine text is
//! edited by this row: LOOP-SPEC.md and META-SPEC.md must stay
//! byte-identical before/after (any wording drift the pin exposes is
//! reported, not fixed).
//!
//! T156 doctrine: cycle 71 practiced a THIRD budget-death variant the T63
//! paragraph did not name — T150-impl died on the 35-minute MINUTES
//! budget at 48/80 with the work complete AND committed (3f5a99a) but the
//! goal unaccepted, and the orchestrator applied orchestrator-finish
//! directly (review the branch, run the gates, merge if green — NO
//! resume burned; routing d1790691515-11). A resume for a fully-committed
//! child is waste (the resumed child re-verifies work already committed),
//! and a paragraph that names only "uncommitted" work for the finish
//! recipe invites the wrong first recovery. The T63 paragraph now carries
//! ONE routing sentence immediately after the resume-first rule, naming
//! the three-way routing with its discriminating test: work INCOMPLETE
//! (uncommitted or partial) takes the ONE resume relaunch (the standing
//! rule, unchanged); work complete and committed but goal unaccepted goes
//! to orchestrator-finish directly (T150-impl precedent, cycle 71,
//! alongside T55); resume-exhausted or unrecoverable goes to next-cycle
//! recovery with a recipe on the row (T28 precedent). The sentence
//! loosens nothing: the ONE-resume cap and the standing recipes it
//! supplements are pinned unchanged AFTER it (leg (s)), and the
//! never-remove-unmerged-work rule (T19's pre-harvest guard, leg (d)) is
//! untouched. Legs (r)–(t) pin the sentence's stable tokens exactly-once
//! inside step 2 (the T64 window pattern), its placement between the
//! resume-first rule and the ONE-cap/standing-recipes sentences, and the
//! standing recipes' bytes — the T55/T28 fallback list the sentence
//! extends, never rewrites.
//!
//! T173 doctrine: the cycle-79 eval (EVALUATION.md §2.1) found MINUTES —
//! not iterations — are now the BINDING child budget: 5 of 7 impl
//! children in the cycles-76–78 delta died at the 35-minute wall with
//! iterations to spare (t162 54/80, t163 46/80, t164 50/80
//! uncommitted→resume, t165 65/80, t167 47/80; the two survivors are the
//! small rows) and 3 of 5 validators finished within ~1–4.5 min of the
//! 40-minute wall (t167-val 39m16s, t168-val 38m22s, t162-val 35m46s).
//! glm throughput runs ~1.3–1.9 iters/min, so an 80-iter budget needs
//! 42–62 min, and a kimi validator needs ~45–50 min for a full 60-iter
//! run. Both templates' minutes moved to 50 in ONE commit (impl 35→50,
//! validator 40→50; max_iters stay 80/60, delegate's built-in defaults
//! stay 40/35, META-SPEC.md is untouched — the T21/T24 precedent: the
//! LOOP-SPEC §6-override paragraph governs the live budget, so leg (q)'s
//! §6 argv pin does NOT move). Legs (u)–(x) pin the raise: (u) the
//! step-2 launch block's budget lines as ONE contiguous fragment — the
//! step-2 template's minutes had NO pin before, so a revert to 35 stayed
//! full-suite GREEN; (v) the explicitness sentence naming 50 with
//! delegate's defaults staying 40/35 (the template overrides minutes
//! explicitly); (w) the NEW impl-side measure clause, the T21-class
//! pattern: the cycles-76–78 census (5 of 7 impl children at the
//! 35-minute wall, 4 committed + 1 resume) as the raise's evidence, a
//! tripwire (>2 of the next 8 impl children still die at the 50-minute
//! budget), and the spec-size-discipline remedy; (x) the step-4 evidence
//! extension — 3 of 5 validators within ~1–4.5 min of the wall, the
//! raise's reason — with the measure clause re-keyed 60/40 → 60/50 (the
//! T155 threshold needle carries the pair now). Every needle and comment
//! quoting the old numbers moves with the doctrine in this same commit;
//! the T156 block's historical 35-minute reference (T150-impl's death)
//! stays as history.
//!
//! T175 doctrine: the Pipeline-overlap paragraph taught the GOAL's export
//! swap (T161: an impl launched into the 2-impl overlap exports the
//! role-keyed slot) but said nothing about the SPEC's `check:` line — and
//! spec check lines hardcode a dir (specs/t162 names the default
//! `target-shared`). Cycle 77 bit: t165's goal gate ran its check
//! (naming the DEFAULT `target-shared`) while T162's impl child actively
//! built into that dir — cargo lock contention plus the T52
//! same-artifact-name class (a gate can execute a binary compiled from a
//! foreign checkout's source) rejected the child's goal on GREEN work;
//! fixed mid-flight by role-keying the check to `target-shared-impl-a`
//! (607e877). T144's scrub (src/tools.rs) makes a bare `cargo`
//! content-correct but cold, so the check's own export is the ONLY
//! target dir the goal gate sees. The paragraph now carries ONE
//! sentence: at dispatch into the overlap the orchestrator ALSO re-keys
//! the spec's `check:` line export to the SAME role-keyed slot before
//! launch, so the goal's export and the check's export always name one
//! dir. Leg (y) pins the sentence's load-bearing tokens exactly-once
//! inside the paragraph's window (the T64 pattern over the
//! "**Pipeline overlap (T44, T161)" lead and the "## Phase 3 — Wrap"
//! boundary): deleting the sentence, duplicating it, or moving it out of
//! the paragraph (e.g. into step 2's launch block, which already pins
//! the GOAL's export swap) all go red. The solo default is
//! doctrine-unchanged — spec authors keep writing check lines against
//! the default `target-shared`; the re-key is a dispatch-time
//! orchestrator act, only when slotting a child into impl-a/impl-b.
//!
//! T186 doctrine: two cycle-84 incidents share one root — §2 step 5's
//! harvest + removal mechanics were underspecified. (1) HARVEST GAP ×2:
//! a FRESH child launched into a reused worktree ROTATES its
//! predecessor's `.chug/events.jsonl` to `.chug/events-<ts>.jsonl`
//! (T10/T7), so the old "harvest every child run's
//! `.chug/events.jsonl`" reading silently dropped every rotated segment,
//! and `git worktree remove` then deleted them untracked — the t183 impl
//! child's two glm segments (an 80/80 death + its T63 resume) and the
//! t181 impl child's glm segment (20/80 first-try) were never harvested,
//! t181's even filed under one combined name while the harvested file
//! held `runs: 1`, kimi only. Cycle-83's kimi harvest of t180 did it
//! right (`events-t180-impl-…` runs: 2 AND `events-t180-validate-…` as
//! separate files) — a written recipe is not a mechanism, so step 5 now
//! carries the glob (`.chug/events*.jsonl`), the rotation mechanism, and
//! one-file-per-segment naming inspected from each file's `run_start`.
//! (2) ZOMBIE COLLISION: cycle-84 seg-1 declared the t183 validator
//! (pid 6260) dead via a TRUNCATED ps read, removed its worktree while
//! the child lived, and the zombie's cargo suites raced the wrap
//! goal-gate into a ~30-minute "check command failed" rejection (the
//! code change was weighed and rejected, eval-triage d1790856539-8) —
//! step 5 now makes liveness a removal precondition (`delegate status`
//! or `kill -0 <pid>`, never a truncated `ps … | head` read) with the
//! defunct-zombie caveat (a bare ps -p / kill -0 hit must also exclude
//! `ps -p <pid> -o stat=` showing `Z`, which satisfies kill -0 while
//! holding no files — the cycle-85 inverse: two defunct validator pids
//! read ALIVE on a bare ps -p moments before a safe removal). Legs
//! (aa)–(ac) pin both rules + the Phase-3 wrap bullet's amendment in the
//! T48/T64 pattern: every needle exactly-once file-wide and inside its
//! window (step 5 = `5. **` through `6. **`, reusing the T120 anchors;
//! Phase 3 = `## Phase 3 — Wrap` through `## Hard rules`), with the
//! removal precondition ordered after the harvest glob and before the
//! merge sentence it guards. Reverting either doctrine sentence drops
//! its needles to 0 and the named leg goes red; duplicating one fires
//! the count-2 leg.

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
        "80/50 impl, 60/50 validate",
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
/// T63-resume echo's budgets ("80/50 impl, 60/50 validate" — pre-T173
/// "80/35 impl, 60/40 validate"); THIS needle pins step 4's own launch
/// bytes as the adjacency anchor for the census/measure tokens — the
/// first step-4 numbers pin, not a duplicate of leg (d). T173 moved the
/// minutes digit 40→50; the needle moved with the doctrine in the same
/// commit. Presence is asserted via the in-window find (below), not a
/// separate file-wide count leg — the numbers are already pinned once and
/// this row does not duplicate that pin.
const STEP4_BUDGET_NUMBERS: &str = "`max_iters: 60`, `max_minutes: 50`";

/// The census clause's stable core tokens — the evidence the 60/50 budgets
/// stand on (four budget deaths in four cycles, 66–69; T173's leg (x)
/// extends the paragraph with the cycle-79 census without touching this
/// sentence). Contiguous as
/// written; must occur EXACTLY once in LOOP-SPEC.md.
const STEP4_CENSUS_NEEDLE: &str = "4 of the last 4 validator children died at budget";

/// The measure clause's census-threshold token — the tripwire that keeps
/// the budgets self-governing, EXTENDED by T173 to carry the re-keyed
/// budget pair (">1 of the next 8 validator runs still dies at 60/50" —
/// reverting the measure clause to the pre-T173 60/40 goes red here, the
/// exact unpinned-revert class T155's verdict flagged). Contiguous as
/// written; EXACTLY once.
const STEP4_MEASURE_THRESHOLD_NEEDLE: &str =
    ">1 of the next 8 validator runs still dies at 60/50";

/// The step-4 evidence extension's census token (T173) — the cycle-79
/// validator census (3 of 5 validators finished within ~1–4.5 min of the
/// wall), the raise's reason. Contiguous as written; EXACTLY once.
const STEP4_EVIDENCE_NEEDLE: &str = "3 of 5 validators";

/// The step-4 evidence extension's wall token (T173) — the 40-minute wall
/// the raise moved (validator minutes 40→50). Contiguous as written;
/// EXACTLY once.
const STEP4_EVIDENCE_WALL_NEEDLE: &str = "40-minute wall";

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
/// `max_minutes: 50` numbers pair (pre-T173 40) — the same paragraph
/// region (the budget parenthetical), not a disjoint section (the T24
/// non-localizing observation). Delete the rationale text — T149's
/// observed text-revert mutant — and every needle goes red at count 0 /
/// window-miss; move the rationale out of step 4 and it dies on the
/// window find; reorder census/measure and it dies on the ordering
/// assert; revert the measure clause's budget pair to the pre-T173 60/40
/// and the extended threshold needle goes red (T173's re-key is
/// load-bearing). Leg (d)'s step-2 echo pin is untouched and stays green
/// under this row's mutants — that co-green gap is exactly what T149's
/// verdict flagged.
#[test]
fn step4_budget_rationale_tokens_occur_once_beside_the_budget_numbers() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        STEP4_BUDGET_NUMBERS.starts_with("`max_iters: 60`")
            && STEP4_BUDGET_NUMBERS.ends_with("`max_minutes: 50`"),
        "the numbers needle must be step 4's contiguous 60/50 pair verbatim"
    );
    assert!(
        STEP4_CENSUS_NEEDLE.starts_with("4 of the last 4")
            && STEP4_CENSUS_NEEDLE.ends_with("died at budget"),
        "the census needle must carry the four-deaths evidence verbatim"
    );
    assert!(
        STEP4_MEASURE_THRESHOLD_NEEDLE.starts_with(">1 of the next 8")
            && STEP4_MEASURE_THRESHOLD_NEEDLE.ends_with("60/50"),
        "the measure threshold needle must carry the >1-of-8 tripwire and \
         the re-keyed 60/50 pair verbatim"
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
            "step-4 window must carry {STEP4_BUDGET_NUMBERS:?} — the 60/50 \
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
        "the rationale must sit in the SAME paragraph region as the 60/50 \
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
/// full-suite GREEN. T173 did NOT touch §6's argv (META-SPEC.md is
/// untouched — the T21/T24 precedent: the LOOP-SPEC §6-override paragraph
/// governs the live budget at 60/50, while §6's own template stays
/// byte-identical and this pin stays at 60/40). The fragment is asserted
/// contiguously (one
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

// ---- T156 — the T63 committed-variant routing sentence ----
//
// Cycle 71's T150-impl budget death — work complete AND committed, goal
// unaccepted, orchestrator-finish with NO resume burned — was practiced
// and logged (routing d1790691515-11) but the T63 paragraph named only
// the "incomplete work" resume rule and the two standing recipes. The
// paragraph now carries ONE routing sentence immediately after the
// resume-first rule; these legs pin the sentence's tokens, its placement,
// and the neighbors it must not rewrite.

/// The routing sentence's stable tokens — the three-way routing's
/// discriminating vocabulary, each contiguous as written and asserted
/// EXACTLY once in LOOP-SPEC.md plus inside step 2's window. Bare words
/// that pre-date the row are deliberately NOT pinned alone
/// ("orchestrator-finish" already names the standing recipes and the
/// decision-log sentence; "next-cycle recovery" the fallback list) — the
/// tokens below are unique to the NEW sentence.
const ROUTING_TOKENS: [&str; 9] = [
    "routing discriminator",
    "work INCOMPLETE",
    "(uncommitted or partial)",
    "complete and committed",
    "orchestrator-finish directly",
    "NO resume burned",
    "T150-impl",
    "cycle 71",
    "d1790691515-11",
];

/// (r) T156 — the routing sentence exists, exactly once, inside step 2:
/// every stable token occurs exactly once in LOOP-SPEC.md and inside the
/// polling-paragraph window (STEP2_ANCHOR .. STEP3_HEADING, the T64
/// loose-heading scope pattern). Delete the sentence — the text-revert
/// mutant — and every token goes red at count 0; move the sentence out of
/// step 2 and the window check dies; reword one token and that token goes
/// red while its neighbors stay green (the per-token diagnosis).
#[test]
fn routing_sentence_tokens_exactly_once_inside_step_2() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        ROUTING_TOKENS.contains(&"complete and committed")
            && ROUTING_TOKENS.iter().any(|t| t.contains("INCOMPLETE")),
        "the token set must carry both routing arms' discriminating phrases"
    );
    assert!(
        ROUTING_TOKENS.contains(&"T150-impl")
            && ROUTING_TOKENS.iter().any(|t| t.starts_with("d179")),
        "the token set must carry the T150-impl precedent and its routing id"
    );
    let spec = loop_spec();
    let anchor = spec
        .find(STEP2_ANCHOR)
        .expect("step-2 polling anchor present");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading present");
    let window = &spec[anchor..step3];
    for token in ROUTING_TOKENS {
        assert_eq!(
            spec.matches(token).count(),
            1,
            "LOOP-SPEC must state the routing token {token:?} exactly once — \
             zero means the routing sentence was deleted or the token \
             reworded (the T156 text-revert mutant), more than one means it \
             is stated twice"
        );
        assert!(
            window.contains(token),
            "the routing token {token:?} must sit inside step 2's window \
             (anchor {anchor}..step3 {step3}) — the routing sentence \
             drifted out of the T63 polling paragraph"
        );
    }
}

/// The routing sentence's adjacency to the resume-first rule — one
/// contiguous fragment spanning the rule's terminator, the sentence's
/// first two words, and the wrap between them (the T47_EXPORT_PREFIX
/// idiom: the wrap is spelled so a rewrap cannot silently unpin it). This
/// is the "immediately after the resume-first rule" placement, byte-exact.
const ROUTING_ADJACENCY: &str = concat!(
    "instead of starting cold. The\n",
    "   routing discriminator"
);

/// The T19 rationale's stable core — the never-remove-unmerged-work guard
/// must still read BETWEEN the routing sentence and the ONE-resume cap
/// (leg (d) pins its tokens inside the window; this leg pins its slot).
const T19_RATIONALE: &str = "works because the worktree is never removed pre-harvest";

/// The standing recipes' T55/T28 fallback anchors, contiguous as written.
/// "row, " disambiguates the T28 anchor from the routing sentence's own
/// "(T28 precedent)" mention; each must occur EXACTLY once.
const T55_FALLBACK: &str = "T55 precedent";
const T28_FALLBACK: &str = "row, T28 precedent";

/// (s) T156 — placement, and the not-loosening leg: the routing sentence
/// sits IMMEDIATELY after the resume-first rule (the adjacency fragment
/// above, byte-exact including the wrap) and BEFORE the T19 rationale,
/// the ONE-resume cap, the standing recipes, the nohup fallback, and
/// step 3's heading, reading in the written order. The committed-variant
/// routing adds a SHORTCUT past the resume for already-committed work; it
/// neither raises the ONE-resume cap (still stated once, still after the
/// sentence) nor removes the T55/T28 fallback (still byte-identical after
/// the cap — leg (t) pins its bytes), and the T19 pre-harvest guard still
/// stands between them.
#[test]
fn routing_sentence_sits_after_resume_first_rule_before_cap_and_fallback() {
    // Needle self-check (T48 idiom).
    assert!(
        ROUTING_ADJACENCY.starts_with("instead of starting cold.")
            && ROUTING_ADJACENCY.ends_with("routing discriminator"),
        "the adjacency needle must span the resume-first rule's terminator \
         and the routing sentence's first two words"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(ROUTING_ADJACENCY).count(),
        1,
        "the routing sentence must open IMMEDIATELY after the resume-first \
         rule's terminator (\"instead of starting cold.\" + wrap + \"The \
         routing discriminator\"), exactly once — zero means the sentence \
         moved, was deleted, or was rewrapped away from the rule"
    );
    let routing = spec
        .find("routing discriminator")
        .expect("the routing sentence's opener present (leg (r) covers its tokens)");
    let t19 = spec.find(T19_RATIONALE).unwrap_or_else(|| {
        panic!(
            "the T19 rationale present (leg (d) pins its tokens) — the \
             never-remove-unmerged-work guard must survive the T156 edit"
        )
    });
    let cap = spec
        .find(RESUME_CAP)
        .expect("the ONE-resume cap present (leg (a) pins its count)");
    let t55 = spec
        .find(T55_FALLBACK)
        .expect("the T55 fallback anchor present");
    let t28 = spec
        .find(T28_FALLBACK)
        .expect("the T28 fallback anchor present");
    let fallback = spec
        .find(NOHUP_FALLBACK)
        .expect("the nohup fallback present (leg (c) pins its bytes)");
    let step3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading present");
    assert!(
        routing < t19
            && t19 < cap
            && cap < t55
            && t55 < t28
            && t28 < fallback
            && fallback < step3,
        "after the routing sentence the paragraph must read in the written \
         order — T19 rationale, ONE-resume cap, standing recipes (T55 then \
         T28), nohup fallback, step 3 (routing {routing}, t19 {t19}, cap \
         {cap}, t55 {t55}, t28 {t28}, fallback {fallback}, step3 {step3}) — \
         the sentence supplements the resume-first rule, never rewrites the \
         cap or the fallback list"
    );
}

/// The standing-recipes sentence the routing sentence extends —
/// BYTE-IDENTICAL INCLUDING its wrapped line breaks (the
/// T47_EXPORT_PREFIX idiom): the new sentence names the same T55/T28
/// precedents but must not rewrite the fallback list they anchor (the
/// ONE-resume cap's fallback stays intact for resume-exhausted deaths).
const STANDING_RECIPES: &str = concat!(
    "the standing recipes (orchestrator-finish for complete-but-uncommitted\n",
    "   work, T55 precedent; next-cycle recovery with a recipe written on the\n",
    "   row, T28 precedent)."
);

/// (t) T156 — the standing-recipes sentence survives BYTE-IDENTICAL
/// (wrapped line breaks included) exactly once: the routing sentence
/// supplements the fallback list, never rewrites it. Mutate any byte of
/// the list — e.g. rewording "complete-but-uncommitted" toward the new
/// sentence's "complete and committed" — and this goes red while legs
/// (r)/(s) stay green (they pin different bytes).
#[test]
fn standing_recipes_sentence_survives_byte_identical() {
    // Needle self-check (T48 idiom).
    assert!(
        STANDING_RECIPES.contains("complete-but-uncommitted")
            && STANDING_RECIPES.ends_with("row, T28 precedent)."),
        "the needle must be the standing-recipes sentence verbatim, \
         wrapping included"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(STANDING_RECIPES).count(),
        1,
        "the standing-recipes sentence must survive byte-identical \
         (wrapping included) exactly once — the T156 routing sentence \
         supplements the T55/T28 fallback list, never rewrites or rewraps \
         it"
    );
}

// ---- T173 — the child minutes budgets: 35/40 → 50/50, both roles ----
//
// The cycle-79 eval found MINUTES the binding child budget (5 of 7 impl
// children died at the 35-minute wall with iterations to spare; 3 of 5
// validators finished within ~1–4.5 min of the 40-minute wall), so both
// templates' minutes moved to 50 in ONE commit with these pins. max_iters
// stay 80/60, delegate's built-in defaults stay 40/35, and META-SPEC.md
// is untouched (the T21/T24 precedent — leg (q)'s argv pin stays 60/40).

/// The step-2 launch template's budget lines as ONE contiguous fragment,
/// byte-identical INCLUDING the wrapped line break and the delegate
/// block's 5-space indentation ("max_iters:" + three spaces before the
/// 80 — the template's argv shape, not the prose pairs). The step-2
/// template's minutes had NO pin before this leg, so a revert to 35
/// stayed full-suite GREEN — the RED-proof gap this leg closes. Must
/// occur EXACTLY once in LOOP-SPEC.md.
const IMPL_TEMPLATE_BUDGETS: &str = concat!(
    "     max_iters:   80\n",
    "     max_minutes: 50"
);

/// The explicitness sentence's budget pair — the reworded sentence
/// naming 50 on both surfaces of the template's intro line. Contiguous
/// as written; EXACTLY once.
const IMPL_EXPLICIT_PAIR: &str =
    "`max_iters: 80` and `max_minutes: 50` are explicit";

/// The explicitness sentence's defaults token — delegate's built-in
/// defaults STAY 40/35 (the template overrides minutes explicitly; the
/// pre-T173 wording was "defaults are 40/35"). Contiguous as written;
/// EXACTLY once.
const IMPL_DEFAULTS_UNCHANGED: &str = "defaults stay 40/35";

/// The impl measure clause's census token — the raise's evidence (the
/// cycles-76–78 census: 5 of 7 impl children dead at the wall). The
/// T21-class pattern: a budget's parenthetical carries its own census.
/// Contiguous as written; EXACTLY once.
const IMPL_CENSUS_NEEDLE: &str = "5 of 7 impl children";

/// The impl measure clause's old-wall token — the 35-minute wall the
/// census died on (the raise's baseline; also the T156 history's
/// T150-impl wall — this needle pins the LIVE census sentence's wall,
/// whose count is asserted exactly-once here). Contiguous as written;
/// EXACTLY once.
const IMPL_OLD_WALL_NEEDLE: &str = "35-minute wall";

/// The impl measure clause's tripwire+budget token, byte-identical
/// INCLUDING its wrapped line break (the file wraps between "impl" and
/// "children"; the concat! spells the wrap so a rewrap cannot silently
/// unpin it — the T47_EXPORT_PREFIX idiom): >2 of the next 8 impl
/// children dying at the 50-MINUTE budget trips the clause.
const IMPL_MEASURE_TRIPWIRE: &str = concat!(
    ">2 of the next 8 impl\n",
    "   children still die at the 50-minute budget"
);

/// The impl measure clause's remedy token — spec-size discipline instead
/// of further raises (T110's ceiling already governs iterations; this
/// clause governs minutes). Contiguous as written; EXACTLY once.
const IMPL_MEASURE_REMEDY: &str = "spec-size discipline";

/// (u) T173 — the step-2 launch template carries the raised minutes as
/// ONE contiguous fragment inside step 2's launch block (the T64
/// loose-heading window), BEFORE the explicitness sentence that explains
/// it. Reverting the template's minutes to 35 — the eval's observed
/// binding-budget death class — goes red here at count 0 (the fragment
/// is byte-pinned, so any renumber or reindent dies too); moving the
/// fragment out of step 2 dies on the window find.
#[test]
fn impl_template_budget_fragment_survives_in_step2_launch_block() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        IMPL_TEMPLATE_BUDGETS.starts_with("     max_iters:")
            && IMPL_TEMPLATE_BUDGETS.ends_with("max_minutes: 50"),
        "the template-budgets needle must be the step-2 launch block's \
         contiguous 80/50 lines verbatim, indentation included"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(IMPL_TEMPLATE_BUDGETS).count(),
        1,
        "LOOP-SPEC's step-2 launch template must carry the 80/50 budget \
         lines byte-identically exactly once — zero means the minutes were \
         reverted (the pre-T173 35) or the block was rewrapped/reindented, \
         more than one means the template is stated twice"
    );
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let budgets = window
        .find(IMPL_TEMPLATE_BUDGETS)
        .unwrap_or_else(|| {
            panic!(
                "step-2 window must carry the template budget fragment \
                 {IMPL_TEMPLATE_BUDGETS:?} (moved out of step 2?)"
            )
        });
    let explicit = window
        .find(IMPL_EXPLICIT_PAIR)
        .unwrap_or_else(|| {
            panic!(
                "step-2 window must carry {IMPL_EXPLICIT_PAIR:?} (leg (v) \
                 covers its census; this leg pins the template-first order)"
            )
        });
    assert!(
        budgets < explicit,
        "the launch template must precede the explicitness sentence — \
         template fragment ({budgets}), then the sentence naming 50 \
         ({explicit})"
    );
}

/// (v) T173 — the explicitness sentence names 50 and keeps delegate's
/// defaults at 40/35: both tokens occur EXACTLY once each in LOOP-SPEC.md,
/// inside step 2's window, pair first then the defaults clause — the
/// template overrides minutes explicitly WITHOUT raising the tool's
/// built-in defaults (req: defaults stay 40/35). Reverting the sentence's
/// numbers (either digit) or restoring the pre-T173 "defaults are 40/35"
/// wording goes red at count 0.
#[test]
fn impl_explicitness_sentence_names_50_with_defaults_unchanged() {
    // Needle self-checks (T48 idiom).
    assert!(
        IMPL_EXPLICIT_PAIR.starts_with("`max_iters: 80`")
            && IMPL_EXPLICIT_PAIR.ends_with("are explicit"),
        "the explicitness needle must be the template-intro sentence's \
         80/50 pair verbatim"
    );
    assert!(
        IMPL_DEFAULTS_UNCHANGED.starts_with("defaults stay")
            && IMPL_DEFAULTS_UNCHANGED.ends_with("40/35"),
        "the defaults needle must carry the stay-40/35 language verbatim"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (
            IMPL_EXPLICIT_PAIR,
            "the step-2 explicitness sentence's 80/50 pair",
        ),
        (
            IMPL_DEFAULTS_UNCHANGED,
            "the step-2 defaults-stay-40/35 clause",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             sentence was reverted (the pre-T173 35/`defaults are 40/35` \
             wording) or rewrapped across a line break, more than one \
             means it is stated twice"
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
    let pair = window
        .find(IMPL_EXPLICIT_PAIR)
        .expect("step-2 window carries the explicitness pair (leg (u) orders it)");
    let defaults = window
        .find(IMPL_DEFAULTS_UNCHANGED)
        .expect("step-2 window carries the defaults clause");
    assert!(
        pair < defaults,
        "the explicitness sentence must read pair-then-defaults — pair \
         ({pair}), defaults stay 40/35 ({defaults})"
    );
}

/// (w) T173 — the NEW impl-side measure clause (the T21-class pattern)
/// survives in step 2 with its census evidence, old-wall baseline,
/// tripwire, and remedy in written order: the cycles-76–78 census (5 of 7
/// impl children at the 35-minute wall), the >2-of-8 tripwire at the
/// 50-minute budget (wrapped line break included), and the
/// spec-size-discipline remedy. Delete the clause — T155's observed
/// unpinned-rationale class — and every needle goes red at count 0;
/// move it out of step 2 and it dies on the window find; reorder and it
/// dies on the ordering assert.
#[test]
fn impl_minutes_measure_clause_census_tripwire_remedy_in_step2() {
    // Needle self-checks (T48 idiom).
    assert!(
        IMPL_CENSUS_NEEDLE.starts_with("5 of 7")
            && IMPL_CENSUS_NEEDLE.ends_with("impl children"),
        "the impl census needle must carry the five-of-seven evidence \
         verbatim"
    );
    assert!(
        IMPL_OLD_WALL_NEEDLE.starts_with("35-minute")
            && IMPL_OLD_WALL_NEEDLE.ends_with("wall"),
        "the old-wall needle must be the 35-minute wall verbatim"
    );
    assert!(
        IMPL_MEASURE_TRIPWIRE.starts_with(">2 of the next 8")
            && IMPL_MEASURE_TRIPWIRE.ends_with("50-minute budget"),
        "the tripwire needle must carry the >2-of-8 tripwire and the \
         50-minute budget verbatim, wrapped line break included"
    );
    assert!(
        IMPL_MEASURE_REMEDY.starts_with("spec-size")
            && IMPL_MEASURE_REMEDY.ends_with("discipline"),
        "the remedy needle must carry the spec-size-discipline remedy \
         verbatim"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (IMPL_CENSUS_NEEDLE, "the impl measure clause's 5-of-7 census"),
        (
            IMPL_OLD_WALL_NEEDLE,
            "the impl measure clause's 35-minute-wall baseline",
        ),
        (
            IMPL_MEASURE_TRIPWIRE,
            "the impl measure clause's >2-of-8 tripwire at the 50-minute \
             budget",
        ),
        (
            IMPL_MEASURE_REMEDY,
            "the impl measure clause's spec-size-discipline remedy",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             impl measure clause was deleted (or a needle was rewrapped \
             across a line break), more than one means it is stated twice"
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
    let census = window
        .find(IMPL_CENSUS_NEEDLE)
        .expect("step-2 window carries the impl census");
    let wall = window
        .find(IMPL_OLD_WALL_NEEDLE)
        .expect("step-2 window carries the 35-minute wall");
    let tripwire = window
        .find(IMPL_MEASURE_TRIPWIRE)
        .expect("step-2 window carries the impl tripwire (wrapped)");
    let remedy = window
        .find(IMPL_MEASURE_REMEDY)
        .expect("step-2 window carries the impl remedy");
    assert!(
        census < wall && wall < tripwire && tripwire < remedy,
        "the impl measure clause must read census-wall-tripwire-remedy — \
         census ({census}), 35-minute wall ({wall}), tripwire ({tripwire}), \
         remedy ({remedy})"
    );
}

/// (x) T173 — step 4's evidence EXTENSION survives between the T155
/// census clause and the re-keyed measure tripwire: the cycle-79
/// validator census (3 of 5 validators finishing within ~1–4.5 min of the
/// 40-minute wall — the raise's reason) occurs EXACTLY once in
/// LOOP-SPEC.md, inside step 4's window, ordered census → evidence →
/// 40-minute wall → measure tripwire. Deleting the extension — the
/// T149/T155 unpinned-rationale class — goes red at count 0; reverting
/// the measure clause's budget pair is leg (p)'s extended threshold
/// needle, not this leg.
#[test]
fn step4_evidence_extension_sits_between_census_and_tripwire() {
    // Needle self-checks (T48 idiom).
    assert!(
        STEP4_EVIDENCE_NEEDLE.starts_with("3 of 5")
            && STEP4_EVIDENCE_NEEDLE.ends_with("validators"),
        "the evidence needle must carry the three-of-five validator census \
         verbatim"
    );
    assert!(
        STEP4_EVIDENCE_WALL_NEEDLE.starts_with("40-minute")
            && STEP4_EVIDENCE_WALL_NEEDLE.ends_with("wall"),
        "the evidence wall needle must be the 40-minute wall verbatim"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (
            STEP4_EVIDENCE_NEEDLE,
            "the step-4 cycle-79 validator census (3 of 5)",
        ),
        (
            STEP4_EVIDENCE_WALL_NEEDLE,
            "the step-4 40-minute-wall baseline",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             cycle-79 evidence extension was deleted, more than one means \
             it is stated twice"
        );
    }
    let start = spec
        .find(STEP4_HEADING_LOOSE)
        .expect("step-4 heading (`4. **`) present");
    let end = start
        + spec[start..]
            .find(STEP5_HEADING_LOOSE)
            .expect("step-5 heading present after step 4's");
    let window = &spec[start..end];
    let census = window
        .find(STEP4_CENSUS_NEEDLE)
        .expect("step-4 window carries the T155 census clause (leg (p))");
    let evidence = window
        .find(STEP4_EVIDENCE_NEEDLE)
        .expect("step-4 window carries the cycle-79 evidence");
    let wall = window
        .find(STEP4_EVIDENCE_WALL_NEEDLE)
        .expect("step-4 window carries the 40-minute wall");
    let threshold = window
        .find(STEP4_MEASURE_THRESHOLD_NEEDLE)
        .expect("step-4 window carries the re-keyed measure tripwire (leg (p))");
    assert!(
        census < evidence && evidence < wall && wall < threshold,
        "the evidence extension must sit between the T155 census and the \
         re-keyed measure tripwire — census ({census}), 3-of-5 evidence \
         ({evidence}), 40-minute wall ({wall}), 60/50 tripwire ({threshold})"
    );
}

/// T175 — the Pipeline-overlap paragraph's window: from the paragraph's
/// bold lead to the Phase 3 heading (the paragraph is Phase 2's last
/// block, so the next heading bounds it — the T64 loose-heading window
/// pattern).
const OVERLAP_LEAD: &str = "**Pipeline overlap (T44, T161)";
const PHASE3_HEADING: &str = "## Phase 3 — Wrap";

/// T175 — the check-line re-key sentence's load-bearing tokens: (1) the
/// ACT (re-key the check line — the spec's `check:` line export, not the
/// goal's), (2) the DESTINATION (the SAME role-keyed slot the goal's
/// export was swapped to), (3) the REASON (T144's scrub: the check's own
/// export is the only target dir the goal gate sees), and (4) the
/// EVIDENCE (the cycle-77 bite: t165's rejection, fixed mid-flight by
/// 607e877). Each must occur EXACTLY once in LOOP-SPEC.md and INSIDE the
/// overlap paragraph's window.
const CHECK_REKEY_TOKENS: [&str; 5] = [
    "re-keys the spec's `check:` line export",
    "SAME role-keyed slot",
    "the only target dir the goal gate sees",
    "t165",
    "607e877",
];

/// (y) T175 — the overlap dispatch check-line re-key: the Pipeline-overlap
/// paragraph's ONE sentence (at dispatch into the overlap the
/// orchestrator ALSO re-keys the spec's `check:` line export to the SAME
/// role-keyed slot before launch) pins its load-bearing tokens
/// exactly-once INSIDE the paragraph's window — non-vacuous: deleting
/// the sentence goes red at count 0, duplicating it goes red at count 2,
/// and moving it out of the paragraph goes red on window containment.
/// The sentence must also sit AFTER the T161 slot sentence it extends
/// (its `target-shared-impl-b` bytes) and BEFORE the Doctrine-items rule
/// — it is an overlap-dispatch obligation, not an exemption. The solo
/// default is unchanged: no leg pins a spec-author-side re-key, and the
/// T161 sentences' bytes are untouched.
#[test]
fn overlap_check_rekey_tokens_exactly_once_inside_pipeline_overlap_paragraph() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        CHECK_REKEY_TOKENS[0].starts_with("re-keys")
            && CHECK_REKEY_TOKENS[0].contains("check:"),
        "token 0 must be the re-key ACT naming the spec's check line"
    );
    assert!(
        CHECK_REKEY_TOKENS
            .iter()
            .any(|t| t.contains("role-keyed slot"))
            && CHECK_REKEY_TOKENS.iter().any(|t| t.contains("goal gate")),
        "the token set must carry the same-slot destination and the \
         goal-gate reason"
    );
    assert!(
        CHECK_REKEY_TOKENS.contains(&"t165")
            && CHECK_REKEY_TOKENS.contains(&"607e877"),
        "the token set must carry the cycle-77 evidence (t165's rejection, \
         the 607e877 mid-flight fix)"
    );
    let spec = loop_spec();
    let start = spec
        .find(OVERLAP_LEAD)
        .expect("the Pipeline-overlap paragraph's bold lead present");
    let end = start
        + spec[start..]
            .find(PHASE3_HEADING)
            .expect("the Phase 3 heading present after the overlap paragraph");
    let window = &spec[start..end];
    for token in CHECK_REKEY_TOKENS {
        assert_eq!(
            spec.matches(token).count(),
            1,
            "LOOP-SPEC must state the check-line re-key token {token:?} \
             exactly once — zero means the T175 sentence was deleted, more \
             than one means it is stated twice (the T156 text-revert \
             mutant class)"
        );
        assert!(
            window.contains(token),
            "the re-key token {token:?} must sit inside the Pipeline-overlap \
             paragraph's window ({start}..{end}) — the sentence drifted out \
             of the overlap doctrine (e.g. into step 2's launch block, \
             which pins the GOAL's export swap, not the check line's)"
        );
    }
    // Placement within the paragraph: the re-key sentence extends the
    // T161 slot sentence (the goal's export swap it must match) and sits
    // BEFORE the Doctrine-items-never-overlap rule.
    let slot = window
        .find("target-shared-impl-b")
        .expect("the T161 slot sentence present in the window (untouched)");
    let rekey = window
        .find(CHECK_REKEY_TOKENS[0])
        .expect("the re-key sentence present in the window (count leg above)");
    let doctrine = window
        .find("Doctrine items NEVER overlap")
        .expect("the Doctrine-items rule present in the window (untouched)");
    assert!(
        slot < rekey && rekey < doctrine,
        "the re-key sentence must sit after the T161 slot sentence it \
         extends and before the Doctrine-items rule (slot {slot}, re-key \
         {rekey}, doctrine rule {doctrine})"
    );
}

// ---- T176 — the impl goal template's clippy bar: `-D warnings` ----
//
// The template's "Keep cargo build + clippy + test green" sentence named no
// lint level, so an impl child could honestly claim clippy-green while
// warnings stood: cycle 77's T166 impl committed with a `needless_lifetimes`
// warning under a "clippy clean" claim and the kimi validator caught it
// (finding 1), the fix landing on-branch at 7911b3c — validator time spent
// on a mechanical nit the child's own gate should have caught. The
// validator-side gates and the orchestrator's post-merge gates already run
// clippy with `-D` clean; only the child's self-check bar was unspecified.
// The template sentence now names the level, and a prose sentence after the
// launch block spells the semantics (zero warnings, not exit-0 clippy) and
// names the evidence. The sweep-the-family check found exactly ONE goal-
// template surface carrying the bare phrase — step 2's impl-child template;
// step 4's FAIL-arc fix-up child has no template of its own (its goal is
// step 2's with the findings pasted in), so this one edit covers both.

/// The template's lint sentence, byte-identical INCLUDING its wrapped line
/// break (13-space indent — the goal string's continuation indent). Must
/// occur EXACTLY once in LOOP-SPEC.md.
const CLIPPY_DENY_SENTENCE: &str = concat!(
    "Keep cargo build +\n",
    "             clippy `-D warnings` + test green."
);

/// The load-bearing lint level. Must occur EXACTLY once inside the goal
/// string's window (the template surface) — one template, one level; the
/// prose evidence sentence repeats the flag, so the count is scoped to the
/// goal string, not the whole file.
const CLIPPY_DENY_FLAG: &str = "-D warnings";

/// The OLD bare phrase, wrap-insensitive: the pinned sentence reads
/// "clippy `-D warnings` + test green", so this substring survives a revert
/// under ANY rewrapping. Must occur ZERO times in LOOP-SPEC.md.
const BARE_CLIPPY_PHRASE: &str = "clippy + test green";

/// The evidence clause's instance tag — the cycle and TODO item.
const T166_INSTANCE: &str = "cycle-77 T166";

/// The evidence clause's warning lint — the nit the T166 impl shipped.
const T166_WARNING: &str = "needless_lifetimes";

/// The evidence clause's on-branch fix ref.
const T166_FIX_REF: &str = "7911b3c";

/// (z) T176 — the goal template's clippy bar names the lint level. The new
/// sentence occurs EXACTLY once (byte-identical, wrap included) inside
/// step 2's window, ordered after the T47 export prefix it extends and
/// before the `Commit your work here.` sentence it precedes; `-D warnings`
/// occurs EXACTLY once inside the goal string's window (one template
/// surface, one level); the OLD bare phrase occurs ZERO times anywhere in
/// LOOP-SPEC.md (the wrap-insensitive revert detector); and the evidence
/// clause's three tokens (instance tag, warning lint, fix ref) each occur
/// EXACTLY once, inside step 2's window. Revert the template sentence and
/// TWO legs go red at once — the sentence's count drops to 0 AND the bare
/// phrase reappears (the RED proof); duplicate the sentence and the count-2
/// leg fires; move the evidence clause out of step 2 and the window leg
/// fires.
#[test]
fn goal_template_clippy_bar_denies_warnings() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        CLIPPY_DENY_SENTENCE.starts_with("Keep cargo build")
            && CLIPPY_DENY_SENTENCE.ends_with("test green."),
        "the clippy-bar needle must be the goal template's lint sentence \
         verbatim (wrap + continuation indent included)"
    );
    assert_eq!(CLIPPY_DENY_FLAG, "-D warnings");
    assert!(
        !CLIPPY_DENY_SENTENCE.contains(BARE_CLIPPY_PHRASE),
        "the bare-phrase detector must not match the pinned sentence itself \
         (the backticked flag separates `clippy` from `+ test green`)"
    );
    assert!(
        T166_INSTANCE.starts_with("cycle-77") && T166_INSTANCE.ends_with("T166"),
        "the evidence tag must name the cycle and the item"
    );

    let spec = loop_spec();

    // Presence, exactly-once, byte-identical.
    assert_eq!(
        spec.matches(CLIPPY_DENY_SENTENCE).count(),
        1,
        "the goal template must carry the `-D warnings` clippy bar \
         byte-identical exactly once — zero means the sentence was \
         reverted to the bare phrase (the T176 fix undone), more than one \
         means it is stated twice"
    );
    // The OLD bare phrase, wrap-insensitive, gone everywhere.
    assert_eq!(
        spec.matches(BARE_CLIPPY_PHRASE).count(),
        0,
        "the bare `clippy + test green` phrase must not survive anywhere in \
         LOOP-SPEC — the pre-T176 template reverted under a different \
         wrapping, or a new goal-template surface reintroduced it"
    );

    // Scope: the sentence lives INSIDE step 2's window, ordered after the
    // T47 export prefix it extends and before the `Commit your work here.`
    // sentence it precedes (the template's sentence order preserved).
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let t47 = window
        .find(T47_EXPORT_PREFIX)
        .expect("step-2 window must carry the T47 export prefix (untouched)");
    let sentence = window
        .find(CLIPPY_DENY_SENTENCE)
        .unwrap_or_else(|| {
            panic!(
                "step-2 window must carry the clippy-bar sentence (deleted, \
                 or moved out of step 2?)"
            )
        });
    let here = window
        .find(COMMIT_HERE)
        .expect("step-2 window must carry `Commit your work here.` (untouched)");
    assert!(
        t47 < sentence && sentence < here,
        "the clippy-bar sentence must sit between the T47 export prefix \
         ({t47}) and the `Commit your work here.` sentence ({here}) — the \
         template's sentence order preserved (found at {sentence})"
    );

    // One template surface, one lint level: inside the goal string's window
    // (the T47 prefix through the DO-NOT sentence that closes it),
    // `-D warnings` occurs EXACTLY once.
    let goal_start = start + t47;
    let goal_end = start
        + window
            .find(DO_NOT_TOUCH_SENTENCE)
            .expect(
                "step-2 window must carry the DO-NOT closing sentence \
                 (untouched)",
            )
        + DO_NOT_TOUCH_SENTENCE.len();
    let goal_window = &spec[goal_start..goal_end];
    assert_eq!(
        goal_window.matches(CLIPPY_DENY_FLAG).count(),
        1,
        "the goal string must carry `-D warnings` exactly once — zero means \
         the lint level was reverted, more than one means a second \
         statement drifted into the template surface"
    );

    // The evidence clause is load-bearing too: the cycle-77 T166 instance
    // tag, the warning lint it shipped, and the on-branch fix ref each
    // occur EXACTLY once, inside step 2's window (the prose sentence the
    // template change points at).
    for (token, what) in [
        (T166_INSTANCE, "the cycle-77 T166 instance tag"),
        (
            T166_WARNING,
            "the needless_lifetimes warning the T166 impl shipped",
        ),
        (T166_FIX_REF, "the on-branch fix ref 7911b3c"),
    ] {
        assert_eq!(
            spec.matches(token).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             evidence clause was deleted, more than one means it is stated \
             twice"
        );
        assert!(
            window.contains(token),
            "the evidence token {token:?} must sit inside step 2's window — \
             the evidence clause drifted out of the launch-block doctrine"
        );
    }
}

// ---- T186 — step 5: harvest ALL events segments + the live-child removal precondition ----
//
// Two cycle-84 incidents, one root (see the module docs for the full
// narrative): the harvest gap (rotated segments dropped, then deleted
// with the removed worktree) and the zombie collision (a worktree
// removed under a live child after a truncated ps read). Step 5 now
// carries BOTH rules; legs (aa)–(ac) pin them in the T48/T64 pattern —
// every needle exactly-once in LOOP-SPEC.md and inside its window, so a
// revert drops the count to 0, a duplicate fires count-2, and a move out
// of the window dies on the find.

/// The harvest-all glob — the live stream AND every rotated segment,
/// copied one harvested file per source file. Must occur EXACTLY once in
/// LOOP-SPEC.md.
const HARVEST_ALL_GLOB: &str = ".chug/events*.jsonl";

/// The harvest shape — one harvested file per source file (never one
/// combined file per worktree). Must occur EXACTLY once.
const ONE_FILE_PER_SEGMENT: &str = "ONE harvested file per source file";

/// The rotation mechanism's row tag (T7: a fresh run rotates the
/// transcript; T10: the events stream's rotation mirrors it) — the
/// reason a reused worktree holds MORE than one segment. Must occur
/// EXACTLY once.
const ROTATION_TAG: &str = "T10/T7";

/// The per-segment naming rule's inspect token — each harvested file is
/// named per the run segment(s) it ACTUALLY contains, read off its
/// `run_start`. Must occur EXACTLY once (step 4's T69 run_start-latch
/// sentence does not collide with this longer needle).
const RUN_START_INSPECT: &str = "`run_start` model/spec";

/// The combined-name ban — never `impl-validate` for a single-segment
/// file (the t181 harvest's exact mistake). Must occur EXACTLY once (the
/// t181 evidence sentence says "one combined name" WITHOUT the token).
const COMBINED_NAME_BAN: &str = "combined `impl-validate` name";

/// Harvest evidence (1/2): the t183 impl child's two lost glm segments.
/// Must occur EXACTLY once.
const T183_HARVEST_LOSS: &str = "the t183 impl child's two glm segments";

/// Harvest evidence (2/2): the t181 impl child's lost glm segment. Must
/// occur EXACTLY once.
const T181_HARVEST_LOSS: &str = "t181 impl child's glm segment";

/// The removal precondition's subject — NO child pid launched in that
/// worktree still alive. Must occur EXACTLY once.
const LIVE_PID_PRECONDITION: &str = "NO child pid launched in that worktree";

/// The sanctioned liveness sources, paired. Must occur EXACTLY once
/// (step 2's polling paragraph names `delegate status` separately; this
/// longer needle is the step-5 pairing with `kill -0`).
const LIVENESS_SOURCE: &str = "liveness comes from `delegate status` or";

/// The kill-based liveness test. Must occur EXACTLY once (the
/// defunct-zombie caveat repeats bare `kill -0`, not the `<pid>` form).
const KILL_ZERO_PID: &str = "kill -0 <pid>";

/// The truncated-ps ban — the cycle-84 seg-1 failure mode. Must occur
/// EXACTLY once (`\u{2026}` = the ellipsis, spelled as an escape so an
/// editor normalization cannot silently unpin it, the DENSITY_NEEDLE
/// idiom).
const TRUNCATED_PS_BAN: &str = concat!("truncated `ps \u{2026} | head` read");

/// The zombie-collision evidence: the t183 validator's pid, named. Must
/// occur EXACTLY once.
const PID_6260: &str = "pid 6260";

/// The defunct-zombie caveat's exact check — a bare ps hit must also
/// read the state column. Must occur EXACTLY once.
const ZOMBIE_STAT_CHECK: &str = "-o stat=";

/// The defunct-zombie state the check must exclude. Must occur EXACTLY
/// once.
const SHOWING_Z: &str = "showing `Z`";

/// The defunct-zombie state's name. Must occur EXACTLY once (the
/// cycle-85 inverse sentence repeats the bare "defunct" stem — this
/// needle is the hyphenated state name).
const DEFUNCT_ZOMBIE: &str = "defunct-zombie";

/// The cycle-85 inverse incident tag — two defunct validator pids read
/// ALIVE on a bare ps -p moments before a safe removal. Must occur
/// EXACTLY once.
const CYCLE85_INVERSE: &str = "cycle-85 inverse";

/// The merge sentence the removal precondition must precede — the whole
/// point is that no remove (and no merge) happens while a child lives.
/// Kept line-safe ("Only then merge to" — the spec wraps after `to`).
/// Must occur EXACTLY once.
const ONLY_THEN_MERGE: &str = "Only then merge to";

/// The Phase-3 wrap bullet's pointer at the harvest-all doctrine. Must
/// occur EXACTLY once in LOOP-SPEC.md (step 5 itself spells the doctrine
/// out without the hyphenated token).
const WRAP_HARVEST_ALL: &str = "step 5's harvest-all";

/// The heading that closes the wrap-bullet window (the window OPENS at
/// the pre-existing T175 `PHASE3_HEADING` const above — reused, not
/// redefined).
const HARD_RULES_HEADING: &str = "## Hard rules";

/// (aa) T186 — the harvest-all doctrine: ALL of the worktree's
/// `.chug/events*.jsonl` files (the live stream AND every rotated
/// segment), ONE harvested file per source file, each named per the run
/// segment(s) it ACTUALLY contains (never a combined `impl-validate`
/// name for a single-segment file), with the T10/T7 rotation mechanism
/// and the t183/t181 losses named. Every load-bearing token occurs
/// EXACTLY once in LOOP-SPEC.md, inside step 5's window (the T64
/// loose-heading scope pattern, reusing the T120 step-5 anchors).
/// Revert the harvest-all sentence (e.g. back to "harvest every child
/// run's `.chug/events.jsonl`") and the glob + shape needles drop to 0 —
/// this leg goes red; duplicate a token and the count-2 leg fires; move
/// the doctrine out of step 5 and the window leg fires.
#[test]
fn harvest_all_needles_occur_exactly_once_inside_step_5() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        HARVEST_ALL_GLOB.starts_with(".chug/events") && HARVEST_ALL_GLOB.ends_with("*.jsonl"),
        "the harvest-all glob must name the worktree's `.chug/` events \
         streams with the wildcard"
    );
    assert!(
        ROTATION_TAG == "T10/T7" && COMBINED_NAME_BAN.contains("impl-validate"),
        "the rotation tag and the combined-name ban must carry their \
         load-bearing tokens verbatim"
    );
    let spec = loop_spec();
    let tokens: [(&str, &str); 7] = [
        (HARVEST_ALL_GLOB, "the harvest-all glob"),
        (ONE_FILE_PER_SEGMENT, "the one-file-per-source-file shape"),
        (ROTATION_TAG, "the T10/T7 rotation tag"),
        (RUN_START_INSPECT, "the run_start inspect token"),
        (COMBINED_NAME_BAN, "the combined-name ban"),
        (T183_HARVEST_LOSS, "the t183 harvest-loss evidence"),
        (T181_HARVEST_LOSS, "the t181 harvest-loss evidence"),
    ];
    for (needle, what) in tokens {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             harvest-all doctrine was reverted (or the needle was \
             rewrapped across a line break, which also breaks the spec's \
             own line-wise grep), more than one means it is stated twice"
        );
    }
    let start = spec
        .find(STEP5_HEADING_LOOSE)
        .expect("step-5 heading (`5. **`) present");
    let end = start
        + spec[start..]
            .find(STEP6_HEADING_LOOSE)
            .expect("step-6 heading present after step 5's");
    let window = &spec[start..end];
    for (needle, what) in tokens {
        assert!(
            window.contains(needle),
            "step 5's window must carry {what} ({needle:?}) — the \
             harvest-all doctrine was deleted or moved out of step 5"
        );
    }
}

/// (ab) T186 — the live-child removal precondition: before ANY
/// `git worktree remove`, verify NO child pid launched in that worktree
/// is still alive — liveness comes from `delegate status` or
/// `kill -0 <pid>`, NEVER a truncated `ps … | head` read — and a bare
/// `ps -p <pid>` / `kill -0` hit must ALSO exclude the defunct-zombie
/// state (`ps -p <pid> -o stat=` showing `Z`, which satisfies kill -0
/// while holding no files). Every load-bearing token occurs EXACTLY once
/// in LOOP-SPEC.md, inside step 5's window, and the precondition sits
/// AFTER the harvest glob it extends and BEFORE the `Only then merge to
/// main` sentence it guards (the ordering IS the doctrine: harvest what
/// exists, prove no live child, only then remove + merge). Revert the
/// precondition sentence and every count drops to 0 — this leg goes red;
/// move it after the merge sentence and the ordering leg fires; the
/// pid 6260 + cycle-85 evidence tokens keep both incidents (the
/// safe-direction miss and the exactness miss) named.
#[test]
fn removal_precondition_exactly_once_inside_step_5_before_merge() {
    // Needle self-check (T48 idiom).
    assert!(
        TRUNCATED_PS_BAN.starts_with("truncated `ps")
            && TRUNCATED_PS_BAN.ends_with("read")
            && KILL_ZERO_PID.starts_with("kill -0"),
        "the truncated-ps ban and the kill -0 needle must carry their \
         liveness language verbatim"
    );
    let spec = loop_spec();
    let tokens: [(&str, &str); 10] = [
        (LIVE_PID_PRECONDITION, "the live-child removal precondition"),
        (LIVENESS_SOURCE, "the sanctioned liveness sources"),
        (KILL_ZERO_PID, "the kill -0 <pid> liveness test"),
        (TRUNCATED_PS_BAN, "the truncated `ps … | head` ban"),
        (PID_6260, "the pid 6260 zombie evidence"),
        (ZOMBIE_STAT_CHECK, "the -o stat= state-column check"),
        (SHOWING_Z, "the `Z` defunct state"),
        (DEFUNCT_ZOMBIE, "the defunct-zombie state name"),
        (CYCLE85_INVERSE, "the cycle-85 inverse evidence"),
        (ONLY_THEN_MERGE, "the Only-then-merge sentence"),
    ];
    for (needle, what) in tokens {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             removal precondition was reverted (or the needle was \
             rewrapped across a line break), more than one means it is \
             stated twice"
        );
    }
    let start = spec
        .find(STEP5_HEADING_LOOSE)
        .expect("step-5 heading (`5. **`) present");
    let end = start
        + spec[start..]
            .find(STEP6_HEADING_LOOSE)
            .expect("step-6 heading present after step 5's");
    let window = &spec[start..end];
    for (needle, what) in tokens {
        assert!(
            window.contains(needle),
            "step 5's window must carry {what} ({needle:?}) — the \
             removal precondition was deleted or moved out of step 5"
        );
    }
    // The ordering IS the doctrine: harvest-all first, then the live-child
    // check, and only then the merge sentence the precondition guards.
    let harvest = window
        .find(HARVEST_ALL_GLOB)
        .expect("step-5 window must carry the harvest-all glob (leg (aa))");
    let precondition = window
        .find(LIVE_PID_PRECONDITION)
        .expect("step-5 window must carry the removal precondition");
    let merge = window
        .find(ONLY_THEN_MERGE)
        .expect("step-5 window must carry the merge sentence");
    assert!(
        harvest < precondition && precondition < merge,
        "the removal precondition must sit AFTER the harvest-all glob \
         ({harvest}) and BEFORE the merge sentence ({merge}) — found at \
         {precondition}: harvesting precedes proving no live child, and \
         proving precedes merging"
    );
}

/// (ac) T186 — the Phase-3 wrap bullet matches the doctrine (harvest =
/// ALL segments): the bullet now points at §2 step 5's harvest-all
/// instead of implying one file per item. The pointer occurs EXACTLY
/// once in LOOP-SPEC.md, inside the Phase-3 window (the `## Phase 3 —
/// Wrap` heading through `## Hard rules`). Revert the bullet to the
/// one-file reading and the needle drops to 0 — this leg goes red;
/// duplicate it and the count-2 leg fires.
#[test]
fn wrap_bullet_names_the_harvest_all_doctrine_inside_phase_3() {
    // Needle self-check (T48 idiom).
    assert!(
        WRAP_HARVEST_ALL.starts_with("step 5's") && WRAP_HARVEST_ALL.ends_with("harvest-all"),
        "the wrap-bullet pointer must name §2 step 5's harvest-all \
         doctrine verbatim"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(WRAP_HARVEST_ALL).count(),
        1,
        "LOOP-SPEC must state the wrap bullet's harvest-all pointer \
         exactly once — zero means the Phase-3 bullet was reverted to \
         the one-file reading, more than one means it is stated twice"
    );
    let start = spec
        .find(PHASE3_HEADING)
        .expect("Phase-3 heading present");
    let end = start
        + spec[start..]
            .find(HARD_RULES_HEADING)
            .expect("Hard-rules heading present after Phase 3's");
    let window = &spec[start..end];
    assert!(
        window.contains(WRAP_HARVEST_ALL),
        "the Phase-3 window must carry {WRAP_HARVEST_ALL:?} — the \
         harvest bullet drifted out of the wrap section"
    );
}


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
//!
//! T207 doctrine: cycle-94 seg-3 (glm) ran its orchestrator to the
//! 200/200 iteration ceiling over 5h01m and DIED at the ceiling with the
//! work done but the wrap unwritten — T204's merge (dbce0e2) was
//! committed, but the row flip, the push (18 commits sat unpushed), the
//! Outcomes entry, the harvest, and the decision records were all
//! missing, and the ledger at death was ~150 iterations stale (it named
//! an impl pid from the arc's middle); cycle-95 spent ~6 iterations
//! reconstructing the true state from the git record before any
//! productive work. This is the T10/T12 loss class one level up —
//! children die between the code commit and the row flip, and so do
//! orchestrators. Root cause: step 6's stop-dispatch margin ("fewer than
//! 15 iterations left") was calibrated when orchestrator budgets were 120
//! iterations; loopd now launches 200 iterations / 360 minutes, and the
//! wrap tail (final gates ~5–10 min wall + row flips + Outcomes +
//! harvest + release check + push + goal gate) measures ~15–25
//! iterations — a 15-iteration margin at 200 is structurally too thin,
//! and seg-3 dispatched a validation round inside the margin and never
//! came back. Two doctrine edits, pinned by legs (af)–(ag) in the
//! T48/T64 pattern: (1) the step-6 threshold is raised to 30 WITH the
//! calibration sentence that sizes it (the number is sized to the wrap
//! tail's measured cost at the 200-iteration orchestrator budget), and
//! the OLD 15-threshold phrasing is asserted ZERO times (the
//! BARE_CLIPPY_PHRASE revert-detector idiom); (2) Phase 3 gains a hard
//! rule — crossing the stop-dispatch boundary makes the orchestrator's
//! NEXT ledger write carry a wrap-state note naming every
//! merged-but-unflipped row, every unharvested worktree, every unpushed
//! commit count, and every missing decision record, so a mid-wrap death
//! is recoverable from the ledger alone with zero git reconstruction.
//! The T81 anti-sprint-burn guard is untouched by this row — a wrap IS
//! an act, and its own pin lives in tests/loopd_model_routing.rs.

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

// ---- T235 — the orchestrator gate surfaces pin the exact clippy form ----
//
// The cycle-107 wrap's final gates ran `cargo clippy --all-targets -- -D
// warnings` in main and went RED on an unused-`mut` in T233's NEW cfg(test)
// code (src/daemon.rs's test module; the 1-keyword fix landed at 61b0f5e) —
// and BOTH the t233 impl child AND its kimi validator had passed the same
// code. The escape path, verified at cycle-108 eval time (I1): spec check
// lines carry no clippy leg at all (the goal gate never lints), nextest
// compiles cfg(test) code but never LINTS it, the impl goal's short-form
// sentence is "clippy `-D warnings`" (a child can — and the t233 child did
// — run a narrower form that never compiles cfg(test) code), and the
// orchestrator's OWN gate surfaces named clippy with NO pinned form —
// whether cfg(test) got linted pre-merge was orchestrator discretion, not
// doctrine. T235 pins the exact form at all three gate surfaces: step 3's
// review gates (the systematic PRE-merge catch, the row's point), step 5's
// post-merge re-run (backstop), and Phase 3's final gates (the catch that
// fired in cycle 107, now guaranteed rather than chosen). The check-line
// surface is deliberately UNCHANGED — spec check lines stay test-only (the
// goal gate's job is test-green; lint is these gates' job) — and the impl
// goal template is untouched (T176's prose already demands the form).
// T253 (cycle 131) keyed the form to the RELEASE profile at all four
// surfaces — every gate surface runs the one profile the loop warms
// (T78/T82), so the dev-profile form was a cross-profile cold-scale leg
// waiting to fire (the cycle-130 t251-impl kill); the count stays 4 and
// the needle self-check carries the new exact form.

/// The exact clippy form every orchestrator gate surface must name — the
/// RELEASE-profile form (T253): `--release` between `--all-targets` and
/// the lint level, exactly as the gates run it. Must occur EXACTLY once
/// inside EACH of the three gate windows and exactly FOUR times in
/// LOOP-SPEC.md overall — the three gate surfaces plus the T176 prose
/// paragraph's pre-existing mention in step 2's window.
const CLIPPY_ALL_TARGETS_FORM: &str = "cargo clippy --all-targets --release -- -D warnings";

/// Phase 3's final-gates bullet anchor — the form must sit after it inside
/// the Phase-3 window (the bullet is where the cycle-107 catch fired).
const FINAL_GATES_BULLET: &str = "Final gates green in main";

/// (aa) T235 — the three orchestrator gate surfaces each name the exact
/// clippy form, and the file-wide count matches so a fourth-surface
/// mention OR a silent removal both go red. Each surface is located by its
/// existing stable anchors (the pin style this file already uses): step
/// 3's review paragraph (window `3. **Review.**` .. `4. **`), step 5's
/// post-merge gate text (window `5. **` .. `6. **`), and Phase 3's
/// final-gates bullet (window `## Phase 3 — Wrap` .. `## Hard rules`, the
/// form AFTER the bullet anchor). Revert any surface's amendment and its
/// window find (or the count) goes red; add an unpinned fourth mention and
/// the count-4 leg fires; remove the T176 prose mention and the count
/// drops to 3.
#[test]
fn orchestrator_gate_surfaces_pin_the_exact_clippy_form() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert_eq!(
        CLIPPY_ALL_TARGETS_FORM, "cargo clippy --all-targets --release -- -D warnings",
        "the needle must be the exact --all-targets --release form verbatim \
         (T253: the release profile is the one the loop's gates warm)"
    );

    let spec = loop_spec();

    // The file-wide count: three gate surfaces + the pre-existing T176
    // prose mention = 4. A fourth unpinned surface (count 5) and a silent
    // removal from any surface (count 3, or a window find below dying)
    // both go red.
    assert_eq!(
        spec.matches(CLIPPY_ALL_TARGETS_FORM).count(),
        4,
        "LOOP-SPEC must carry the exact clippy form exactly four times — \
         the three orchestrator gate surfaces (step 3 review, step 5 \
         post-merge, Phase 3 final gates) plus the T176 prose paragraph's \
         pre-existing mention; more means an unpinned fourth surface \
         appeared, fewer means one was removed"
    );

    // Surface (a): step 3's review-gate paragraph — the systematic
    // PRE-merge catch.
    let s3 = spec
        .find(STEP3_HEADING)
        .expect("step-3 heading (`3. **Review.**`) present");
    let s3_end = s3
        + spec[s3..]
            .find(STEP4_HEADING_LOOSE)
            .expect("step-4 heading present after step 3's");
    assert_eq!(
        spec[s3..s3_end].matches(CLIPPY_ALL_TARGETS_FORM).count(),
        1,
        "step 3's review-gate paragraph must name the exact clippy form \
         exactly once — zero means the pre-merge catch was reverted, more \
         than one means the statement drifted or was duplicated"
    );

    // Surface (b): step 5's post-merge re-run — the backstop.
    let s5 = spec
        .find(STEP5_HEADING_LOOSE)
        .expect("step-5 heading (`5. **`) present");
    let s5_end = s5
        + spec[s5..]
            .find(STEP6_HEADING_LOOSE)
            .expect("step-6 heading present after step 5's");
    assert_eq!(
        spec[s5..s5_end].matches(CLIPPY_ALL_TARGETS_FORM).count(),
        1,
        "step 5's post-merge gate text must name the exact clippy form \
         exactly once — zero means the backstop was reverted, more than \
         one means the statement drifted or was duplicated"
    );

    // Surface (c): Phase 3's final-gates bullet — the catch that fired in
    // cycle 107, now guaranteed rather than chosen. The form must sit
    // AFTER the bullet anchor (the final-gates sentence itself, not some
    // other Phase-3 paragraph).
    let p3 = spec.find(PHASE3_HEADING).expect("Phase-3 heading present");
    let p3_end = p3
        + spec[p3..]
            .find(HARD_RULES_HEADING)
            .expect("Hard-rules heading present after Phase 3's");
    let window = &spec[p3..p3_end];
    let bullet = window
        .find(FINAL_GATES_BULLET)
        .expect("Phase-3 window must carry the final-gates bullet anchor");
    let form_in_p3 = window[bullet..]
        .find(CLIPPY_ALL_TARGETS_FORM)
        .unwrap_or_else(|| {
            panic!(
                "Phase 3's final-gates bullet must name the exact clippy \
                 form after the bullet anchor — the cycle-107 catch is no \
                 longer orchestrator discretion"
            )
        });
    assert_eq!(
        window.matches(CLIPPY_ALL_TARGETS_FORM).count(),
        1,
        "Phase 3 must carry the exact clippy form exactly once (at the \
         final-gates bullet, {form_in_p3} bytes after its anchor) — more \
         than one means an unpinned Phase-3 mention drifted in"
    );
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
const TRUNCATED_PS_BAN: &str = "truncated `ps \u{2026} | head` read";

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

// ---- T196 — the impl goal template bans tree-wide formatters ----
//
// The T188 fmt-noise class: cycle 87's t188 round-3 glm impl child ran a
// mass `cargo fmt` across the whole repo — 83 files, +12.6k/−9.2k
// uncommitted fmt noise on top of the real change — and the T63 resume had
// to spend its first act stripping the noise to the 775 real insertions
// before the fix-up could proceed (the diff was unreviewable until
// stripped). The template said nothing about formatting, so a child that
// decided to "tidy up" had license. The template now carries ONE ban
// sentence, woven into the goal text: never run tree-wide formatters —
// format nothing you did not rewrite. Step 4's FAIL-arc fix-up child has
// no goal text of its own (its goal is step 2's with the findings pasted
// in — the same one-template-covers-fix-ups fact the T176 pin records), so
// this one edit covers both.
//
// The pin (T48/T64/T176 pattern): the ban's two load-bearing tokens occur
// EXACTLY once in LOOP-SPEC.md, both inside step 2's window, the ban
// sitting between the clippy-bar sentence it extends and the `Commit your
// work here.` sentence it precedes.

/// The ban's subject — TREE-WIDE formatting is the banned act (a per-file
/// format of a file you rewrote is not). Must occur EXACTLY once in
/// LOOP-SPEC.md.
const FMT_BAN_TREE_WIDE: &str = "tree-wide";

/// The ban's named tool. Must occur EXACTLY once in LOOP-SPEC.md.
const FMT_BAN_TOOL: &str = "cargo fmt";

/// (ad) T196 — the impl goal template bans tree-wide formatters: the
/// ban's two load-bearing tokens occur EXACTLY once in LOOP-SPEC.md, both
/// inside step 2's window (the T64 loose-heading scope), and the ban
/// sentence sits BETWEEN the clippy-bar sentence it extends and the
/// `Commit your work here.` sentence it precedes (the template's sentence
/// order preserved). Revert the ban sentence and BOTH tokens drop to 0 —
/// this leg goes red (the deletion proof is in the commit message);
/// state the ban twice and the count-2 leg fires; move it out of step 2
/// or reorder it past the commitment sentence and the window/ordering
/// legs fire.
#[test]
fn impl_goal_template_bans_tree_wide_formatters() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        FMT_BAN_TREE_WIDE.starts_with("tree-") && FMT_BAN_TREE_WIDE.ends_with("wide"),
        "the subject needle must be the hyphenated `tree-wide` form"
    );
    assert_eq!(FMT_BAN_TOOL, "cargo fmt");

    let spec = loop_spec();
    let tokens: [(&str, &str); 2] = [
        (FMT_BAN_TREE_WIDE, "the tree-wide formatter ban"),
        (FMT_BAN_TOOL, "the `cargo fmt` ban"),
    ];
    for (needle, what) in tokens {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             T196 ban sentence was deleted, more than one means it is \
             stated twice (the T156 text-revert mutant class)"
        );
    }

    // Scope + order: inside step 2's window, after the clippy-bar sentence
    // the ban extends and before the commitment sentence it precedes.
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    for (needle, what) in tokens {
        assert!(
            window.contains(needle),
            "step 2's window must carry {what} ({needle:?}) — the ban \
             drifted out of the impl goal template"
        );
    }
    let clippy = window
        .find(CLIPPY_DENY_SENTENCE)
        .expect("step-2 window must carry the clippy-bar sentence (T176, untouched)");
    let ban = window
        .find(FMT_BAN_TREE_WIDE)
        .expect("step-2 window must carry the fmt ban (count leg above)");
    let here = window
        .find(COMMIT_HERE)
        .expect("step-2 window must carry `Commit your work here.` (untouched)");
    assert!(
        clippy < ban && ban < here,
        "the fmt ban must sit between the clippy-bar sentence ({clippy}) \
         and the `Commit your work here.` sentence ({here}) — the \
         template's sentence order preserved (found at {ban})"
    );
}

// ---- T201 — the validator verdict-first doctrine ----
//
// The unannounced-verdict class: a validator that reaches its verdict but
// dies before announcing it burns a recovery cycle — t185-validate (50-min
// wall, verdict recovered via T63 resume), t188-round4 (60/60, verdict
// recovered from transcript), t192-validate (60/60, verdict recovered from
// the log — the 4th of the class), plus the t186 post-goal_complete wedge
// (14 min silent, verdict fully rendered in its log). Every instance cost
// a transcript-archaeology pass or a resume (~10–20 min) because the
// verdict existed ONLY inside the dying child's context. One sentence
// kills the class for every death mode: the moment the verdict is DECIDED
// it lands in `.chug/verdict.md` — a budget death, a wedge, a SIGKILL: the
// verdict is already on disk. META-SPEC §6's template gains the
// write-first half; LOOP-SPEC step 4 gains the recovery half (read
// verdict.md BEFORE spending a T63 resume — a written verdict IS the
// verdict).
//
// One pin covers both carriers: `verdict.md` occurs EXACTLY once in EACH
// file, inside the §6 Validate window (STEP6_VALIDATE_HEADING ..
// STEP7_HEADING, the T126 anchors) and inside step 4's window
// respectively, and in META-SPEC the write sentence precedes the
// `End with a verdict line` sentence it front-runs.

/// The verdict artifact's path token — the file a dying validator's
/// verdict must already sit in. Must occur EXACTLY once in META-SPEC.md
/// (the §6 write-first sentence) AND exactly once in LOOP-SPEC.md (step
/// 4's recovery sentence) — one carrier each, no forks.
const VERDICT_FILE: &str = "verdict.md";

/// (ae) T201 — verdict-first: `verdict.md` occurs EXACTLY once in
/// META-SPEC.md (the §6 template's write-first sentence) AND exactly once
/// in LOOP-SPEC.md (step 4's recovery sentence), each inside its carrier
/// window, and the §6 write sentence sits BEFORE the `End with a verdict
/// line` sentence it front-runs. Delete EITHER sentence and its file's
/// count drops to 0 — this leg goes red (the deletion proof is in the
/// commit message); state either sentence twice and the count-2 leg
/// fires; move a sentence out of its window and the window leg fires.
#[test]
fn validator_verdict_first_file_named_once_in_each_spec() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert_eq!(
        VERDICT_FILE, "verdict.md",
        "the needle must be the bare verdict.md token verbatim"
    );

    let meta = meta_spec();
    let spec = loop_spec();
    let meta_count = meta.matches(VERDICT_FILE).count();
    let spec_count = spec.matches(VERDICT_FILE).count();
    assert_eq!(
        meta_count, 1,
        "META-SPEC must name .chug/verdict.md exactly once — zero means \
         the §6 verdict-first sentence was deleted, more than one means \
         the write rule is stated twice"
    );
    assert_eq!(
        spec_count, 1,
        "LOOP-SPEC must name .chug/verdict.md exactly once — zero means \
         the step-4 recovery sentence was deleted, more than one means it \
         is stated twice"
    );

    // Scope: the write sentence lives inside §6's Validate window; the
    // recovery sentence inside step 4's window.
    let mstart = meta
        .find(STEP6_VALIDATE_HEADING)
        .expect("META-SPEC §6 Validate heading present");
    let mend = mstart
        + meta[mstart..]
            .find(STEP7_HEADING)
            .expect("META-SPEC step-7 heading present after §6's");
    assert!(
        meta[mstart..mend].contains(VERDICT_FILE),
        "META-SPEC §6's Validate window must carry {VERDICT_FILE:?} — the \
         write-first sentence drifted out of the validator goal template"
    );
    let sstart = spec
        .find(STEP4_HEADING_LOOSE)
        .expect("step-4 heading (`4. **`) present");
    let send = sstart
        + spec[sstart..]
            .find(STEP5_HEADING_LOOSE)
            .expect("step-5 heading present after step 4's");
    assert!(
        spec[sstart..send].contains(VERDICT_FILE),
        "step 4's window must carry {VERDICT_FILE:?} — the recovery \
         sentence drifted out of the FAIL-arc doctrine"
    );

    // Order: the write-first sentence precedes the verdict-line sentence
    // it front-runs (write FIRST, then the wrap-up summary line).
    let write = meta[mstart..mend]
        .find(VERDICT_FILE)
        .expect("§6 window carries the write sentence (leg above)");
    let end_line = meta[mstart..mend]
        .find("End with a verdict line")
        .expect("§6 window carries the `End with a verdict line` sentence");
    assert!(
        write < end_line,
        "the verdict-first sentence must precede `End with a verdict \
         line` — the write happens FIRST, the summary line after ({write} \
         vs {end_line})"
    );
}

// ---- T207 — the orchestrator wrap window: step-6 threshold 15 → 30 + the wrap-state note ----
//
// Cycle-94 seg-3 (glm, 5h01m) ran its orchestrator to the 200/200
// iteration ceiling and died with the work done but the wrap unwritten:
// T204's merge (dbce0e2) was committed, but the row flip, the push (18
// commits sat unpushed), the Outcomes entry, the harvest, and the decision
// records were all missing, and the ledger at death was ~150 iterations
// stale (it named a mid-arc impl pid); cycle-95 spent ~6 iterations
// reconstructing the true state from the git record before any productive
// work. This is the T10/T12 loss class one level up — children die between
// the code commit and the row flip, and so do orchestrators. Root cause:
// step 6's stop-dispatch margin (15 iterations) was calibrated when
// orchestrator budgets were 120 iterations; loopd now launches 200
// iterations / 360 minutes, and the wrap tail (final gates ~5–10 min wall
// + row flips + Outcomes + harvest + release check + push + goal gate)
// measures ~15–25 iterations — a 15-iteration margin at 200 is
// structurally too thin, and seg-3 dispatched a validation round inside
// the margin and never came back. Legs (af)–(ag) pin both doctrine edits
// in the T48/T64 pattern: every needle exactly-once in LOOP-SPEC.md and
// inside its window; the OLD threshold phrasing asserted ZERO times (the
// BARE_CLIPPY_PHRASE revert-detector idiom). The T81 anti-sprint-burn
// guard is untouched — a wrap IS an act, and its own pin lives in
// tests/loopd_model_routing.rs.

/// The raised threshold's commitment needle — verbatim per the spec's
/// line-wise check: grep. Must occur EXACTLY once in LOOP-SPEC.md.
const WRAP_WINDOW_THRESHOLD: &str = "Fewer than 30 iterations left";

/// The OLD threshold phrasing — the revert detector (the BARE_CLIPPY_PHRASE
/// idiom). Must occur ZERO times in LOOP-SPEC.md after the raise: a revert
/// to the seg-3-death margin (the exact T207 fix undone) resurrects it and
/// the leg goes red.
const OLD_WRAP_THRESHOLD: &str = "Fewer than 15 iterations left";

/// The calibration sentence's wrap tail — the cost the threshold is sized
/// against. Must occur EXACTLY once.
const WRAP_TAIL_NEEDLE: &str = "wrap tail";

/// The calibration sentence's measured range — the wrap tail's
/// ~15–25 iteration cost, en dash spelled as an escape so an editor
/// normalization cannot silently unpin it (\u{2013} = en dash, the
/// DENSITY_NEEDLE idiom). Must occur EXACTLY once.
const MEASURED_RANGE_NEEDLE: &str = "~15\u{2013}25 iteration cost";

/// The calibration sentence's orchestrator-budget token — the 200 / 360
/// pair loopd launches at (loopd.sh's `--max-iters 200 --max-minutes 360`).
/// Must occur EXACTLY once.
const ORCH_BUDGET_NEEDLE: &str = "200-iteration / 360-minute";

/// The calibration sentence's death evidence — the seg-3 ceiling death
/// with the merge committed and the wrap unwritten. Must occur EXACTLY
/// once.
const SEG3_DEATH_NEEDLE: &str = "cycle-94 seg-3 died 200/200";

/// The calibration sentence's failure mode — the validation round
/// dispatched inside the margin that never came back. Must occur EXACTLY
/// once.
const MARGIN_DISPATCH_NEEDLE: &str = "dispatching a validation round inside the margin";

/// (af) T207 — step 6's stop-dispatch threshold is 30, WITH its
/// calibration: the raised threshold occurs EXACTLY once in LOOP-SPEC.md
/// inside step 6's window (the T64 loose-heading scope: step 6's heading
/// through the Phase-3 heading), the OLD 15-threshold phrasing occurs ZERO
/// times anywhere in the file, and the calibration sentence's tokens — the
/// wrap tail the number is sized against, the measured ~15–25 iteration
/// range, the 200-iteration / 360-minute orchestrator budget loopd now
/// launches, the seg-3 ceiling death, and the margin-dispatch failure
/// mode — each occur EXACTLY once, AFTER the threshold they calibrate.
/// Reverting the threshold sentence (the text-revert mutant) drops the
/// raised needle to 0 AND resurrects the old one — TWO legs of this file
/// go red at once; moving the calibration out of step 6 dies on the
/// window find.
#[test]
fn wrap_window_threshold_is_30_with_calibration_inside_step_6() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        WRAP_WINDOW_THRESHOLD.starts_with("Fewer than 30")
            && WRAP_WINDOW_THRESHOLD.ends_with("left"),
        "the threshold needle must carry the raised 30 threshold verbatim"
    );
    assert!(
        OLD_WRAP_THRESHOLD.starts_with("Fewer than 15") && OLD_WRAP_THRESHOLD.ends_with("left"),
        "the revert detector must be the OLD 15 threshold phrasing verbatim"
    );
    assert!(
        MEASURED_RANGE_NEEDLE.starts_with("~15")
            && MEASURED_RANGE_NEEDLE.ends_with("iteration cost"),
        "the measured-range needle must carry the ~15–25 iteration cost \
         verbatim, en dash included"
    );
    assert!(
        ORCH_BUDGET_NEEDLE.starts_with("200-iteration")
            && ORCH_BUDGET_NEEDLE.ends_with("360-minute"),
        "the budget needle must carry the 200-iteration / 360-minute pair \
         verbatim"
    );
    let spec = loop_spec();

    // The raised threshold, exactly once; the old one, zero times.
    assert_eq!(
        spec.matches(WRAP_WINDOW_THRESHOLD).count(),
        1,
        "LOOP-SPEC step 6 must state the 30-iteration stop-dispatch \
         threshold exactly once — zero means the threshold sentence was \
         deleted (or rewrapped across a line break, which also breaks the \
         spec's own line-wise grep), more than one means it is stated twice"
    );
    assert_eq!(
        spec.matches(OLD_WRAP_THRESHOLD).count(),
        0,
        "the OLD 15-iteration threshold phrasing must not survive anywhere \
         in LOOP-SPEC — the pre-T207 step 6 was reverted (the exact \
         cycle-94 seg-3 death class), or a second budget check \
         reintroduced the old number"
    );

    // Scope + order: inside step 6's window, calibration AFTER threshold.
    let start = spec
        .find(STEP6_HEADING_LOOSE)
        .expect("step-6 heading (`6. **`) present");
    let end = start
        + spec[start..]
            .find(PHASE3_HEADING)
            .expect("the Phase-3 heading present after step 6's");
    let window = &spec[start..end];
    let threshold = window
        .find(WRAP_WINDOW_THRESHOLD)
        .expect("step-6 window must carry the raised threshold (moved out of step 6?)");
    for (needle, what) in [
        (WRAP_TAIL_NEEDLE, "the wrap tail the threshold is sized against"),
        (MEASURED_RANGE_NEEDLE, "the measured ~15–25 iteration cost"),
        (
            ORCH_BUDGET_NEEDLE,
            "the 200-iteration / 360-minute orchestrator budget",
        ),
        (SEG3_DEATH_NEEDLE, "the cycle-94 seg-3 ceiling-death evidence"),
        (MARGIN_DISPATCH_NEEDLE, "the margin-dispatch failure mode"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             calibration sentence was deleted (or a needle was rewrapped \
             across a line break), more than one means it is stated twice"
        );
        let at = window.find(needle).unwrap_or_else(|| {
            panic!(
                "step 6's window must carry {what} ({needle:?}) — the \
                 calibration drifted out of the budget check"
            )
        });
        assert!(
            threshold < at,
            "the calibration must FOLLOW the threshold it sizes — threshold \
             ({threshold}), then {what} ({at})"
        );
    }
}

/// The wrap-state rule's tokens — the hard rule's bold lead, the boundary
/// link to step 6, the NEXT-ledger-write commitment, the four named items
/// in the order the rule lists them, the recoverability claim, and the
/// seg-3/cycle-95 evidence. Each contiguous as written and EXACTLY once
/// in LOOP-SPEC.md.
const WRAP_STATE_LEAD: &str = "Wrap-state note at the boundary (T207";
const BOUNDARY_LINK: &str = "stop-dispatch boundary (step 6)";
const WRAP_STATE_COMMITMENT: &str = "NEXT ledger write must carry a wrap-state note";
const UNFLIPPED_ROWS_NEEDLE: &str = "merged-but-unflipped row";
const UNHARVESTED_NEEDLE: &str = "unharvested worktree";
const UNPUSHED_NEEDLE: &str = "unpushed commit count";
const MISSING_RECORDS_NEEDLE: &str = "missing decision record";
const LEDGER_RECOVERABLE_NEEDLE: &str = "zero git reconstruction";
const STALE_LEDGER_EVIDENCE: &str = "seg-3 death left a ledger ~150";
const CYCLE95_COST_NEEDLE: &str = "cycle-95";

/// (ag) T207 — Phase 3's wrap-state-note hard rule: the rule's trigger
/// (crossing the stop-dispatch boundary, linked to step 6), its one-write
/// commitment, its four named items in the written order, the
/// recoverability claim, and the stale-ledger / cycle-95 evidence — every
/// token EXACTLY once in LOOP-SPEC.md and inside the Phase-3 window (the
/// `## Phase 3 — Wrap` heading through `## Hard rules`, the T186
/// anchors), with the rule as the window's FIRST bullet (before the
/// TODO.md-truthful duty it precedes — the note is due before any other
/// wrap duty). Delete the rule — the text-revert mutant — and every token
/// drops to 0 and this leg goes red; duplicate a token and the count-2
/// leg fires; move the rule out of Phase 3 (e.g. into step 6's budget
/// check, whose boundary the trigger names) and the window leg fires;
/// demote it below the TODO.md bullet and the first-bullet ordering
/// fires.
#[test]
fn wrap_state_note_rule_exactly_once_as_phase3_first_bullet() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        WRAP_STATE_LEAD.starts_with("Wrap-state note") && WRAP_STATE_LEAD.contains("T207"),
        "the lead needle must be the wrap-state bullet's bold lead verbatim"
    );
    assert!(
        WRAP_STATE_COMMITMENT.starts_with("NEXT")
            && WRAP_STATE_COMMITMENT.ends_with("wrap-state note"),
        "the commitment needle must carry the next-ledger-write commitment \
         verbatim"
    );
    assert!(
        UNFLIPPED_ROWS_NEEDLE.ends_with("row")
            && UNHARVESTED_NEEDLE.ends_with("worktree")
            && UNPUSHED_NEEDLE.ends_with("count")
            && MISSING_RECORDS_NEEDLE.ends_with("record"),
        "the four item needles must name the rule's four mandated \
         disclosures verbatim"
    );
    let spec = loop_spec();
    let tokens: [(&str, &str); 10] = [
        (WRAP_STATE_LEAD, "the wrap-state bullet's bold lead"),
        (BOUNDARY_LINK, "the boundary link to step 6"),
        (WRAP_STATE_COMMITMENT, "the NEXT-ledger-write commitment"),
        (UNFLIPPED_ROWS_NEEDLE, "the merged-but-unflipped-rows item"),
        (UNHARVESTED_NEEDLE, "the unharvested-worktrees item"),
        (UNPUSHED_NEEDLE, "the unpushed-commit-count item"),
        (MISSING_RECORDS_NEEDLE, "the missing-decision-records item"),
        (LEDGER_RECOVERABLE_NEEDLE, "the zero-git-reconstruction claim"),
        (STALE_LEDGER_EVIDENCE, "the stale-ledger evidence"),
        (CYCLE95_COST_NEEDLE, "the cycle-95 archaeology-cost evidence"),
    ];
    for (needle, what) in &tokens {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} ({needle:?}) exactly once — zero \
             means the wrap-state rule was deleted (or a needle was \
             rewrapped across a line break, which also breaks the spec's \
             own line-wise grep), more than one means it is stated twice"
        );
    }

    // Scope: the rule lives INSIDE Phase 3's window (the T186 anchors).
    let start = spec
        .find(PHASE3_HEADING)
        .expect("Phase-3 heading present");
    let end = start
        + spec[start..]
            .find(HARD_RULES_HEADING)
            .expect("Hard-rules heading present after Phase 3's");
    let window = &spec[start..end];
    for (needle, what) in &tokens {
        assert!(
            window.contains(needle),
            "the Phase-3 window must carry {what} ({needle:?}) — the \
             wrap-state rule drifted out of the wrap section"
        );
    }

    // The rule is the window's FIRST bullet, before the TODO.md-truthful
    // duty it precedes.
    let lead = window
        .find(WRAP_STATE_LEAD)
        .expect("window carries the wrap-state lead (count leg above)");
    let todo_duty = window
        .find("- TODO.md truthful")
        .expect("Phase-3 window must carry the TODO.md-truthful bullet (untouched)");
    assert!(
        lead < todo_duty,
        "the wrap-state rule must be Phase 3's FIRST bullet — the note is \
         due before the other wrap duties: lead ({lead}), TODO.md bullet \
         ({todo_duty})"
    );

    // The four named disclosures read in the written order.
    let unflipped = window
        .find(UNFLIPPED_ROWS_NEEDLE)
        .expect("window carries the unflipped-rows item (count leg above)");
    let unharvested = window
        .find(UNHARVESTED_NEEDLE)
        .expect("window carries the unharvested-worktrees item (count leg above)");
    let unpushed = window
        .find(UNPUSHED_NEEDLE)
        .expect("window carries the unpushed-commits item (count leg above)");
    let missing = window
        .find(MISSING_RECORDS_NEEDLE)
        .expect("window carries the missing-records item (count leg above)");
    assert!(
        unflipped < unharvested && unharvested < unpushed && unpushed < missing,
        "the four named disclosures must read in the written order — \
         unflipped rows ({unflipped}), unharvested worktrees \
         ({unharvested}), unpushed commits ({unpushed}), missing records \
         ({missing})"
    );
}

// ---- T209 — the dispatch-time spec-size gate (the T173 measure clause resolves) ----
//
// The T110 filing-time ~500-line estimate ceiling exists, but nothing
// enforced it AT DISPATCH, and the cycles-91–94 census says the cost is
// now the binding one: t197 impl died 50m29s at 54/80 (minutes-bound,
// goal unaccepted), t203 impl died 50m35s at 57/80 (minutes-bound), and
// the T204 arc lost 6 glm impl/fixup segments (iteration ×4, time ×2,
// stuck ×1) plus both kimi validators at 60/60 — 10 child segments, 9
// aborts, ZERO goal-accepted children, all on one row that filed at
// "~800" and landed ~3,000+ all-in across both halves (child B alone:
// ~2,100 added lines, 4× the ceiling, because the split was BY COMPONENT
// — inference core / daemon surface — not by acceptance surface). That
// trips LOOP-SPEC's T173 measure clause ("if >2 of the next 8 impl
// children still die at the 50-minute budget with the goal unaccepted,
// the next eval considers spec-size discipline instead of further
// raises"), and the remedy is a dispatch-time gate, NOT further budget
// raises. Legs (ah)–(ak) pin the doctrine edits in the T48/T64 pattern:
// every needle exactly-once, inside its step's window, in the written
// order — the gate BEFORE the launch template it gates, the
// by-acceptance-surface rule after the gate, the RESOLVED marker after
// the tripwire it resolves, and the META-META-SPEC dispatch-contract
// sentence in the spec-quality-bar window after the ~400 band it
// qualifies.

/// The dispatch gate's action needle — the orchestrator's re-read of the
/// spec's `estimate:` line, in one contiguous run, verbatim per the
/// spec's line-wise check: grep (so the LOOP-SPEC wrap must keep it on a
/// single line). Must occur EXACTLY once in LOOP-SPEC.md.
const DISPATCH_GATE_NEEDLE: &str = "re-reads the spec's `estimate:` line";

/// The by-acceptance-surface rule's needle — the split rule's core claim,
/// contiguous as written. Must occur EXACTLY once in LOOP-SPEC.md.
const SPLIT_RULE_NEEDLE: &str = "by acceptance surface, not by component";

/// The naming rule's needle — each split half's own independently
/// gateable `check:` surface, contiguous as written. Must occur EXACTLY
/// once in LOOP-SPEC.md.
const NAMING_RULE_NEEDLE: &str = "independently-gateable `check:` surface";

/// The launch template's opening line — the gate must sit BEFORE it (the
/// gate is a dispatch precondition, not a post-hoc note; unique in the
/// file today).
const LAUNCH_BLOCK_OPEN: &str = "delegate  action:";

/// The T173 clause's RESOLVED marker — the measure-clause resolution
/// language (the T110 precedent: "Measure clause RESOLVED at the
/// cycle-60 eval (T110)"), with this row's ref. Must occur EXACTLY once
/// in LOOP-SPEC.md.
const T173_RESOLVED_MARKER: &str = "Measure clause RESOLVED at the cycle-96 eval (T209)";

/// The validator note's evidence needle — both T204 validators died at
/// budget WITH verdicts written, so the T155 clause's letter (verdict
/// UNANNOUNCED) is not tripped. Must occur EXACTLY once in LOOP-SPEC.md.
const VALIDATOR_NOTE_NEEDLE: &str = "both t204 validators died 60/60 WITH verdicts written";

/// The META-META-SPEC sentence's commitment needle — the estimate is a
/// dispatch-time contract, contiguous as written. Must occur EXACTLY
/// once in META-META-SPEC.md.
const DISPATCH_CONTRACT_NEEDLE: &str = "DISPATCH-TIME contract";

/// (ah) T209 — step 2's dispatch-time spec-size gate: the gate needle
/// occurs EXACTLY once in LOOP-SPEC.md, inside step 2's window (the T64
/// loose-heading scope), BEFORE the launch template it gates (the gate
/// is a precondition — an over-ceiling row is never dispatched, so the
/// check must precede `delegate`'s launch block, not follow it). Delete
/// the gate and this goes red at count 0; moving it below the launch
/// block (or into the polling paragraph) dies on the ordering assert; a
/// duplicate statement elsewhere also goes red.
#[test]
fn dispatch_size_gate_sits_before_the_launch_template_in_step2() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        DISPATCH_GATE_NEEDLE.starts_with("re-reads")
            && DISPATCH_GATE_NEEDLE.contains("`estimate:`")
            && DISPATCH_GATE_NEEDLE.ends_with("line"),
        "the gate needle must carry the estimate-line re-read verbatim"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(DISPATCH_GATE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the dispatch-time estimate re-read exactly \
         once — zero means the dispatch gate was deleted (or rewrapped \
         across a line break, which also breaks the spec check's \
         line-wise grep), more than one means it is stated twice"
    );
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let gate = window
        .find(DISPATCH_GATE_NEEDLE)
        .expect("step-2 window must carry the dispatch gate (moved out of step 2?)");
    let launch = window
        .find(LAUNCH_BLOCK_OPEN)
        .expect("step-2 window must carry the launch template (`delegate  action:`)");
    assert!(
        gate < launch,
        "the dispatch gate must sit BEFORE the launch template it gates — \
         gate ({gate}), launch block ({launch})"
    );
}

/// (ai) T209 — the split rule is BY ACCEPTANCE SURFACE, not by component:
/// both the split rule's needle and the naming rule's needle occur
/// EXACTLY once in LOOP-SPEC.md, inside step 2's window, AFTER the
/// dispatch gate they qualify. The T204 evidence is the named cost: a
/// by-component split left child B at ~2,100 added lines, 4× the
/// ceiling. Delete either rule and its leg goes red at count 0; move it
/// out of step 2 and the window find dies; reorder it before the gate
/// and the ordering assert dies.
#[test]
fn split_rule_is_by_acceptance_surface_with_naming_rule_in_step2() {
    // Needle self-checks (T48 idiom).
    assert!(
        SPLIT_RULE_NEEDLE.starts_with("by acceptance")
            && SPLIT_RULE_NEEDLE.ends_with("component"),
        "the split-rule needle must carry the by-acceptance-surface claim \
         verbatim"
    );
    assert!(
        NAMING_RULE_NEEDLE.starts_with("independently")
            && NAMING_RULE_NEEDLE.ends_with("surface"),
        "the naming needle must carry the independently-gateable check: \
         surface requirement verbatim"
    );
    let spec = loop_spec();
    assert_eq!(
        spec.matches(SPLIT_RULE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the by-acceptance-surface rule exactly once \
         — zero means the split rule was deleted (or rewrapped across a \
         line break), more than one means it is stated twice"
    );
    assert_eq!(
        spec.matches(NAMING_RULE_NEEDLE).count(),
        1,
        "LOOP-SPEC must state the independently-gateable check: surface \
         naming rule exactly once — zero means the naming rule was \
         deleted (or rewrapped across a line break), more than one means \
         it is stated twice"
    );
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let gate = window
        .find(DISPATCH_GATE_NEEDLE)
        .expect("step-2 window must carry the dispatch gate");
    let split = window
        .find(SPLIT_RULE_NEEDLE)
        .expect("step-2 window must carry the by-acceptance-surface rule");
    let naming = window
        .find(NAMING_RULE_NEEDLE)
        .expect("step-2 window must carry the naming rule");
    assert!(
        gate < split && split < naming,
        "the split rules must FOLLOW the dispatch gate they qualify, in \
         written order — gate ({gate}), acceptance-surface rule ({split}), \
         naming rule ({naming})"
    );
}

/// (aj) T209 — the T173 measure clause carries its RESOLVED marker: the
/// marker occurs EXACTLY once in LOOP-SPEC.md, inside step 2's window,
/// AFTER the byte-pinned >2-of-8 tripwire it resolves (the T110 leg-(h)
/// ordering pattern), and the validator note (req 5) occurs EXACTLY once
/// inside step 4's window (the same paragraph that carries the T155
/// validator measure clause). Delete the marker or the note and the
/// counts go red; move the note out of step 4 and the window find dies;
/// reorder the marker before its tripwire and the ordering assert dies.
#[test]
fn t173_measure_clause_resolved_marker_and_validator_note() {
    // Needle self-checks (T48 idiom).
    assert!(
        T173_RESOLVED_MARKER.starts_with("Measure clause RESOLVED")
            && T173_RESOLVED_MARKER.ends_with("(T209)"),
        "the marker must be the T110-style resolution language with this \
         row's ref verbatim"
    );
    assert!(
        VALIDATOR_NOTE_NEEDLE.starts_with("both t204 validators")
            && VALIDATOR_NOTE_NEEDLE.ends_with("verdicts written"),
        "the validator-note needle must carry the died-at-budget-with-\
         verdicts evidence verbatim"
    );
    let spec = loop_spec();

    // The RESOLVED marker, exactly once, inside step 2's window, AFTER
    // the tripwire it resolves.
    assert_eq!(
        spec.matches(T173_RESOLVED_MARKER).count(),
        1,
        "LOOP-SPEC must mark the T173 measure clause RESOLVED exactly \
         once — zero means the resolution marker was deleted (or \
         rewrapped across a line break), more than one means it is \
         stated twice"
    );
    let start = spec
        .find(STEP2_HEADING_LOOSE)
        .expect("step-2 heading (`2. **`) present");
    let end = start
        + spec[start..]
            .find(STEP3_HEADING)
            .expect("step-3 heading present after step 2's");
    let window = &spec[start..end];
    let tripwire = window
        .find(IMPL_MEASURE_TRIPWIRE)
        .expect("step-2 window must carry the >2-of-8 tripwire (T173 leg w)");
    let resolved = window
        .find(T173_RESOLVED_MARKER)
        .expect("step-2 window must carry the RESOLVED marker (moved out of step 2?)");
    assert!(
        tripwire < resolved,
        "the RESOLVED marker must FOLLOW the measure clause it resolves — \
         tripwire ({tripwire}), marker ({resolved})"
    );

    // The validator note, exactly once, inside step 4's window (the T155
    // budget-rationale anchors).
    assert_eq!(
        spec.matches(VALIDATOR_NOTE_NEEDLE).count(),
        1,
        "LOOP-SPEC must record the T204 validator-death note exactly once \
         — zero means the note was deleted (or rewrapped across a line \
         break), more than one means it is stated twice"
    );
    let step4_start = spec
        .find(STEP4_HEADING_LOOSE)
        .expect("step-4 heading (`4. **`) present");
    let step4_end = step4_start
        + spec[step4_start..]
            .find(STEP5_HEADING_LOOSE)
            .expect("step-5 heading present after step 4's");
    let step4 = &spec[step4_start..step4_end];
    assert!(
        step4.contains(VALIDATOR_NOTE_NEEDLE),
        "step 4's window must carry the validator note — it belongs in \
         the paragraph that carries the validator measure clause, not \
         another section"
    );
}

/// (ak) T209 — META-META-SPEC's spec-quality bar carries the
/// dispatch-contract sentence: the needle occurs EXACTLY once in
/// META-META-SPEC.md, inside the spec-quality-bar window (the T114/T125
/// anchors), AFTER the ~400 band it qualifies and BEFORE the Priority
/// doctrine sentence that closes the bar's estimate region. Delete the
/// sentence and this goes red at count 0; move it out of the quality
/// bar (or past the Priority doctrine) and the window/ordering asserts
/// die.
#[test]
fn meta_meta_estimate_is_a_dispatch_time_contract() {
    // Needle self-check (T48 idiom).
    assert!(
        DISPATCH_CONTRACT_NEEDLE.starts_with("DISPATCH-TIME")
            && DISPATCH_CONTRACT_NEEDLE.ends_with("contract"),
        "the contract needle must carry the DISPATCH-TIME contract claim \
         verbatim"
    );
    let spec = meta_meta_spec();
    assert_eq!(
        spec.matches(DISPATCH_CONTRACT_NEEDLE).count(),
        1,
        "META-META-SPEC must state the estimate-is-a-dispatch-contract \
         sentence exactly once — zero means the sentence was deleted (or \
         rewrapped across a line break), more than one means it is \
         stated twice"
    );
    let start = spec
        .find(EXTEND_TODO_HEADING)
        .expect("the Extend-TODO heading present");
    let end = start
        + spec[start..]
            .find(HANDOFF_HEADING)
            .expect("the Handoff heading present after the Extend-TODO heading");
    let window = &spec[start..end];
    let band = window
        .find(SPLIT_BAND_NEEDLE)
        .expect("the spec-quality-bar window must carry the ~400 band (T125)");
    let contract = window
        .find(DISPATCH_CONTRACT_NEEDLE)
        .expect("the spec-quality-bar window must carry the dispatch-\
                 contract sentence (moved out of the bar?)");
    let priority = window
        .find(PRIORITY_DOCTRINE)
        .expect("the spec-quality-bar window must carry the Priority \
                 doctrine sentence");
    assert!(
        band < contract && contract < priority,
        "the dispatch-contract sentence must sit after the ~400 band it \
         qualifies and before the Priority doctrine — band ({band}), \
         contract ({contract}), priority ({priority})"
    );
}

// ---- T228 — verify the indictment before filing a bug row (the T212 false-indictment lesson) ----
//
// The cycle-98 eval filed T212 on a FALSE indictment ("the T197 drift
// advisory false-positives on a correctly pre-keyed branch") asserted
// from secondhand Outcomes text without reading the code path — the
// WARNs were TRUE positives (the goal gate reads the spec ARG path every
// iteration, `src/driver.rs:1171`, so the on-branch T175 re-keys never
// reached it). The filing's premise directed round 1's fix, the round-1
// kimi validator FAILed it with three-way proof, and the whole round was
// reverted sha256-byte-identical — one impl round (70/80) + one
// validation round (48/60) + the revert, the most expensive eval-quality
// defect in the corpus. META-META-SPEC's filing bar already said "Verify
// T1/T2/T4/T5 actually worked before filing anything adjacent", but that
// clause covers fix-ADJACENCY, not a fresh bug row whose premise indicts
// a specific component. The bar now carries the verify-the-indictment
// clause; this leg pins it in the T48/T64/T114 pattern: every needle
// exactly-once, inside the Extend-TODO window, AFTER the verify-adjacent
// sentence it extends.

/// The indictment clause's commitment needle — the verify-before-filing
/// demand for any row whose premise indicts a component, contiguous as
/// written (single-line per the spec's ~79-col wrap). Must occur EXACTLY
/// once in META-META-SPEC.md.
const INDICTMENT_VERIFY_NEEDLE: &str = "MUST be verified against the code before filing";

/// The indictment clause's fallback needle — the un-verifiable case files
/// the symptom and demotes the mechanism to a hypothesis instead of
/// premising the row on it, contiguous as written. Must occur EXACTLY
/// once in META-META-SPEC.md.
const HYPOTHESIS_FALLBACK_NEEDLE: &str = "a HYPOTHESIS in repo-context, never the row's premise";

/// The indictment clause's lesson-cite needle — the T212 record the
/// clause exists for, contiguous as written. Must occur EXACTLY once in
/// META-META-SPEC.md.
const T212_LESSON_NEEDLE: &str = "the indictment was inverted, a full round reverted";

/// The verify-adjacent sentence (the existing filing bar) the indictment
/// clause sits AFTER — the clause extends it, never replaces or precedes
/// it (the bar keeps its history: fix-adjacency first, then the
/// component-indictment bar).
const VERIFY_ADJACENT_NEEDLE: &str = "Verify T1/T2/T4/T5 actually worked";

/// (al) T228 — the verify-the-indictment clause occurs EXACTLY once in
/// META-META-SPEC.md, inside the Extend-TODO window (the T64/T114
/// loose-heading scope), AFTER the verify-adjacent sentence it extends.
/// Delete the clause and all three needles go red at count 0 (which also
/// breaks any line-wise grep of them); rewrap a needle across a line
/// break and it goes red the same way; a duplicate statement of any
/// needle elsewhere also goes red (count 2); moving the clause out of
/// the Extend-TODO section (or before the sentence it extends) dies on
/// the window/ordering assert.
#[test]
fn indictment_clause_exactly_once_inside_extend_todo_after_verify_adjacent() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        INDICTMENT_VERIFY_NEEDLE.starts_with("MUST be verified")
            && INDICTMENT_VERIFY_NEEDLE.ends_with("before filing"),
        "the verify needle must carry the verify-before-filing demand \
         verbatim (capital MUST, per the clause's line-wise wrapping)"
    );
    assert!(
        HYPOTHESIS_FALLBACK_NEEDLE.starts_with("a HYPOTHESIS")
            && HYPOTHESIS_FALLBACK_NEEDLE.ends_with("the row's premise"),
        "the hypothesis needle must carry the symptom-not-premise \
         fallback language verbatim"
    );
    assert!(
        T212_LESSON_NEEDLE.starts_with("the indictment was inverted")
            && T212_LESSON_NEEDLE.ends_with("full round reverted"),
        "the lesson needle must carry the T212 inverted-indictment \
         language verbatim"
    );
    let spec = meta_meta_spec();
    for (needle, what) in [
        (
            INDICTMENT_VERIFY_NEEDLE,
            "the verify-the-indictment-before-filing demand",
        ),
        (
            HYPOTHESIS_FALLBACK_NEEDLE,
            "the symptom-plus-hypothesis fallback",
        ),
        (T212_LESSON_NEEDLE, "the T212 lesson citation"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-META-SPEC must state {what} exactly once — zero means the \
             clause was deleted (or a needle was rewrapped across a line \
             break), more than one means it is stated twice"
        );
    }
    let start = spec
        .find(EXTEND_TODO_HEADING)
        .expect("the Extend-TODO heading present");
    let end = start
        + spec[start..]
            .find(HANDOFF_HEADING)
            .expect("the Handoff heading present after the Extend-TODO heading");
    let window = &spec[start..end];
    let adjacent = window.find(VERIFY_ADJACENT_NEEDLE).expect(
        "the Extend-TODO window must carry the verify-adjacent sentence \
         (the T1/T2/T4/T5 filing bar)",
    );
    let verify = window.find(INDICTMENT_VERIFY_NEEDLE).expect(
        "the Extend-TODO window must carry the verify-the-indictment \
         clause (deleted, or moved out of the section?)",
    );
    let hypothesis = window.find(HYPOTHESIS_FALLBACK_NEEDLE).expect(
        "the Extend-TODO window must carry the symptom-plus-hypothesis \
         fallback (rewrapped across a line break?)",
    );
    let lesson = window.find(T212_LESSON_NEEDLE).expect(
        "the Extend-TODO window must carry the T212 lesson citation \
         (rewrapped across a line break?)",
    );
    assert!(
        adjacent < verify && verify < hypothesis && hypothesis < lesson,
        "the indictment clause must sit INSIDE the Extend-TODO window, \
         AFTER the verify-adjacent sentence it extends ({adjacent}), in \
         the clause's own order — verify ({verify}), hypothesis fallback \
         ({hypothesis}), lesson cite ({lesson})"
    );
}

// ---- T227 — the TODO.md-edit gate must not swallow a red guard through a pipe ----
//
// Cycle-101 pushed a RED main (c06a555): the T220/T223 notes cells carried
// literal `|` characters that split the rows into 8 cells (the T37/T8
// class), `cargo test --test todo_consistency` was red — and the
// orchestrator ran the guard INSIDE a pipe chain without `set -o
// pipefail`, so the chain reported the filter's exit 0 and the red was
// swallowed into a push (fixed forward in 6365b50). Cycle-103 added a
// THIRD instance while the row sat queued: the filing cycle's own T225
// flip ran `cargo test … | grep "test result"` — grep exits 0 on
// matching the FAILED line — and pushed a red row-format guard live
// (71ca2c1, fixed d2bf500). Two of the three recurrences are ORCHESTRATOR
// filter chains, not model carelessness: the T67→T164 pipe lint covers
// spec `check:` lines only, and the orchestrator's ad-hoc gate chains had
// no carrier. Step 5's TODO.md-edit paragraph now carries the sentence
// beside the gate it qualifies; this leg pins it in the T64/T120 pattern:
// every needle exactly-once file-wide, inside step 5's window (the T120
// loose-heading anchors), AFTER the gate sentence it guards and BEFORE
// the sed bookkeeping paragraph. Deleting the sentence drops every needle
// to 0 and the leg goes red; a duplicate statement or a rewrap across a
// line break goes red the same way.

/// The unpiped-gate rule's commitment needle — UNPIPED, or pipefail
/// first, contiguous as written (single-line per the spec's ~79-col
/// wrap). Must occur EXACTLY once in LOOP-SPEC.md.
const GATE_UNPIPED_NEEDLE: &str = "todo_consistency run is UNPIPED, or the chain begins";

/// The rule's mechanism needle — the filter-exit-0 fact the rule exists
/// for, contiguous as written. Must occur EXACTLY once in LOOP-SPEC.md.
const PIPED_GATE_MASKS_RED_NEEDLE: &str = "a piped gate whose filter exits 0 reports";

/// The mechanism's outcome + lesson-cite needle — green-on-red and the
/// c06a555 record, contiguous as written. Must occur EXACTLY once in
/// LOOP-SPEC.md.
const RED_GUARD_LESSON_NEEDLE: &str = "green on a red guard (the c06a555 lesson)";

/// The rule's scope needle — it binds ANY ad-hoc orchestrator gate chain
/// through the filter list, contiguous as written. Must occur EXACTLY
/// once in LOOP-SPEC.md.
const AD_HOC_SCOPE_NEEDLE: &str = "ANY ad-hoc gate chain the orchestrator pipes through";

/// The enumerated filter list the scope clause names, contiguous as
/// written. Must occur EXACTLY once in LOOP-SPEC.md.
const FILTER_LIST_NEEDLE: &str = "tail/head/grep.";

/// The gate sentence (the existing TODO.md-edit rule) the new sentence
/// sits AFTER — the carrier extends the gate, never replaces or precedes
/// it. T253 keyed the guard to the RELEASE profile (the profile the gates
/// run, T78).
const GATE_SENTENCE_NEEDLE: &str = "runs `cargo test --release --test todo_consistency` (seconds)";

/// The sed bookkeeping paragraph that CLOSES the region — the sentence
/// must sit before it (inside the TODO.md-edit paragraph, not drifted
/// into the sed rule that follows).
const SED_PARAGRAPH_NEEDLE: &str = "When editing repo files with";

/// (am) T227 — the unpiped-gate sentence occurs EXACTLY once in
/// LOOP-SPEC.md, inside step 5's window (the T64/T120 loose-heading
/// scope), AFTER the gate sentence it guards and BEFORE the sed
/// bookkeeping paragraph. Delete the sentence and all five needles go red
/// at count 0 (which also breaks any line-wise grep of them); rewrap a
/// needle across a line break and it goes red the same way; a duplicate
/// statement of any needle elsewhere also goes red (count 2); moving the
/// sentence out of step 5, before the gate it guards, or into the sed
/// paragraph dies on the window/ordering asserts.
#[test]
fn todo_edit_gate_unpiped_needles_exactly_once_inside_step_5() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        GATE_UNPIPED_NEEDLE.starts_with("todo_consistency run")
            && GATE_UNPIPED_NEEDLE.ends_with("the chain begins"),
        "the unpiped-gate needle must carry the UNPIPED-or-pipefail \
         commitment verbatim (contiguous as written, per the spec's wrap)"
    );
    assert!(
        PIPED_GATE_MASKS_RED_NEEDLE.starts_with("a piped gate")
            && PIPED_GATE_MASKS_RED_NEEDLE.ends_with("exits 0 reports"),
        "the mechanism needle must carry the filter-exit-0 language verbatim"
    );
    assert!(
        RED_GUARD_LESSON_NEEDLE.starts_with("green on a red guard")
            && RED_GUARD_LESSON_NEEDLE.ends_with("(the c06a555 lesson)"),
        "the lesson needle must carry the c06a555 citation verbatim"
    );
    assert!(
        AD_HOC_SCOPE_NEEDLE.starts_with("ANY ad-hoc")
            && AD_HOC_SCOPE_NEEDLE.ends_with("pipes through"),
        "the scope needle must carry the ANY-ad-hoc-chain language verbatim \
         (capital ANY, per the clause's wrapping)"
    );
    assert!(
        FILTER_LIST_NEEDLE.starts_with("tail/") && FILTER_LIST_NEEDLE.ends_with("grep."),
        "the filter-list needle must carry the tail/head/grep enumeration \
         verbatim"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (
            GATE_UNPIPED_NEEDLE,
            "the todo_consistency-run-is-UNPIPED-or-pipefail commitment",
        ),
        (
            PIPED_GATE_MASKS_RED_NEEDLE,
            "the filter-exit-0-masks-red mechanism",
        ),
        (RED_GUARD_LESSON_NEEDLE, "the c06a555 lesson citation"),
        (
            AD_HOC_SCOPE_NEEDLE,
            "the ANY-ad-hoc-gate-chain scope clause",
        ),
        (FILTER_LIST_NEEDLE, "the tail/head/grep filter list"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             unpiped-gate sentence was deleted (or a needle was rewrapped \
             across a line break), more than one means it is stated twice"
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
    let gate = window.find(GATE_SENTENCE_NEEDLE).expect(
        "step-5's window must carry the gate sentence \
         (`cargo test --release --test todo_consistency` (seconds)) the unpiped rule \
         guards",
    );
    let unpiped = window.find(GATE_UNPIPED_NEEDLE).unwrap_or_else(|| {
        panic!(
            "step-5's window must carry {GATE_UNPIPED_NEEDLE:?} — the \
             unpiped-gate sentence was deleted, or moved out of step 5's \
             TODO.md-edit paragraph"
        )
    });
    let masks = window.find(PIPED_GATE_MASKS_RED_NEEDLE).expect(
        "step-5's window must carry the filter-exit-0 mechanism \
         (rewrapped across a line break?)",
    );
    let lesson = window.find(RED_GUARD_LESSON_NEEDLE).expect(
        "step-5's window must carry the c06a555 lesson citation \
         (rewrapped across a line break?)",
    );
    let scope = window.find(AD_HOC_SCOPE_NEEDLE).expect(
        "step-5's window must carry the ANY-ad-hoc-gate-chain scope \
         clause (rewrapped across a line break?)",
    );
    let filter_list = window.find(FILTER_LIST_NEEDLE).expect(
        "step-5's window must carry the tail/head/grep filter list \
         (rewrapped across a line break?)",
    );
    let sed = window.find(SED_PARAGRAPH_NEEDLE).expect(
        "step-5's window must carry the sed bookkeeping paragraph \
         (the region's closer)",
    );
    assert!(
        gate < unpiped
            && unpiped < masks
            && masks < lesson
            && lesson < scope
            && scope < filter_list
            && filter_list < sed,
        "the unpiped-gate sentence must sit INSIDE step 5's \
         TODO.md-edit paragraph — AFTER the gate sentence it guards \
         ({gate}) and BEFORE the sed bookkeeping paragraph ({sed}), in \
         the sentence's own order: commitment ({unpiped}), mechanism \
         ({masks}), lesson cite ({lesson}), scope ({scope}), filter list \
         ({filter_list})"
    );
}

// ---- T231 — the TODO-edit guard floor must name the T57 main-dedicated target dir ----
//
// The guard sentence ran `cargo test --test todo_consistency` with NO
// target dir, and its "(seconds)" silently assumed whichever cache the
// invocation hit — the DEFAULT `target/` is cold for the whole crate
// after EVERY version bump by construction (a manifest-only change
// stales every artifact), and the cycle-104 wrap paid the measured
// cost: four consecutive guard/gate runs died `timed out after 300s
// (process group killed)` with `Compiling chug v0.17.1` as the last
// line (23:08, 23:17, 23:22, 23:27 UTC) — ~20 minutes of wrap wall
// burned on cold-compile timeouts right after the v0.17.1 bump,
// recovered only as partial compiles warmed the cache. The T57 ALWAYS
// rule already governs the post-merge and final gates; the TODO-edit
// guard floor was the one main-tree cargo run the rule's text did not
// reach (its sentence sat two paragraphs away, and the guard's own
// "(seconds)" framing read as exempt-from-cargo-discipline). Step 5's
// guard paragraph now names the main-dedicated dir IN the invocation;
// this leg pins it in the T227 pattern: every needle exactly-once
// file-wide, inside step 5's window (the T120 loose-heading anchors),
// AFTER the T227 unpiped sentence's closing filter-list anchor and
// BEFORE the sed bookkeeping paragraph — the amendment is ADJACENT to
// the T227 sentence (it amends the TARGET-DIR shape of the invocation,
// never the pipe discipline, which stays verbatim). Reverting the env
// prefix drops the invocation needle to 0 (which also breaks any
// line-wise grep of it); deleting the whole guard sentence drops every
// needle to 0 / window-miss — the leg goes red either way; a duplicate
// statement or a rewrap across a line break goes red the same way.

/// The amended invocation needle — the T57 main-dedicated dir prefix +
/// the guard's test selection, contiguous as written (single line, per
/// the file's long-inline-command precedent at the T82 runner
/// templates). T253 keyed the guard to the RELEASE profile — the
/// profile the gates run in that dir (T78) — so a debug-profile guard
/// leg there is cold-scale (the cycle-130 fire: a 300,136ms kill, its
/// identical `--release` re-run instant). Must occur EXACTLY once in
/// LOOP-SPEC.md.
const GUARD_TARGET_DIR_NEEDLE: &str =
    "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main cargo test --release --test todo_consistency";

/// The never-the-bare-default needle — the guard builds in the
/// main-dedicated dir, not the cold-by-construction default, contiguous
/// as written. Must occur EXACTLY once in LOOP-SPEC.md.
const GUARD_NEVER_DEFAULT_NEEDLE: &str = "never the bare default `target/`";

/// The why needle — the mechanism that colds the default dir on every
/// version bump, contiguous as written. Must occur EXACTLY once in
/// LOOP-SPEC.md.
const GUARD_COLD_BY_CONSTRUCTION_NEEDLE: &str = "a version bump colds that dir by";

/// The evidence needle — the cycle-104 wrap's four 300s guard/gate
/// timeouts, contiguous as written. Must occur EXACTLY once in
/// LOOP-SPEC.md.
const GUARD_CYCLE104_EVIDENCE_NEEDLE: &str = "four consecutive 300s guard/gate";

/// The (seconds) tie-back needle — the shared dir's warmth is what
/// makes the gate sentence's "(seconds)" claim true, contiguous as
/// written. Must occur EXACTLY once in LOOP-SPEC.md.
const GUARD_SECONDS_TIEBACK_NEEDLE: &str = "makes the (seconds) above true";

/// The exemption needle — the guard needs the env prefix only, NO
/// T195 touch (the main-dedicated exemption), contiguous as written.
/// Must occur EXACTLY once in LOOP-SPEC.md.
const GUARD_NO_TOUCH_NEEDLE: &str = "needs the env prefix only, NO T195 touch";

/// (an) T231 — the guard floor's main-dedicated-dir sentence occurs
/// EXACTLY once (each needle), inside step 5's window (the T64/T120
/// loose-heading scope), AFTER the T227 unpiped sentence it extends
/// (its closing filter-list anchor) and BEFORE the sed bookkeeping
/// paragraph — the same TODO.md-edit guard paragraph, not a disjoint
/// section. Revert the env prefix from the sentence and the invocation
/// needle goes red at count 0; delete the whole guard sentence and
/// every needle goes red / window-misses (the RED-proof is not vacuous:
/// six needles, an exactly-once sweep, and a window+ordering chain all
/// load-bearing); a duplicate statement of any needle goes red at count
/// 2; a rewrap across a line break goes red the same way; moving the
/// sentence out of step 5, before the T227 sentence it extends, or into
/// the sed paragraph dies on the window/ordering asserts.
#[test]
fn todo_edit_guard_floor_targets_main_dedicated_dir_inside_step_5() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        GUARD_TARGET_DIR_NEEDLE
            .starts_with("CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main")
            && GUARD_TARGET_DIR_NEEDLE.ends_with("cargo test --release --test todo_consistency"),
        "the invocation needle must carry the T57 env prefix + the guard's \
         release-profile test selection verbatim (contiguous as written, \
         per T253)"
    );
    assert!(
        GUARD_NEVER_DEFAULT_NEEDLE.starts_with("never the bare default")
            && GUARD_NEVER_DEFAULT_NEEDLE.ends_with("`target/`"),
        "the never-default needle must carry the not-the-default-dir \
         commitment verbatim"
    );
    assert!(
        GUARD_COLD_BY_CONSTRUCTION_NEEDLE.starts_with("a version bump")
            && GUARD_COLD_BY_CONSTRUCTION_NEEDLE.ends_with("colds that dir by"),
        "the why needle must carry the cold-by-construction mechanism \
         verbatim"
    );
    assert!(
        GUARD_CYCLE104_EVIDENCE_NEEDLE.starts_with("four consecutive")
            && GUARD_CYCLE104_EVIDENCE_NEEDLE.ends_with("300s guard/gate"),
        "the evidence needle must carry the cycle-104 4x300s timeouts \
         verbatim"
    );
    assert!(
        GUARD_SECONDS_TIEBACK_NEEDLE.starts_with("makes the (seconds)")
            && GUARD_SECONDS_TIEBACK_NEEDLE.ends_with("above true"),
        "the tie-back needle must carry the (seconds)-warmth claim verbatim"
    );
    assert!(
        GUARD_NO_TOUCH_NEEDLE.starts_with("needs the env prefix")
            && GUARD_NO_TOUCH_NEEDLE.ends_with("NO T195 touch"),
        "the exemption needle must carry the env-prefix-only-no-touch \
         language verbatim (capital NO, per the clause's wrapping)"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (
            GUARD_TARGET_DIR_NEEDLE,
            "the env-prefixed todo_consistency guard invocation",
        ),
        (
            GUARD_NEVER_DEFAULT_NEEDLE,
            "the never-the-bare-default commitment",
        ),
        (
            GUARD_COLD_BY_CONSTRUCTION_NEEDLE,
            "the version-bump-colds-the-default mechanism",
        ),
        (
            GUARD_CYCLE104_EVIDENCE_NEEDLE,
            "the cycle-104 4x300s timeout evidence",
        ),
        (
            GUARD_SECONDS_TIEBACK_NEEDLE,
            "the (seconds)-warmth tie-back",
        ),
        (
            GUARD_NO_TOUCH_NEEDLE,
            "the env-prefix-only-no-touch exemption",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             guard-floor target-dir sentence was deleted (or a needle \
             was rewrapped across a line break), more than one means it \
             is stated twice"
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
    let gate = window.find(GATE_SENTENCE_NEEDLE).expect(
        "step-5's window must carry the gate sentence \
         (`cargo test --release --test todo_consistency` (seconds)) the target-dir \
         rule qualifies",
    );
    let unpiped = window.find(GATE_UNPIPED_NEEDLE).expect(
        "step-5's window must carry the T227 unpiped-gate sentence \
         (the adjacent doctrine this row amends beside, never instead)",
    );
    let filter_list = window.find(FILTER_LIST_NEEDLE).expect(
        "step-5's window must carry the T227 sentence's closing \
         tail/head/grep filter list (the anchor region the new sentence \
         sits after)",
    );
    let invocation = window.find(GUARD_TARGET_DIR_NEEDLE).unwrap_or_else(|| {
        panic!(
            "step-5's window must carry {GUARD_TARGET_DIR_NEEDLE:?} — the \
             guard-floor target-dir sentence was deleted, its env prefix \
             reverted, or it moved out of step 5's TODO.md-edit paragraph"
        )
    });
    let never_default = window.find(GUARD_NEVER_DEFAULT_NEEDLE).expect(
        "step-5's window must carry the never-the-bare-default commitment \
         (rewrapped across a line break?)",
    );
    let cold = window.find(GUARD_COLD_BY_CONSTRUCTION_NEEDLE).expect(
        "step-5's window must carry the cold-by-construction mechanism \
         (rewrapped across a line break?)",
    );
    let evidence = window.find(GUARD_CYCLE104_EVIDENCE_NEEDLE).expect(
        "step-5's window must carry the cycle-104 timeout evidence \
         (rewrapped across a line break?)",
    );
    let seconds = window.find(GUARD_SECONDS_TIEBACK_NEEDLE).expect(
        "step-5's window must carry the (seconds)-warmth tie-back \
         (rewrapped across a line break?)",
    );
    let no_touch = window.find(GUARD_NO_TOUCH_NEEDLE).expect(
        "step-5's window must carry the env-prefix-only-no-touch \
         exemption (rewrapped across a line break?)",
    );
    let sed = window.find(SED_PARAGRAPH_NEEDLE).expect(
        "step-5's window must carry the sed bookkeeping paragraph \
         (the region's closer)",
    );
    assert!(
        gate < unpiped
            && unpiped < filter_list
            && filter_list < invocation
            && invocation < never_default
            && never_default < cold
            && cold < evidence
            && evidence < seconds
            && seconds < no_touch
            && no_touch < sed,
        "the target-dir sentence must sit INSIDE step 5's TODO.md-edit \
         guard paragraph — AFTER the T227 unpiped sentence it extends \
         (gate {gate}, commitment {unpiped}, filter list {filter_list}) \
         and BEFORE the sed bookkeeping paragraph ({sed}), in the \
         sentence's own order: invocation ({invocation}), never-default \
         ({never_default}), cold mechanism ({cold}), cycle-104 evidence \
         ({evidence}), (seconds) tie-back ({seconds}), no-touch \
         exemption ({no_touch})"
    );
}

// ---- T251 — the ctx-edit casualty clauses: mutation-checkpoint ordering + read-first recovery ----
//
// Cycle 128 (glm orchestrator, events-20261006-182121.jsonl) fast-forwarded
// main bd66e31→c982efa (the T249 landing), wrote three bookkeeping files
// dirty (TODO flip, EVALUATION.md Outcomes, DEPENDENCIES.md), and then —
// under the T230 occupancy nudge — had an ACCEPTED LIVE_CTX edit collapse
// its context 108,645 → 1,004 tokens in ONE edit (iter 103), amputating the
// turns covering the merge and the dirty files' provenance. It spent ~45
// iterations rediscovering its own state (reflog, the three dirty files,
// the merge story) and landed the bookkeeping VERBATIM (cfef97f) — zero
// work lost ONLY because the files were still on disk. Nothing in doctrine
// required the read-first behavior, and the fleet census (cycle-130 eval,
// I1) shows deep collapses are the feature's NORMAL operation: 10 accepted
// LIVE_CTX edits since T192, 8 of them >75% single-edit collapses, 4 of
// them 89–99%. The hazard is collapse TIMING (mid-mutation, uncommitted
// bookkeeping), not depth — a driver-side ratio/shrink-floor gate was
// weighed and REJECTED at filing (it fights the primary use pattern; the
// T192 pinned/pair/shrink guards already reject malformed edits), so the
// fix is doctrine, not code. Two clauses, both naming the casualty:
// (1) step 5's MUTATION-CHECKPOINT ORDERING — the row-flip/Outcomes
// bookkeeping is written to disk BEFORE any irreversible mutation (a merge
// or fast-forward into main, a push) wherever the content is knowable
// pre-mutation, committed IMMEDIATELY after, and NO child dispatch may
// happen between the mutation and its bookkeeping commit; (2) the Hard
// rules' READ-FIRST RECOVERY rule — repo state the orchestrator does not
// remember producing (dirty files, a moved HEAD, commits it did not watch
// land) is load-bearing until reconstruction proves it disposable, and the
// four state-destroying git commands are banned against unremembered state
// (the cycle-61 kill rule's verify-then-act discipline, payloads there,
// repo state here). Legs (ao)–(ap) pin both clauses in the T48/T64
// pattern: every needle exactly-once file-wide and inside its section's
// window (step 5 = `5. **` through `6. **`, reusing the T120 anchors;
// Hard rules = `## Hard rules` through end-of-file, the spec's LAST
// section), in the written order. Deleting either clause drops its needles
// to 0 and the named leg goes red; duplicating one fires the count-2 leg;
// moving either out of its window (or against its anchors) dies on the
// ordering asserts. Scope discipline: LOOP-SPEC.md and this file ONLY —
// the T192 shrink/pair/pinned guards are untouched (collapse depth stays
// the model's choice), and no driver code changes.

/// The ordering clause's lead — the bolded clause name + T-number. Must
/// occur EXACTLY once in LOOP-SPEC.md.
const MUTATION_CHECKPOINT_LEAD: &str = "Mutation-checkpoint ordering (T251)";

/// The write-before commitment — bookkeeping to disk BEFORE the
/// irreversible mutation. Must occur EXACTLY once in LOOP-SPEC.md.
const WRITTEN_BEFORE_MUTATION: &str = "MUST be written to disk BEFORE the";

/// The knowability carve-out — where the content is knowable pre-mutation
/// (the row flip's merge ref is NOT, which is why the commitment splits
/// into write-before + commit-immediately). Must occur EXACTLY once.
const KNOWABLE_PRE_MUTATION: &str = "knowable pre-mutation";

/// The commit-immediately commitment. Must occur EXACTLY once.
const COMMITTED_IMMEDIATELY: &str = "IMMEDIATELY after the mutation lands";

/// The no-dispatch commitment — nothing flies between the mutation and its
/// bookkeeping commit. Must occur EXACTLY once.
const NO_DISPATCH_BETWEEN: &str = "NO further child dispatch";

/// The no-dispatch window's span — mutation to bookkeeping commit. Must
/// occur EXACTLY once.
const BETWEEN_MUTATION_AND_COMMIT: &str =
    "between an irreversible mutation and its bookkeeping";

/// The casualty-evidence needle — names the ctx-edit casualty. Must occur
/// EXACTLY once (the Hard-rules rule names the same casualty WITHOUT the
/// "The evidence is the cycle-128" prefix, so this count stays one).
const CTX_EDIT_EVIDENCE: &str = "The evidence is the cycle-128 ctx-edit casualty";

/// The collapse-depth needle — the accepted 99% LIVE_CTX collapse. Must
/// occur EXACTLY once.
const COLLAPSE_99: &str = "99% LIVE_CTX collapse";

/// The timing needle — the collapse fired BETWEEN the fast-forward and the
/// bookkeeping commit (the exact hazard the clause closes). Must occur
/// EXACTLY once.
const FIRED_BETWEEN_FF_AND_COMMIT: &str =
    "fired BETWEEN the fast-forward and the bookkeeping commit";

/// The verbatim-recovery needle — on-disk dirty files were the only reason
/// the recovery landed the bookkeeping verbatim. Must occur EXACTLY once.
const DIRTY_FILES_REASON: &str = "on-disk dirty files were the only reason";

/// The operational summary's write half — while the story is still in
/// context. Must occur EXACTLY once.
const STORY_IN_CONTEXT: &str = "while the story is still";

/// The operational summary's commit half. Must occur EXACTLY once.
const COMMIT_THE_MOMENT: &str = "commit it the moment the mutation lands.";

/// Step-5 anchors bracketing the clause's position: the Outcomes-per-item
/// tail sentence and the push lead it follows (the clause CAPS step 5's
/// bookkeeping-ordering rules, before step 6's budget check).
const OUTCOMES_PER_ITEM_TAIL: &str = "children never own the row.";
const PUSH_LEAD: &str = "**Push after each item";

/// (ao) T251 — step 5's mutation-checkpoint ordering clause: every
/// load-bearing needle occurs EXACTLY once in LOOP-SPEC.md, all inside
/// step 5's window (the T64 loose-heading pattern), and the clause sits
/// AFTER the Outcomes-per-item + push bookkeeping rules it caps and BEFORE
/// step 6's heading. Delete the clause and the needles go red at count 0;
/// duplicate any needle and this fires at count 2; move the clause out of
/// step 5 (or before the push rule it extends) and the ordering assert
/// dies.
#[test]
fn mutation_checkpoint_ordering_clause_exactly_once_inside_step_5() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        MUTATION_CHECKPOINT_LEAD.starts_with("Mutation-checkpoint")
            && MUTATION_CHECKPOINT_LEAD.ends_with("(T251)"),
        "the lead needle must carry the clause's name + T-number verbatim"
    );
    assert!(
        WRITTEN_BEFORE_MUTATION.starts_with("MUST be written to disk")
            && WRITTEN_BEFORE_MUTATION.ends_with("BEFORE the"),
        "the write-before needle must carry the ordering commitment verbatim"
    );
    assert!(
        CTX_EDIT_EVIDENCE.starts_with("The evidence is the cycle-128")
            && CTX_EDIT_EVIDENCE.ends_with("ctx-edit casualty"),
        "the evidence needle must name the ctx-edit casualty verbatim"
    );
    assert!(
        FIRED_BETWEEN_FF_AND_COMMIT.starts_with("fired BETWEEN")
            && FIRED_BETWEEN_FF_AND_COMMIT.ends_with("bookkeeping commit"),
        "the timing needle must carry the between-FF-and-commit fact \
         verbatim"
    );
    assert!(
        DIRTY_FILES_REASON.starts_with("on-disk dirty files")
            && DIRTY_FILES_REASON.ends_with("the only reason"),
        "the verbatim-recovery needle must carry the dirty-files fact \
         verbatim"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (MUTATION_CHECKPOINT_LEAD, "the clause's bolded lead"),
        (WRITTEN_BEFORE_MUTATION, "the write-before commitment"),
        (KNOWABLE_PRE_MUTATION, "the knowability carve-out"),
        (COMMITTED_IMMEDIATELY, "the commit-immediately commitment"),
        (NO_DISPATCH_BETWEEN, "the no-dispatch commitment"),
        (BETWEEN_MUTATION_AND_COMMIT, "the no-dispatch window's span"),
        (CTX_EDIT_EVIDENCE, "the ctx-edit casualty evidence"),
        (COLLAPSE_99, "the 99% LIVE_CTX collapse depth"),
        (FIRED_BETWEEN_FF_AND_COMMIT, "the collapse-timing evidence"),
        (DIRTY_FILES_REASON, "the verbatim-recovery fact"),
        (STORY_IN_CONTEXT, "the write-while-in-context summary"),
        (COMMIT_THE_MOMENT, "the commit-the-moment summary"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             mutation-checkpoint ordering clause was deleted (or a needle \
             was rewrapped across a line break), more than one means it is \
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
    let outcomes_tail = window.find(OUTCOMES_PER_ITEM_TAIL).expect(
        "step-5's window must carry the Outcomes-per-item tail \
         (\"children never own the row\") the clause follows",
    );
    let push_lead = window.find(PUSH_LEAD).expect(
        "step-5's window must carry the push lead (\"**Push after each \
         item\") the clause follows",
    );
    let lead = window.find(MUTATION_CHECKPOINT_LEAD).unwrap_or_else(|| {
        panic!(
            "step-5's window must carry the mutation-checkpoint ordering \
             clause — it was deleted, or moved out of step 5"
        )
    });
    let written = window.find(WRITTEN_BEFORE_MUTATION).expect(
        "the write-before commitment must sit inside step 5's window \
         (rewrapped across a line break?)",
    );
    let knowable = window.find(KNOWABLE_PRE_MUTATION).expect(
        "the knowability carve-out must sit inside step 5's window \
         (rewrapped?)",
    );
    let immediately = window.find(COMMITTED_IMMEDIATELY).expect(
        "the commit-immediately commitment must sit inside step 5's \
         window (rewrapped?)",
    );
    let no_dispatch = window.find(NO_DISPATCH_BETWEEN).expect(
        "the no-dispatch commitment must sit inside step 5's window \
         (rewrapped?)",
    );
    let span = window.find(BETWEEN_MUTATION_AND_COMMIT).expect(
        "the no-dispatch window's span must sit inside step 5's window \
         (rewrapped?)",
    );
    let evidence = window.find(CTX_EDIT_EVIDENCE).expect(
        "the ctx-edit casualty evidence must sit inside step 5's window \
         (rewrapped?)",
    );
    let collapse = window.find(COLLAPSE_99).expect(
        "the 99% LIVE_CTX collapse depth must sit inside step 5's window \
         (rewrapped?)",
    );
    let fired = window.find(FIRED_BETWEEN_FF_AND_COMMIT).expect(
        "the collapse-timing evidence must sit inside step 5's window \
         (rewrapped?)",
    );
    let dirty = window.find(DIRTY_FILES_REASON).expect(
        "the verbatim-recovery fact must sit inside step 5's window \
         (rewrapped?)",
    );
    let story = window.find(STORY_IN_CONTEXT).expect(
        "the write-while-in-context summary must sit inside step 5's \
         window (rewrapped?)",
    );
    let moment = window.find(COMMIT_THE_MOMENT).expect(
        "the commit-the-moment summary must sit inside step 5's window \
         (rewrapped?)",
    );
    assert!(
        outcomes_tail < push_lead
            && push_lead < lead
            && lead < written
            && written < knowable
            && knowable < immediately
            && immediately < no_dispatch
            && no_dispatch < span
            && span < evidence
            && evidence < collapse
            && collapse < fired
            && fired < dirty
            && dirty < story
            && story < moment,
        "the ordering clause must CAP step 5's bookkeeping rules — AFTER \
         the Outcomes-per-item tail ({outcomes_tail}) and the push lead \
         ({push_lead}), BEFORE step 6 — and read in its own written order \
         (lead {lead}, write-before {written}, knowable {knowable}, \
         immediately {immediately}, no-dispatch {no_dispatch}, span \
         {span}, evidence {evidence}, collapse {collapse}, timing {fired}, \
         dirty-files {dirty}, in-context {story}, commit-the-moment \
         {moment})"
    );
}

/// The recovery rule's lead — the bolded rule name. Must occur EXACTLY
/// once in LOOP-SPEC.md.
const READ_FIRST_LEAD: &str = "Read-first recovery against unremembered state";

/// The load-bearing commitment. Must occur EXACTLY once.
const TREAT_AS_LOAD_BEARING: &str = "MUST treat that state as";

/// The reconstruction recipe — files, then reflog, then the story. Must
/// occur EXACTLY once.
const READ_FILES_REFLOG: &str = "read the files, read the reflog";

/// The banned-commands lead — the first state-destroying command. Must
/// occur EXACTLY once (`git clean` alone also appears in the build-cache
/// text, so the needle carries the NEVER-run lead).
const NEVER_RUN_CHECKOUT: &str = "NEVER run `git checkout --`";

/// The banned-commands list's tail — reset --hard + worktree remove
/// against unremembered state, contiguous as written. Must occur EXACTLY
/// once.
const BANNED_LIST_TAIL: &str =
    "`git reset --hard`, or `git worktree remove` against unremembered state";

/// The disposable gate — reconstruction must PROVE the state disposable
/// before any destroy. Must occur EXACTLY once.
const PROVES_DISPOSABLE: &str = "proves it disposable";

/// The casualty naming — context amputation, the rule's why. Must occur
/// EXACTLY once.
const AMPUTATION_WHY: &str = "context amputation makes the orchestrator";

/// The stranger phrase — the casualty's named mechanism. Must occur
/// EXACTLY once.
const STRANGER_TO_OWN_WORK: &str = "stranger to its own work";

/// The kill-rule tie — the same verify-then-act discipline, WITHOUT
/// re-quoting the kill rule's pinned "verify-then-kill is SEQUENTIAL"
/// needle (leg (k) holds that exactly-once; a re-quote would fire it at
/// count 2). Must occur EXACTLY once.
const VERIFY_THEN_ACT: &str = "the same verify-then-act discipline";

/// The payloads/repo-state discriminator. Must occur EXACTLY once.
const PAYLOADS_THERE_REPO_STATE_HERE: &str = "payloads there, repo state here";

/// The rule's closing imperative. Must occur EXACTLY once.
const READ_RECONSTRUCT_ACT: &str = "reconstruct, then act.";

/// Hard-rules anchors bracketing the rule's position: the single-driver
/// bullet's tail it sits AFTER, and the two bullets it precedes
/// (Children-never-edit, README gate).
const CHAT_TAIL_ANCHOR: &str = "trusted not to run chat turns in the repo mid-cycle).";
const CHILDREN_NEVER_EDIT: &str =
    "Children never edit TODO.md or LEDGER.md in the main tree.";
const README_GATE_LEAD: &str = "README gate before";

/// (ap) T251 — the Hard rules' read-first recovery rule: every
/// load-bearing needle occurs EXACTLY once in LOOP-SPEC.md, all inside the
/// Hard-rules window (the spec's LAST section — `## Hard rules` through
/// end-of-file), and the rule sits AFTER the single-driver bullet's tail
/// and BEFORE the Children-never-edit and README-gate bullets. Delete the
/// rule and the needles go red at count 0; duplicate any needle and this
/// fires at count 2; move the rule out of Hard rules (or against its
/// anchor bullets) and the ordering assert dies.
#[test]
fn read_first_recovery_rule_exactly_once_inside_hard_rules() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        READ_FIRST_LEAD.starts_with("Read-first")
            && READ_FIRST_LEAD.ends_with("unremembered state"),
        "the lead needle must carry the read-first language + its scope \
         (unremembered state) verbatim"
    );
    assert!(
        TREAT_AS_LOAD_BEARING.starts_with("MUST treat")
            && TREAT_AS_LOAD_BEARING.ends_with("state as"),
        "the load-bearing needle must carry the MUST commitment verbatim"
    );
    assert!(
        NEVER_RUN_CHECKOUT.starts_with("NEVER run")
            && NEVER_RUN_CHECKOUT.ends_with("checkout --`"),
        "the banned-commands lead must carry the NEVER + the first \
         state-destroying command verbatim"
    );
    assert!(
        BANNED_LIST_TAIL.starts_with("`git reset --hard`")
            && BANNED_LIST_TAIL.ends_with("unremembered state"),
        "the banned-list tail must carry reset --hard + worktree remove \
         against unremembered state verbatim"
    );
    assert!(
        !VERIFY_THEN_ACT.contains("verify-then-kill is SEQUENTIAL"),
        "the kill-rule tie must NOT re-quote the kill rule's pinned \
         verify-then-kill needle (leg (k) holds it exactly-once)"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (READ_FIRST_LEAD, "the rule's bolded lead"),
        (TREAT_AS_LOAD_BEARING, "the load-bearing commitment"),
        (READ_FILES_REFLOG, "the files-then-reflog recipe"),
        (NEVER_RUN_CHECKOUT, "the banned-commands lead"),
        (BANNED_LIST_TAIL, "the banned-commands list's tail"),
        (PROVES_DISPOSABLE, "the proves-it-disposable gate"),
        (AMPUTATION_WHY, "the context-amputation why"),
        (STRANGER_TO_OWN_WORK, "the stranger-to-own-work mechanism"),
        (VERIFY_THEN_ACT, "the verify-then-act tie to the kill rule"),
        (PAYLOADS_THERE_REPO_STATE_HERE, "the payloads/repo-state split"),
        (READ_RECONSTRUCT_ACT, "the closing imperative"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             read-first recovery rule was deleted (or a needle was \
             rewrapped across a line break), more than one means it is \
             stated twice"
        );
    }
    let hr = spec
        .find(HARD_RULES_HEADING)
        .expect("Hard-rules heading (`## Hard rules`) present");
    // The Hard rules are the spec's LAST section — the window runs to
    // end-of-file.
    let window = &spec[hr..];
    let chat_tail = window.find(CHAT_TAIL_ANCHOR).expect(
        "the Hard-rules window must carry the single-driver bullet's tail \
         (the rule's preceding anchor)",
    );
    let lead = window.find(READ_FIRST_LEAD).unwrap_or_else(|| {
        panic!(
            "the Hard-rules window must carry the read-first recovery rule \
             — it was deleted, or moved out of Hard rules"
        )
    });
    let treat = window.find(TREAT_AS_LOAD_BEARING).expect(
        "the load-bearing commitment must sit inside the Hard-rules window \
         (rewrapped across a line break?)",
    );
    let files = window.find(READ_FILES_REFLOG).expect(
        "the files-then-reflog recipe must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let never_run = window.find(NEVER_RUN_CHECKOUT).expect(
        "the banned-commands lead must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let banned = window.find(BANNED_LIST_TAIL).expect(
        "the banned-commands list's tail must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let disposable = window.find(PROVES_DISPOSABLE).expect(
        "the proves-it-disposable gate must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let amputation = window.find(AMPUTATION_WHY).expect(
        "the context-amputation why must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let stranger = window.find(STRANGER_TO_OWN_WORK).expect(
        "the stranger-to-own-work mechanism must sit inside the \
         Hard-rules window (rewrapped?)",
    );
    let verify_act = window.find(VERIFY_THEN_ACT).expect(
        "the verify-then-act tie must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let payloads = window.find(PAYLOADS_THERE_REPO_STATE_HERE).expect(
        "the payloads/repo-state split must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let imperative = window.find(READ_RECONSTRUCT_ACT).expect(
        "the closing imperative must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let children = window.find(CHILDREN_NEVER_EDIT).expect(
        "the Hard-rules window must carry the Children-never-edit bullet \
         (the rule's following anchor)",
    );
    let readme = window.find(README_GATE_LEAD).expect(
        "the Hard-rules window must carry the README-gate bullet (the \
         rule's section stays anchored through its end)",
    );
    assert!(
        chat_tail < lead
            && lead < treat
            && treat < files
            && files < never_run
            && never_run < banned
            && banned < disposable
            && disposable < amputation
            && amputation < stranger
            && stranger < verify_act
            && verify_act < payloads
            && payloads < imperative
            && imperative < children
            && children < readme,
        "the read-first rule must sit INSIDE the Hard-rules window — \
         AFTER the single-driver bullet's tail ({chat_tail}) and BEFORE \
         the Children-never-edit ({children}) and README-gate ({readme}) \
         bullets — and read in its own written order (lead {lead}, \
         load-bearing {treat}, recipe {files}, banned lead {never_run}, \
         banned tail {banned}, disposable {disposable}, amputation \
         {amputation}, stranger {stranger}, verify-then-act {verify_act}, \
         payloads {payloads}, imperative {imperative})"
    );
}

// ---- T255 — the goal-boundary race + the recovery-PROCEEDS completion directive ----
//
// The zombie-todo no-op class fired TWICE. (1) Cycle 146's orchestrator
// exited goal-accepted with its final todo t289 ("goal_complete with the
// cycle-146 summary") un-flipped — a todo whose completion condition IS
// the `goal_complete` call can never be flipped, because acceptance ends
// the run before the queued flip lands — and the next cold launch read the
// un-flipped todo as open bookkeeping and re-claimed the finished cycle
// (watch-listed at the cycle-148 eval, trigger d1791339250-2). (2) Cycle
// 169's first segment exited goal-accepted with its final wrap todo t356
// un-flipped (07:36:09Z), and the 07:40:19Z cold launch re-verified the
// already-complete wrap, retitled t356, and goal-completed in 5 iterations
// re-claiming cycle 169 (stream events-20261007-074553.jsonl) — one cycle
// slot burned on a no-op. The second fire executed the standing re-file
// trigger, so the fix is doctrine, two clauses riding LOOP-SPEC.md:
// (a) Phase 3's wrap final-steps area gains the goal-boundary race
// paragraph — the final todo flip MUST land BEFORE the `goal_complete`
// call, and a todo whose completion condition IS `goal_complete` itself
// must NEVER be filed; (b) the T251 read-first recovery hard rule gains a
// completion directive — a cold cycle whose reconstruction finds the
// previous cycle's books CLOSED repairs the stale bookkeeping and then
// PROCEEDS into its own Phase-1 disposition (or Phase-2 work) in the SAME
// run (cycle-150's recovery-then-full-disposition, the mandated shape), it
// never `goal_complete`s on the verification alone (the cycle-169 echo,
// the counter-example). Leg (aq) pins both clauses in the T48/T64 pattern:
// every needle exactly-once file-wide, clause (a) inside the Phase-3
// window (`## Phase 3` through `## Hard rules`) AFTER the final-gates
// bullet and BEFORE the T100 tag paragraph, clause (b) inside the
// Hard-rules window AFTER the read-first rule's closing imperative
// ("reconstruct, then act.") and BEFORE the Children-never-edit bullet.
// Deleting either clause drops its needles to 0 (the RED leg was proven
// against the pre-T255 LOOP-SPEC.md before the doctrine landed); a
// duplicate fires the count-2 leg; moving either out of its window (or
// against its anchors) dies on the ordering asserts. Additive only: no
// existing pinned sentence is edited, no section renumbered.

/// Clause (a)'s lead — the bolded paragraph name + T-number. Must occur
/// EXACTLY once in LOOP-SPEC.md.
const GOAL_BOUNDARY_LEAD: &str = "final todo flip lands BEFORE `goal_complete` (T255";

/// The MUST commitment — the flip precedes the call. Must occur EXACTLY
/// once.
const FLIP_MUST_PRECEDE_CALL: &str = "MUST land BEFORE the `goal_complete` call";

/// The mechanism — acceptance ends the run, so the queued flip never
/// executes. Must occur EXACTLY once.
const ACCEPTANCE_ENDS_RUN: &str = "acceptance ends the run the instant the call lands";

/// The NEVER-file ban's object — a todo whose completion condition IS the
/// call. Must occur EXACTLY once.
const NEVER_FILE_GOAL_COMPLETE_TODO: &str = "completion condition IS `goal_complete` itself";

/// The consequence — the next cold cycle re-closes a closed wrap. Must
/// occur EXACTLY once.
const RE_CLOSES_CLOSED_WRAP: &str = "re-closes an already-closed wrap";

/// The class name. Must occur EXACTLY once.
const ZOMBIE_TODO_CLASS: &str = "zombie-todo no-op class";

/// Fire 1's evidence anchor — todo id included. Must occur EXACTLY once.
const FIRE1_ANCHOR: &str = "cycle-146 t289";

/// Fire 2's evidence anchor — todo id included. Must occur EXACTLY once.
const FIRE2_ANCHOR: &str = "cycle-169 t356";

/// Clause (a)'s closing imperative. Must occur EXACTLY once.
const FLIP_FIRST_THEN_CALL: &str = "Flip first, then call.";

/// Clause (b)'s lead — the completion directive's name + T-number. Must
/// occur EXACTLY once.
const COMPLETION_DIRECTIVE_LEAD: &str = "Completion directive (T255";

/// The PROCEEDS-never-re-claims commitment. Must occur EXACTLY once.
const PROCEEDS_NEVER_RECLAIMS: &str = "completed-recovery cycle PROCEEDS, never re-claims";

/// The closed-books test's subject leg. Must occur EXACTLY once.
const BOOKS_CLOSED: &str = "finds the previous cycle's books CLOSED";

/// The closed-books test's evidence leg — wrap pushed + main==origin/main
/// + probes/audit green, contiguous as written. Must occur EXACTLY once.
const BOOKS_CLOSED_EVIDENCE: &str = "`main == origin/main`, probes/audit green";

/// The repair step — stale bookkeeping is repaired, not re-verified forever.
/// Must occur EXACTLY once.
const REPAIRS_STALE_BOOKKEEPING: &str = "repairs the stale bookkeeping it inherited";

/// The repair's form — flip/retitle the inherited todo. Must occur EXACTLY
/// once.
const FLIP_OR_RETITLE: &str = "flip or retitle the un-flipped todo";

/// The SAME-run disposition commitment — the cold cycle proceeds into its
/// own Phase-1 disposition (Phase-2 work included) in the SAME run. Must
/// occur EXACTLY once.
const OWN_DISPOSITION_SAME_RUN: &str =
    "own Phase-1 disposition (or Phase-2 work) in the SAME run";

/// The never-completes ban — no goal_complete on the verification alone.
/// Must occur EXACTLY once.
const NEVER_COMPLETES_ON_VERIFICATION: &str =
    "`goal_complete`s on the verification alone";

/// The mandated shape — cycle-150's recovery-then-full-disposition. Must
/// occur EXACTLY once.
const MANDATED_SHAPE: &str = "recovery-then-full-disposition is the mandated shape";

/// The counter-example — the cycle-169 5-iteration echo. Must occur
/// EXACTLY once.
const COUNTER_EXAMPLE_ECHO: &str = "5-iteration echo";

/// The counter-example's close — the class's second fire. Must occur
/// EXACTLY once.
const SECOND_FIRE_CLOSE: &str = "is the counter-example and the class's second fire";

/// Clause (a)'s Phase-3 neighbors: the final-gates bullet (the
/// `goal_complete` mention) it sits AFTER, and the T100 tag paragraph it
/// precedes.
const TAG_AT_WRAP_ANCHOR: &str = "Tag at wrap (T100";

/// (aq) T255 — the goal-boundary race clause (a, Phase 3's wrap
/// final-steps area) + the recovery-PROCEEDS completion directive (clause
/// b, appended to the T251 read-first rule): every load-bearing needle
/// occurs EXACTLY once in LOOP-SPEC.md, clause (a) sits inside the
/// Phase-3 window AFTER the final-gates bullet and BEFORE the T100 tag
/// paragraph, and clause (b) sits inside the Hard-rules window AFTER the
/// read-first rule's closing imperative and BEFORE the Children-never-edit
/// bullet. Delete either clause and its needles go red at count 0;
/// duplicate any needle and this fires at count 2; move either clause out
/// of its window (or against its anchors) and the ordering assert dies.
#[test]
fn goal_boundary_race_and_recovery_proceeds_clauses_pinned() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        GOAL_BOUNDARY_LEAD.starts_with("final todo flip")
            && GOAL_BOUNDARY_LEAD.ends_with("(T255"),
        "clause (a)'s lead needle must carry the flip-before-call language \
         + the T-number verbatim"
    );
    assert!(
        FLIP_MUST_PRECEDE_CALL.starts_with("MUST land BEFORE")
            && FLIP_MUST_PRECEDE_CALL.ends_with("call"),
        "the MUST needle must carry the flip-precedes-call commitment verbatim"
    );
    assert!(
        NEVER_FILE_GOAL_COMPLETE_TODO.starts_with("completion condition IS")
            && NEVER_FILE_GOAL_COMPLETE_TODO.ends_with("itself"),
        "the NEVER-file needle must carry the banned todo shape verbatim"
    );
    assert!(
        FIRE1_ANCHOR.starts_with("cycle-146") && FIRE1_ANCHOR.ends_with("t289"),
        "fire 1's anchor must name the cycle AND the todo id (t289)"
    );
    assert!(
        FIRE2_ANCHOR.starts_with("cycle-169") && FIRE2_ANCHOR.ends_with("t356"),
        "fire 2's anchor must name the cycle AND the todo id (t356)"
    );
    assert!(
        PROCEEDS_NEVER_RECLAIMS.contains("PROCEEDS")
            && PROCEEDS_NEVER_RECLAIMS.ends_with("never re-claims"),
        "clause (b)'s commitment needle must carry PROCEEDS + never \
         re-claims verbatim"
    );
    assert!(
        BOOKS_CLOSED.starts_with("finds the previous")
            && BOOKS_CLOSED.ends_with("books CLOSED"),
        "the closed-books needle must carry the reconstruction verdict \
         verbatim"
    );
    assert!(
        BOOKS_CLOSED_EVIDENCE.contains("`main == origin/main`")
            && BOOKS_CLOSED_EVIDENCE.ends_with("probes/audit green"),
        "the closed-books evidence needle must carry all three probes \
         (wrap pushed named by its lead, main==origin/main, probes/audit) \
         verbatim"
    );
    assert!(
        MANDATED_SHAPE.starts_with("recovery-then-full-disposition")
            && MANDATED_SHAPE.ends_with("mandated shape"),
        "the mandated-shape needle must name cycle-150's shape verbatim"
    );
    assert!(
        !BOOKS_CLOSED.contains("reconstruct, then act."),
        "the closed-books needle must NOT re-quote the T251 rule's pinned \
         closing imperative (leg (ap) holds it exactly-once)"
    );
    let spec = loop_spec();
    for (needle, what) in [
        (GOAL_BOUNDARY_LEAD, "clause (a)'s bolded lead"),
        (FLIP_MUST_PRECEDE_CALL, "the flip-precedes-call commitment"),
        (ACCEPTANCE_ENDS_RUN, "the acceptance-ends-the-run mechanism"),
        (NEVER_FILE_GOAL_COMPLETE_TODO, "the NEVER-file ban"),
        (RE_CLOSES_CLOSED_WRAP, "the re-closes-a-closed-wrap consequence"),
        (ZOMBIE_TODO_CLASS, "the class name"),
        (FIRE1_ANCHOR, "fire 1's cycle-146 t289 anchor"),
        (FIRE2_ANCHOR, "fire 2's cycle-169 t356 anchor"),
        (FLIP_FIRST_THEN_CALL, "clause (a)'s closing imperative"),
        (COMPLETION_DIRECTIVE_LEAD, "clause (b)'s directive lead"),
        (PROCEEDS_NEVER_RECLAIMS, "the PROCEEDS-never-re-claims commitment"),
        (BOOKS_CLOSED, "the closed-books verdict"),
        (BOOKS_CLOSED_EVIDENCE, "the closed-books evidence probes"),
        (REPAIRS_STALE_BOOKKEEPING, "the repair step"),
        (FLIP_OR_RETITLE, "the flip/retitle repair form"),
        (OWN_DISPOSITION_SAME_RUN, "the SAME-run disposition commitment"),
        (NEVER_COMPLETES_ON_VERIFICATION, "the never-completes ban"),
        (MANDATED_SHAPE, "cycle-150's mandated shape"),
        (COUNTER_EXAMPLE_ECHO, "the cycle-169 echo counter-example"),
        (SECOND_FIRE_CLOSE, "the second-fire close"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the T255 \
             clause was deleted (or a needle was rewrapped across a line \
             break), more than one means it is stated twice"
        );
    }
    // Clause (a): inside Phase 3, AFTER the final-gates bullet (the
    // `goal_complete` mention) and BEFORE the T100 tag paragraph.
    let p3 = spec
        .find(PHASE3_HEADING)
        .expect("Phase-3 heading (`## Phase 3`) present");
    let p3_end = p3
        + spec[p3..]
            .find(HARD_RULES_HEADING)
            .expect("Hard-rules heading present after Phase 3");
    let phase3 = &spec[p3..p3_end];
    let final_gates = phase3.find(FINAL_GATES_BULLET).expect(
        "the Phase-3 window must carry the final-gates bullet (clause (a)'s \
         preceding anchor)",
    );
    let lead = phase3.find(GOAL_BOUNDARY_LEAD).unwrap_or_else(|| {
        panic!(
            "the Phase-3 window must carry the goal-boundary race clause — \
             it was deleted, or moved out of Phase 3"
        )
    });
    let must = phase3.find(FLIP_MUST_PRECEDE_CALL).expect(
        "the flip-precedes-call commitment must sit inside the Phase-3 \
         window (rewrapped across a line break?)",
    );
    let acceptance = phase3.find(ACCEPTANCE_ENDS_RUN).expect(
        "the acceptance-ends-the-run mechanism must sit inside the Phase-3 \
         window (rewrapped?)",
    );
    let never_file = phase3.find(NEVER_FILE_GOAL_COMPLETE_TODO).expect(
        "the NEVER-file ban must sit inside the Phase-3 window (rewrapped?)",
    );
    let re_closes = phase3.find(RE_CLOSES_CLOSED_WRAP).expect(
        "the re-closes consequence must sit inside the Phase-3 window \
         (rewrapped?)",
    );
    let class = phase3.find(ZOMBIE_TODO_CLASS).expect(
        "the class name must sit inside the Phase-3 window (rewrapped?)",
    );
    let fire1 = phase3.find(FIRE1_ANCHOR).expect(
        "fire 1's anchor must sit inside the Phase-3 window (rewrapped?)",
    );
    let fire2 = phase3.find(FIRE2_ANCHOR).expect(
        "fire 2's anchor must sit inside the Phase-3 window (rewrapped?)",
    );
    let flip_first = phase3.find(FLIP_FIRST_THEN_CALL).expect(
        "clause (a)'s closing imperative must sit inside the Phase-3 \
         window (rewrapped?)",
    );
    let tag = phase3.find(TAG_AT_WRAP_ANCHOR).expect(
        "the Phase-3 window must carry the T100 tag paragraph (clause (a)'s \
         following anchor)",
    );
    assert!(
        final_gates < lead
            && lead < must
            && must < acceptance
            && acceptance < re_closes
            && re_closes < never_file
            && never_file < class
            && class < fire1
            && fire1 < fire2
            && fire2 < flip_first
            && flip_first < tag,
        "the goal-boundary race clause must sit INSIDE the Phase-3 window — \
         AFTER the final-gates bullet ({final_gates}) and BEFORE the T100 \
         tag paragraph ({tag}) — and read in its own written order (lead \
         {lead}, MUST {must}, acceptance {acceptance}, re-closes \
         {re_closes}, NEVER-file {never_file}, class {class}, fire1 \
         {fire1}, fire2 {fire2}, flip-first {flip_first})"
    );
    // Clause (b): inside the Hard-rules window (the spec's LAST section),
    // appended to the read-first recovery rule — AFTER its closing
    // imperative ("reconstruct, then act.") and BEFORE the
    // Children-never-edit bullet.
    let hr = spec
        .find(HARD_RULES_HEADING)
        .expect("Hard-rules heading (`## Hard rules`) present");
    let hard_rules = &spec[hr..];
    let imperative = hard_rules.find(READ_RECONSTRUCT_ACT).expect(
        "the Hard-rules window must carry the read-first rule's closing \
         imperative (clause (b)'s preceding anchor)",
    );
    let directive = hard_rules.find(COMPLETION_DIRECTIVE_LEAD).unwrap_or_else(
        || {
            panic!(
                "the Hard-rules window must carry the recovery-PROCEEDS \
                 completion directive — it was deleted, or moved out of \
                 the read-first rule"
            )
        },
    );
    let proceeds = hard_rules.find(PROCEEDS_NEVER_RECLAIMS).expect(
        "the PROCEEDS-never-re-claims commitment must sit inside the \
         Hard-rules window (rewrapped?)",
    );
    let books = hard_rules.find(BOOKS_CLOSED).expect(
        "the closed-books verdict must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let probes = hard_rules.find(BOOKS_CLOSED_EVIDENCE).expect(
        "the closed-books evidence probes must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let repairs = hard_rules.find(REPAIRS_STALE_BOOKKEEPING).expect(
        "the repair step must sit inside the Hard-rules window (rewrapped?)",
    );
    let flip_retitle = hard_rules.find(FLIP_OR_RETITLE).expect(
        "the flip/retitle repair form must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let own_disposition = hard_rules.find(OWN_DISPOSITION_SAME_RUN).expect(
        "the SAME-run disposition commitment must sit inside the \
         Hard-rules window (rewrapped?)",
    );
    let never_completes = hard_rules.find(NEVER_COMPLETES_ON_VERIFICATION).expect(
        "the never-completes ban must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let mandated = hard_rules.find(MANDATED_SHAPE).expect(
        "cycle-150's mandated shape must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let echo = hard_rules.find(COUNTER_EXAMPLE_ECHO).expect(
        "the cycle-169 echo counter-example must sit inside the Hard-rules \
         window (rewrapped?)",
    );
    let second_fire = hard_rules.find(SECOND_FIRE_CLOSE).expect(
        "the second-fire close must sit inside the Hard-rules window \
         (rewrapped?)",
    );
    let children = hard_rules.find(CHILDREN_NEVER_EDIT).expect(
        "the Hard-rules window must carry the Children-never-edit bullet \
         (the directive's following anchor)",
    );
    assert!(
        imperative < directive
            && directive < proceeds
            && proceeds < books
            && books < probes
            && probes < repairs
            && repairs < flip_retitle
            && flip_retitle < own_disposition
            && own_disposition < never_completes
            && never_completes < mandated
            && mandated < echo
            && echo < second_fire
            && second_fire < children,
        "the completion directive must sit INSIDE the Hard-rules window — \
         AFTER the read-first rule's closing imperative ({imperative}) and \
         BEFORE the Children-never-edit bullet ({children}) — and read in \
         its own written order (lead {directive}, PROCEEDS {proceeds}, \
         books {books}, probes {probes}, repairs {repairs}, flip/retitle \
         {flip_retitle}, own disposition {own_disposition}, \
         never-completes {never_completes}, mandated {mandated}, echo \
         {echo}, second-fire {second_fire})"
    );
}

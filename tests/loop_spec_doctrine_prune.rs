//! T261 — doctrine-prune pins: triage-on-change, backfill batching at the
//! wrap, the weekly README cold-read audit.
//!
//! Operator 2026-10-07 asked "do we have doctrine that's no longer
//! needed?" and three cycle-20-era rules were pruned, each replaced by a
//! MECHANICAL trigger (never a vibe — the T189-lane rule: prune by rule):
//!
//! (a) eval-triage records were per candidate per eval (LOOP-SPEC Phase 1)
//! — dozens of `decision_log` writes recording "no" repeatedly on a quiet
//! loop; now ONE record per candidate whose disposition CHANGED since the
//! last eval, keyed by change-detection against the previous eval's
//! recorded dispositions (a quiet eval writes ZERO, a changed disposition
//! exactly ONE, and the record NAMES the change).
//!
//! (b) `outcome` backfills were scattered per item per cycle (one record
//! per id at each row flip, one Outcomes entry per landing); now they
//! batch at the WRAP BOUNDARY — one append pass per cycle, still one
//! record per id (record honesty unchanged, the F13 id→label join
//! intact), and ONE Outcomes entry per cycle carrying the wrap's item
//! table (the cycle-79-style one-line backfills become the wrap's table).
//!
//! (c) the README cold-read usability audit ran on EVERY eval (META-META
//! §6) and kept re-writing the same finding; now it runs WEEKLY, keyed by
//! a UTC-week marker (`README-audit:` in EVALUATION.md) — a same-week
//! eval skips it — while docs-vs-reality drift stays a per-eval surface
//! (the digest/delta surfacing is mechanical and cheap).
//!
//! These pins assert the pruned doctrine exists, NAMES its trigger, and
//! sits where the spec requires — plus the prune's negative half: the old
//! cadence sentences must be GONE (a partial revert that keeps the new
//! sentences while restoring the old per-flip/per-item/per-eval rules is
//! the regression class each negative leg closes). No production code
//! changes; doctrine-only. The pin file greps LOOP-SPEC.md and
//! META-META-SPEC.md only — never a spec prose copy (the T67 self-match
//! lesson).
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree. T78 idiom: multi-word needles are matched
//! against whitespace-collapsed text because the doctrine prose wraps
//! mid-phrase. Deletion-proof convention: every count leg doubles as the
//! deletion check (delete the sentence → count 0 → RED); the
//! GREEN → delete → RED → restore → GREEN hand-check was run before
//! committing and is stated in the commit message.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

fn meta_meta_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading META-META-SPEC.md from the runtime checkout: {e}"))
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
const PHASE3_HEADING: &str = "## Phase 3";
const HARD_RULES_HEADING: &str = "## Hard rules";
const STEP5_HEADING: &str = "5. **";
const STEP6_HEADING: &str = "6. **";
const WRITE_EVAL_HEADING: &str = "## Write `EVALUATION.md`";
const EXTEND_TODO_HEADING: &str = "## Extend `TODO.md`";

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

// ---- prune (a): triage records ride disposition CHANGE ----

/// The Phase-1 lead: the cadence rule's bolded name + T-number. Must occur
/// EXACTLY once in LOOP-SPEC.md.
const TRIAGE_LEAD: &str = "Triage records ride disposition CHANGE, not the calendar (T261)";

/// The mechanical trigger, named at the rule itself — Phase-1 window only
/// (the Phase-3 restatement carries its own needle below).
const CHANGE_DETECTION_P1: &str = "the mechanical trigger is change-detection";

/// The quiet-eval consequence (req 5 pin 1): a quiet eval writes ZERO
/// triage records.
const QUIET_ZERO: &str = "writes ZERO triage records";

/// The changed-disposition consequence (req 5 pin 2): exactly ONE record,
/// with the record-class named.
const EXACTLY_ONE: &str = "writes exactly ONE `eval-triage`";

/// The change the record NAMES — the spec's disposition-change examples
/// must survive (they define what "changed" means mechanically).
const NEW_REJECT: &str = "a new reject, a reject→file, a file→abandon";

/// The Phase-1 change-detection baseline — what the previous eval's
/// dispositions are read from (the mechanical input, not memory).
const PRIOR_BASELINE: &str = "the previous eval's recorded dispositions";

/// (a) The triage-on-change rule: lead, trigger, both consequences, the
/// named change examples, and the baseline — each EXACTLY once inside the
/// Phase-1 window (flat-matched). Deleting the rule drops every count to
/// 0; restoring the old per-eval cadence alongside it duplicates the
/// lead; rewording away the trigger or a consequence breaks its leg.
#[test]
fn triage_on_change_rule_lives_in_phase1_with_trigger_and_consequences() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        TRIAGE_LEAD.contains("disposition CHANGE") && TRIAGE_LEAD.ends_with("(T261)"),
        "the lead needle must carry the cadence rule's name + T-number verbatim"
    );
    assert!(
        QUIET_ZERO.starts_with("writes ZERO") && QUIET_ZERO.ends_with("triage records"),
        "the quiet-eval needle must carry the zero-records consequence verbatim"
    );
    assert!(
        EXACTLY_ONE.starts_with("writes exactly ONE") && EXACTLY_ONE.contains("`eval-triage`"),
        "the changed-disposition needle must carry the exactly-one consequence \
         with the record class named"
    );
    assert!(
        NEW_REJECT.contains("reject→file") && NEW_REJECT.contains("file→abandon"),
        "the change-examples needle must carry the spec's three examples"
    );
    let spec = flat(&loop_spec());
    let p1 = window(
        &spec,
        &flat(PHASE1_HEADING),
        &flat(PHASE2_HEADING),
        "Phase-1",
    );
    for (needle, what) in [
        (TRIAGE_LEAD, "the triage-on-change lead"),
        (CHANGE_DETECTION_P1, "the change-detection trigger"),
        (QUIET_ZERO, "the quiet-eval zero-records consequence"),
        (EXACTLY_ONE, "the changed-disposition exactly-one consequence"),
        (NEW_REJECT, "the disposition-change examples"),
        (PRIOR_BASELINE, "the change-detection baseline"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            p1.matches(&needle).count(),
            1,
            "LOOP-SPEC Phase 1 must state {what} exactly once — zero means \
             the T261 triage-on-change rule was deleted or the needle \
             rewrapped, more than one means it is stated twice"
        );
    }
}

/// (a.3) The Phase-3 restatement: the decisions bullet carries the same
/// cadence at the accounting surface — the T261 change-detection trigger
/// named and the quiet-eval zero — each EXACTLY once inside the Phase-3
/// wrap window (the existing class-token pins in
/// loop_spec_decision_records.rs still hold: this leg adds the cadence
/// wording, it does not duplicate the class tokens).
#[test]
fn phase3_decisions_bullet_carries_the_change_trigger_and_quiet_zero() {
    let spec = flat(&loop_spec());
    let p3 = window(
        &spec,
        &flat(PHASE3_HEADING),
        &flat(HARD_RULES_HEADING),
        "Phase-3",
    );
    for (needle, what) in [
        (
            "the T261 change-detection trigger",
            "the Phase-3 cadence restatement's trigger",
        ),
        ("a quiet eval writes none", "the Phase-3 quiet-eval zero"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            p3.matches(&needle).count(),
            1,
            "the Phase-3 wrap window must state {what} exactly once — zero \
             means the T261 cadence restatement was dropped from the \
             decisions bullet, more than one means it is stated twice"
        );
    }
}

/// (a.neg) The OLD cadence must be GONE: "every filed row AND every
/// weighed-and-rejected candidate" (the per-candidate-per-eval rule) and
/// the Phase-3 "filed rows AND rejected candidates" parenthetical — count
/// ZERO file-wide, flat-matched. A partial revert that reinstates the old
/// sentence alongside the new rule fires this at count 1.
#[test]
fn old_per_candidate_triage_cadence_is_gone() {
    let spec = flat(&loop_spec());
    for (needle, what) in [
        (
            "Every filed row AND every weighed-and-rejected candidate",
            "the old Phase-1 per-candidate rule",
        ),
        (
            "filed rows AND rejected candidates",
            "the old Phase-3 parenthetical",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            0,
            "LOOP-SPEC must NOT carry {what} anymore ({needle:?}) — the T261 \
             prune replaced the per-candidate-per-eval cadence with \
             triage-on-change; a reinstated copy is a partial revert"
        );
    }
}

// ---- prune (b): outcome backfills batch at the wrap ----

/// The step-5 backfill-batching lead: the per-flip write is FORBIDDEN, the
/// wrap pass is the single backfill point.
const BACKFILL_BATCH_LEAD: &str = "backfills are NOT written here";

/// The step-5 Outcomes-batching lead: ONE entry per cycle, written at wrap.
const OUTCOMES_BATCH_LEAD: &str =
    "Outcomes batch at the wrap (T261; the wrap-boundary trigger)";

/// The named trigger — BOTH step-5 carriers (backfills + Outcomes) name
/// the wrap boundary, so the count is exactly TWO in the step-5 window.
const WRAP_BOUNDARY_TRIGGER: &str = "the wrap-boundary trigger";

/// The record-honesty clause: batching moves the TIMING, not the shape —
/// still one record per id, the F13 join unchanged.
const ONE_RECORD_PER_ID: &str = "Still one record per id";

/// The wrap table, named twice in step 5 (the table's shape + the
/// cycle-79 backfills' promotion into it).
const WRAP_TABLE_SHAPE: &str = "the wrap's table, one row per item";
const CYCLE79_PROMOTION: &str = "one-line backfills become the wrap's table";

/// The step-5 per-cycle entry count — ONE Outcomes entry per cycle
/// (capital ONE; the Phase-3 restatement uses lowercase and is pinned
/// separately by its window).
const ONE_ENTRY_PER_CYCLE_STEP5: &str = "ONE Outcomes entry per cycle";

/// (b) The step-5 batching rule: both leads, the trigger (count TWO — both
/// carriers name it), the record-honesty clause, the wrap-table shape, and
/// the per-cycle entry count — inside step 5's window (flat-matched).
/// Deleting either carrier drops its leg to 0; restoring the old
/// per-flip/per-item sentences alongside duplicates the trigger past two.
#[test]
fn step5_batches_backfills_and_outcomes_at_the_wrap_boundary() {
    assert!(
        WRAP_BOUNDARY_TRIGGER.starts_with("the wrap-boundary"),
        "the trigger needle must name the wrap boundary verbatim"
    );
    let spec = flat(&loop_spec());
    let s5 = window(&spec, STEP5_HEADING, STEP6_HEADING, "step-5");
    for (needle, what, want) in [
        (
            BACKFILL_BATCH_LEAD,
            "the backfill-batching lead",
            1,
        ),
        (
            OUTCOMES_BATCH_LEAD,
            "the Outcomes-batching lead",
            1,
        ),
        (
            WRAP_BOUNDARY_TRIGGER,
            "the wrap-boundary trigger (both carriers name it)",
            2,
        ),
        (
            ONE_RECORD_PER_ID,
            "the record-honesty clause (one record per id)",
            1,
        ),
        (WRAP_TABLE_SHAPE, "the wrap table's shape", 1),
        (
            CYCLE79_PROMOTION,
            "the cycle-79 one-line backfills' promotion into the wrap table",
            1,
        ),
        (
            ONE_ENTRY_PER_CYCLE_STEP5,
            "the one-Outcomes-entry-per-cycle count",
            1,
        ),
    ] {
        let needle = flat(needle);
        assert_eq!(
            s5.matches(&needle).count(),
            want,
            "LOOP-SPEC step 5 must state {what} exactly {want} time(s) — \
             zero means the T261 batching rule was deleted or the needle \
             rewrapped, a different count means it is stated the wrong \
             number of times"
        );
    }
}

/// (b.3) The Phase-3 restatements: the decisions bullet's
/// "`outcome` backfills batched at the wrap" and the Outcomes bullet's
/// "one Outcomes entry per cycle, written at the wrap boundary" — each
/// EXACTLY once inside the Phase-3 wrap window (the "`outcome` backfills"
/// class-token count is loop_spec_decision_records.rs's pin and still
/// holds: this leg pins the cadence wording AROUND it).
#[test]
fn phase3_bullets_carry_the_wrap_batching_restatement() {
    let spec = flat(&loop_spec());
    let p3 = window(
        &spec,
        &flat(PHASE3_HEADING),
        &flat(HARD_RULES_HEADING),
        "Phase-3",
    );
    for (needle, what) in [
        (
            "`outcome` backfills batched at the wrap",
            "the decisions bullet's wrap-batching cadence",
        ),
        (
            "one Outcomes entry per cycle, written at the wrap boundary",
            "the Outcomes bullet's wrap-batching cadence",
        ),
    ] {
        let needle = flat(needle);
        assert_eq!(
            p3.matches(&needle).count(),
            1,
            "the Phase-3 wrap window must state {what} exactly once — zero \
             means the T261 restatement was dropped, more than one means it \
             is stated twice"
        );
    }
}

/// (b.neg) The OLD scatter must be GONE: the per-flip `outcome`-record
/// sentence and the per-item Outcomes-entry rule (both in step 5), plus
/// the Phase-3 "per-item entries were written at each landing" clause —
/// count ZERO file-wide, flat-matched. A partial revert that reinstates
/// the per-item cadence beside the batching rule fires this at count 1.
#[test]
fn old_per_item_scatter_is_gone() {
    let spec = flat(&loop_spec());
    for (needle, what) in [
        (
            "The row-flip commit appends an `outcome` record",
            "the old per-flip backfill sentence",
        ),
        (
            "Outcomes are per-item",
            "the old per-item Outcomes rule",
        ),
        (
            "per-item entries were written at each landing",
            "the old Phase-3 per-item-landing clause",
        ),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            0,
            "LOOP-SPEC must NOT carry {what} anymore ({needle:?}) — the T261 \
             prune batched the backfills and the Outcomes entries at the \
             wrap; a reinstated copy is a partial revert"
        );
    }
}

// ---- prune (c): the README cold-read audit is weekly ----

/// The §6 lead: the weekly cadence + the trigger name.
const WEEKLY_LEAD: &str = "WEEKLY, not per-eval";

/// The mechanical trigger, stated as the UTC week.
const UTC_WEEK_TRIGGER: &str = "the mechanical trigger is the UTC week";

/// The week computation — mechanical, not judgment.
const WEEK_CMD: &str = "date -u +%G-W%V";

/// The marker in EVALUATION.md.
const MARKER: &str = "`README-audit:` marker line in EVALUATION.md";

/// The req-5 pin 3: a same-week eval SKIPS the cold-read.
const SAME_WEEK_SKIP: &str = "a same-week eval SKIPS the cold-read";

/// The marker's semantics — it names the week it last ran.
const MARKER_SEMANTICS: &str = "the marker names the week it last ran";

/// The per-eval half that never skips — the digest/delta drift surfacing.
const DRIFT_NEVER_SKIPS: &str = "per-eval half that NEVER skips";

/// The corpus item's cadence split: drift every eval, cold-read weekly.
const ITEM6_DRIFT_PER_EVAL: &str = "docs-vs-reality drift every eval";
const ITEM6_WEEKLY_TRIGGER: &str = "runs on the T261 UTC-week trigger";

/// (c) The weekly README audit: the lead, the trigger, the week command,
/// the marker, the same-week skip, the marker semantics, and the
/// never-skips drift half — inside META-META-SPEC's Write-EVALUATION
/// window (the corpus item sits before it; both pinned here). The
/// eval_outcomes_carry.rs census pins (`Outcomes` exactly twice, one
/// `verbatim` in this window) must hold AROUND this text — this leg
/// asserts the weekly rule carries none of those tokens itself.
#[test]
fn readme_cold_read_audit_is_weekly_with_utc_marker_skip() {
    assert!(
        WEEKLY_LEAD == "WEEKLY, not per-eval",
        "the lead needle must be the weekly-cadence phrase verbatim"
    );
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    let wev = window(
        &spec,
        &flat(WRITE_EVAL_HEADING),
        &flat(EXTEND_TODO_HEADING),
        "Write-EVALUATION",
    );
    for (needle, what) in [
        (WEEKLY_LEAD, "the weekly-cadence lead"),
        (UTC_WEEK_TRIGGER, "the UTC-week trigger"),
        (WEEK_CMD, "the mechanical week computation"),
        (MARKER, "the README-audit marker"),
        (SAME_WEEK_SKIP, "the same-week skip"),
        (MARKER_SEMANTICS, "the marker's last-run semantics"),
        (DRIFT_NEVER_SKIPS, "the never-skips drift half"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            wev.matches(&needle).count(),
            1,
            "META-META-SPEC §6 must state {what} exactly once — zero means \
             the T261 weekly-audit rule was deleted or the needle \
             rewrapped, more than one means it is stated twice"
        );
    }
    // The corpus item's cadence split — drift per-eval, cold-read weekly —
    // lives BEFORE the Write-EVALUATION window (in the Read-first list),
    // so it is pinned file-wide instead.
    for (needle, what) in [
        (ITEM6_DRIFT_PER_EVAL, "the corpus item's per-eval drift half"),
        (ITEM6_WEEKLY_TRIGGER, "the corpus item's weekly-trigger half"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "META-META-SPEC's Read-first corpus item 6 must state {what} \
             exactly once — zero means the T261 cadence split was dropped \
             from the corpus list, more than one means it is stated twice"
        );
    }
    // Collision guard: the weekly rule must not disturb the
    // eval_outcomes_carry census pins it shares the window with.
    assert_eq!(
        wev.matches("Outcomes").count(),
        2,
        "the Write-EVALUATION window must still carry `Outcomes` exactly \
         twice (the eval_outcomes_carry census) — the T261 weekly rule \
         drifted an extra copy in"
    );
    assert_eq!(
        wev.matches("verbatim").count(),
        1,
        "the Write-EVALUATION window must still carry `verbatim` exactly \
         once (the eval_outcomes_carry census) — the T261 weekly rule \
         drifted an extra copy in"
    );
}

/// (c.neg) The OLD per-eval cadence must be GONE: the corpus item's
/// "docs-vs-reality drift AND a full cold-read usability audit" coupling
/// (the audit ran on EVERY eval) — count ZERO file-wide, flat-matched.
/// A partial revert that re-couples the audit to every eval fires this.
#[test]
fn old_per_eval_readme_audit_cadence_is_gone() {
    let spec = flat(&meta_meta_spec());
    assert_eq!(
        spec.matches("drift AND a full cold-read usability audit").count(),
        0,
        "META-META-SPEC must NOT couple the cold-read audit to every eval \
         anymore — the T261 prune moved it to the UTC-week trigger; a \
         reinstated coupling is a partial revert"
    );
}

/// The triggers are named in BOTH doctrine files the spec names
/// (requirement 4): change-detection and the wrap boundary in LOOP-SPEC,
/// the UTC week in META-META-SPEC — one file-wide presence leg per file so
/// a trigger's deletion from either file goes red even if its rule's own
/// window legs were also touched.
#[test]
fn each_prunes_mechanical_trigger_is_named_in_its_doctrine_files() {
    let ls = flat(&loop_spec());
    for (needle, what) in [
        ("change-detection", "the triage prune's trigger"),
        ("the wrap-boundary trigger", "the backfill prune's trigger"),
    ] {
        assert!(
            ls.contains(needle),
            "LOOP-SPEC must name {what} ({needle:?}) — the T261 prunes keep \
             a mechanical trigger each, named in doctrine"
        );
    }
    let mm = flat(&meta_meta_spec());
    assert!(
        mm.contains("the UTC week"),
        "META-META-SPEC must name the README-audit prune's trigger (the UTC \
         week) — the T261 prunes keep a mechanical trigger each, named in \
         doctrine"
    );
}

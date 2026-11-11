//! T362 — orchestrator grind delegation: fixture/gate iteration moves to
//! glm children; inline bash budget + shape metric.
//!
//! The T361 cycle ran 95 inline bash calls by the kimi orchestrator —
//! fixture battery iterations, gate windows, consistency runs, heredoc
//! writes — while the validation rigor itself (RED-proven fixtures, kimi
//! verdicts) was right. The driver was wrong: the most expensive model in
//! the routing table was doing mechanical per-fixture grind the T81
//! coding model exists for. The remedy is three surfaces, each pinned
//! here: (1) LOOP-SPEC §2 gains a **Grind delegation (T362)** block
//! between step 6's budget check and the Trivial-row bundling heading —
//! fixture iteration loops, gate-window driving, and battery re-runs are
//! delegated to glm children by default, with TWO carve-outs (the verdict
//! remains kimi's; items under ~20 steps may stay inline) and an inline
//! bash budget (< 20 orchestrator inline-bash calls per cycle); (2) the
//! same block names the new `eval-triage` class `orchestrator-inline-grind`
//! for over-budget cycles with no delegate covering the phase; (3) the
//! Phase-3 Outcomes pin mandates one GENERATED shape line per cycle
//! (`scripts/shape_metric.sh`), so the class is measurable over time
//! instead of discovered by the operator reading a cycle log by hand.
//!
//! Compat surface pinned by construction: validation rigor unchanged —
//! the kimi verdict carve-out is the load-bearing sentence, so delegating
//! the grind can never read as delegating the verdict (validator
//! independence, the T81 family-independence rule).
//!
//! Legs:
//! (a) doctrine pins — the delegation default, the inline bash budget and
//!     its over-budget clause, the two carve-outs, the shape-line
//!     sentence, and the orchestrator-inline-grind class sentence, each
//!     as ONE flat needle at the strongest exactly-once scope the token
//!     supports (the T78 flat idiom: the sentences wrap mid-phrase in
//!     the file, so needles are matched against whitespace-collapsed
//!     text);
//! (b) the validator-independence pin — the carve-out sentence pins that
//!     the verdict remains kimi's, T81 reference intact;
//! (c) placement — the block sits AFTER step 6's budget-check paragraph
//!     and BEFORE the Trivial-row bundling heading (the reviewed
//!     placement; step 3's window is untouched — pinned placement tests
//!     guard it);
//! (d) fixture legs over scripts/shape_metric.sh via std::process::Command
//!     running `sh` on tempfile synthetic streams: the spec's Tests legs
//!     (inline bash below budget with the fixture phase delegated; status
//!     polls and collect results are NOT launches; empty stream; malformed
//!     trailing line tolerated).
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree.

use std::path::Path;
use std::process::Command;

// ---- (a) doctrine needles ----

/// The block's title token (the bold marker itself is not pinned — the
/// wording is; a moved or duplicated block goes red here).
const BLOCK_TITLE: &str = "Grind delegation (T362)";

/// The delegation default: fixture iteration loops, gate windows, and
/// battery re-runs ride glm children; the orchestrator reviews OUTPUTS
/// and owns decisions.
const DELEGATION_DEFAULT: &str = "delegated to glm children by default; the orchestrator reviews child OUTPUTS and owns decisions, never drives per-fixture bash itself";

/// Carve-out (i) — the verdict stays kimi's (validator independence
/// untouched, the T81 family-independence rule). The em dash is spelled
/// as an escape so an editor normalization cannot silently unpin the
/// needle (\u{2014} — the sentence's two non-ASCII bytes are em dashes).
const CARVEOUT_KIMI: &str = "the verdict remains kimi's \u{2014} validator independence is untouched (the T81 family-independence rule)";

/// Carve-out (ii) — small items may stay inline; delegation overhead
/// dominates there.
const CARVEOUT_INLINE: &str = "items under ~20 steps may stay inline (delegation overhead dominates there)";

/// The inline bash budget.
const BUDGET_SENTENCE: &str =
    "routine items target fewer than 20 orchestrator inline-bash calls per cycle";

/// The over-budget clause: the wrap's Outcomes entry must name the reason.
const BUDGET_OVER: &str = "going over requires the wrap's Outcomes entry to name the reason";

/// The named delegation path for the recurring orchestrator-side
/// RED-proof-for-independence reason.
const REDPROOF_PATH: &str = "the mutation legs ride the validator's mutation-testing mandate; the orchestrator only adjudicates the verdict";

/// The new eval-triage class, named verbatim.
const CLASS_NAME: &str = "orchestrator-inline-grind";

/// The class's recording sentence: via `decision_log`, on the shape
/// line's trigger.
const CLASS_SENTENCE: &str =
    "the new `eval-triage` class `orchestrator-inline-grind` is recorded via `decision_log`";

/// The class's trigger predicate: over budget AND no delegate launch
/// covering the same phase.
const CLASS_TRIGGER: &str =
    "shows the bash count at 20 or above with no delegate launch covering the same phase";

/// The class's recurrence channel (the doctrine tune).
const CLASS_TUNE: &str = "recurrence drives the doctrine tune through the T261 channel";

// ---- (a) Phase-3 shape-line needles ----

/// The shape line's form, verbatim (wrapped across two file lines —
/// matched against flat() text).
const SHAPE_FORM: &str = "shape: bash=<bash-count> delegate=<delegate-count> (<roles>)";

/// The generation mandate: the script, the stream, the stamp.
const SHAPE_GENERATED: &str =
    "GENERATED by `scripts/shape_metric.sh` over the cycle's events stream at the prep stamp";

/// The discipline the generated figure rides.
const SHAPE_DISCIPLINE: &str = "the T305 generated-figure discipline";

/// The roles semantics: the wrap prep's own enumeration of the children
/// it dispatched.
const SHAPE_ROLES: &str = "roles is the wrap prep's own enumeration of the children it dispatched, slash-joined, e.g. impl/validate/fixup";

/// The exactly-one mandate + the over-budget mirror of the Phase-2
/// budget (the reason must be named in the SAME entry).
const SHAPE_EXACTLY_ONE: &str = "exactly one shape line";
const SHAPE_OVER: &str = "a bash count at 20 or above requires the same entry to name the reason";

// ---- (c) placement anchors (T64 loose-heading pattern) ----

const STEP6: &str = "6. **Budget check";
const TRIVIAL_HEADING: &str = "**Trivial-row bundling (T45)";
/// Step 6's budget-check paragraph TAIL — the block must sit AFTER it.
const STEP6_PARAGRAPH_TAIL: &str = "dispatching a validation round inside the margin";

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy of the spec (the T78 flat idiom): the pinned
/// sentences wrap mid-phrase, so the whole-sentence needles must match
/// whitespace-collapsed text or the pins go red on the REAL doctrine.
fn flat(spec: &str) -> String {
    spec.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Step-window helper (T64 heading-scope pattern): the raw text between
/// two markers.
fn window_between<'a>(spec: &'a str, start_marker: &str, end_marker: &str, what: &str) -> &'a str {
    let start = spec
        .find(start_marker)
        .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s start marker {start_marker:?}"));
    let end = start
        + spec[start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("LOOP-SPEC must carry {what}'s end marker {end_marker:?} after it"));
    &spec[start..end]
}

/// Run scripts/shape_metric.sh under POSIX sh against `stream`, with
/// `roles` as the trailing argv, from the package root (T48: the script
/// is resolved from the checkout the binary RUNS against). Returns the
/// raw stdout and the exit code.
fn run_shape_metric(stream: &Path, roles: &[&str]) -> (String, i32) {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let script = root.join("scripts").join("shape_metric.sh");
    let output = Command::new("sh")
        .arg(&script)
        .arg(stream)
        .args(roles)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("spawning sh {}: {e}", script.display()));
    (
        String::from_utf8(output.stdout).expect("stdout is utf-8"),
        output.status.code().unwrap_or(-1),
    )
}

/// (a) The block and its sentences occur exactly once each — flat
/// whole-file scope, the strongest the tokens support (each needle was
/// zero-hit before this row: any second statement drifts the doctrine).
#[test]
fn doctrine_sentences_occur_exactly_once() {
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        BLOCK_TITLE.starts_with("Grind delegation (T362"),
        "the block-title needle must be the T362 title verbatim"
    );
    assert!(
        DELEGATION_DEFAULT.contains("glm children")
            && DELEGATION_DEFAULT.contains("OUTPUTS")
            && DELEGATION_DEFAULT.contains("never drives per-fixture bash itself"),
        "the delegation-default needle must carry the glm default, the \
         OUTPUT review, and the no-per-fixture-bash mandate"
    );
    assert!(
        CARVEOUT_KIMI.contains("the verdict remains kimi's")
            && CARVEOUT_KIMI.contains("the T81 family-independence rule"),
        "the kimi carve-out needle must carry the verdict carve-out and the \
         T81 reference verbatim"
    );
    assert!(
        CARVEOUT_INLINE.contains('~') && CARVEOUT_INLINE.contains("may stay inline"),
        "the inline carve-out needle must carry the ~20-steps threshold"
    );
    assert!(
        BUDGET_SENTENCE.contains("fewer than 20 orchestrator inline-bash calls"),
        "the budget needle must carry the <20 inline-bash figure"
    );
    assert!(
        CLASS_SENTENCE.contains("eval-triage")
            && CLASS_SENTENCE.contains(CLASS_NAME)
            && CLASS_SENTENCE.contains("decision_log"),
        "the class needle must carry the class family, its name, and the \
         decision_log channel"
    );
    assert!(
        SHAPE_FORM.starts_with("shape: bash=")
            && SHAPE_FORM.contains("delegate=<delegate-count>")
            && SHAPE_FORM.ends_with("(<roles>)"),
        "the shape-form needle must be the line's exact template"
    );
    let spec = flat(&loop_spec());
    for (needle, what) in [
        (BLOCK_TITLE, "the grind-delegation block title"),
        (DELEGATION_DEFAULT, "the delegation default sentence"),
        (CARVEOUT_KIMI, "the kimi-verdict carve-out"),
        (CARVEOUT_INLINE, "the ~20-steps inline carve-out"),
        (BUDGET_SENTENCE, "the inline bash budget"),
        (BUDGET_OVER, "the over-budget reason clause"),
        (REDPROOF_PATH, "the RED-proof delegation path"),
        (CLASS_SENTENCE, "the orchestrator-inline-grind sentence"),
        (CLASS_TRIGGER, "the class trigger predicate"),
        (CLASS_TUNE, "the T261 tune channel"),
        (SHAPE_FORM, "the shape line's form"),
        (SHAPE_GENERATED, "the shape-line generation mandate"),
        (SHAPE_DISCIPLINE, "the T305 generated-figure discipline"),
        (SHAPE_ROLES, "the roles semantics"),
        (SHAPE_EXACTLY_ONE, "the exactly-one mandate"),
        (SHAPE_OVER, "the Phase-3 over-budget reason clause"),
    ] {
        assert_eq!(
            spec.matches(needle).count(),
            1,
            "LOOP-SPEC must state {what} exactly once — zero means the \
             sentence was deleted or reworded (the T362 doctrine is \
             unpinned), more than one means it is stated twice"
        );
    }
    // The class NAME itself is unique file-wide (it must not leak a
    // second, drifted spelling anywhere in the spec).
    assert_eq!(
        spec.matches(CLASS_NAME).count(),
        1,
        "`orchestrator-inline-grind` must be named exactly once — a second \
         occurrence is a drifted duplicate of the class name"
    );
}

/// (a) The shape-line mandate names the script: `scripts/shape_metric.sh`
/// occurs exactly once in LOOP-SPEC.md — the Phase-3 Outcomes sentence is
/// its only spec carrier, so dropping the script reference (the one
/// mechanically enforceable part of the mandate) goes red here.
#[test]
fn shape_metric_script_is_named_in_the_spec_exactly_once() {
    let spec = loop_spec();
    assert_eq!(
        spec.matches("scripts/shape_metric.sh").count(),
        1,
        "LOOP-SPEC must name scripts/shape_metric.sh exactly once — the \
         Phase-3 Outcomes sentence generates its shape line with it; zero \
         means the generation mandate was dropped, more than one means a \
         second drifted in"
    );
}

/// (b) The validator-independence pin: the carve-out sentence pins that
/// the verdict remains kimi's AFTER delegation of the grind phases — the
/// spec's Tests leg 3. The delegation default and the kimi carve-out must
/// live in the SAME block (the carve-out is a boundary on the default,
/// not a stray sentence elsewhere): both are found inside the grind
/// delegation block's span, and the carve-out sits AFTER the default
/// sentence it bounds.
#[test]
fn verdict_stays_kimis_after_delegation() {
    let spec = flat(&loop_spec());
    let title_at = spec
        .find(BLOCK_TITLE)
        .unwrap_or_else(|| panic!("the grind-delegation block must exist"));
    let default_at = spec
        .find(DELEGATION_DEFAULT)
        .expect("the delegation default must exist");
    let carveout_at = spec
        .find(CARVEOUT_KIMI)
        .expect("the kimi-verdict carve-out must exist");
    assert!(
        default_at > title_at && carveout_at > default_at,
        "the kimi carve-out must sit in the grind-delegation block, AFTER \
         the delegation default it bounds (title at {title_at}, default at \
         {default_at}, carve-out at {carveout_at}) — validator independence \
         is untouched by the delegation, the T81 family-independence rule"
    );
}

/// (c) Placement: the block lives AFTER step 6's budget-check paragraph
/// and BEFORE the Trivial-row bundling heading — inside the step-6 →
/// trivial window, with the block title ordered after the budget-check
/// paragraph's tail. Step 3's window is untouched (pinned placement tests
/// guard it; this pin only asserts the NEW block's neighborhood).
#[test]
fn block_sits_between_budget_check_and_trivial_row_bundling() {
    let spec = loop_spec();
    for (marker, what) in [
        (STEP6, "step 6 (Budget check)"),
        (TRIVIAL_HEADING, "the Trivial-row bundling heading"),
    ] {
        assert_eq!(
            spec.matches(marker).count(),
            1,
            "LOOP-SPEC must keep {what} ({marker:?}) exactly once — the \
             T362 block folds BETWEEN them, never renumbers or moves them"
        );
    }
    let window = flat(window_between(
        &spec,
        STEP6,
        TRIVIAL_HEADING,
        "step 6 → trivial-row bundling",
    ));
    let tail_at = window
        .find(STEP6_PARAGRAPH_TAIL)
        .unwrap_or_else(|| panic!("the budget-check paragraph must end inside the window"));
    let title_at = window
        .find(BLOCK_TITLE)
        .unwrap_or_else(|| panic!("the grind-delegation block must sit inside the window"));
    assert!(
        tail_at < title_at,
        "the grind-delegation block must sit AFTER step 6's budget-check \
         paragraph (paragraph tail at {tail_at}, block at {title_at})"
    );
    // The whole doctrine block rides between the two anchors: the
    // budget, both carve-outs, and the class sentence are all inside
    // the window.
    for (needle, what) in [
        (BUDGET_SENTENCE, "the inline bash budget"),
        (CARVEOUT_KIMI, "the kimi-verdict carve-out"),
        (CARVEOUT_INLINE, "the ~20-steps inline carve-out"),
        (CLASS_SENTENCE, "the orchestrator-inline-grind sentence"),
    ] {
        assert!(
            window.contains(needle),
            "{what} must live in the step-6 → trivial-row window (the \
             grind-delegation block), not elsewhere in the spec"
        );
    }
}

/// (c) The Phase-3 shape-line sentence lives inside the Outcomes bullet
/// (the one beginning "EVALUATION.md's **Outcomes** section") — the wrap
/// record's existing carrier, not a new section.
#[test]
fn shape_sentence_lives_in_the_outcomes_bullet() {
    let spec = flat(&loop_spec());
    let bullet_start = spec
        .find("EVALUATION.md's **Outcomes** section is complete and truthful")
        .unwrap_or_else(|| panic!("the Outcomes bullet must exist"));
    let next_bullet = spec[bullet_start + 1..]
        .find("- **Empty-delta wrap subjects")
        .map(|i| bullet_start + 1 + i)
        .unwrap_or_else(|| panic!("the bullet after the Outcomes bullet must exist"));
    let sentence_at = spec
        .find(SHAPE_GENERATED)
        .expect("the shape-line generation mandate must exist");
    assert!(
        bullet_start < sentence_at && sentence_at < next_bullet,
        "the shape-line sentence must live INSIDE the Outcomes bullet \
         (bullet at {bullet_start}, sentence at {sentence_at}, next bullet \
         at {next_bullet}) — the wrap's existing record, one addition"
    );
}

// ---- (d) fixture legs over scripts/shape_metric.sh ----

/// A tool_result line, composed verbatim (the events stream's shape:
/// type/name/preview/ts; `\\n` in the preview is the JSON escape, which
/// jq decodes — so a launch preview's FIRST line is what startswith
/// sees).
fn tool_result(name: &str, preview: &str) -> String {
    format!(
        "{{\"duration_ms\":10,\"is_error\":false,\"name\":\"{name}\",\"ok\":true,\
         \"preview\":\"{preview}\",\"ts\":\"2026-11-11T05:15:19.220Z\",\"type\":\"tool_result\"}}"
    )
}

/// A delegate LAUNCH preview — what the delegate tool emits on a launch
/// (verified against the real events stream: "launched: pid ...").
const LAUNCH_PREVIEW: &str = "launched: pid 99280\\nlog: /tmp/chug-loop-t362/.chug/delegate.log\\nevents: /tmp/chug-loop-t362/.chug/events.jsonl";

/// A delegate STATUS poll preview — starts with "state:", NOT a launch.
const STATUS_PREVIEW: &str = "state: running, last_iteration 3";

/// A delegate COLLECT preview — starts with other text, NOT a launch.
const COLLECT_PREVIEW: &str = "collect: goal-accepted, summary: corridor closed";

/// (d) The spec's Tests leg 1, as a fixture: a synthetic item run with
/// 5 orchestrator bash tool_results + 1 delegate launch + roles `impl`
/// yields EXACTLY `shape: bash=5 delegate=1 (impl)` — inline bash below
/// the 20-call budget WITH the delegate call present (the fixture phase
/// delegated), the T362 target shape for a routine item.
#[test]
fn synthetic_item_run_is_below_budget_with_the_fixture_phase_delegated() {
    let mut lines = Vec::new();
    for i in 0..5 {
        lines.push(tool_result("bash", &format!("fixture leg {i} output")));
    }
    lines.push(tool_result("delegate", LAUNCH_PREVIEW));
    let dir = tempfile::Builder::new()
        .prefix("t362-item-run")
        .tempdir()
        .expect("fixture dir");
    let stream = dir.path().join("events.jsonl");
    std::fs::write(&stream, lines.join("\n") + "\n").expect("writing the synthetic stream");

    let (stdout, code) = run_shape_metric(&stream, &["impl"]);
    assert_eq!(code, 0, "the script is best-effort and must exit 0");
    assert_eq!(
        stdout, "shape: bash=5 delegate=1 (impl)\n",
        "the shape line must be EXACTLY one line: 5 bash calls, 1 delegate \
         launch, the `impl` role slash-joined alone"
    );
    // The spec's Tests leg 1 assertions, over the parsed record: the
    // delegate call exists and inline bash is BELOW the 20-call budget.
    let bash = stdout
        .split("bash=")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|s| s.parse::<u32>().ok())
        .expect("the shape line must parse a bash count");
    let delegate = stdout
        .split("delegate=")
        .nth(1)
        .and_then(|s| s.split(' ').next())
        .and_then(|s| s.parse::<u32>().ok())
        .expect("the shape line must parse a delegate count");
    assert!(
        bash < 20,
        "inline bash ({bash}) must sit below the <20-call budget — the \
         synthetic item run is the delegated shape, not the T361 95-call \
         grind"
    );
    assert!(
        delegate >= 1,
        "the delegate call must exist — the fixture phase is delegated to \
         a glm child, not driven inline"
    );
}

/// (d) Status polls and collect results are NOT launches: a stream with
/// one real launch plus "state:" polls and a collect result counts ONE
/// delegate launch, not four — a delegate-watching orchestrator loop
/// must not inflate the delegate figure (else the over-budget class
/// trigger never fires for a delegate-heavy inline grind).
#[test]
fn status_polls_and_collect_results_are_not_launches() {
    let lines = [
        tool_result("bash", "gate window 1"),
        tool_result("bash", "gate window 2"),
        tool_result("delegate", LAUNCH_PREVIEW),
        tool_result("delegate", STATUS_PREVIEW),
        tool_result("delegate", STATUS_PREVIEW),
        tool_result("delegate", COLLECT_PREVIEW),
    ];
    let dir = tempfile::Builder::new()
        .prefix("t362-poll-not-launch")
        .tempdir()
        .expect("fixture dir");
    let stream = dir.path().join("events.jsonl");
    std::fs::write(&stream, lines.join("\n") + "\n").expect("writing the synthetic stream");

    let (stdout, code) = run_shape_metric(&stream, &["impl", "validate"]);
    assert_eq!(code, 0, "the script is best-effort and must exit 0");
    assert_eq!(
        stdout, "shape: bash=2 delegate=1 (impl/validate)\n",
        "the launch counts once; the state: polls and the collect result \
         count ZERO launches; the two roles are slash-joined"
    );
}

/// (d) An empty stream yields `shape: bash=0 delegate=0 ()` — empty
/// roles give empty parens, zero counts, exit 0 (best-effort: the line
/// is always printed).
#[test]
fn empty_stream_yields_zero_counts_and_empty_parens() {
    let dir = tempfile::Builder::new()
        .prefix("t362-empty-stream")
        .tempdir()
        .expect("fixture dir");
    let stream = dir.path().join("events.jsonl");
    std::fs::write(&stream, "").expect("writing the empty stream");

    let (stdout, code) = run_shape_metric(&stream, &[]);
    assert_eq!(code, 0, "the script is best-effort and must exit 0");
    assert_eq!(
        stdout, "shape: bash=0 delegate=0 ()\n",
        "an empty stream is zero counts, and no roles means empty parens"
    );
}

/// (d) A malformed trailing line is tolerated with correct counts — the
/// valid lines before it still count (jq `fromjson?` skips the bad line
/// instead of aborting), and the script still exits 0 with its one line.
#[test]
fn malformed_trailing_line_is_tolerated_with_correct_counts() {
    let mut lines = Vec::new();
    for i in 0..3 {
        lines.push(tool_result("bash", &format!("battery rerun {i}")));
    }
    lines.push(tool_result("delegate", LAUNCH_PREVIEW));
    let dir = tempfile::Builder::new()
        .prefix("t362-malformed-tail")
        .tempdir()
        .expect("fixture dir");
    let stream = dir.path().join("events.jsonl");
    let mut body = lines.join("\n") + "\n";
    body.push_str("not json at all\n");
    std::fs::write(&stream, body).expect("writing the synthetic stream");

    let (stdout, code) = run_shape_metric(&stream, &["validate", "fixup"]);
    assert_eq!(code, 0, "the script is best-effort and must exit 0");
    assert_eq!(
        stdout, "shape: bash=3 delegate=1 (validate/fixup)\n",
        "the malformed trailing line must not zero the stream — the three \
         bash calls and one launch before it still count, exit stays 0"
    );
}

/// (d) A MISSING stream is best-effort too: zero counts, exit 0, the
/// line still printed (the documented header contract — the wrap prep
/// must never abort over a shape figure).
#[test]
fn missing_stream_is_best_effort_zero_counts() {
    let dir = tempfile::Builder::new()
        .prefix("t362-missing-stream")
        .tempdir()
        .expect("fixture dir");
    let stream = dir.path().join("events.jsonl"); // never written

    let (stdout, code) = run_shape_metric(&stream, &["impl"]);
    assert_eq!(code, 0, "a missing stream must not abort the script");
    assert_eq!(
        stdout, "shape: bash=0 delegate=0 (impl)\n",
        "a missing stream yields zero counts with the roles still printed"
    );
}

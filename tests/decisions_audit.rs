//! T199 — `scripts/decisions-audit.sh` golden-section pins over a fixture
//! corpus.
//!
//! The audit is the F13 corpus-health surface the distillation work (T200
//! export, later fine-tune) reads before trusting `.chug/decisions.jsonl`,
//! so its counts must be provably the counts the corpus holds and its shape
//! must stay stable across runs (digest embedding + eyeball diffing). These
//! tests pin that against FIXTURE corpora (the repo's own `.chug/` is
//! gitignored and the live corpus grows every cycle — the grandfathered
//! violation count is asserted HERE, over a 7-violation fixture mirroring
//! the live set, never against the live corpus).
//!
//! Runs the real script via bash (no reimplementation: the tests guard the
//! script, they do not duplicate it).

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

fn script_path() -> PathBuf {
    // T48: cargo runs test binaries with cwd = the package root; resolve at
    // runtime (the compile-time env! path is wrong under the T47 shared
    // cache).
    std::env::current_dir()
        .expect("cargo sets the test cwd to the package root")
        .join("scripts/decisions-audit.sh")
}

/// Run the audit against `corpus` and return its stdout. The script reports
/// the empty shape (exit 0, stderr note) for a missing corpus, so success is
/// asserted on every leg.
fn run_audit(corpus: &Path) -> String {
    let out = Command::new("bash")
        .arg(script_path())
        .arg(corpus)
        .output()
        .expect("spawn scripts/decisions-audit.sh");
    assert!(
        out.status.success(),
        "audit exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Assert every needle occurs in `content`, each strictly after the previous
/// one — pins the six-section heading order without pinning volatile paths.
fn assert_named_in_order(content: &str, needles: &[&str]) {
    let mut cursor = 0;
    for needle in needles {
        let at = content[cursor..]
            .find(needle)
            .unwrap_or_else(|| panic!("{needle:?} missing or out of order in {content}"));
        cursor += at + needle.len();
    }
}

/// One corpus line in the LIVE corpus's shape (spaced after colons — the
/// harvest reformats serde's compact output, so the audit must read this,
/// not serde's). Field order is the documented record shape (T70).
fn record(id: &str, class: &str, subject: &str, choice: &str) -> String {
    format!(
        "{{\"id\": \"{id}\", \"ts\": 1790000000, \"class\": \"{class}\", \
         \"subject\": \"{subject}\", \"inputs\": \"evidence for {id}\", \
         \"options\": \"a | b | c\", \"choice\": \"{choice}\", \"confidence\": 0.9}}\n"
    )
}

fn write_corpus(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write fixture corpus");
    path
}

/// The six section headings in render order (the shape-stable skeleton the
/// digest and eyeball diffs rely on). The malformed-chain section (T246)
/// sits right after the unresolved-subject section — both are subject-shape
/// checks on outcome records.
const SECTION_HEADINGS: [&str; 6] = [
    "## records by class",
    "## outcome choice outside closed set (landed-clean | fixed-up | reverted): ",
    "## outcome subject resolves to no existing id: ",
    "## outcome subject resolves to another outcome record (malformed chain): ",
    "## routing/verdict records without an outcome backfill naming their id",
    "## duplicate ids: ",
];

/// The T199 fixture leg: a corpus mirroring the LIVE grandfathered set — 7
/// outcome records with prose choices (the real shapes: merge notes, a
/// correction record, a recipe) among clean records. Pins the seven count,
/// the violation ids, the decoupled subject-lint count, the per-class
/// backfill counts, and the section order, all over one fixture.
#[test]
fn audit_seven_violation_fixture_renders_the_grandfather_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    // Four routing/verdict records: r1 and r2 get backfilled, r3 gets a
    // clean outcome, r4 never does (the one missing-backfill count).
    body.push_str(&record("d1790000000-1", "validation-routing", "T199", "kimi-required"));
    body.push_str(&record("d1790000000-2", "validation-verdict", "T199", "verdict-pass"));
    body.push_str(&record("d1790000000-3", "recovery-routing", "T199-impl", "resume"));
    body.push_str(&record("d1790000000-4", "recovery-routing", "T199-recover", "abort"));
    // The 7 grandfathered prose choices, copied from the live corpus — o3's
    // subject RESOLVES (decoupling the choice violation from the subject
    // violation), the other six name TODO arcs no id matches.
    let prose: [(&str, &str, &str); 7] = [
        (
            "d1790000001-1",
            "T152 arc",
            "landed-clean — merge a59cf97 + flip 69f312d, pushed; post-merge gates \
             green via T82 fallback (family red only under nextest parallelism)",
        ),
        (
            "d1790000001-2",
            "T153 arc",
            "landed-clean — merge efa287a + flip df22034, pushed; no fix-up needed",
        ),
        (
            "d1790000001-3",
            "d1790000000-1",
            "landed-clean — correction record: the T152 outcome belongs to the T152 \
             validation-routing decision; the earlier outcome misattributed its subject",
        ),
        ("d1790000001-4", "T155", "next-cycle-recipe"),
        (
            "d1790000001-5",
            "T156",
            "landed-clean — merge 88119dc + flip in-flight, post-merge gates 1278/1278 green",
        ),
        (
            "d1790000001-6",
            "T157",
            "landed-clean — merge ab421d3 + flip, post-merge gates 1280/1280 green",
        ),
        (
            "d1790000001-7",
            "T158",
            "landed-clean — merge 7b9b2bf (keep-both conflict resolution) + flip, \
             post-merge gates 1286/1286",
        ),
    ];
    for (id, subject, choice) in prose {
        body.push_str(&record(id, "outcome", subject, choice));
    }
    // Clean outcomes: valid choices, subjects that resolve.
    body.push_str(&record("d1790000002-1", "outcome", "d1790000000-1", "landed-clean"));
    body.push_str(&record("d1790000002-2", "outcome", "d1790000000-2", "fixed-up"));
    body.push_str(&record("d1790000002-3", "outcome", "d1790000000-3", "reverted"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let audit = run_audit(&corpus);

    // Header + the five sections, in order.
    assert!(
        audit.starts_with("# decisions corpus audit — "),
        "header: {audit}"
    );
    assert_named_in_order(&audit, &SECTION_HEADINGS);
    assert!(audit.contains("records: 14\n"), "record total:\n{audit}");

    // Section 1: every class, alphabetically, with counts.
    assert_named_in_order(
        &audit,
        &[
            "  - outcome: 10\n",
            "  - recovery-routing: 2\n",
            "  - validation-routing: 1\n",
            "  - validation-verdict: 1\n",
        ],
    );

    // Section 2: count seven (the grandfather count), ids one per line,
    // sorted.
    assert!(
        audit.contains("## outcome choice outside closed set (landed-clean | fixed-up | reverted): 7\n"),
        "violation count:\n{audit}"
    );
    for id in [
        "d1790000001-1",
        "d1790000001-2",
        "d1790000001-3",
        "d1790000001-4",
        "d1790000001-5",
        "d1790000001-6",
        "d1790000001-7",
    ] {
        assert!(audit.contains(&format!("  - {id}\n")), "{id} listed:\n{audit}");
    }
    // The prose itself is NOT echoed (ids only — the count + ids contract).
    assert!(
        !audit.contains("keep-both conflict resolution"),
        "prose choice stays out of the report:\n{audit}"
    );

    // Section 3: six unresolvable subjects — o3's subject IS an existing id
    // (exact-equality resolution, same definition the write-time lint uses),
    // and the clean outcomes all resolve.
    assert!(
        audit.contains("## outcome subject resolves to no existing id: 6\n"),
        "subject count:\n{audit}"
    );
    for subject in ["  - T152 arc\n", "  - T155\n", "  - T158\n"] {
        assert!(audit.contains(subject), "{subject} listed:\n{audit}");
    }
    assert!(
        !audit.contains("  - d1790000000-1\n"),
        "resolved subject o3 is not flagged:\n{audit}"
    );

    // Section 4 (the T246 malformed-chain section): every resolving subject
    // here lands on a NON-outcome record (o3's and the clean outcomes'
    // subjects are routing/verdict records), so the chain count is zero —
    // a resolving subject that is NOT a decision id is the other defect
    // class, pinned separately below.
    assert!(
        audit.contains("## outcome subject resolves to another outcome record (malformed chain): 0\n"),
        "malformed-chain count:\n{audit}"
    );

    // Section 5: per-class counts — only r4 (recovery-routing) is
    // unbackfilled; every class in the set renders, zero or not.
    assert_named_in_order(
        &audit,
        &[
            "## routing/verdict records without an outcome backfill naming their id\n",
            "  - validation-routing: 0\n",
            "  - validation-verdict: 0\n",
            "  - recovery-routing: 1\n",
        ],
    );

    // Section 6: fixture ids are unique.
    assert!(audit.contains("## duplicate ids: 0\n"), "dups:\n{audit}");
}

/// The clean-corpus leg: every outcome choice inside the closed set, every
/// subject resolving to a NON-outcome record, every routing/verdict record
/// backfilled, ids unique — all six sections still print, at zero
/// (shape-stable for the digest).
#[test]
fn audit_clean_fixture_renders_all_six_sections_at_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T199", "kimi-required"));
    body.push_str(&record("d1790000000-2", "validation-verdict", "T199", "verdict-pass"));
    body.push_str(&record("d1790000000-3", "recovery-routing", "T199-impl", "resume"));
    body.push_str(&record("d1790000001-1", "outcome", "d1790000000-1", "landed-clean"));
    body.push_str(&record("d1790000001-2", "outcome", "d1790000000-2", "fixed-up"));
    body.push_str(&record("d1790000001-3", "outcome", "d1790000000-3", "reverted"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let audit = run_audit(&corpus);

    assert_named_in_order(
        &audit,
        &[
            "records: 6\n",
            "## records by class\n",
            "## outcome choice outside closed set (landed-clean | fixed-up | reverted): 0\n",
            "## outcome subject resolves to no existing id: 0\n",
            "## outcome subject resolves to another outcome record (malformed chain): 0\n",
            "## routing/verdict records without an outcome backfill naming their id\n",
            "  - validation-routing: 0\n",
            "  - validation-verdict: 0\n",
            "  - recovery-routing: 0\n",
            "## duplicate ids: 0\n",
        ],
    );
    assert!(audit.contains("  - outcome: 3\n"), "class rows render at zero too:\n{audit}");
    // Zero sections carry no body lines: nothing between the zero heading
    // and the next section.
    assert!(
        !audit.contains("\n  - d1790"),
        "zero sections list no ids:\n{audit}"
    );
}

/// The T246 malformed-chain leg: an outcome whose `subject` resolves to
/// ANOTHER OUTCOME record (not a logged decision id) is the defect class
/// that fired in BOTH of the last two wraps and was caught only by the NEXT
/// eval's manual jq spot-check — the fixture here mirrors the real cycle-116
/// fire (verdict d1791262814-2 backfilled by outcome d1791262925-3; the
/// flip-time outcome d1791264594-4 then subjects d1791262925-3, an outcome
/// id, instead of a decision id). The chain record RESOLVES, so the
/// unresolved-subject section stays silent — exactly why the class went
/// unseen — and the malformed-chain section names the offending outcome's
/// own id. The good backfill (subject resolves to a decision record) is
/// never flagged, and the verdict counts as backfilled: only the new
/// section tells the story.
#[test]
fn audit_flags_an_outcome_subjecting_another_outcome_as_a_malformed_chain() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    // The real cycle-116 shapes: the verdict, its GOOD backfill (subject is
    // a decision id), and the malformed flip-time outcome (subject is an
    // OUTCOME id).
    body.push_str(&record("d1791262814-2", "validation-verdict", "T245", "PASS"));
    body.push_str(&record(
        "d1791262925-3",
        "outcome",
        "d1791262814-2",
        "landed-clean",
    ));
    body.push_str(&record(
        "d1791264594-4",
        "outcome",
        "d1791262925-3",
        "landed-clean",
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let audit = run_audit(&corpus);

    assert_named_in_order(
        &audit,
        &[
            "records: 3\n",
            "## outcome subject resolves to no existing id: 0\n",
            "## outcome subject resolves to another outcome record (malformed chain): 1\n",
        ],
    );
    // The offending OUTCOME record's id is listed (its subject is not).
    assert!(
        audit.contains("  - d1791264594-4\n"),
        "the outcome-subjecting outcome is named:\n{audit}"
    );
    // The victim backfill is NOT echoed anywhere: d1791262925-3 is a
    // well-formed outcome (clean choice, subject resolves to a decision
    // record), so no section lists it.
    assert!(
        !audit.contains("  - d1791262925-3\n"),
        "the good backfill stays unflagged:\n{audit}"
    );
    // The verdict counts as backfilled (an outcome names its id — even
    // though the chain that follow-on outcome belongs to is malformed):
    // the backfill section stays all-zero, the malformed-chain section is
    // the ONLY surface that fires.
    assert_named_in_order(
        &audit,
        &[
            "## routing/verdict records without an outcome backfill naming their id\n",
            "  - validation-routing: 0\n",
            "  - validation-verdict: 0\n",
            "  - recovery-routing: 0\n",
            "## duplicate ids: 0\n",
        ],
    );
}

/// A missing corpus is an empty corpus, not a crash: the all-zero shape with
/// the path named on stderr (a fresh worktree runs the audit before any
/// decision exists).
#[test]
fn audit_missing_corpus_renders_the_empty_shape_and_names_the_path_on_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("decisions.jsonl");

    let out = Command::new("bash")
        .arg(script_path())
        .arg(&missing)
        .output()
        .expect("spawn scripts/decisions-audit.sh");
    assert!(out.status.success(), "missing corpus is not a failure");
    let audit = String::from_utf8_lossy(&out.stdout);
    assert_named_in_order(
        &audit,
        &[
            "records: 0\n",
            "## records by class\n",
            "## outcome choice outside closed set (landed-clean | fixed-up | reverted): 0\n",
            "## outcome subject resolves to no existing id: 0\n",
            "## outcome subject resolves to another outcome record (malformed chain): 0\n",
            "## routing/verdict records without an outcome backfill naming their id\n",
            "  - validation-routing: 0\n",
            "  - validation-verdict: 0\n",
            "  - recovery-routing: 0\n",
            "## duplicate ids: 0\n",
        ],
    );
    // The unrenderable path is named on stderr, never silently swapped for
    // /dev/null in the report header.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no readable corpus")
            && stderr.contains(missing.to_string_lossy().as_ref()),
        "stderr names the missing path: {stderr}"
    );
    assert!(
        audit.starts_with("# decisions corpus audit — "),
        "header still renders:\n{audit}"
    );
}

/// Duplicate ids are counted (count only — the spec pins no id list here):
/// one id appearing three times counts 1.
#[test]
fn audit_counts_duplicate_ids_once_per_duplicated_id() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "outcome", "d1790000000-9", "landed-clean"));
    body.push_str(&record("d1790000000-1", "outcome", "d1790000000-9", "fixed-up"));
    body.push_str(&record("d1790000000-1", "outcome", "d1790000000-9", "reverted"));
    body.push_str(&record("d1790000000-2", "outcome", "d1790000000-9", "landed-clean"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let audit = run_audit(&corpus);
    assert!(audit.contains("## duplicate ids: 1\n"), "one dup id:\n{audit}");
    assert!(audit.contains("records: 4\n"), "all lines still parsed:\n{audit}");
}

/// Malformed lines are dropped, not fatal (the eval-digest fromjson?
/// tolerance): a junk line between records leaves every count intact.
#[test]
fn audit_drops_malformed_lines_without_dying() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T199", "kimi-required"));
    body.push_str("this line is deliberately not json\n");
    body.push_str(&record("d1790000001-1", "outcome", "d1790000000-1", "landed-clean"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let audit = run_audit(&corpus);
    assert!(audit.contains("records: 2\n"), "junk dropped, records intact:\n{audit}");
    assert!(
        audit.contains("## outcome choice outside closed set (landed-clean | fixed-up | reverted): 0\n"),
        "clean corpus stays clean:\n{audit}"
    );
}

/// Same corpus -> byte-identical report (the digest-embedding contract).
#[test]
fn audit_is_deterministic_across_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T199", "kimi-required"));
    body.push_str(&record("d1790000001-1", "outcome", "T199-arc", "fixed-up — with a note"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let first = run_audit(&corpus);
    let second = run_audit(&corpus);
    assert_eq!(first, second, "same corpus -> byte-identical report");
}

/// The scale contract (sub-second at 10x corpus size, req 3): a 10x fixture
/// (the live corpus is ~830 records at landing) is audited in bounded time
/// and every count scales exactly — the set joins stay O(1) per record.
/// The asserted bound is deliberately generous (10s vs the ~0.2s measured)
/// so the leg pins the linear design, not machine speed.
#[test]
fn audit_stays_bounded_and_exact_at_10x_corpus_size() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    // 8300 records: pairs of (routing, backfilling outcome), every 1000th
    // outcome carrying a prose choice (5 violations) and every 1000th pair
    // skipping the backfill (5 unbackfilled routings, whose outcomes also
    // carry the one unresolvable subject shape — 5).
    for i in 0..4150 {
        let routing_id = format!("d1790000{i:04}-1");
        body.push_str(&record(&routing_id, "validation-routing", "T199", "kimi-required"));
        let choice = if i % 1000 == 0 {
            "landed-clean — merge aa00000 + flip, post-merge gates green"
        } else {
            "landed-clean"
        };
        let subject = if i % 1000 == 100 {
            "T199-arc".to_string() // unresolvable subject
        } else {
            routing_id.clone()
        };
        body.push_str(&record(
            &format!("d1790000{i:04}-2"),
            "outcome",
            &subject,
            choice,
        ));
    }
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let start = Instant::now();
    let audit = run_audit(&corpus);
    let elapsed = start.elapsed();

    assert!(audit.contains("records: 8300\n"), "all records parsed:\n{audit}");
    assert!(
        audit.contains(
            "## outcome choice outside closed set (landed-clean | fixed-up | reverted): 5\n"
        ),
        "5 prose choices at 10x:\n{audit}"
    );
    assert!(
        audit.contains("## outcome subject resolves to no existing id: 5\n"),
        "5 unresolvable subjects at 10x:\n{audit}"
    );
    assert!(
        audit.contains(
            "## outcome subject resolves to another outcome record (malformed chain): 0\n"
        ),
        "0 malformed chains at 10x — the id->class join stays O(1):\n{audit}"
    );
    assert!(
        audit.contains("  - validation-routing: 5\n"),
        "5 unbackfilled routings at 10x:\n{audit}"
    );
    assert!(
        elapsed.as_secs() < 10,
        "10x corpus audited in {elapsed:?} — the pass must stay linear"
    );
}

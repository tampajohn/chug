//! T200 — `scripts/decisions-export.sh` golden pins over fixture corpora.
//!
//! The export is the F13 distillation artifact (FEATURES.md phase 2a-ii):
//! the join of each decision record to its outcome label, in the shape a
//! future fine-tune ingests and an eval eyeballs to see what the corpus
//! actually teaches. Its counts must be provably the counts the corpus
//! holds and its bytes must stay stable across runs (determinism is a spec
//! requirement — no wall-clock fields, no locale-dependent ordering). These
//! tests pin that against FIXTURE corpora (the repo's own `.chug/` is
//! gitignored and the live corpus grows every cycle — the live 625/183/7
//! numbers are asserted in the commit message, never here).
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
        .join("scripts/decisions-export.sh")
}

#[derive(Debug)]
struct Export {
    stdout: String,
    stderr: String,
}

/// Run the export against `corpus` and capture both streams. The script
/// reports the empty shape (exit 0, stderr note + summary) for a missing
/// corpus, so success is asserted on every leg.
fn run_export(corpus: &Path) -> Export {
    let out = Command::new("bash")
        .arg(script_path())
        .arg(corpus)
        .output()
        .expect("spawn scripts/decisions-export.sh");
    assert!(
        out.status.success(),
        "export exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    Export {
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Parse every stdout line as one JSON object (the export is a JSONL stream
/// — a row that fails to parse is a shape violation, caught here).
fn rows(export: &Export) -> Vec<serde_json::Value> {
    export
        .stdout
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("export row is not one JSON object ({e}): {line}"))
        })
        .collect()
}

/// One corpus line in the LIVE corpus's shape (spaced after colons — the
/// harvest reformats serde's compact output, so the export must read this,
/// not serde's). Field order is the documented record shape (T70).
fn record_ts(id: &str, class: &str, subject: &str, choice: &str, ts: u64) -> String {
    format!(
        "{{\"id\": \"{id}\", \"ts\": {ts}, \"class\": \"{class}\", \
         \"subject\": \"{subject}\", \"inputs\": \"evidence for {id}\", \
         \"options\": \"a | b | c\", \"choice\": \"{choice}\", \"confidence\": 0.9}}\n"
    )
}

fn record(id: &str, class: &str, subject: &str, choice: &str) -> String {
    record_ts(id, class, subject, choice, 1790000000)
}

fn write_corpus(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).expect("write fixture corpus");
    path
}

/// The exact stdout the golden fixture must produce — the spec-pinned field
/// order `{id, ts, class, subject, inputs, options, choice, confidence,
/// outcome}` rendered compact (one object per line), the label inlined as
/// `{"choice":...,"ts":...}` carrying the OUTCOME's ts, unlabeled rows
/// closing with `"outcome":null`.
const GOLDEN_ROWS: &str = concat!(
    "{\"id\":\"d1790000000-1\",\"ts\":1790000000,\"class\":\"validation-routing\",",
    "\"subject\":\"T200\",\"inputs\":\"evidence for d1790000000-1\",",
    "\"options\":\"a | b | c\",\"choice\":\"kimi-required\",\"confidence\":0.9,",
    "\"outcome\":{\"choice\":\"landed-clean\",\"ts\":1790000100}}\n",
    "{\"id\":\"d1790000000-2\",\"ts\":1790000000,\"class\":\"validation-verdict\",",
    "\"subject\":\"T200\",\"inputs\":\"evidence for d1790000000-2\",",
    "\"options\":\"a | b | c\",\"choice\":\"verdict-pass\",\"confidence\":0.9,",
    "\"outcome\":null}\n",
);

/// (a) Join correctness: each decision row carries its outcome label
/// inlined — and the label's `ts` is the OUTCOME record's ts, distinct from
/// the row's own, proving a real join rather than a field copy. The row's
/// own `choice` stays the decision's choice.
#[test]
fn export_joins_each_decision_to_its_outcome_label() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record_ts(
        "d1790000000-1",
        "validation-routing",
        "T200",
        "kimi-required",
        1790000000,
    ));
    body.push_str(&record_ts(
        "d1790000000-2",
        "validation-verdict",
        "T200",
        "verdict-pass",
        1790000000,
    ));
    body.push_str(&record_ts(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "landed-clean",
        1790000100,
    ));
    body.push_str(&record_ts(
        "d1790000002-1",
        "outcome",
        "d1790000000-2",
        "fixed-up",
        1790000200,
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    let rows = rows(&export);
    assert_eq!(rows.len(), 2, "one row per non-outcome record:\n{export:?}");

    assert_eq!(rows[0]["id"], "d1790000000-1");
    assert_eq!(
        rows[0]["outcome"],
        serde_json::json!({"choice": "landed-clean", "ts": 1790000100}),
        "label inlined with the outcome's own ts:\n{export:?}"
    );
    assert_eq!(rows[0]["choice"], "kimi-required", "row keeps its own choice");

    assert_eq!(rows[1]["id"], "d1790000000-2");
    assert_eq!(
        rows[1]["outcome"],
        serde_json::json!({"choice": "fixed-up", "ts": 1790000200})
    );

    assert_eq!(
        export.stderr, "export: 2 rows, 2 labeled, 0 grandfathered choice-violations\n",
        "summary counts both labeled rows:\n{export:?}"
    );
}

/// (b) Unlabeled records carry `"outcome":null` — the self-describing shape
/// a trainer must read as "no label yet", not as a missing field.
#[test]
fn export_unlabeled_records_carry_null_outcome() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T200", "kimi-required"));
    body.push_str(&record("d1790000000-2", "recovery-routing", "T200", "resume"));
    // The only outcome names d1 — d2 stays unlabeled.
    body.push_str(&record(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "landed-clean",
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    let rows = rows(&export);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["outcome"]["choice"], "landed-clean");
    assert!(
        rows[1]["outcome"].is_null(),
        "unlabeled row carries explicit null:\n{export:?}"
    );
    // And in the raw bytes: the literal null, not an absent key.
    assert!(
        export.stdout.contains("\"choice\":\"resume\",\"confidence\":0.9,\"outcome\":null}\n"),
        "raw unlabeled row ends with the null outcome:\n{}",
        export.stdout
    );
    assert_eq!(
        export.stderr, "export: 2 rows, 1 labeled, 0 grandfathered choice-violations\n",
        "summary counts exactly the labeled row:\n{export:?}"
    );
}

/// (c) Outcome-class records appear ONLY as labels, never as training rows:
/// an outcome-only corpus emits nothing, and in a mixed corpus no row's
/// class is `outcome`.
#[test]
fn export_never_emits_outcome_records_as_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let only_outcomes = write_corpus(
        tmp.path(),
        "only-outcomes.jsonl",
        concat!(
            "{\"id\": \"o1\", \"ts\": 1790000000, \"class\": \"outcome\", ",
            "\"subject\": \"d1\", \"inputs\": \"x\", \"options\": \"landed-clean | fixed-up | reverted\", ",
            "\"choice\": \"landed-clean\", \"confidence\": 1.0}\n",
            "{\"id\": \"o2\", \"ts\": 1790000000, \"class\": \"outcome\", ",
            "\"subject\": \"T200-arc\", \"inputs\": \"x\", \"options\": \"landed-clean | fixed-up | reverted\", ",
            "\"choice\": \"reverted\", \"confidence\": 1.0}\n"
        ),
    );
    let export = run_export(&only_outcomes);
    assert!(
        export.stdout.is_empty(),
        "outcome-only corpus emits no rows:\n{export:?}"
    );
    assert_eq!(
        export.stderr, "export: 0 rows, 0 labeled, 0 grandfathered choice-violations\n",
        "empty shape stays shape-stable:\n{export:?}"
    );

    let mixed = write_corpus(
        tmp.path(),
        "mixed.jsonl",
        &format!(
            "{}{}{}",
            record("d1790000000-1", "validation-routing", "T200", "kimi-required"),
            record("d1790000001-1", "outcome", "d1790000000-1", "landed-clean"),
            record("d1790000002-1", "outcome", "d1790000000-1", "reverted"),
        ),
    );
    let export = run_export(&mixed);
    let rows = rows(&export);
    assert_eq!(rows.len(), 1, "two outcomes, one decision, one row:\n{export:?}");
    assert!(
        rows.iter().all(|r| r["class"] != "outcome"),
        "no outcome class in the rows:\n{export:?}"
    );
}

/// (d) Grandfathered history is never silently dropped: an outcome whose
/// choice fell outside the closed set (T199's write-time enum landed after
/// these records) passes through VERBATIM as the label, and the stderr
/// summary counts EVERY out-of-set outcome — including one whose subject
/// resolves to no id, which otherwise surfaces nowhere.
#[test]
fn export_passes_grandfathered_choice_through_verbatim_and_counts_it() {
    let tmp = tempfile::tempdir().unwrap();
    let prose = "landed-clean — merge aa00000 + flip 69f312d, pushed; post-merge gates \
                 green via T82 fallback";
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T200", "kimi-required"));
    body.push_str(&record("d1790000001-1", "outcome", "d1790000000-1", prose));
    // A second violation whose subject resolves to NO id: never reaches a
    // row, must still be counted (the export never silently drops history).
    body.push_str(&record("d1790000001-2", "outcome", "T200-arc", "next-cycle-recipe"));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    let rows = rows(&export);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["outcome"]["choice"], prose,
        "prose choice inlined verbatim, never clamped:\n{export:?}"
    );
    assert_eq!(
        export.stderr, "export: 1 rows, 1 labeled, 2 grandfathered choice-violations\n",
        "K counts every out-of-set outcome, resolved or not:\n{export:?}"
    );
}

/// The join is deterministic when two outcomes name one id: the FIRST
/// outcome in file order labels the row (the corpus is append-only, so file
/// order is chronology — the earliest recorded verdict wins; later
/// corrections stay in the corpus and, if prose, still count in K).
#[test]
fn export_first_outcome_labels_the_row_when_two_name_one_subject() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T200", "kimi-required"));
    body.push_str(&record_ts(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "fixed-up",
        1790000100,
    ));
    body.push_str(&record_ts(
        "d1790000002-1",
        "outcome",
        "d1790000000-1",
        "landed-clean",
        1790000200,
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    let rows = rows(&export);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0]["outcome"],
        serde_json::json!({"choice": "fixed-up", "ts": 1790000100}),
        "the FIRST outcome in file order labels the row:\n{export:?}"
    );
    assert_eq!(
        export.stderr, "export: 1 rows, 1 labeled, 0 grandfathered choice-violations\n",
        "M counts rows, not labels:\n{export:?}"
    );
}

/// (e) Determinism: same corpus -> byte-identical stdout AND stderr (no
/// wall-clock fields, no locale-dependent ordering — the training artifact
/// and its summary must be re-runnable for eyeball diffing).
#[test]
fn export_is_deterministic_across_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T200", "kimi-required"));
    body.push_str(&record("d1790000000-2", "recovery-routing", "T200-arc", "resume"));
    body.push_str(&record(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "landed-clean — merge 7b9b2bf (keep-both) + flip, gates 1286/1286",
    ));
    body.push_str(&record_ts(
        "d1790000001-2",
        "outcome",
        "d1790000000-1",
        "fixed-up",
        1790000200,
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let first = run_export(&corpus);
    let second = run_export(&corpus);
    assert_eq!(first.stdout, second.stdout, "same corpus -> identical rows");
    assert_eq!(first.stderr, second.stderr, "same corpus -> identical summary");
}

/// (f) Field order and shape, pinned byte-exact (the T62 golden-section
/// pattern): the full stdout of the golden fixture is this constant —
/// compact one-object-per-line JSONL, the nine spec-pinned fields in order,
/// the label as `{"choice":...,"ts":...}` with the outcome's ts, and the
/// summary line shape on stderr.
#[test]
fn export_golden_rows_pin_field_order_and_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record(
        "d1790000000-1",
        "validation-routing",
        "T200",
        "kimi-required",
    ));
    body.push_str(&record(
        "d1790000000-2",
        "validation-verdict",
        "T200",
        "verdict-pass",
    ));
    body.push_str(&record_ts(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "landed-clean",
        1790000100,
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    assert_eq!(export.stdout, GOLDEN_ROWS, "byte-exact row shape:\n{export:?}");
    assert_eq!(
        export.stderr, "export: 2 rows, 1 labeled, 0 grandfathered choice-violations\n",
        "byte-exact summary shape:\n{export:?}"
    );
}

/// Malformed lines drop with the audit's fromjson? tolerance (a torn tail
/// from a killed writer degrades the counts by that line, never kills the
/// run): junk between records leaves every good row and count intact.
#[test]
fn export_drops_malformed_lines_without_dying() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    body.push_str(&record("d1790000000-1", "validation-routing", "T200", "kimi-required"));
    body.push_str("this line is deliberately not json\n");
    body.push_str(&record(
        "d1790000001-1",
        "outcome",
        "d1790000000-1",
        "landed-clean",
    ));
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let export = run_export(&corpus);
    let rows = rows(&export);
    assert_eq!(rows.len(), 1, "junk dropped, good row intact:\n{export:?}");
    assert_eq!(rows[0]["outcome"]["choice"], "landed-clean");
    assert_eq!(
        export.stderr, "export: 1 rows, 1 labeled, 0 grandfathered choice-violations\n",
        "counts intact:\n{export:?}"
    );
}

/// A missing corpus is an empty corpus, not a crash: no rows, the zero
/// summary, and the path named on stderr (a fresh worktree runs the export
/// before any decision exists).
#[test]
fn export_missing_corpus_renders_the_empty_shape_and_names_the_path_on_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("decisions.jsonl");

    let export = run_export(&missing);
    assert!(export.stdout.is_empty(), "no rows from a missing corpus");
    assert!(
        export.stderr.ends_with("export: 0 rows, 0 labeled, 0 grandfathered choice-violations\n"),
        "zero summary still prints:\n{export:?}"
    );
    // The unrenderable path is named too (a separate stderr line, like the
    // audit's missing-corpus note — the summary line follows it).
    let stderr = &export.stderr;
    assert!(
        stderr.contains("no readable corpus")
            && stderr.contains(missing.to_string_lossy().as_ref()),
        "the missing path is named on stderr:\n{export:?}"
    );
}

/// The scale contract (sub-second at 10x corpus size): a 10x fixture (the
/// live corpus is ~835 records at landing) exports in bounded time with
/// every count exact — 4150 decision/outcome pairs, 5 outcomes carrying a
/// prose choice (i % 1000 == 0, subjects that resolve) and 5 naming an
/// unresolvable subject (i % 1000 == 100), so rows 4150, labeled 4145,
/// violations 5. The asserted bound is deliberately generous (10s vs the
/// measured fraction of a second) so the leg pins the linear design, not
/// machine speed.
#[test]
fn export_stays_bounded_and_exact_at_10x_corpus_size() {
    let tmp = tempfile::tempdir().unwrap();
    let mut body = String::new();
    for i in 0..4150 {
        let routing_id = format!("d1790000{i:04}-1");
        body.push_str(&record(&routing_id, "validation-routing", "T200", "kimi-required"));
        let choice = if i % 1000 == 0 {
            "landed-clean — merge aa00000 + flip, post-merge gates green"
        } else {
            "landed-clean"
        };
        let subject = if i % 1000 == 100 {
            "T200-arc".to_string() // unresolvable subject: the row stays null
        } else {
            routing_id.clone()
        };
        body.push_str(&record_ts(
            &format!("d1790000{i:04}-2"),
            "outcome",
            &subject,
            choice,
            1790000100,
        ));
    }
    let corpus = write_corpus(tmp.path(), "decisions.jsonl", &body);

    let start = Instant::now();
    let export = run_export(&corpus);
    let elapsed = start.elapsed();

    assert_eq!(
        export.stderr, "export: 4150 rows, 4145 labeled, 5 grandfathered choice-violations\n",
        "every count exact at 10x:\n{export:?}"
    );
    let lines = export.stdout.lines().count();
    assert_eq!(lines, 4150, "one row per non-outcome record at 10x");
    let rows = rows(&export);
    assert!(
        rows.iter().all(|r| r["class"] != "outcome"),
        "no outcome rows at 10x"
    );
    assert_eq!(
        rows.iter().filter(|r| !r["outcome"].is_null()).count(),
        4145,
        "5 unresolvable-subject outcomes stay null:\n{export:?}"
    );
    assert_eq!(
        rows.iter()
            .filter(|r| r["outcome"]["choice"] == "landed-clean — merge aa00000 + flip, post-merge gates green")
            .count(),
        5,
        "5 prose labels inlined verbatim:\n{export:?}"
    );
    assert!(
        elapsed.as_secs() < 10,
        "10x corpus exported in {elapsed:?} — the two passes must stay linear"
    );
}

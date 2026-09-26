//! T46 — eval digest integration tests.
//!
//! `scripts/eval-digest.sh` is a jq/awk-only pre-digest of `.chug/events*.jsonl`
//! written for the Phase-1 evaluator to read INSTEAD of raw archives, so the
//! counts it reports must be provably the counts the archives contain. These
//! tests pin that against fixture archives (the repo's own `.chug/` is
//! gitignored, so tests build their own corpus), plus the empty-corpus and
//! determinism behaviors the spec names.
//!
//! Runs the real script via bash (no reimplementation: the tests guard the
//! script, they do not duplicate it).

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn script_path() -> PathBuf {
    // T48: cargo runs test binaries with cwd = the package root; the compile-time env! path is wrong under the T47 shared cache (cycle-21) — resolve at runtime.
    std::env::current_dir()
        .expect("cargo sets the test cwd to the package root")
        .join("scripts/eval-digest.sh")
}

/// Run the digest against `root` and return the `.chug/eval-digest.md` text.
fn run_digest(root: &Path, pinned_now: &str) -> String {
    let out = Command::new("bash")
        .arg(script_path())
        .arg(root)
        .env("CHUG_DIGEST_NOW", pinned_now)
        .output()
        .expect("spawn scripts/eval-digest.sh");
    assert!(
        out.status.success(),
        "digest exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(root.join(".chug/eval-digest.md")).expect("digest written")
}

/// The spec-pinned baseline for one archive: `jq -r '.type' FILE | grep -c iteration`.
fn jq_iteration_baseline(file: &Path) -> usize {
    let out = Command::new("bash")
        .arg("-c")
        .arg("jq -r '.type' \"$1\" | grep -c iteration")
        .arg("jq-baseline")
        .arg(file)
        .output()
        .expect("spawn jq baseline");
    // grep -c exits 1 on a zero count; parse whatever it printed anyway.
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

/// One valid events archive: run_start + `iterations` iteration lines with
/// growing cumulative tokens; optionally one failed tool_result. Returns the
/// file content.
fn fixture_events(iterations: u32, with_error: bool) -> String {
    let mut body = String::new();
    body.push_str(concat!(
        "{\"type\":\"run_start\",\"ts\":\"2026-09-25T17:08:31.100Z\",\"mode\":\"run\",",
        "\"model\":\"test-model\",\"spec\":\"/repo/specs/t44-x.md\",\"cwd\":\"/tmp/w\",",
        "\"version\":\"0.1.0\",\"commit\":\"4ea73e3\",\"head_branch\":\"loop-t44\",",
        "\"head_commit\":\"4ea73e3\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}\n"
    ));
    for n in 1..=iterations {
        let tokens = 1_000 * u64::from(n);
        body.push_str(&format!(
            "{{\"input_tokens\":{tokens},\"n\":{n},\"output_tokens\":{},\"ts\":\"2026-09-25T17:08:{n:02}.000Z\",\"type\":\"iteration\"}}\n",
            10 * u64::from(n)
        ));
        if with_error && n == 1 {
            body.push_str(concat!(
                "{\"duration_ms\":7,\"is_error\":true,\"name\":\"bash\",\"ok\":false,",
                "\"preview\":\"error[E0382]: use of moved value: `x`\\n2 | let y = x;\",",
                "\"ts\":\"2026-09-25T17:08:31.500Z\",\"type\":\"tool_result\"}\n"
            ));
        }
    }
    body
}

fn write_archive(chug: &Path, name: &str, content: &str) -> PathBuf {
    let path = chug.join(name);
    std::fs::write(&path, content).expect("write fixture archive");
    path
}

/// The digest section for one events file (from its `### name` header to the
/// next section header).
fn section_of<'a>(digest: &'a str, file_name: &str) -> &'a str {
    let marker = format!("### {file_name}\n");
    let start = digest.find(&marker).unwrap_or_else(|| {
        panic!("section header for {file_name} present in digest:\n{digest}")
    });
    let rest = &digest[start + marker.len()..];
    let end = rest.find("\n### ").map(|i| i + 1).unwrap_or(rest.len());
    &rest[..end]
}

/// The `- iterations: N (last n=N)` count inside a digest section.
fn iterations_in_section(section: &str) -> u32 {
    let line = section
        .lines()
        .find(|l| l.starts_with("- iterations: "))
        .unwrap_or_else(|| panic!("iterations line present in section:\n{section}"));
    line["- iterations: ".len()..]
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .expect("iteration count is a number")
}

#[test]
fn digest_iteration_counts_match_jq_baseline_for_sampled_archives() {
    let tmp = tempfile::tempdir().unwrap();
    let chug = tmp.path().join(".chug");
    std::fs::create_dir_all(&chug).unwrap();

    // 3 sampled archives (spec bar: >=3), incl. the live-file name and the
    // T19/K2 harvest convention names.
    let samples = [
        ("events.jsonl", 5),
        ("events-20260925-170831.jsonl", 3),
        ("events-t44-impl-20260925-180900.jsonl", 7),
    ];
    for (name, iters) in samples {
        write_archive(&chug, name, &fixture_events(iters, true));
    }

    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");

    assert!(digest.contains("Read this FIRST"), "digest opens with the read-first instruction");
    assert!(
        digest.contains("generated-at: 2026-09-26T00:00:00Z"),
        "pinned clock lands in generated-at:\n{digest}"
    );
    assert!(digest.contains("newest-events-mtime:"), "staleness block present");
    assert!(
        digest.contains("- events-moved-during-generation: no"),
        "no race under test conditions:\n{digest}"
    );

    for (name, iters) in samples {
        let baseline = jq_iteration_baseline(&chug.join(name));
        assert_eq!(baseline, iters as usize, "jq baseline sanity for {name}");
        let section = section_of(&digest, name);
        assert_eq!(
            iterations_in_section(section),
            baseline as u32,
            "digest count must equal `jq -r '.type' | grep -c iteration` for {name}"
        );
        // A failure fixture must surface its error class, normalized.
        if name == "events.jsonl" {
            assert!(
                section.contains("error[EN]: use of moved value: `x` x1"),
                "error class present:\n{section}"
            );
        }
    }
}

#[test]
fn empty_chug_produces_a_valid_empty_digest() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".chug")).unwrap();

    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");

    assert!(digest.contains("(no events files in .chug/"), "empty marker:\n{digest}");
    assert!(!digest.contains("### events"), "no per-file sections:\n{digest}");
    assert!(digest.contains("events files: 0 | total iterations: 0"));
    assert!(digest.contains("- events-moved-during-generation: no"));
    assert!(digest.contains("newest-events-mtime: none"));
}

/// A missing `.chug/` directory must not crash either (the script mkdir -p's it).
#[test]
fn missing_chug_dir_is_created_and_yields_a_valid_digest() {
    let tmp = tempfile::tempdir().unwrap();
    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");
    assert!(digest.contains("(no events files in .chug/"), "valid empty digest:\n{digest}");
    assert!(tmp.path().join(".chug/eval-digest.md").is_file(), "digest written");
}

#[test]
fn malformed_lines_are_dropped_not_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    let chug = tmp.path().join(".chug");
    std::fs::create_dir_all(&chug).unwrap();

    let mut body = fixture_events(4, false);
    let pos = body.find('\n').unwrap() + 1; // junk sits BETWEEN lines, not mid-line
    body.insert_str(pos, "this line is deliberately not json\n");
    write_archive(&chug, "events.jsonl", &body);

    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");
    // The naive spec-pinned pipeline dies on a malformed line (jq aborts the
    // stream); the digest's fromjson? parse must instead drop the junk and
    // count the 4 valid iterations.
    let tolerant = Command::new("bash")
        .arg("-c")
        .arg("jq -Rr 'fromjson? | select(.type==\"iteration\") | 1' \"$1\" | wc -l")
        .arg("jq-tolerant")
        .arg(chug.join("events.jsonl"))
        .output()
        .expect("spawn jq tolerant baseline");
    let tolerant: usize = String::from_utf8_lossy(&tolerant.stdout)
        .trim()
        .parse()
        .expect("tolerant baseline is a number");
    assert_eq!(tolerant, 4, "tolerant baseline unaffected by the junk line");
    let section = section_of(&digest, "events.jsonl");
    assert_eq!(iterations_in_section(section), 4, "junk line dropped, count intact");
}

#[test]
fn digest_is_deterministic_under_a_pinned_clock() {
    let tmp = tempfile::tempdir().unwrap();
    let chug = tmp.path().join(".chug");
    std::fs::create_dir_all(&chug).unwrap();
    write_archive(&chug, "events-20260925-170831.jsonl", &fixture_events(6, true));

    let first = run_digest(tmp.path(), "2026-09-26T00:00:00Z");
    std::thread::sleep(std::time::Duration::from_millis(1100));
    let second = run_digest(tmp.path(), "2026-09-26T00:00:00Z");
    assert_eq!(first, second, "same corpus + pinned clock -> byte-identical digest");
}

#[test]
fn digest_reports_todo_status_counts_and_evaluation_age() {
    let tmp = tempfile::tempdir().unwrap();
    let chug = tmp.path().join(".chug");
    std::fs::create_dir_all(&chug).unwrap();
    write_archive(&chug, "events.jsonl", &fixture_events(2, false));
    std::fs::write(
        tmp.path().join("TODO.md"),
        "| id | title | spec | pri | status | notes |\n\
         |----|-------|------|-----|--------|-------|\n\
         | T1 | a | specs/t1.md | 1 | done | ok |\n\
         | T2 | b | specs/t2.md | 2 | todo | pending |\n\
         | T3 | c | specs/t3.md | 2 | todo | pending |\n",
    )
    .unwrap();
    std::fs::write(tmp.path().join("EVALUATION.md"), "# Eval\n").unwrap();

    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");

    assert!(
        digest.contains("- TODO.md status counts: todo=2 in-progress=0 blocked=0 done=1"),
        "status counts parsed:\n{digest}"
    );
    assert!(
        digest.contains("days since last EVALUATION.md write:"),
        "EVALUATION.md age reported:\n{digest}"
    );
}

// --- T62: golden-section pin (the stable OUTPUT SKELETON) --------------------
//
// T46's mutation testing found a survivor class: mutants in the digest's
// output FIELDS (headings, their order, a block's field order) that no test
// pinned. The tests above pin counts and behaviors; this one pins the SHAPE
// the evaluator parses-by-eye: the header line, the section-heading order,
// the per-file field-line order/labels, and the staleness labels. Volatile
// values (timestamps, token counts, durations) are matched by shape, never
// by literal (req 2) — a legitimate data change must not go red.

/// The per-file block's field-line prefixes, in the order the script emits
/// them today (T62 req c). Order + labels only; values are shape-checked.
const GOLDEN_BLOCK_FIELDS: [&str; 10] = [
    "- runs: ",
    "- iterations: ",
    "- wall: ",
    "- tokens (cumulative at last iteration): ",
    "- input context curve (cumulative, iter quartiles): ",
    "- tools: ",
    "- failed tool results: ",
    "- goal: ",
    "- aborts: ",
    "- budget_low fires: ",
];

/// The staleness block's field labels, in emission order (T62 req d).
const GOLDEN_STALENESS_FIELDS: [&str; 4] = [
    "- generated-at: ",
    "- newest-events-mtime: ",
    "- corpus age at generation: ",
    "- events-moved-during-generation: ",
];

/// Tiny full-line shape matcher for the golden pin: `#` matches one-or-more
/// ASCII digits, `*` matches any (possibly empty) character run, anything
/// else matches literally. Pins labels + punctuation while leaving volatile
/// values (timestamps, counts, durations) to the wildcards, so a legitimate
/// data change cannot go red (T62 req 2).
fn matches_shape(line: &str, pattern: &str) -> bool {
    fn go(line: &[char], pat: &[char]) -> bool {
        let Some((p, rest)) = pat.split_first() else {
            return line.is_empty();
        };
        match *p {
            '*' => (0..=line.len()).any(|n| go(&line[n..], rest)),
            '#' => {
                let n = line.iter().take_while(|c| c.is_ascii_digit()).count();
                n > 0 && go(&line[n..], rest)
            }
            c => line.first() == Some(&c) && go(&line[1..], rest),
        }
    }
    go(
        &line.chars().collect::<Vec<_>>(),
        &pattern.chars().collect::<Vec<_>>(),
    )
}

/// Every `label` occurs exactly once as a line start, in the given order.
fn assert_labels_in_order(text: &str, labels: &[&str], what: &str) {
    let mut cursor = 0;
    for label in labels {
        let at = cursor
            + text[cursor..].find(label).unwrap_or_else(|| {
                panic!("{what}: label `{label}` missing or out of order:\n{text}")
            });
        assert!(
            at == 0 || text.as_bytes()[at - 1] == b'\n',
            "{what}: label `{label}` must start its own line:\n{text}"
        );
        cursor = at + label.len();
    }
}

/// The one line of `text` starting with `label` matches `pattern`.
fn assert_line_shape(text: &str, label: &str, pattern: &str, what: &str) {
    let hits: Vec<&str> = text.lines().filter(|l| l.starts_with(label)).collect();
    assert_eq!(
        hits.len(),
        1,
        "{what}: expected exactly one `{label}` line:\n{text}"
    );
    assert!(
        matches_shape(hits[0], pattern),
        "{what}: line does not match pinned shape `{pattern}`:\n  {}",
        hits[0]
    );
}

/// One events archive exercising EVERY field line the digest can emit for a
/// file (verifying call, ok + failed tool results, accepted goal, budget
/// abort, budget_low fire, output truncation), so the golden pin covers the
/// full block skeleton. Serialization shapes mirror src/eventlog.rs.
fn golden_fixture_events() -> String {
    let mut body = String::new();
    body.push_str(concat!(
        "{\"type\":\"run_start\",\"ts\":\"2026-09-25T17:08:31.100Z\",\"mode\":\"run\",",
        "\"model\":\"golden-model\",\"spec\":\"/repo/specs/t62-golden.md\",\"cwd\":\"/tmp/w\",",
        "\"version\":\"0.1.0\",\"commit\":\"4ea73e3\",\"head_branch\":\"loop-t62\",",
        "\"head_commit\":\"4ea73e3\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}\n"
    ));
    for n in 1..=4u32 {
        let tokens = 1_000 * u64::from(n);
        body.push_str(&format!(
            "{{\"input_tokens\":{tokens},\"n\":{n},\"output_tokens\":{},\"ts\":\"2026-09-25T17:08:{:02}.000Z\",\"type\":\"iteration\"}}\n",
            10 * u64::from(n),
            31 + n
        ));
    }
    body.push_str(concat!(
        "{\"type\":\"tool_result\",\"ts\":\"2026-09-25T17:08:36.000Z\",\"name\":\"bash\",",
        "\"ok\":false,\"is_error\":true,\"duration_ms\":7,",
        "\"preview\":\"error[E0382]: use of moved value: `x`\\n2 | let y = x;\"}\n"
    ));
    body.push_str(concat!(
        "{\"type\":\"tool_result\",\"ts\":\"2026-09-25T17:08:37.000Z\",\"name\":\"read_file\",",
        "\"ok\":true,\"is_error\":false,\"duration_ms\":3,\"preview\":\"ok\"}\n"
    ));
    body.push_str("{\"type\":\"verifying\",\"ts\":\"2026-09-25T17:09:00.000Z\",\"cmd\":\"cargo test\"}\n");
    body.push_str("{\"type\":\"goal\",\"ts\":\"2026-09-25T17:09:10.000Z\",\"outcome\":\"accepted\",\"summary\":\"done\"}\n");
    body.push_str(concat!(
        "{\"type\":\"abort\",\"ts\":\"2026-09-25T17:09:20.000Z\",",
        "\"reason\":\"iteration budget exceeded\",\"model\":\"golden-model\",",
        "\"budget_kind\":\"iterations\",\"budget_max\":50}\n"
    ));
    body.push_str(concat!(
        "{\"type\":\"budget_low\",\"ts\":\"2026-09-25T17:09:25.000Z\",",
        "\"remaining_iters\":3,\"remaining_secs\":120,\"remaining_tokens\":null}\n"
    ));
    body.push_str("{\"type\":\"output_truncated\",\"ts\":\"2026-09-25T17:09:30.000Z\"}\n");
    body
}

/// T62 — the golden-section pin: header line, heading order, per-file field
/// order/labels, staleness labels. Values stay shape-pinned, never literal.
#[test]
fn golden_section_pins_the_digest_output_skeleton() {
    let tmp = tempfile::tempdir().unwrap();
    let chug = tmp.path().join(".chug");
    std::fs::create_dir_all(&chug).unwrap();
    write_archive(&chug, "events.jsonl", &golden_fixture_events());

    let digest = run_digest(tmp.path(), "2026-09-26T00:00:00Z");

    // (a) the header line's shape, exactly.
    assert!(
        digest.starts_with("# Eval digest — T46 pre-computed Phase-1 corpus summary\n"),
        "digest must open with the pinned header line:\n{digest}"
    );
    // The summary line under it carries the same four fields the empty digest
    // pins — here shape-only (repo path + counts are corpus-dependent).
    let summary = digest.lines().nth(2).expect("summary line after the header");
    assert!(
        summary.starts_with("generated-at: ")
            && summary.contains(" | repo: ")
            && summary.contains(" | events files: ")
            && summary.contains(" | total iterations: "),
        "summary line shape drifted:\n  {summary}"
    );

    // (b) the three section headings, each once, in this order.
    for heading in ["## Events files", "## Corpus inputs", "## Staleness"] {
        assert_eq!(
            digest.matches(heading).count(),
            1,
            "heading `{heading}` appears exactly once:\n{digest}"
        );
    }
    assert_labels_in_order(
        &digest,
        &[
            "# Eval digest — T46 pre-computed Phase-1 corpus summary",
            "Read this FIRST",
            "## Events files",
            "## Corpus inputs",
            "## Staleness",
        ],
        "skeleton order",
    );

    // The one file's block: from its `### name` header to the next `## `.
    let block = {
        let marker = "### events.jsonl\n";
        let start = digest.find(marker).expect("block header for events.jsonl");
        let rest = &digest[start + marker.len()..];
        let end = rest.find("\n## ").expect("## Corpus inputs closes the file block");
        &rest[..end]
    };
    assert!(
        block.trim_start().starts_with('('),
        "block opens with the size/mtime metadata line:\n{block}"
    );
    assert_line_shape(block, "(", "(# bytes | mtime *)", "block metadata");

    // (c) the block's field lines: pinned order (labels only) ...
    assert_labels_in_order(block, &GOLDEN_BLOCK_FIELDS, "field-line order");
    // ... then each field line's label/punctuation shape, values wild-carded.
    assert_line_shape(block, "- runs: ", "- runs: # | model: * | spec: * | iters-ceil: #", "runs");
    assert_line_shape(block, "- iterations: ", "- iterations: # (last n=#) | verifying calls: #", "iterations");
    assert_line_shape(block, "- wall: ", "- wall: #s (* | * -> *", "wall");
    assert_line_shape(block, "- tokens (cumulative at last iteration): ", "- tokens (cumulative at last iteration): #* in / #* out", "tokens");
    assert_line_shape(block, "- input context curve (cumulative, iter quartiles): ", "- input context curve (cumulative, iter quartiles): #*@#* -> *", "curve");
    assert_line_shape(block, "- tools: ", "- tools: *", "tools");
    assert_line_shape(block, "- failed tool results: ", "- failed tool results: # — classes (first line, digits->N):", "failed tool results");
    assert_line_shape(block, "- goal: ", "- goal: accepted #", "goal");
    assert_line_shape(block, "- aborts: ", "- aborts: #:", "aborts");
    assert_line_shape(block, "- budget_low fires: ", "- budget_low fires: # (first at remaining_iters=#) | output_truncated: #", "budget_low/output_truncated");

    // (d) the staleness block's field labels, in order, shape-pinned.
    let staleness = &digest[digest.find("## Staleness").expect("staleness heading")..];
    assert_labels_in_order(staleness, &GOLDEN_STALENESS_FIELDS, "staleness labels");
    assert_line_shape(staleness, "- generated-at: ", "- generated-at: *", "generated-at");
    assert_line_shape(staleness, "- newest-events-mtime: ", "- newest-events-mtime: * (.chug/events.jsonl)", "newest-events-mtime");
    assert_line_shape(staleness, "- corpus age at generation: ", "- corpus age at generation: *", "corpus age");
    assert_line_shape(staleness, "- events-moved-during-generation: ", "- events-moved-during-generation: no", "events-moved-during-generation");
}

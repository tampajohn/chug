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

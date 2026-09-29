//! T8 — TODO.md ↔ specs/ consistency guard.
//!
//! SELF-SPEC mandates one spec file per TODO row (`specs/t<N>-<slug>.md`),
//! but the loop once closed T1–T6 with no specs on disk and nothing noticed
//! (EVALUATION.md I10). This integration test parses the real TODO.md table
//! and rejects rows whose spec reference is malformed or dangling, so the
//! ledger of record cannot silently drift from the repo again.

use std::collections::HashSet;

const STATUSES: [&str; 4] = ["todo", "in-progress", "blocked", "done"];

/// Cells of one TODO.md table row, or `None` for anything that is not a
/// data row: non-`|` lines, the header row, and the dash separator. Shared
/// by both guards (T8 structural, T150 estimate) so their row parsing
/// cannot drift apart.
fn table_cells(raw: &str) -> Option<Vec<&str>> {
    let line = raw.trim();
    if !line.starts_with('|') {
        return None;
    }
    let cells: Vec<&str> = line
        .trim_start_matches('|')
        .trim_end_matches('|')
        .split('|')
        .map(str::trim)
        .collect();
    // Header row.
    if cells.first() == Some(&"id") {
        return None;
    }
    // Separator row (all-dash cells).
    if cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-'))
    {
        return None;
    }
    Some(cells)
}

/// Human-readable label naming a violated row: `row N`, or `row N (T<n>)`
/// when the id cell is non-empty.
fn row_label(row: usize, id: &str) -> String {
    if id.is_empty() {
        format!("row {row}")
    } else {
        format!("row {row} ({id})")
    }
}

/// Validate every data row of a TODO.md table. `spec_exists` resolves a
/// spec-cell path (`specs/t<N>-<slug>.md`) to whether the file is on disk.
/// Returns one human-readable problem per violation, each naming its row by
/// line number (and id cell when parseable). Header and separator rows are
/// skipped; non-table lines (incl. blank trailing lines) are ignored.
fn validate_todo_table(md: &str, spec_exists: impl Fn(&str) -> bool) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen_ids: HashSet<u64> = HashSet::new();
    for (idx, raw) in md.lines().enumerate() {
        let cells = match table_cells(raw) {
            Some(cells) => cells,
            None => continue,
        };
        let row = idx + 1; // 1-based, names the row in TODO.md
        let name = row_label(row, cells.first().copied().unwrap_or(""));
        if cells.len() != 6 {
            problems.push(format!(
                "{name}: expected exactly 6 cells, got {}",
                cells.len()
            ));
            continue;
        }
        let [id, _title, spec, pri, status, _notes]: [&str; 6] =
            cells.try_into().expect("length checked above");

        // id: ^T[0-9]+$, unique.
        let id_num = match id
            .strip_prefix('T')
            .filter(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
            .and_then(|digits| digits.parse::<u64>().ok())
        {
            Some(n) => n,
            None => {
                problems.push(format!("{name}: id '{id}' must match ^T[0-9]+$"));
                continue;
            }
        };
        if !seen_ids.insert(id_num) {
            problems.push(format!("{name}: duplicate id 'T{id_num}'"));
        }

        // pri: an integer.
        if pri.parse::<i64>().is_err() {
            problems.push(format!("{name}: pri '{pri}' is not an integer"));
        }

        // status: one of the SELF-SPEC states.
        if !STATUSES.contains(&status) {
            problems.push(format!(
                "{name}: status '{status}' must be one of {STATUSES:?}"
            ));
        }

        // spec: ^specs/t<N>-[a-z0-9-]+\.md$ with N == the row's id number
        // (case-insensitive on the `t`).
        if !spec_ref_matches_id(spec, id_num) {
            problems.push(format!(
                "{name}: spec '{spec}' must match specs/t{id_num}-<slug>.md"
            ));
        } else if !spec_exists(spec) {
            problems.push(format!("{name}: spec file '{spec}' does not exist"));
        }
    }
    problems
}

/// `specs/t<N>-<slug>.md` where `<N>` is the row's id number (the `t` is
/// case-insensitive) and `<slug>` is non-empty `[a-z0-9-]+`.
fn spec_ref_matches_id(spec: &str, id_num: u64) -> bool {
    let Some(rest) = spec.strip_prefix("specs/") else {
        return false;
    };
    let Some(rest) = rest.strip_prefix('t').or_else(|| rest.strip_prefix('T')) else {
        return false;
    };
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.parse::<u64>() != Ok(id_num) {
        return false;
    }
    let Some(slug) = rest[digits.len()..].strip_prefix('-') else {
        return false;
    };
    let Some(slug) = slug.strip_suffix(".md") else {
        return false;
    };
    !slug.is_empty()
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn row(id: &str, spec: &str, pri: &str, status: &str) -> String {
    format!("| {id} | some title | {spec} | {pri} | {status} | notes |\n")
}

fn table(rows: &str) -> String {
    format!("# TODO\n\n| id | title | spec | pri | status | notes |\n|----|-------|------|-----|--------|-------|\n{rows}")
}

#[test]
fn todo_md_is_consistent_with_specs_on_disk() {
    // T48: cargo runs test binaries with cwd = the package root; the compile-time env! path is wrong under the T47 shared cache (cycle-21) — resolve at runtime.
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let md = std::fs::read_to_string(root.join("TODO.md")).expect("TODO.md readable");
    let problems = validate_todo_table(&md, |spec| root.join(spec).is_file());
    assert!(
        problems.is_empty(),
        "TODO.md/spec drift detected:\n{}",
        problems.join("\n")
    );
}

#[test]
fn well_formed_fixture_passes() {
    let md = table(&format!(
        "{}{}{}",
        row("T1", "specs/t1-alpha.md", "1", "done"),
        row("T2", "specs/t2-beta-gamma.md", "3", "in-progress"),
        row("T12", "specs/T12-upper-t-is-fine.md", "4", "blocked"),
    ));
    // Even the missing-file check passes when the predicate says they exist.
    let problems = validate_todo_table(&md, |_| true);
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn malformed_rows_are_rejected_naming_the_row() {
    let md = table(&format!(
        "{}{}{}{}{}{}{}",
        row("T1", "specs/t1-ok.md", "1", "donee"),      // bad status
        row("T2", "specs/t3-wrong-number.md", "2", "todo"), // id/spec mismatch
        row("T4", "specs/t4-gone.md", "2", "done"),     // missing file
        row("Tx", "specs/tx-bad-id.md", "1", "todo"),   // bad id
        row("T5", "specs/t5-dup.md", "high", "todo"),   // non-integer pri
        row("T5", "specs/t5-dup2.md", "1", "todo"),     // duplicate id
        "| T6 | too | few | cells |\n",                 // wrong cell count
    ));
    let problems = validate_todo_table(&md, |spec| spec != "specs/t4-gone.md");
    let joined = problems.join("\n");
    for expected in [
        "row 5 (T1): status 'donee'",
        "row 6 (T2): spec 'specs/t3-wrong-number.md' must match specs/t2-<slug>.md",
        "row 7 (T4): spec file 'specs/t4-gone.md' does not exist",
        "row 8 (Tx): id 'Tx' must match ^T[0-9]+$",
        "row 9 (T5): pri 'high' is not an integer",
        "row 10 (T5): duplicate id 'T5'",
        "row 11 (T6): expected exactly 6 cells, got 4",
    ] {
        assert!(joined.contains(expected), "missing problem:\n{expected}\nin:\n{joined}");
    }
}

// ---------------------------------------------------------------------------
// T67 — spec `check:` lines never invoke `cargo test --lib`.
//
// This crate is binary-only (`src/main.rs`, no `lib.rs`), so
// `cargo test --lib` exits 101 ("no library targets found in package
// `chug`") — and `goal_complete` re-runs a spec's `check:` line as the impl
// child's goal gate. Nine child streams (t22/t25/t26/t29/t39/t42/t58/t59/
// t64) died on that unsatisfiable gate, the latest (t64) after its work was
// already committed and 14/14 green on leg 1. The lint below keeps the
// specs corpus clean; the doctrine pin keeps META-META-SPEC.md's
// convention sentence from being silently reverted.
//
// t67's own spec spells the flag `--l[i]b` (a BRE class matching the
// literal `i`) so its prose does not carry the literal token its own gate
// greps `specs/t*.md` for — the lint still matches the literal here.

const CARGO_TEST_LIB: &str = "cargo test --lib";

/// Every `specs/t*.md` file on disk (sorted), mirroring the shell glob the
/// spec's acceptance grep uses. Asserts an implausible-shrink floor so the
/// lint cannot pass vacuously because specs/ went missing.
fn t_spec_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let specs = root.join("specs");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&specs)
        .unwrap_or_else(|e| panic!("specs/ readable from {root:?}: {e}"))
        .filter_map(|entry| {
            let entry = entry.expect("specs/ entry readable");
            let name = entry.file_name();
            let name = name.to_string_lossy();
            (name.starts_with('t') && name.ends_with(".md")).then(|| entry.path())
        })
        .collect();
    files.sort();
    assert!(
        files.len() >= 60,
        "expected the full t-spec corpus (68 files at T67 time), got {} — the lint must scan the real corpus",
        files.len()
    );
    files
}

/// Every check-shaped line of a spec as `(1-based line, rest-of-line)`.
/// Two shapes exist in the corpus: col-0 `check:` (every t-spec but t52)
/// and heading `## check:` (t52), so leading `#`s and whitespace are
/// stripped before matching the prefix. Whole-file scope stays with the
/// gate's grep; this catches the gate line wherever it is spelled.
fn check_lines(text: &str) -> Vec<(usize, &str)> {
    text.lines()
        .enumerate()
        .filter_map(|(idx, raw)| {
            let line = raw.trim_start().trim_start_matches('#').trim_start();
            line.strip_prefix("check:").map(|check| (idx + 1, check))
        })
        .collect()
}

#[test]
fn spec_check_lines_never_invoke_cargo_test_lib() {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let mut offenders = Vec::new();
    let mut saw_t64 = false;
    for path in t_spec_files(&root) {
        let name = path
            .file_name()
            .expect("t-spec path has a file name")
            .to_string_lossy()
            .into_owned();
        if name == "t64-validator-survivor-pins.md" {
            saw_t64 = true;
        }
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} readable: {e}", path.display()));
        for (line_no, check) in check_lines(&text) {
            if check.contains(CARGO_TEST_LIB) {
                offenders.push(format!(
                    "{name}:{line_no}: check line invokes `{CARGO_TEST_LIB}` — binary-only crate — use cargo test or cargo test --bin chug"
                ));
            }
        }
    }
    assert!(
        saw_t64,
        "lint must cover the historical defect site t64-validator-survivor-pins.md"
    );
    assert!(
        offenders.is_empty(),
        "spec check lines must not invoke cargo test --lib (binary-only crate — use cargo test or cargo test --bin chug):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn metameta_doctrine_pins_no_library_targets_rule() {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let text = std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .expect("META-META-SPEC.md readable");
    assert!(
        text.contains("no library targets"),
        "META-META-SPEC.md lost the T67 convention sentence: a spec's `check:` line must never \
         invoke `cargo test --lib` — binary-only crate, `--lib` exits 101 `no library targets \
         found` at the goal gate (use plain `cargo test` or `cargo test --bin chug`)"
    );
}

// ---------------------------------------------------------------------------
// T150 — every `todo`-status TODO row's spec carries an `estimate:` line.
//
// META-META-SPEC's spec-quality bar requires every specs/t<N>-*.md to carry
// an `estimate: ~N changed lines` line — the T110 filing-time ceiling
// (~500 hard, ~400 should-split) reads it. The cycle-65 codex intake filed
// 10 rows (T134–T143) whose specs carried NO estimate line, and 4 of those
// 10 rows died 80/80 mid-work; doctrine existed, nothing enforced it at
// filing time. A retroactive pin over the whole corpus is infeasible (121
// of 141 spec files predate the rule), so the pin binds exactly where it
// matters: specs named by `todo`-status rows — every row files as `todo`,
// and done rows are exempt history. At an empty queue the repo-live leg is
// vacuously green; the synthetic legs below keep the guard non-vacuous.

/// Does one line carry the estimate shape? Leading `#`s and whitespace are
/// stripped first (mirroring the T67 check-line reader, so col-0 and
/// heading forms both count), the line must name `estimate:`, and the tail
/// after that token must contain `~` immediately followed by an ASCII
/// digit — `estimate: ~70 changed lines` qualifies; a bare `estimate:` or
/// `estimate: pending` does not (a bare word is not an estimate). Byte
/// scanning is UTF-8-safe: `~` and ASCII digits never occur inside a
/// multi-byte character's encoding.
fn line_carries_estimate(raw: &str) -> bool {
    let line = raw.trim_start().trim_start_matches('#').trim_start();
    let Some((_before, tail)) = line.split_once("estimate:") else {
        return false;
    };
    tail.as_bytes()
        .windows(2)
        .any(|w| w[0] == b'~' && w[1].is_ascii_digit())
}

/// T150 — validate every `todo`-status row's spec for an estimate line.
/// Pure over (table text, spec-reader closure): `spec_reader` resolves a
/// spec-cell path to the file's text, `None` = missing/unreadable (flagged
/// fail-closed; the T8 guard separately reports the missing file).
/// Returns one human-readable problem per violating row, named exactly the
/// way `validate_todo_table` names them. Only well-formed 6-cell rows are
/// judged (the T8 guard owns structural drift) and only the `todo` status
/// is checked (filing time; done/in-progress/blocked rows are exempt).
fn validate_todo_estimate_lines(
    md: &str,
    spec_reader: impl Fn(&str) -> Option<String>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (idx, raw) in md.lines().enumerate() {
        let Some(cells) = table_cells(raw) else {
            continue;
        };
        if cells.len() != 6 {
            continue; // malformed row — the T8 guard reports the cell count
        }
        let [id, _title, spec, _pri, status, _notes]: [&str; 6] =
            cells.try_into().expect("length checked above");
        if status != "todo" {
            continue; // history exempt: the pin binds rows still in the queue
        }
        match spec_reader(spec) {
            None => problems.push(format!(
                "{}: spec file '{spec}' unreadable — estimate line unverifiable",
                row_label(idx + 1, id)
            )),
            Some(text) if !text.lines().any(line_carries_estimate) => problems.push(format!(
                "{}: spec '{spec}' carries no `estimate: ~<number>` line — \
                 META-META-SPEC's spec bar requires one at filing time (the T110 ceiling reads it)",
                row_label(idx + 1, id)
            )),
            Some(_) => {}
        }
    }
    problems
}

#[test]
fn todo_rows_specs_carry_estimate_lines_on_disk() {
    // T48: resolve the repo root at runtime — cargo runs test binaries with
    // cwd = the package root (compile-time env! paths break under the T47
    // shared cache).
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let md = std::fs::read_to_string(root.join("TODO.md")).expect("TODO.md readable");
    let problems = validate_todo_estimate_lines(&md, |spec| {
        std::fs::read_to_string(root.join(spec)).ok()
    });
    assert!(
        problems.is_empty(),
        "todo-status rows whose specs lack an `estimate: ~<number>` line:\n{}",
        problems.join("\n")
    );
}

#[test]
fn todo_row_spec_without_estimate_line_is_flagged() {
    let md = table(&row("T150", "specs/t150-estimate-less.md", "3", "todo"));
    let problems = validate_todo_estimate_lines(&md, |_| {
        Some("# T150 — a spec\n\n## Repo context\n\nNo estimate marker in this body.\n".into())
    });
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("row 5 (T150)")
            && problems[0].contains("specs/t150-estimate-less.md")
            && problems[0].contains("estimate: ~<number>"),
        "{problems:?}"
    );
}

#[test]
fn todo_row_spec_with_estimate_line_is_clean() {
    let md = table(&row("T150", "specs/t150-estimated.md", "3", "todo"));
    let problems = validate_todo_estimate_lines(&md, |_| {
        Some("# T150 — a spec\n\nestimate: ~120 changed lines\n\n## Repo context\n".into())
    });
    assert!(problems.is_empty(), "{problems:?}");
    // Heading form counts too (mirrors the T67 check-line reader).
    let problems = validate_todo_estimate_lines(&md, |_| {
        Some("## Heading\n\n## estimate: ~120 changed lines\n".into())
    });
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn done_row_spec_without_estimate_line_is_not_flagged() {
    // History exempt: done rows predate the rule (121/141 legacy specs).
    let md = table(&row("T8", "specs/t8-todo-spec-guard.md", "2", "done"));
    let problems = validate_todo_estimate_lines(&md, |_| Some("no estimate in this one\n".into()));
    assert!(problems.is_empty(), "{problems:?}");
}

#[test]
fn estimate_line_without_a_number_is_flagged() {
    let md = table(&row("T150", "specs/t150-bare-estimate.md", "3", "todo"));
    let problems = validate_todo_estimate_lines(&md, |_| {
        Some("estimate: pending\n\nestimate:\n\nthe estimate: is high ~ but no digits follow\n".into())
    });
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("row 5 (T150)")
            && problems[0].contains("specs/t150-bare-estimate.md"),
        "{problems:?}"
    );
}

#[test]
fn unreadable_todo_spec_is_flagged_fail_closed() {
    // A missing/unreadable spec cannot be verified — flagged fail-closed
    // (the T8 guard separately reports the missing file itself).
    let md = table(&row("T150", "specs/t150-gone.md", "3", "todo"));
    let problems = validate_todo_estimate_lines(&md, |_| None);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("unreadable"), "{problems:?}");
}

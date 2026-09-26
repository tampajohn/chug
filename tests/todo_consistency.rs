//! T8 — TODO.md ↔ specs/ consistency guard.
//!
//! SELF-SPEC mandates one spec file per TODO row (`specs/t<N>-<slug>.md`),
//! but the loop once closed T1–T6 with no specs on disk and nothing noticed
//! (EVALUATION.md I10). This integration test parses the real TODO.md table
//! and rejects rows whose spec reference is malformed or dangling, so the
//! ledger of record cannot silently drift from the repo again.

use std::collections::HashSet;

const STATUSES: [&str; 4] = ["todo", "in-progress", "blocked", "done"];

/// Validate every data row of a TODO.md table. `spec_exists` resolves a
/// spec-cell path (`specs/t<N>-<slug>.md`) to whether the file is on disk.
/// Returns one human-readable problem per violation, each naming its row by
/// line number (and id cell when parseable). Header and separator rows are
/// skipped; non-table lines (incl. blank trailing lines) are ignored.
fn validate_todo_table(md: &str, spec_exists: impl Fn(&str) -> bool) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen_ids: HashSet<u64> = HashSet::new();
    for (idx, raw) in md.lines().enumerate() {
        let line = raw.trim();
        if !line.starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line
            .trim_start_matches('|')
            .trim_end_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        // Header row.
        if cells.first() == Some(&"id") {
            continue;
        }
        // Separator row (all-dash cells).
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-'))
        {
            continue;
        }
        let row = idx + 1; // 1-based, names the row in TODO.md
        let name = match cells.first().copied().unwrap_or("") {
            "" => format!("row {row}"),
            id => format!("row {row} ({id})"),
        };
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

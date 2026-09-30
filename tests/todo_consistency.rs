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
// T67 → T164 — spec `check:` line lint: no `--lib`, no failure-masking pipes,
// no absolute-path `cd`.
//
// This crate is binary-only (`src/main.rs`, no `lib.rs`), so
// `cargo test --lib` exits 101 ("no library targets found in package
// `chug`") — and `goal_complete` re-runs a spec's `check:` line as the impl
// child's goal gate. Nine child streams (t22/t25/t26/t29/t39/t42/t58/t59/
// t64) died on that unsatisfiable gate, the latest (t64) after its work was
// already committed and 14/14 green on leg 1. The T67 lint guarded that by
// matching the literal `cargo test --lib` — but the literal misses flag-
// in-between spellings (`cargo test --release --lib`: the t160 escape, whose
// `--lib … | tail -3` leg exited 101 while the goal gate PASSED, because a
// pipeline's exit status is the last command's). T164 generalizes the lint
// to three mechanical rules over every check line of every specs/t*.md:
//
//   (a) `--lib` must not be passed to cargo in ANY flag position (token
//       match, not substring — the t160 escape);
//   (b) a check line piping a cargo command through tail/head/grep MUST
//       contain `pipefail` — otherwise the filter masks the failure;
//   (c) a check line must not `cd` to an absolute path (the worktree-
//       relative rule — the T21 anomaly class: the gate must test the impl
//       child's worktree, never a pinned directory).
//
// t67's own spec spells the flag `--l[i]b` (a BRE class matching the
// literal `i`) so its prose does not carry the literal token its own gate
// greps `specs/t*.md` for — the lint matches bare tokens only, so a quoted
// needle naming the flag (`grep -q 'cargo test --lib' …`) is not a
// pass-to-cargo and stays clean.

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

/// Shell segments of a check line, each paired with the separator that ENDED
/// it (`'\0'` for the final segment). `&&`, `||`, `;`, and `|` separate
/// segments; only `|` (and `||`) marks a pipeline, which leg (b) needs.
/// Quoted runs (`'…'`, `"…"`) are opaque: separators inside a grep needle
/// (t40 greps LOOP-SPEC for a needle containing a literal `|`) do not split.
/// Separator runs longer than two bytes tokenize left-to-right, each `&&`/`||`
/// pair one separator: a leftover lone `&` is ordinary segment text (a lone
/// `&` is not a separator here), a leftover lone `|` a one-byte separator —
/// deterministic, and never the T177 `start > i` slice panic.
fn shell_segments(line: &str) -> Vec<(char, &str)> {
    let mut segments: Vec<(char, &str)> = Vec::new();
    let mut quote: Option<char> = None;
    let mut start = 0;
    let mut chars = line.char_indices();
    while let Some((i, ch)) = chars.next() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        let (sep, width) = match ch {
            ';' => (';', 1),
            '&' if line[i + 1..].starts_with('&') => {
                chars.next(); // consume the second '&' — the `||` arm's shape;
                              // without it a third `&` re-matched and pushed
                              // `&line[start..i]` with start > i (T177 panic)
                ('&', 2)
            }
            '|' if line[i + 1..].starts_with('|') => {
                chars.next(); // consume the second '|'
                ('|', 2)
            }
            '|' => ('|', 1),
            '\'' | '"' => {
                quote = Some(ch);
                continue;
            }
            _ => continue,
        };
        segments.push((sep, &line[start..i]));
        start = i + width;
    }
    segments.push(('\0', &line[start..]));
    segments
}

/// Tokens of one shell segment: whitespace-separated outside quotes, and a
/// quoted run is ONE token with the quote characters stripped. A grep needle
/// like `'cargo test --lib'` is therefore a single opaque token that can
/// never re-assemble into the bare `cargo` / `--lib` tokens the lint
/// matches (the shell would strip the quotes — `cargo test '--lib'` really
/// does pass `--lib` — so quoted flag passes stay flagged).
fn segment_tokens(segment: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    for ch in segment.chars() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                in_token = true;
            }
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            c => {
                current.push(c);
                in_token = true;
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    tokens
}

/// T164 — the three banned check-line shapes, one human-readable problem per
/// violated rule (each names the rule). `check` is a check line's payload —
/// the text after `check:`. Rules:
///
/// (a) `--lib` passed to cargo in ANY flag position — the T67 literal
///     `cargo test --lib` missed `cargo test --release --lib` (the t160
///     escape); token-scoped per shell segment, so a quoted needle naming
///     the flag is not a pass-to-cargo;
/// (b) a cargo command piped through tail/head/grep with no `pipefail`
///     anywhere on the line — the pipeline's exit status is the filter's,
///     so the filter masks a RED build/test leg;
/// (c) `cd` to an absolute path (`/…` or `~…`) — the worktree-relative
///     rule: check lines run in the impl child's worktree cwd.
fn check_line_violations(check: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let segments = shell_segments(check);
    let tokenized: Vec<Vec<String>> = segments
        .iter()
        .map(|(_, seg)| segment_tokens(seg))
        .collect();
    let has_cargo = |t: &[String]| t.iter().any(|w| w == "cargo");

    // (a) `--lib` in ANY flag position.
    if tokenized
        .iter()
        .any(|t| has_cargo(t) && t.iter().any(|w| w == "--lib"))
    {
        problems.push(
            "check line passes `--lib` to cargo — binary-only crate, `--lib` exits 101 \
             `no library targets found` at the goal gate (use plain `cargo test` or \
             `cargo test --bin chug [<filter>]`)"
                .to_string(),
        );
    }

    // (b) failure-masking pipe: cargo piped through tail/head/grep without
    //     `pipefail` (t160: the `--lib … | tail -3` leg exited 101 and the
    //     goal gate passed).
    let piped = (0..tokenized.len().saturating_sub(1)).any(|i| {
        segments[i].0 == '|'
            && has_cargo(&tokenized[i])
            && matches!(
                tokenized[i + 1].first().map(String::as_str),
                Some("tail") | Some("head") | Some("grep")
            )
    });
    if piped && !check.contains("pipefail") {
        problems.push(
            "check line pipes a cargo command through tail/head/grep without `pipefail` — \
             the pipeline's exit status is the filter's, masking the build/test failure \
             (prefix `set -o pipefail;` or drop the filter)"
                .to_string(),
        );
    }

    // (c) absolute-path `cd` (the T21 anomaly class).
    for t in &tokenized {
        for (i, w) in t.iter().enumerate() {
            if w == "cd"
                && let Some(arg) = t[i + 1..]
                    .iter()
                    .find(|a| a.starts_with('/') || a.starts_with('~'))
            {
                problems.push(format!(
                    "check line cd's to the absolute path `{arg}` — check lines are \
                     worktree-relative: they run in the impl child's worktree cwd, never \
                     cd to a pinned directory"
                ));
            }
        }
    }
    problems
}

#[test]
fn spec_check_lines_are_lint_clean() {
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
            for problem in check_line_violations(check) {
                offenders.push(format!("{name}:{line_no}: {problem}"));
            }
        }
    }
    assert!(
        saw_t64,
        "lint must cover the historical defect site t64-validator-survivor-pins.md"
    );
    assert!(
        offenders.is_empty(),
        "spec check lines must satisfy the T164 lint (no `--lib`, no failure-masking pipes, no absolute-path `cd`):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn check_line_lint_flags_lib_in_any_flag_position() {
    // The t160 escape: the T67 literal `cargo test --lib` missed this shape.
    let problems = check_line_violations("cargo test --release --lib mcp_serve");
    assert!(problems.iter().any(|p| p.contains("--lib")), "{problems:?}");
    assert!(
        check_line_violations("cargo test --lib")
            .iter()
            .any(|p| p.contains("--lib")),
        "the plain T67 shape must stay flagged"
    );
    // A quoted flag pass still reaches cargo (the shell strips the quotes)…
    assert!(
        check_line_violations("cargo test '--lib'")
            .iter()
            .any(|p| p.contains("--lib")),
        "a quoted --lib still reaches cargo and must stay flagged"
    );
    // …but a quoted needle NAMING the flag is not a pass-to-cargo.
    assert!(
        check_line_violations("grep -q 'cargo test --lib' META-META-SPEC.md && cargo test")
            .is_empty()
    );
    // Compliant spellings pass.
    assert!(check_line_violations("cargo test --test mcp_serve").is_empty());
    assert!(check_line_violations("cargo test --bin chug mcp_serve").is_empty());
}

#[test]
fn check_line_lint_flags_cargo_piped_through_a_filter_without_pipefail() {
    for banned in [
        "cargo test 2>&1 | tail -3",
        "cargo test 2>&1 | head -5",
        "cargo test 2>&1 | grep -q ok",
        "cargo clippy --release --all-targets | tail -5",
    ] {
        let problems = check_line_violations(banned);
        assert!(
            problems.iter().any(|p| p.contains("pipefail")),
            "{banned}: {problems:?}"
        );
    }
    // The pipefail prefix clears the leg…
    assert!(
        check_line_violations("set -o pipefail; cargo test 2>&1 | tail -3").is_empty(),
        "pipefail-prefixed pipe must pass"
    );
    // …and filters not downstream of cargo, or no filter at all, never trip it.
    assert!(
        check_line_violations("grep -c 'x' LOOP-SPEC.md | grep -q '^1$' && cargo test").is_empty()
    );
    assert!(check_line_violations("cargo test --test todo_consistency").is_empty());
}

#[test]
fn check_line_lint_flags_absolute_path_cd() {
    for banned in [
        // The T21 anomaly class: the main repo is not the worktree under test.
        "cd /Users/jadams/workspace/chug && cargo test",
        // A pinned worktree path is still absolute — the check line must be
        // runnable from whatever cwd the gate runs it in.
        "cd /private/tmp/chug-loop-t156 && cargo test --test loop_spec_recovery",
    ] {
        let problems = check_line_violations(banned);
        assert!(
            problems.iter().any(|p| p.contains("absolute path")),
            "{banned}: {problems:?}"
        );
    }
    // Worktree-relative forms are the rule.
    assert!(check_line_violations("cargo test").is_empty());
    assert!(check_line_violations("cd specs && grep -q x t1.md && cargo test").is_empty());
}

#[test]
fn shell_segments_tokenizes_any_ampersand_run_without_panicking() {
    // T177: the `&&` arm matched a second `&` via `line[i + 1..]` without
    // consuming it, so in a `&&&` run the third `&` re-matched the arm and
    // pushed `&line[start..i]` with `start = i_prev + 2 > i` — a slice
    // panic. A malformed check line must tokenize deterministically, never
    // crash the corpus lint. Chosen segmentation for odd runs: each `&&`
    // pair is one separator left-to-right; a leftover lone `&` is ordinary
    // segment text (a lone `&` has never been a separator here); an even
    // run of four yields two `&&` separators with an empty middle segment.
    for line in [
        "a &&& b",      // the T177 panic shape
        "a &&&& b",     // even run: two `&&` separators
        "a &&&",        // odd run at end of line
        "cargo test &", // trailing lone `&`
        "cargo test & ", // `& ` at end of line
        "cargo test &&", // trailing `&&`
        // Sweep-the-family pins: `||` already consumes its second char and
        // `;` is single-byte, so the match-2-consume-1 class cannot occur
        // there — `a ||| b` falls back to a one-byte `|` separator and
        // `;;;` to three, both segment- (never panic-) producing.
        "a ||| b",
        "a ;;; b",
    ] {
        let _ = shell_segments(line);
    }
    // The chosen odd-run segmentation, pinned.
    assert_eq!(
        shell_segments("a &&& b"),
        vec![('&', "a "), ('\0', "& b")]
    );
    assert_eq!(
        shell_segments("a &&&& b"),
        vec![('&', "a "), ('&', ""), ('\0', " b")]
    );
}

#[test]
fn check_line_lint_survives_a_malformed_ampersand_run() {
    // T177: a check line containing `&&&` must yield lint segments (a
    // finding, or none for a clean command) instead of crashing
    // todo_consistency — which runs in every docs-only gate floor.
    let problems = check_line_violations("cargo test --release --lib mcp_serve &&& cargo test");
    assert!(problems.iter().any(|p| p.contains("--lib")), "{problems:?}");
    assert!(
        check_line_violations("cargo test --test todo_consistency &&& echo ok").is_empty(),
        "a clean command behind a malformed `&&&` run must stay clean"
    );
}

#[test]
fn shell_segments_keeps_the_well_formed_and_segmentation() {
    // T177 requirement: zero behavior change for well-formed input — the
    // fix touches only the malformed-run path.
    assert_eq!(shell_segments("a && b"), vec![('&', "a "), ('\0', " b")]);
    assert_eq!(
        shell_segments("a && b && c"),
        vec![('&', "a "), ('&', " b "), ('\0', " c")]
    );
}

#[test]
fn sample_spec_text_with_banned_check_line_fails_every_leg() {
    // T164 requirement 4: a sample spec text whose check line carries all
    // three banned shapes fails the lint, naming each rule.
    let spec = "# T999 — sample\n\nestimate: ~1 changed line\n\ncheck: cd /Users/jadams/workspace/chug && cargo test --release --lib mcp_serve 2>&1 | tail -3\n";
    let problems: Vec<String> = check_lines(spec)
        .into_iter()
        .flat_map(|(_, check)| check_line_violations(check))
        .collect();
    assert_eq!(problems.len(), 3, "{problems:?}");
    assert!(problems[0].contains("--lib"), "{problems:?}");
    assert!(problems[1].contains("pipefail"), "{problems:?}");
    assert!(problems[2].contains("absolute path"), "{problems:?}");
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

#[test]
fn metameta_doctrine_pins_pipe_masking_ban() {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let text = std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .expect("META-META-SPEC.md readable");
    assert!(
        text.contains("pipefail"),
        "META-META-SPEC.md lost the T164 pipe-masking sentence: a spec's `check:` line that \
         pipes a cargo command through tail/head/grep must set `pipefail` first — a pipeline's \
         exit status is the last command's, so the filter masks a RED build/test leg (t160's \
         `--lib … | tail -3` leg exited 101 and its goal gate passed)"
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

//! T189 — the low-stakes validation lane: a MECHANICAL gates-only predicate.
//!
//! Operator 2026-10-01: kimi adversarial validation is 15–30 min per item —
//! right for core logic, overkill for chores. LOOP-SPEC §2 step 4 states the
//! lane verbatim; this module is its reference implementation, so the
//! doctrine's conjunction and the code's arithmetic cannot drift apart
//! without a pin going red:
//!
//! - **gates-only when ALL four hold** — (a) the diff touches NO core-list
//!   file (step 4's REQUIRED list + the loop/spec doctrine files);
//!   (b) ≤ ~150 changed lines (added+deleted); (c) no new tool/command
//!   surface (schema enum, CLI flag, MCP tool, hook event); (d) no
//!   CI/workflow or spec `check:` line change. ANY single one flipped →
//!   full adversarial validation.
//! - **the inputs are computed from the diff, never the model's say-so**:
//!   `git diff --numstat` (file list + line counts) plus the unified diff
//!   (added-line surface markers, `check:`-line changes) — both read by
//!   [`lane_inputs`], whose four-field result the orchestrator records
//!   verbatim via `decision_log` (class `validation-routing`,
//!   [`render_inputs`]).
//! - **the lane changes review depth, never the gates or the quality
//!   floor**: a gates-only item still runs build + clippy + the full suite
//!   in its worktree, byte-clean review, and the scope check — it skips the
//!   kimi child, never the gates — and a lane-eligible diff the gates catch
//!   red still gets a fix-up child.
//! - **auto-spec'd runs (T188) default to the lane**; `--validate` forces
//!   full adversarial and `--no-validate` forces gates-only (operator
//!   override, recorded — [`record_override`], carried to the orchestrator
//!   on the goal as a lane directive).
//!
//! Conservative by construction: every heuristic here errs toward FULL
//! (a false positive costs a validation round; a false negative would
//! silently skip one), and any predicate the caller cannot evaluate
//! defaults to full — the T80 ambiguity rule, one level up.
//!
//! The predicate trio (`parse_numstat`/`lane_inputs`/`lane_verdict`/
//! `render_inputs` and its types) is the doctrine's REFERENCE
//! IMPLEMENTATION: the orchestrator computes the four inputs from git's
//! output and records them; these functions pin the exact arithmetic the
//! LOOP-SPEC sentence states, so doctrine and code cannot drift apart
//! without this module's pins going red. They have no runtime call site —
//! deliberately: exposing them as a tool or CLI flag would itself be new
//! command surface (clause (c) forces FULL for it). Hence the module-level
//! `allow(dead_code)` — the same shape as attach.rs/observ.rs/complete.rs.
#![allow(dead_code)]

use std::path::Path;

use serde_json::json;

use crate::decisions;

/// Step 4's core list (clause (a), first half): a diff touching any of
/// these forces FULL adversarial validation. Matched with `contains` — a
/// rename row (`src/{a => b}/driver.rs`) or a moved core file must not
/// slip the lane; a stray false positive only costs a validation round.
pub const CORE_FILES: [&str; 4] = [
    "src/driver.rs",
    "src/api.rs",
    "src/tools.rs",
    "src/events.rs",
];

/// The loop/spec doctrine files (clause (a), second half — the doctrine
/// clause): any edit to one of these stays FULL validation, always.
pub const DOCTRINE_FILES: [&str; 4] = [
    "LOOP-SPEC.md",
    "META-SPEC.md",
    "META-META-SPEC.md",
    "SELF-SPEC.md",
];

/// Clause (b): the changed-lines budget (added+deleted). The doctrine's
/// "~150" is pinned here at exactly 150 — `> 150` flips the lane.
pub const LANE_MAX_LINES: u64 = 150;

/// Clause (c)'s surface markers, scanned on ADDED diff lines only (a
/// surface REMOVED is not a new surface). Each entry is `(marker, kind)`.
/// Deliberately broad anchors — a false positive forces FULL, the safe
/// direction:
/// - `#[arg(` — a new clap flag; `CliCommand::` — a new subcommand arm;
/// - `"enum"` — a new tool JSON-schema enum; `pub fn schema()` — a new
///   builtin tool's schema push;
/// - `_TOOL: &str` — a new MCP tool name const (the mcp_serve.rs pattern);
/// - `ToolUse` — the hook-event strings (PreToolUse/PostToolUse and any
///   future sibling).
pub const SURFACE_MARKERS: [(&str, &str); 6] = [
    ("#[arg(", "cli-flag"),
    ("CliCommand::", "cli-subcommand"),
    ("\"enum\"", "schema-enum"),
    ("pub fn schema()", "tool-schema"),
    ("_TOOL: &str", "mcp-tool"),
    ("ToolUse", "hook-event"),
];

/// The four mechanical inputs (requirement 2: computed from the diff, not
/// the model's say-so). Field order is predicate order — [`render_inputs`]
/// and the LOOP-SPEC sentence follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneInputs {
    /// (a) any core-list or doctrine file touched.
    pub core_file_touched: bool,
    /// (b) added+deleted lines, from `git diff --numstat`.
    pub changed_lines: u64,
    /// (c) a new tool/command surface marker on an added line.
    pub new_surface: bool,
    /// (d) a `.github/` path changed, or a `check:` line changed.
    pub check_or_ci_changed: bool,
}

/// The lane verdict: `gates-only` (skip the kimi child, never the gates)
/// or `full-adversarial` (the kimi child, META-SPEC §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lane {
    GatesOnly,
    FullAdversarial,
}

impl Lane {
    /// The record's `choice` token (decision_log, class
    /// `validation-routing`).
    pub fn as_str(self) -> &'static str {
        match self {
            Lane::GatesOnly => "gates-only",
            Lane::FullAdversarial => "full-adversarial",
        }
    }
}

/// One `git diff --numstat` row: added, deleted, path. Binary rows carry
/// `-` for both counts and contribute 0 lines (the file still counts for
/// the core/doctrine/CI path checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumstatRow {
    pub added: u64,
    pub deleted: u64,
    pub path: String,
}

/// Parse `git diff --numstat` output (TAB-separated `added\tdeleted\tpath`
/// rows). Unparseable rows are dropped (never guessed); a `-` count
/// (binary) parses as 0.
pub fn parse_numstat(text: &str) -> Vec<NumstatRow> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let added = parts.next()?;
            let deleted = parts.next()?;
            let path = parts.next()?.trim();
            if path.is_empty() {
                return None;
            }
            let count = |s: &str| s.trim().parse::<u64>().unwrap_or(0);
            Some(NumstatRow {
                added: count(added),
                deleted: count(deleted),
                path: path.to_string(),
            })
        })
        .collect()
}

/// The candidate paths one numstat row names. A rename row names BOTH
/// endpoints — `a.md => b.md`, or the braced `dir/{old => new}/rest` —
/// and a rename INTO a core/doctrine/CI path must not slip the lane, so
/// every endpoint is checked.
fn numstat_path_candidates(path: &str) -> Vec<String> {
    if let Some((dir, rest)) = path.split_once('{')
        && let Some((inner, after)) = rest.split_once('}')
    {
        return inner
            .split(" => ")
            .map(|side| format!("{dir}{side}{after}"))
            .collect();
    }
    if let Some((old, new)) = path.split_once(" => ") {
        return vec![old.to_string(), new.to_string()];
    }
    vec![path.to_string()]
}

/// Clause (a): does the path hit the core list or the doctrine files?
/// Candidate paths (rename endpoints included) are matched with
/// `contains` so a moved or nested core file cannot slip the lane; a
/// stray false positive only costs a validation round.
fn is_core_or_doctrine_path(path: &str) -> bool {
    numstat_path_candidates(path)
        .iter()
        .any(|candidate| {
            CORE_FILES
                .iter()
                .chain(DOCTRINE_FILES.iter())
                .any(|core| candidate.contains(core))
        })
}

/// Clause (d), half one: a CI/workflow path (anything under `.github/`),
/// rename endpoints included.
fn is_ci_path(path: &str) -> bool {
    numstat_path_candidates(path)
        .iter()
        .any(|candidate| candidate.starts_with(".github/"))
}

/// Clause (d), half two: a spec `check:` line change — any diff line
/// (added or removed; a change is always both) whose payload, trimmed,
/// starts with `check:`. The `++`/`--` file headers are excluded so a
/// renamed spec path cannot masquerade as a check-line change.
fn diff_changes_check_line(diff_text: &str) -> bool {
    diff_text.lines().any(|line| {
        let payload = if line.starts_with("+++") || line.starts_with("---") {
            return false;
        } else if let Some(rest) = line.strip_prefix('+') {
            rest
        } else if let Some(rest) = line.strip_prefix('-') {
            rest
        } else {
            return false;
        };
        payload.trim_start().starts_with("check:")
    })
}

/// Compute the four inputs from the diff artifacts: `numstat_text` is
/// `git diff --numstat` output, `diff_text` the unified `git diff`. Both
/// are git's word, never the model's.
pub fn lane_inputs(numstat_text: &str, diff_text: &str) -> LaneInputs {
    let rows = parse_numstat(numstat_text);
    LaneInputs {
        core_file_touched: rows.iter().any(|r| is_core_or_doctrine_path(&r.path)),
        changed_lines: rows.iter().map(|r| r.added + r.deleted).sum(),
        new_surface: diff_text.lines().any(|line| {
            line.starts_with('+')
                && !line.starts_with("+++")
                && SURFACE_MARKERS.iter().any(|(marker, _)| line.contains(marker))
        }),
        check_or_ci_changed: rows.iter().any(|r| is_ci_path(&r.path))
            || diff_changes_check_line(diff_text),
    }
}

/// The lane verdict — the conjunctive predicate, verbatim from LOOP-SPEC
/// §2 step 4: gates-only when ALL four inputs are clear, otherwise full
/// adversarial validation.
pub fn lane_verdict(inputs: &LaneInputs) -> Lane {
    if inputs.core_file_touched
        || inputs.changed_lines > LANE_MAX_LINES
        || inputs.new_surface
        || inputs.check_or_ci_changed
    {
        Lane::FullAdversarial
    } else {
        Lane::GatesOnly
    }
}

/// The canonical `inputs` string for the per-item routing record: all four
/// inputs named, in predicate order (requirement 2 — the record names the
/// four inputs plus the verdict).
pub fn render_inputs(inputs: &LaneInputs) -> String {
    format!(
        "core-file-touched={}; changed-lines={} (budget {LANE_MAX_LINES}); \
         new-surface={}; check-or-ci-changed={}",
        inputs.core_file_touched,
        inputs.changed_lines,
        inputs.new_surface,
        inputs.check_or_ci_changed,
    )
}

/// The auto-spec lane overrides (T189 requirement 4): `--validate` forces
/// full adversarial, `--no-validate` forces gates-only. Both are operator
/// overrides — recorded via [`record_override`] and carried to the
/// orchestrator on the goal ([`LaneOverride::directive`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneOverride {
    Full,
    GatesOnly,
}

impl LaneOverride {
    /// The CLI flag that selected this override.
    pub fn flag(self) -> &'static str {
        match self {
            LaneOverride::Full => "--validate",
            LaneOverride::GatesOnly => "--no-validate",
        }
    }

    /// The record's `choice` token.
    pub fn choice(self) -> &'static str {
        match self {
            LaneOverride::Full => "full-adversarial",
            LaneOverride::GatesOnly => "gates-only",
        }
    }

    /// The record's `inputs` string: the override plus the four lane
    /// inputs it bypasses, named.
    pub fn record_inputs(self) -> String {
        format!(
            "operator override {}: {} for every item this run, regardless of \
             the four lane inputs (core-file-touched, changed-lines, \
             new-surface, check-or-ci-changed)",
            self.flag(),
            self.choice(),
        )
    }

    /// The goal directive that carries the override to the orchestrator
    /// (the goal is re-read every iteration; the spec is the contract but
    /// the goal is where per-run directives ride).
    pub fn directive(self) -> &'static str {
        match self {
            LaneOverride::Full => {
                "\n\nValidation lane (T189, --validate operator override): \
                 FULL adversarial validation (the kimi child, LOOP-SPEC §2 \
                 step 4) is REQUIRED for every item this run — the \
                 gates-only lane is off."
            }
            LaneOverride::GatesOnly => {
                "\n\nValidation lane (T189, --no-validate operator override): \
                 the gates-only lane applies to EVERY item this run \
                 regardless of its diff — the kimi validation child is \
                 skipped, while build + clippy + the full suite in the \
                 worktree, byte-clean review, and the scope check stay \
                 REQUIRED (the lane skips the kimi child, never the gates)."
            }
        }
    }
}

/// The goal directive for the DEFAULT auto-spec lane (no override): the
/// self-contained lane instruction — predicate, recording duty, and the
/// never-the-gates floor — so the default is wired, not just doctrinal.
/// Built from the constants above so the directive cannot drift from the
/// predicate the code pins (one source of truth).
pub fn predicate_directive() -> String {
    format!(
        "\n\nValidation lane (T189, LOOP-SPEC §2 step 4): this run defaults \
         to the low-stakes lane — per item, BEFORE dispatch, compute the \
         four inputs mechanically from the item's diff (`git diff \
         --numstat` + `git diff`): core-file-touched ({}, or a doctrine \
         file: {}), changed-lines (added+deleted, budget {LANE_MAX_LINES}), \
         new-surface (a new tool/command surface: schema enum, CLI flag, \
         MCP tool, hook event), check-or-ci-changed (a .github/ path or any \
         diff line whose payload starts `check:`). Record the routing via \
         decision_log (class validation-routing) naming all four inputs \
         plus the verdict. ALL four clear \u{2192} gates-only: run build + \
         clippy + the full suite in the worktree, byte-clean review, and \
         the scope check — only the kimi validation child is skipped (the \
         lane skips the kimi child, never the gates), and a gates-red diff \
         still gets a fix-up child. ANY input flipped \u{2192} full \
         adversarial validation.",
        CORE_FILES.join(", "),
        DOCTRINE_FILES.join(", "),
    )
}

/// Record the operator's lane override via `decision_log` (class
/// `validation-routing`) — requirement 4's "operator override, recorded".
/// Best-effort by the decision_log contract: the caller warns on `Err` and
/// never aborts the run.
pub fn record_override(cwd: &Path, lane: LaneOverride) -> anyhow::Result<String> {
    let result = decisions::decision_log(
        cwd,
        &json!({
            "class": "validation-routing",
            "subject": "auto-spec-lane",
            "inputs": lane.record_inputs(),
            "options": "gates-only | full-adversarial",
            "choice": lane.choice(),
            "confidence": 1.0,
        }),
    )?;
    Ok(result.content)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A numstat fixture builder: rows joined with tabs, one per line.
    fn numstat(rows: &[(&str, &str, &str)]) -> String {
        rows.iter()
            .map(|(a, d, p)| format!("{a}\t{d}\t{p}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The all-clear baseline: a small chore diff (a one-file fix + docs)
    /// that lands gates-only.
    fn clear_numstat() -> String {
        numstat(&[("6", "2", "src/todos.rs"), ("4", "1", "README.md")])
    }

    #[test]
    fn parse_numstat_reads_counts_and_paths_and_drops_garbage() {
        let rows = parse_numstat(&numstat(&[
            ("12", "3", "src/main.rs"),
            ("-", "-", "assets/logo.png"),
            ("0", "7", "README.md"),
        ]));
        assert_eq!(rows.len(), 3, "all three rows parse");
        assert_eq!(
            rows[0],
            NumstatRow { added: 12, deleted: 3, path: "src/main.rs".into() }
        );
        assert_eq!(
            rows[1],
            NumstatRow { added: 0, deleted: 0, path: "assets/logo.png".into() },
            "a binary row (`-` counts) parses as 0/0 but keeps the path"
        );
        assert_eq!(rows[2].deleted, 7);
        // An empty or header-less line is dropped, never guessed.
        assert!(parse_numstat("not a numstat row").is_empty());
        assert!(parse_numstat("").is_empty());
    }

    // ---------- the four predicate pins (spec Tests §1) ----------
    // Each of the four inputs flips the lane: core-file touch → full,
    // >150 lines → full, new surface → full, check:/CI change → full.

    #[test]
    fn all_clear_lands_gates_only() {
        let inputs = lane_inputs(&clear_numstat(), "");
        assert_eq!(
            inputs,
            LaneInputs {
                core_file_touched: false,
                changed_lines: 13,
                new_surface: false,
                check_or_ci_changed: false,
            }
        );
        assert_eq!(lane_verdict(&inputs), Lane::GatesOnly);
    }

    /// Pin (a): a core-list file touch forces FULL — each of the four
    /// files, plus the doctrine clause (any doctrine file, too).
    #[test]
    fn core_file_touch_forces_full() {
        for core in CORE_FILES {
            let inputs = lane_inputs(&numstat(&[("1", "1", core)]), "");
            assert!(inputs.core_file_touched, "{core} is core-listed");
            assert_eq!(lane_verdict(&inputs), Lane::FullAdversarial, "{core}");
        }
        for doctrine in DOCTRINE_FILES {
            let inputs = lane_inputs(&numstat(&[("1", "0", doctrine)]), "");
            assert!(inputs.core_file_touched, "{doctrine} is doctrine");
            assert_eq!(lane_verdict(&inputs), Lane::FullAdversarial, "{doctrine}");
        }
        // Rename rows: a rename INTO a core file cannot slip the lane —
        // both the whole-path form and git's braced form name the core
        // endpoint, and the braced form's non-core endpoint stays inert.
        let into_core = numstat(&[("2", "2", "docs/guide.md => src/driver.rs")]);
        assert!(lane_inputs(&into_core, "").core_file_touched);
        let braced = numstat(&[("2", "2", "src/{driver.rs => nested/driver.rs}")]);
        assert!(lane_inputs(&braced, "").core_file_touched);
        let away_from_core = numstat(&[("2", "2", "src/{old => new}/helper.rs")]);
        assert!(
            !lane_inputs(&away_from_core, "").core_file_touched,
            "a rename between two non-core paths stays in the lane"
        );
    }

    /// Pin (b): the line budget flips at the boundary — 150 stays
    /// gates-only, 151 forces FULL (the doctrine's "~150" pinned exactly).
    #[test]
    fn over_line_budget_forces_full() {
        let at_budget = lane_inputs(&numstat(&[("150", "0", "src/todos.rs")]), "");
        assert_eq!(at_budget.changed_lines, LANE_MAX_LINES);
        assert_eq!(
            lane_verdict(&at_budget),
            Lane::GatesOnly,
            "150 is within the budget"
        );

        let over = lane_inputs(
            &numstat(&[("75", "75", "src/todos.rs"), ("1", "0", "README.md")]),
            "",
        );
        assert_eq!(over.changed_lines, LANE_MAX_LINES + 1);
        assert_eq!(lane_verdict(&over), Lane::FullAdversarial, "151 forces full");
    }

    /// Pin (c): a new tool/command surface forces FULL — every marker
    /// category, on an ADDED line only (the same text as context or as a
    /// removal is not a new surface).
    #[test]
    fn new_surface_forces_full() {
        let cases: [(&str, &str); 6] = [
            (
                "cli-flag",
                "+    #[arg(long, default_value_t = false)]\n        validate: bool,",
            ),
            (
                "cli-subcommand",
                "+    /// A new subcommand.\n+    CliCommand::NewThing => {}",
            ),
            (
                "schema-enum",
                "+            \"action\": {\"type\": \"string\", \"enum\": [\"a\", \"b\"]}",
            ),
            ("tool-schema", "+pub fn schema() -> Value {"),
            (
                "mcp-tool",
                "+const CHUG_NEW_TOOL: &str = \"chug_new\"; // _TOOL: &str",
            ),
            (
                "hook-event",
                "+                        \"event\": \"PreToolUse\",",
            ),
        ];
        for (kind, diff) in cases {
            let inputs = lane_inputs(&clear_numstat(), diff);
            assert!(inputs.new_surface, "{kind} marker must be detected");
            assert_eq!(lane_verdict(&inputs), Lane::FullAdversarial, "{kind}");
        }
        // The same markers as context (` `) or removed (`-`) lines are NOT
        // a new surface — only additions grow the surface.
        let context = "     #[arg(long, default_value_t = false)]\n";
        let removed = "-const CHUG_OLD_TOOL: &str = \"chug_old\";\n";
        assert!(!lane_inputs(&clear_numstat(), context).new_surface);
        assert!(!lane_inputs(&clear_numstat(), removed).new_surface);
    }

    /// Pin (d): a CI/workflow path or a spec `check:` line change forces
    /// FULL — added or removed check lines both count, a context line does
    /// not, and a `++`/`--` file header is not a check-line change.
    #[test]
    fn check_or_ci_change_forces_full() {
        // CI/workflow path (numstat half).
        let ci = lane_inputs(&numstat(&[("2", "1", ".github/workflows/ci.yml")]), "");
        assert!(ci.check_or_ci_changed);
        assert_eq!(lane_verdict(&ci), Lane::FullAdversarial);

        // Spec `check:` line change (diff half): added and removed.
        let added = lane_inputs(&clear_numstat(), "+check: cargo test\n");
        assert!(added.check_or_ci_changed);
        assert_eq!(lane_verdict(&added), Lane::FullAdversarial);
        let removed = lane_inputs(&clear_numstat(), "-check: cargo nextest run --release\n");
        assert!(removed.check_or_ci_changed);
        assert_eq!(lane_verdict(&removed), Lane::FullAdversarial);
        // Indented payload (a wrapped spec line) still counts.
        let indented = lane_inputs(&clear_numstat(), "+  check: cargo test\n");
        assert!(indented.check_or_ci_changed);

        // Non-changes: a context line, a file header, a non-check payload.
        let quiet = " check: cargo test\n+++ b/specs/x.md\n--- a/specs/x.md\n+the check: is quoted mid-sentence\n";
        let inputs = lane_inputs(&clear_numstat(), quiet);
        assert!(
            !inputs.check_or_ci_changed,
            "context/headers/mid-line mentions are not check changes"
        );
        assert_eq!(lane_verdict(&inputs), Lane::GatesOnly);
    }

    /// The conjunction is strict: exactly the all-clear shape lands
    /// gates-only, and each single flip alone forces FULL (pinning the
    /// predicate's ANY-one-flipped semantics, not just its parts).
    #[test]
    fn lane_is_conjunctive_any_single_flip_forces_full() {
        let base = LaneInputs {
            core_file_touched: false,
            changed_lines: 10,
            new_surface: false,
            check_or_ci_changed: false,
        };
        assert_eq!(lane_verdict(&base), Lane::GatesOnly);
        let flips = [
            LaneInputs { core_file_touched: true, ..base.clone() },
            LaneInputs { changed_lines: LANE_MAX_LINES + 1, ..base.clone() },
            LaneInputs { new_surface: true, ..base.clone() },
            LaneInputs { check_or_ci_changed: true, ..base.clone() },
        ];
        for flipped in &flips {
            assert_eq!(
                lane_verdict(flipped),
                Lane::FullAdversarial,
                "a single flipped input must force full: {flipped:?}"
            );
        }
    }

    /// The canonical record string names ALL FOUR inputs (requirement 2's
    /// "with the four inputs named") plus the budget.
    #[test]
    fn render_inputs_names_all_four() {
        let s = render_inputs(&LaneInputs {
            core_file_touched: false,
            changed_lines: 42,
            new_surface: true,
            check_or_ci_changed: false,
        });
        for name in [
            "core-file-touched=false",
            "changed-lines=42 (budget 150)",
            "new-surface=true",
            "check-or-ci-changed=false",
        ] {
            assert!(s.contains(name), "the record must name {name}: {s}");
        }
    }

    /// The operator override is RECORDED (requirement 4): both overrides
    /// append one validation-routing record whose class/subject/choice/
    /// inputs shape pins, naming the four inputs the override bypasses.
    #[test]
    fn record_override_writes_validation_routing_record() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path();
        let mut ids = Vec::new();
        for lane in [LaneOverride::Full, LaneOverride::GatesOnly] {
            let content = record_override(cwd, lane).expect("the override records");
            assert!(content.starts_with("recorded d"), "returns the record id: {content}");
            ids.push(content);
        }
        assert!(ids[0] != ids[1], "each record gets its own id");
        let text = std::fs::read_to_string(cwd.join(".chug/decisions.jsonl")).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "one record per override call");
        for (line, want_choice) in lines.iter().zip(["full-adversarial", "gates-only"]) {
            let record: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(record["class"], "validation-routing");
            assert_eq!(record["subject"], "auto-spec-lane");
            assert_eq!(record["choice"], want_choice);
            assert_eq!(record["options"], "gates-only | full-adversarial");
            let inputs = record["inputs"].as_str().unwrap();
            for name in [
                "core-file-touched",
                "changed-lines",
                "new-surface",
                "check-or-ci-changed",
            ] {
                assert!(inputs.contains(name), "the override record names {name}: {inputs}");
            }
        }
        // Each record names ITS flag.
        assert!(lines[0].contains("--validate"));
        assert!(lines[1].contains("--no-validate"));
    }

    /// The directives carry their lane to the orchestrator: the override
    /// directives name their flag and the never-the-gates floor, and the
    /// default directive states the predicate + the recording duty.
    #[test]
    fn directives_carry_the_lane() {
        let full = LaneOverride::Full.directive();
        assert!(full.contains("--validate"));
        assert!(full.contains("FULL adversarial"));
        let gates = LaneOverride::GatesOnly.directive();
        assert!(gates.contains("--no-validate"));
        assert!(
            gates.contains("skips the kimi child, never the gates"),
            "the override directive carries the gates-still-required floor: {gates}"
        );
        let default = predicate_directive();
        assert!(default.contains("BEFORE dispatch"));
        assert!(default.contains("decision_log"));
        assert!(default.contains("full adversarial validation"));
        // The directive is built from the constants — the file lists and
        // the budget cannot drift from the predicate the code pins.
        assert!(default.contains(&CORE_FILES.join(", ")));
        assert!(default.contains(&DOCTRINE_FILES.join(", ")));
        assert!(default.contains(&format!("budget {LANE_MAX_LINES}")));
    }
}

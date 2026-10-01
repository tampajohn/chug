//! T188 — auto-spec: chug drafts its own spec from a bare goal.
//!
//! The spec tax is the #1 barrier for task-class work: every run needs a
//! spec with a `check:` line, and authoring one for a chore costs more than
//! the chore. This module is the draft phase + acceptance gate:
//!
//! - **Draft** (headless): a plan-mode session (`PlanKind::SpecDraft`)
//!   reusing T73's read-only six-tool loop — the model explores the repo
//!   read-only and hands the whole spec to `submit_plan`, which writes it
//!   verbatim to `.chug/auto-spec.md` (gitignored, resumable). No new I/O.
//! - **Acceptance gate** (headless): the draft is accepted only if it is
//!   structurally complete — concern/requirements/tests/acceptance
//!   sections plus `check:` and `estimate:` lines — and the check is not
//!   vacuous (`check: true`-class) and **executes and passes** a dry-run
//!   on the current tree. A rejected draft is redrafted ONCE with the
//!   failure as feedback; a second failure aborts with the draft and the
//!   failing output — the check is never loosened to pass.
//! - **Chat**: `/auto-spec <request>` drafts with ONE direct LLM call (no
//!   tools — a nested plan session would rotate the live chat transcript),
//!   shows the draft, and `/auto-spec-approve` gates it through the SAME
//!   validation + dry-run before the turn starts (the T146 approve pattern:
//!   review a file, approve explicitly).
//!
//! Doctrine: auto-spec is for task-class work (chores, small features);
//! adversarial/loop work keeps hand-written specs.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};

use crate::api::{ContentBlock, Llm, Message, ObsCtx};
use crate::driver;
use crate::plan::PlanKind;

/// The drafted spec lands here (under the run cwd). `.chug/` is gitignored,
/// so the drafted spec is committed to the session, not the repo.
pub const AUTO_SPEC_REL: &str = ".chug/auto-spec.md";

/// The draft session's own budgets — bounded, independent of the run's
/// (the draft is one submit, not a second run).
const DRAFT_MAX_ITERS: u32 = 15;
const DRAFT_MAX_MINUTES: u64 = 10;

/// Spec-draft preamble: the read-only contract plus the exact spec format
/// the gate enforces. The headings/lines here and `validate_spec`'s
/// required set are one contract — drift breaks the `draft_preamble_lists`
/// pin.
pub const DRAFT_PREAMBLE: &str = "You are chug in spec-draft mode: a READ-ONLY drafting session. Explore the repository with your read-only tools (read_file, grep, glob, list_dir, web_fetch), then call submit_plan ONCE with the complete drafted spec as markdown. submit_plan is the only write available and it ends the session — the spec text goes in submit_plan's `plan` argument (it is written to the spec file verbatim), not to any file. You are not implementing anything.

The spec you draft must follow chug's SPEC format EXACTLY — the acceptance gate rejects a draft that misses any piece, and the `check:` line is dry-run executed before the run starts:

- A `check: <shell command>` line: a real, non-interactive verification command that is GREEN ON THE CURRENT TREE (the dry-run runs it now and requires exit 0) and stays meaningful once the task is done. Never vacuous: `true`, `exit 0`, `echo ...`, or any command that passes without testing the repo is rejected.
- An `estimate: ~N changed lines (...)` line.
- These markdown sections, each as a `## ` heading: Concern (why the work matters), Requirements (what must hold), Tests (how it is verified), Acceptance (the concrete bar, restating the check).

Keep the spec small and concrete — auto-spec is for task-class work (chores, small features), not adversarial loop work.";

/// The anti-stall kick for spec-draft sessions.
pub const DRAFT_KICK: &str =
    "You have not called submit_plan. Keep exploring read-only, then call submit_plan with the complete drafted spec.";

/// The chat draft call has no tools: the mechanical repo scan is inlined
/// under this heading in the system prompt instead.
const REPO_SCAN_HEADING: &str = "## Repository scan (mechanical, read-only)";

/// A cheap mechanical scan of `cwd` for the tool-less chat draft call: the
/// top-level entries, the crate identity, and which doctrine files exist.
/// Bounded output; failures degrade to notes (never an error).
pub fn repo_scan(cwd: &Path) -> String {
    let mut out = String::new();
    let mut entries: Vec<String> = match fs::read_dir(cwd) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.path().is_dir() {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .collect(),
        Err(e) => vec![format!("<unreadable: {e}>")],
    };
    entries.sort();
    out.push_str(&format!(
        "{}\n{}\n",
        REPO_SCAN_HEADING,
        entries
            .iter()
            .take(80)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    ));
    for (label, rel) in [
        ("Cargo.toml", "Cargo.toml"),
        ("README.md", "README.md"),
        ("SPEC.md", "SPEC.md"),
    ] {
        if let Ok(text) = fs::read_to_string(cwd.join(rel)) {
            let head: String = text.lines().take(12).collect::<Vec<_>>().join("\n");
            out.push_str(&format!("\n--- {label} (first lines) ---\n{head}\n"));
        }
    }
    out
}

/// The chat draft's system prompt: the same spec-format contract as the
/// headless preamble, but tool-less — the model works from the request and
/// the inlined scan.
pub fn chat_draft_system_prompt(scan: &str) -> String {
    format!(
        "You are chug drafting a spec for the operator's request. You have NO \
         tools in this call: work from the request and the repository scan \
         below, and answer with ONLY the complete spec markdown — no prose \
         before or after.\n\n{}\n\n{}\n\nThe spec you draft must follow \
         chug's SPEC format EXACTLY: a `check: <shell command>` line that is \
         GREEN ON THE CURRENT TREE and never vacuous (`true`, `exit 0`, \
         `echo ...` are rejected), an `estimate: ~N changed lines (...)` \
         line, and these `## ` sections: Concern, Requirements, Tests, \
         Acceptance.",
        DRAFT_PREAMBLE.lines().nth(1).unwrap_or(""),
        scan
    )
}

/// The draft-session goal text: the operator request plus (on a redraft)
/// why the previous draft was rejected. The failure rides the goal so the
/// second draft can fix it — the gate itself never edits the spec.
fn draft_goal_text(goal: &str, feedback: Option<&str>) -> String {
    let mut text = format!("Operator request: {goal}");
    if let Some(feedback) = feedback {
        text.push_str(&format!(
            "\n\n## Previous draft rejected — fix these and resubmit\n\n{feedback}"
        ));
    }
    text
}

/// Structurally validate a drafted spec: the four `## ` sections plus a
/// non-empty `check:` and `estimate:` line. `Err` carries operator-readable
/// feedback (it becomes the redraft's fix-list, verbatim).
pub fn validate_spec(text: &str) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("the draft is empty".to_string());
    }
    let mut missing = Vec::new();
    for section in ["Concern", "Requirements", "Tests", "Acceptance"] {
        if !has_section(text, section) {
            missing.push(format!("## {section}"));
        }
    }
    if driver::parse_check_command(text).is_none() {
        missing.push("a `check: <shell command>` line".to_string());
    }
    if !has_estimate_line(text) {
        missing.push("an `estimate: ~N changed lines` line".to_string());
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the draft is missing required pieces: {}",
            missing.join(", ")
        ))
    }
}

/// A `## <name>` section heading (level 1 or 2, name up to whitespace/`:`).
fn has_section(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        let heading = line.trim().trim_start_matches('#').trim();
        heading == name
            || heading.starts_with(name)
                && heading[name.len()..]
                    .starts_with(|c: char| c.is_whitespace() || c == ':')
    })
}

/// First non-empty `estimate:` line (same prefix-parse shape as
/// `driver::parse_check_command`).
fn has_estimate_line(text: &str) -> bool {
    text.lines().any(|line| {
        line.trim()
            .strip_prefix("estimate:")
            .is_some_and(|rest| !rest.trim().is_empty())
    })
}

/// One shell segment (no operators left): does it pass without testing the
/// repo? Conservative — only the certainly-vacuous `check: true` class.
fn is_vacuous_segment(seg: &str) -> bool {
    let seg = seg.trim();
    if seg.is_empty() || seg.starts_with('#') {
        return true;
    }
    let first = seg.split_whitespace().next().unwrap_or_default();
    let rest = seg[first.len()..].trim();
    match first {
        // `exit 0` is vacuous; `exit 3` fails (the dry-run catches it) —
        // only a bare `exit` or `exit 0` is. Same shape for `cd`: bare
        // `cd` goes to $HOME (always exit 0, tests nothing); `cd src`
        // can FAIL (src may not exist) — a real, if weak, assertion.
        "true" | ":" | "pwd" => true,
        "cd" => rest.is_empty(),
        "exit" => rest.is_empty() || rest == "0",
        "echo" | "printf" => true,
        _ => false,
    }
}

/// The vacuous-check predicate: does EVERY shell segment of `check` (across
/// `;`/`&&`/`||`/`|` and newlines) pass without testing the repo? A real
/// segment anywhere (`cargo test`, `test -f …`, a pipeline consumer) means
/// not vacuous — false negatives are left to the dry-run, false positives
/// would reject honest checks.
pub fn is_vacuous_check(check: &str) -> bool {
    check
        .split([';', '\n'])
        .flat_map(|seg| seg.split("&&").flat_map(|s| s.split("||")))
        .all(|seg| seg.split('|').all(is_vacuous_segment))
}

/// Dry-run the drafted check: EXECUTE it (`tools::run_shell` — the same
/// wrapper, timeout, and T144 target-dir scrub the real goal gate uses) and
/// require exit 0. `Err` carries the failing output for the feedback/abort.
fn dry_run_check(cwd: &Path, check: &str) -> Result<(), String> {
    let outcome = crate::tools::run_shell(cwd, check, Duration::from_secs(crate::tools::CHECK_TIMEOUT_SECS))
        .map_err(|e| format!("the check could not execute: {e:#}"))?;
    if !outcome.timed_out && outcome.exit_code == Some(0) {
        return Ok(());
    }
    let exit_label = match outcome.exit_code {
        Some(code) => code.to_string(),
        None => "timeout".to_string(),
    };
    Err(format!(
        "the check exited {} (it must exit 0 on the current tree):\n$ {}\n{}",
        exit_label,
        check,
        crate::tools::truncate_middle(&outcome.output, 2_000, 2_000)
    ))
}

/// The headless draft phase + acceptance gate (T188 reqs 1/2/5).
///
/// Runs at most two draft sessions (initial + one redraft with feedback).
/// Each accepted-requirements draft is dry-run gated; the FIRST draft that
/// passes everything wins and its path is returned. Both attempts failing
/// aborts with the draft + the failing output — never a loosened check.
pub(crate) fn draft_and_gate(
    cwd: &Path,
    goal: &str,
    model: &str,
    max_tokens_per_request: u32,
    client: &mut dyn Llm,
    sink: &mut dyn crate::events::EventSink,
) -> anyhow::Result<PathBuf> {
    let spec_path = cwd.join(AUTO_SPEC_REL);
    let mut feedback: Option<String> = None;
    let mut last_failure = String::new();
    for attempt in 1..=2 {
        eprintln!(
            "chug: auto-spec: drafting spec (attempt {attempt}/2, read-only plan session)…"
        );
        let cfg = driver::PlanConfig {
            cwd: cwd.to_path_buf(),
            kind: PlanKind::SpecDraft,
            spec_path: None,
            goal: draft_goal_text(goal, feedback.as_deref()),
            model: model.to_string(),
            max_iters: DRAFT_MAX_ITERS,
            max_minutes: DRAFT_MAX_MINUTES,
            max_tokens: 0,
            max_tokens_per_request,
            out_path: Some(spec_path.clone()),
            goal_pack: None,
        };
        let mcp = crate::mcp::McpRegistry::new(cwd, true, None)?;
        let code = driver::run_plan_loop(
            cfg,
            client,
            sink,
            crate::observ::global(),
            mcp,
        )?;
        let text = match fs::read_to_string(&spec_path) {
            Ok(text) if code == 0 => text,
            Ok(text) => {
                last_failure = format!(
                    "draft session exited {code}; last draft kept at {}:\n\n{text}",
                    spec_path.display()
                );
                feedback = Some(format!(
                    "the draft session ended without submit_plan (exit code {code}); \
                     call submit_plan ONCE with the complete spec"
                ));
                continue;
            }
            Err(e) => {
                last_failure = format!("draft session exited {code} without writing a draft: {e}");
                feedback = Some(
                    "the previous session produced no draft: call submit_plan ONCE with \
                     the complete spec as markdown"
                        .to_string(),
                );
                continue;
            }
        };
        let shown = format!("[chug] auto-spec draft:\n{text}");
        sink.emit(crate::events::Event::ModelText(shown));
        match validate_spec(&text) {
            Ok(()) => {}
            Err(report) => {
                last_failure = format!("draft rejected: {report}\n\n--- draft ---\n{text}");
                feedback = Some(format!("{report}. Resubmit the complete corrected spec."));
                continue;
            }
        }
        let check = driver::parse_check_command(&text)
            .ok_or_else(|| anyhow!("validated draft lost its check: line"))?;
        if is_vacuous_check(&check) {
            let report = format!(
                "the check is vacuous (`{check}` passes without testing the repo); \
                 draft a real verification command that is green on the current tree"
            );
            last_failure = format!("draft rejected: {report}\n\n--- draft ---\n{text}");
            feedback = Some(report);
            continue;
        }
        match dry_run_check(cwd, &check) {
            Ok(()) => {
                eprintln!(
                    "chug: auto-spec: draft accepted -> {} (check dry-run passed)",
                    spec_path.display()
                );
                return Ok(spec_path);
            }
            Err(output) => {
                last_failure = format!(
                    "draft rejected by the dry-run gate: {output}\n\n--- draft ---\n{text}"
                );
                feedback = Some(format!(
                    "the previous `check:` line failed its dry-run: {output}\n\
                     Fix the check (keep it non-vacuous and green on the current tree) \
                     and resubmit the complete spec."
                ));
            }
        }
    }
    bail!(
        "auto-spec: the drafted check never passed the acceptance gate after 2 attempts \
         (the check is never loosened to pass). {last_failure}"
    )
}

/// Draft a spec for a chat request: ONE direct LLM call (no tools — a
/// nested plan session would rotate the live chat transcript), the same
/// validation + vacuous gate, one redraft with feedback. On success the
/// draft is written to `.chug/auto-spec.md` and returned; on failure the
/// reason is returned for the operator (nothing is written).
pub(crate) fn chat_draft(
    cwd: &Path,
    request: &str,
    client: &mut dyn Llm,
    trace: Option<&str>,
) -> Result<String, String> {
    let scan = repo_scan(cwd);
    let system = chat_draft_system_prompt(&scan);
    let mut feedback: Option<String> = None;
    for _attempt in 1..=2 {
        let mut prompt = format!("Draft the spec for this request:\n\n{request}");
        if let Some(feedback) = &feedback {
            prompt.push_str(&format!(
                "\n\nYour previous draft was rejected: {feedback}\n\
                 Answer again with ONLY the complete corrected spec markdown."
            ));
        }
        let messages = [Message::user(vec![ContentBlock::text_block(prompt)])];
        let resp = client
            .complete(&system, &messages, &[], &ObsCtx { trace_id: trace, iteration: 0 })
            .map_err(|e| format!("the draft call failed: {e:#}"))?;
        let text = resp.text();
        match validate_spec(&text) {
            Ok(()) => {}
            Err(report) => {
                feedback = Some(format!("{report}. Every required piece must be present."));
                continue;
            }
        }
        let check = match driver::parse_check_command(&text) {
            Some(check) => check,
            None => unreachable!("validated"),
        };
        if is_vacuous_check(&check) {
            feedback = Some(format!(
                "the check is vacuous (`{check}`); use a real verification command \
                 that is green on the current tree"
            ));
            continue;
        }
        fs::create_dir_all(cwd.join(".chug")).ok();
        fs::write(cwd.join(AUTO_SPEC_REL), &text)
            .with_context(|| format!("writing {}", AUTO_SPEC_REL))
            .map_err(|e| e.to_string())?;
        return Ok(text);
    }
    Err(match feedback {
        Some(f) => f,
        None => "the draft call produced nothing".to_string(),
    })
}

/// The approve-side gate shared by chat: validate the (possibly
/// operator-edited) draft, refuse vacuous checks, and dry-run the check.
/// `Err` is the operator-readable refusal — the check is never loosened.
pub(crate) fn approve_gate(cwd: &Path, text: &str) -> Result<String, String> {
    validate_spec(text).map_err(|report| {
        format!(
            "auto-spec: draft rejected — {report}. Edit {} and /auto-spec-approve again.",
            AUTO_SPEC_REL
        )
    })?;
    let check = driver::parse_check_command(text).ok_or_else(|| "unreachable".to_string())?;
    if is_vacuous_check(&check) {
        return Err(format!(
            "auto-spec: draft rejected — the check is vacuous (`{check}` passes without \
             testing the repo). Edit {} and /auto-spec-approve again.",
            AUTO_SPEC_REL
        ));
    }
    dry_run_check(cwd, &check).map_err(|output| {
        format!(
            "auto-spec: draft rejected by the dry-run gate — {output}\n\
             Edit {} and /auto-spec-approve again.",
            AUTO_SPEC_REL
        )
    })?;
    Ok(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD_SPEC: &str = "\
# T-fix — rename `run_loop`'s unused sink parameter

check: cargo build

estimate: ~10 changed lines

## Concern

A dead parameter reads as load-bearing.

## Requirements

- The parameter is gone; call sites updated.

## Tests

- `cargo build` green.

## Acceptance

- `cargo build` exits 0.
";

    // ---------- draft shape: all required pieces (fixture) ----------

    #[test]
    fn validate_spec_accepts_a_complete_fixture() {
        validate_spec(GOOD_SPEC).expect("the complete fixture validates");
        assert_eq!(
            driver::parse_check_command(GOOD_SPEC).as_deref(),
            Some("cargo build")
        );
    }

    #[test]
    fn validate_spec_rejects_each_missing_piece_by_name() {
        for (needle, text) in [
            ("## Concern", GOOD_SPEC.replace("## Concern", "## Why")),
            ("## Requirements", GOOD_SPEC.replace("## Requirements", "## Must")),
            ("## Tests", GOOD_SPEC.replace("## Tests", "## Verification")),
            ("## Acceptance", GOOD_SPEC.replace("## Acceptance", "## Bar")),
            (
                "check:",
                GOOD_SPEC.replace("check: cargo build", "checked: cargo build"),
            ),
            (
                "estimate:",
                GOOD_SPEC.replace("estimate: ~10 changed lines", "size: small"),
            ),
        ] {
            let err = validate_spec(&text).expect_err(needle);
            assert!(
                err.contains(needle),
                "rejection must name the missing piece {needle}: {err}"
            );
        }
        assert!(validate_spec("").is_err(), "empty draft rejected");
    }

    #[test]
    fn has_section_tolerates_level_and_trailing_text_not_synonyms() {
        assert!(has_section("## Concern\nx", "Concern"));
        assert!(has_section("# Concern\nx", "Concern"));
        assert!(has_section("## Concern: why\nx", "Concern"));
        assert!(!has_section("## Concerns\nx", "Concern"), "no prefix bleed");
        assert!(!has_section("pre Concern\nx", "Concern"));
    }

    // ---------- the vacuous predicate ----------

    #[test]
    fn vacuous_predicate_pins_the_true_class_and_near_misses() {
        for vacuous in [
            "true",
            "  true  ",
            ":",
            "exit 0",
            "exit",
            "echo ok",
            "echo",
            "printf 'green'",
            "# just a comment",
            "true && echo done",
            "echo a; echo b",
            "true || echo fallback",
            "true | true",
            "cd",
        ] {
            assert!(
                is_vacuous_check(vacuous),
                "{vacuous:?} must classify as vacuous"
            );
        }
        for real in [
            "cargo test",
            "cargo build",
            "test -f marker.txt",
            "make check",
            "true && cargo test",
            "echo starting; cargo build",
            "true | cargo test",
            "echo ok | grep ok",
            "exit 3",
            "cd src",
            "./scripts/gate.sh",
        ] {
            assert!(
                !is_vacuous_check(real),
                "{real:?} must NOT classify as vacuous"
            );
        }
    }

    // ---------- the dry-run gate ----------

    #[test]
    fn dry_run_gate_executes_and_requires_exit_zero() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            dry_run_check(tmp.path(), "test -f missing.txt").is_err(),
            "a failing check is rejected with output"
        );
        fs::write(tmp.path().join("marker.txt"), "x").unwrap();
        dry_run_check(tmp.path(), "test -f marker.txt")
            .expect("a green check passes the dry-run");
        let err = dry_run_check(tmp.path(), "definitely-not-a-command-xyz").unwrap_err();
        assert!(
            err.contains("could not execute") || err.contains("127"),
            "command-not-found is a dry-run failure: {err}"
        );
    }

    // ---------- the chat draft call (scripted Llm, no network) ----------

    /// A scripted end_turn response body carrying `text` (the ScriptedLlm
    /// constructor takes raw bodies).
    fn text_body(text: &str) -> serde_json::Value {
        serde_json::json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "text", "text": text}]
        })
    }

    #[test]
    fn chat_draft_writes_the_file_and_reports_vacuous_rejections() {
        use crate::api::ScriptedLlm;
        let tmp = tempfile::tempdir().unwrap();
        // One call: a complete draft -> written, returned verbatim.
        let mut llm = ScriptedLlm::new(vec![text_body(GOOD_SPEC)]);
        let text = chat_draft(tmp.path(), "rename a param", &mut llm, None)
            .expect("a good first draft is accepted");
        assert_eq!(text, GOOD_SPEC);
        assert_eq!(
            fs::read_to_string(tmp.path().join(AUTO_SPEC_REL)).unwrap(),
            GOOD_SPEC
        );
        assert_eq!(llm.calls.len(), 1);

        // Vacuous check -> one redraft with feedback -> second call accepted.
        let vacuous = GOOD_SPEC.replace("check: cargo build", "check: true");
        let mut llm = ScriptedLlm::new(vec![text_body(&vacuous), text_body(GOOD_SPEC)]);
        let text = chat_draft(tmp.path(), "rename a param", &mut llm, None)
            .expect("the redraft is accepted");
        assert_eq!(text, GOOD_SPEC);
        assert_eq!(llm.calls.len(), 2, "exactly one redraft");
        let second_prompt = format!("{:?}", llm.calls[1].1);
        assert!(
            second_prompt.contains("vacuous"),
            "the redraft prompt carries the rejection feedback: {second_prompt}"
        );

        // Vacuous twice -> Err with the reason, NOTHING written.
        let vacuous_only = GOOD_SPEC.replace("check: cargo build", "check: exit 0");
        let mut llm =
            ScriptedLlm::new(vec![text_body(&vacuous_only), text_body(&vacuous_only)]);
        let err = chat_draft(tmp.path(), "rename a param", &mut llm, None)
            .expect_err("two vacuous drafts refuse");
        assert!(err.contains("vacuous"), "{err}");
        assert_eq!(llm.calls.len(), 2, "bounded: two attempts");
    }

    // ---------- the approve gate ----------

    #[test]
    fn approve_gate_never_loosens_the_check() {
        let tmp = tempfile::tempdir().unwrap();
        // The dry-run gate EXECUTES `check: cargo build` in cwd — give
        // tmp a real (tiny) cargo project so that leg is green.
        std::fs::create_dir(tmp.path().join("src")).unwrap();
        std::fs::write(
            tmp.path().join("Cargo.toml"),
            "[package]\nname = \"dry-run-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("src/main.rs"), "fn main() {}\n").unwrap();
        approve_gate(tmp.path(), GOOD_SPEC).expect("a green non-vacuous draft approves");
        // The operator edited the check to something vacuous -> refused.
        let err = approve_gate(tmp.path(), &GOOD_SPEC.replace("check: cargo build", "check: true"))
            .expect_err("vacuous edits are refused");
        assert!(err.contains("vacuous"), "{err}");
        // A check that cannot pass the dry-run -> refused with the output.
        let err = approve_gate(
            tmp.path(),
            &GOOD_SPEC.replace("check: cargo build", "check: test -f nope.txt"),
        )
        .expect_err("failing checks are refused");
        assert!(err.contains("dry-run"), "{err}");
        // Structure edits that drop a section -> refused.
        let err = approve_gate(tmp.path(), "check: true\nestimate: 1")
            .expect_err("structural holes are refused");
        assert!(err.contains("missing required pieces"), "{err}");
    }
}

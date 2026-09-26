//! T73 phase 1 — `chug plan`: read-only planning mode.
//!
//! Benchmark: Claude Code's plan mode — a read-only session in which the
//! model explores the repo and produces an implementation plan, with zero
//! mutation surface. chug's twist (FEATURES.md header): it must also serve
//! the autonomous loop — an orchestrator can dispatch a plan child into a
//! worktree to draft an approach before burning an impl child.
//!
//! The whole plan-mode surface lives here (the T37 webfetch.rs / T70
//! decisions.rs one-tool-per-file pattern) so src/tools.rs does not grow:
//!
//! - **Tool contract**: EXACTLY five tools are advertised to the API —
//!   `read_file`, `grep`, `glob`, `list_dir`, and the new `submit_plan`.
//!   Nothing else: no write_file/edit_file/bash/delegate/web_fetch/
//!   update_ledger/goal_complete/decision_log, and no MCP schemas.
//! - **`submit_plan`** is the single deliberate write/exit path: input
//!   `{ "plan": string }` (required, min length 1). With `--out`, the plan
//!   string is written VERBATIM to that path (parent dirs created; a path
//!   escaping the cwd sandbox is a tool error — the same lexical rule
//!   `write_file` enforces, stated in plan-mode words); without `--out` the
//!   plan surfaces on stdout. Either way the session then ends successfully.
//! - **Defense in depth**: schema filtering alone is not trusted. Dispatch
//!   rejects EVERY other registered tool name with a tool error naming the
//!   allowed set — it never executes, and the loop continues so the model
//!   can route around the error.

use std::fs;
use std::path::Path;

use anyhow::{anyhow, bail, Context};
use serde_json::{json, Value};

use crate::tools::{self, ToolCtx, ToolResult};

/// The four read-only exploration tools plan mode inherits from the builtin
/// registry (filtered from `tools::tool_schemas()` by name).
pub const READ_ONLY_TOOLS: [&str; 4] = ["read_file", "grep", "glob", "list_dir"];

/// The complete plan-mode tool surface: the read-only four plus the one
/// deliberate write/exit path. Pinned with exact cardinality by tests.
pub const PLAN_TOOL_NAMES: [&str; 5] = ["read_file", "grep", "glob", "list_dir", "submit_plan"];

/// Plan-mode preamble: read-only contract, one exit.
pub const PLAN_PREAMBLE: &str = "You are chug in plan mode: a READ-ONLY planning session. Explore the repository with your read-only tools (read_file, grep, glob, list_dir), then call submit_plan ONCE with the complete implementation plan as markdown. submit_plan is the only write available and it ends the session — write the plan in it, not to any file. You are not implementing anything: do not claim work is done, and do not describe actions as taken.";

/// The anti-stall kick text for plan mode (the autonomous KICK names
/// goal_complete and ledger updates, neither of which exists here).
pub const PLAN_KICK: &str = "You have not called submit_plan. Keep exploring read-only, then call submit_plan with the complete plan.";

/// JSON schema for the `submit_plan` tool (plan-mode only; absent from the
/// run/chat registries).
pub fn schema() -> Value {
    json!({
        "name": "submit_plan",
        "description": "Submit the complete implementation plan and end the plan session. The ONLY write available in plan mode: the plan string is written verbatim to the --out path when one was configured, else printed to stdout. Input is the full plan as markdown (min length 1). Call it exactly once, when the plan is finished.",
        "input_schema": {
            "type": "object",
            "properties": {
                "plan": {"type": "string", "description": "The full implementation plan, as markdown"}
            },
            "required": ["plan"]
        }
    })
}

/// The exact tool list a plan-mode run advertises: the read-only four,
/// filtered from the builtin registry by name (so their schemas can never
/// drift from the run-mode ones), plus `submit_plan`. No MCP schemas are
/// ever appended to this list.
pub fn tool_schemas() -> Vec<Value> {
    let mut out: Vec<Value> = tools::tool_schemas()
        .into_iter()
        .filter(|s| {
            s.get("name")
                .and_then(Value::as_str)
                .is_some_and(|n| READ_ONLY_TOOLS.contains(&n))
        })
        .collect();
    out.push(schema());
    out
}

/// Plan-mode dispatch. Allowed names route to the normal dispatcher; every
/// other name — including all the write/exit tools and any `mcp__*` import —
/// is rejected with a tool error naming the allowed set, without executing.
pub fn dispatch(ctx: &ToolCtx, name: &str, input: &Value, out: Option<&Path>) -> ToolResult {
    match plan_inner(ctx, name, input, out) {
        Ok(result) => result,
        Err(e) => ToolResult {
            content: format!("tool error: {e:#}"),
            is_error: true,
        },
    }
}

fn plan_inner(
    ctx: &ToolCtx,
    name: &str,
    input: &Value,
    out: Option<&Path>,
) -> anyhow::Result<ToolResult> {
    if name == "submit_plan" {
        return submit_plan(ctx, input, out);
    }
    if READ_ONLY_TOOLS.contains(&name) {
        return Ok(tools::dispatch(ctx, name, input));
    }
    bail!(
        "plan mode: tool `{name}` is not available. The allowed set is exactly: {}. \
         Keep exploring read-only and call submit_plan with the finished plan.",
        PLAN_TOOL_NAMES.join(", ")
    )
}

/// The `submit_plan` write/exit path: write the plan verbatim to `out`
/// (creating parent dirs; sandbox-enforced) or leave it for stdout, and
/// report success so the driver can end the session with exit 0.
fn submit_plan(ctx: &ToolCtx, input: &Value, out: Option<&Path>) -> anyhow::Result<ToolResult> {
    let plan = input
        .get("plan")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string field: plan"))?;
    if plan.is_empty() {
        bail!("submit_plan: `plan` must not be empty (min length 1)");
    }
    let Some(out) = out else {
        return Ok(ToolResult {
            content: format!(
                "plan received ({} bytes) — printed to stdout; the plan session ends",
                plan.len()
            ),
            is_error: false,
        });
    };
    // Sandbox: the same lexical rule write_file enforces via resolve_safe,
    // stated in plan-mode words (there is no bash here to escape with, so
    // the stock refusal's bash suffix would be a lie).
    let resolved = match tools::resolve_safe(&ctx.cwd, &out.to_string_lossy()) {
        Ok(path) => path,
        Err(_) => bail!(
            "submit_plan --out {out:?} escapes the cwd sandbox: plan mode writes only \
             inside the working directory (a relative path with no `..` traversal, or \
             an absolute path inside it)"
        ),
    };
    if let Some(parent) = resolved.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent dirs for {}", resolved.display()))?;
    }
    fs::write(&resolved, plan)
        .with_context(|| format!("writing plan to {}", resolved.display()))?;
    Ok(ToolResult {
        content: format!(
            "plan written to {} ({} bytes); the plan session ends",
            resolved.display(),
            plan.len()
        ),
        is_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn ctx(cwd: &Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(1),
        }
    }

    // ---------- the five-tool surface, exact cardinality ----------

    #[test]
    fn plan_tool_list_is_exactly_the_five_names() {
        let schemas = tool_schemas();
        let names: Vec<&str> = schemas
            .iter()
            .filter_map(|s| s.get("name").and_then(Value::as_str))
            .collect();
        // Exact cardinality: a sixth schema added, or one dropped, is RED.
        assert_eq!(names.len(), 5, "{names:?}");
        let mut sorted = names.clone();
        sorted.sort_unstable();
        let mut expected = PLAN_TOOL_NAMES.to_vec();
        expected.sort_unstable();
        assert_eq!(sorted, expected, "plan surface must be exactly the five");
        // No duplicates hiding behind the cardinality check.
        let set: std::collections::BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(set.len(), 5, "duplicate names in the plan surface: {names:?}");
    }

    #[test]
    fn plan_read_only_schemas_match_the_run_mode_ones_byte_for_byte() {
        // The four read-only schemas are FILTERED from the builtin registry,
        // not re-declared: any run-mode description drift shows up here too,
        // and a plan-local re-declaration would fail this byte compare.
        let schemas = tool_schemas();
        let plan: Vec<&Value> = schemas
            .iter()
            .filter(|s| {
                s.get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|n| READ_ONLY_TOOLS.contains(&n))
            })
            .collect();
        assert_eq!(plan.len(), 4);
        for schema in plan {
            let name = schema["name"].as_str().unwrap();
            let builtins = tools::tool_schemas();
            let builtin = builtins
                .iter()
                .find(|s| s.get("name").and_then(Value::as_str) == Some(name))
                .unwrap_or_else(|| panic!("{name} missing from the builtin registry"));
            assert_eq!(schema, builtin, "{name} must be the builtin schema");
        }
    }

    // ---------- defense in depth: the rejection sweep ----------

    /// One leg per excluded registered tool. Each leg asserts (a) the result
    /// is a tool error naming the allowed set — the wording only the plan
    /// gate produces, so a leg where the tool actually executed cannot pass —
    /// and (b) the tool's specific side effect did NOT happen.
    #[test]
    fn every_excluded_tool_is_rejected_naming_the_allowed_set_without_executing() {
        let tmp = tempfile::tempdir().unwrap();
        // Pre-existing file the edit_file leg would have to change.
        fs::write(tmp.path().join("target.txt"), "original").unwrap();
        // A directory a delegate launch would log into (delegate writes
        // <child-cwd>/.chug/delegate.log at spawn).
        let child_dir = tempfile::tempdir().unwrap();

        // (name, input, side-effect check) — the check receives the tmp cwd.
        type Leg<'a> = (&'a str, Value, Box<dyn Fn(&Path)>);
        let legs: Vec<Leg> = vec![
            (
                "write_file",
                json!({"path": "escape.md", "content": "mutated"}),
                Box::new(|cwd: &Path| assert!(!cwd.join("escape.md").exists())),
            ),
            (
                "edit_file",
                json!({"path": "target.txt", "old": "original", "new": "mutated"}),
                Box::new(|cwd: &Path| {
                    assert_eq!(
                        fs::read_to_string(cwd.join("target.txt")).unwrap(),
                        "original"
                    );
                }),
            ),
            (
                "bash",
                json!({"command": "touch pwned-by-bash.txt"}),
                Box::new(|cwd: &Path| assert!(!cwd.join("pwned-by-bash.txt").exists())),
            ),
            (
                "delegate",
                json!({
                    "action": "launch",
                    "cwd": child_dir.path().display().to_string(),
                    "spec": child_dir.path().join("s.md").display().to_string(),
                    "goal": "g",
                    "model": "m"
                }),
                Box::new(move |_: &Path| {
                    assert!(
                        !child_dir.path().join(".chug/delegate.log").exists(),
                        "delegate must never spawn in plan mode"
                    );
                }),
            ),
            (
                "web_fetch",
                json!({"url": "http://127.0.0.1:1/x"}),
                Box::new(|_: &Path| {}), // message assert below is the leg
            ),
            (
                "update_ledger",
                json!({"content": "MUTATED LEDGER"}),
                Box::new(|cwd: &Path| assert!(!cwd.join("LEDGER.md").exists())),
            ),
            (
                "goal_complete",
                json!({"summary": "claim done"}),
                Box::new(|_: &Path| {}),
            ),
            (
                "decision_log",
                json!({
                    "class": "outcome",
                    "subject": "T73",
                    "inputs": "i",
                    "options": "o",
                    "choice": "landed-clean",
                    "confidence": 0.5
                }),
                Box::new(|cwd: &Path| assert!(
                    !cwd.join(".chug/decisions.jsonl").exists(),
                    "decision_log must never write in plan mode"
                )),
            ),
        ];
        assert_eq!(legs.len(), 8, "one leg per excluded registered tool");

        for (name, input, check) in legs {
            let result = dispatch(&ctx(tmp.path()), name, &input, None);
            assert!(result.is_error, "{name}: must be rejected, got {result:?}");
            assert!(
                result.content.contains("plan mode"),
                "{name}: rejection must come from the plan gate, not the tool: {}",
                result.content
            );
            assert!(
                result.content.contains("submit_plan"),
                "{name}: rejection must name the allowed set: {}",
                result.content
            );
            check(tmp.path());
        }
    }

    // ---------- submit_plan behavior ----------

    const PLAN: &str = "# Plan\n\n1. add the flag\n2. pin the parse\n";

    #[test]
    fn submit_plan_writes_out_file_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("plan.md");
        let result = dispatch(
            &ctx(tmp.path()),
            "submit_plan",
            &json!({"plan": PLAN}),
            Some(&out),
        );
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(fs::read(&out).unwrap(), PLAN.as_bytes(), "verbatim bytes");
        assert!(result.content.contains("plan.md"), "{}", result.content);
    }

    #[test]
    fn submit_plan_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("scratch/deep/plan.md");
        let result = dispatch(
            &ctx(tmp.path()),
            "submit_plan",
            &json!({"plan": PLAN}),
            Some(&out),
        );
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(fs::read(&out).unwrap(), PLAN.as_bytes());
    }

    #[test]
    fn submit_plan_rejects_sandbox_escape_naming_the_rule() {
        let tmp = tempfile::tempdir().unwrap();
        // Relative `..` traversal …
        let result = dispatch(
            &ctx(tmp.path()),
            "submit_plan",
            &json!({"plan": PLAN}),
            Some(&tmp.path().join("../x.md")),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("sandbox"),
            "error must name the sandbox rule: {}",
            result.content
        );
        // … and an absolute path outside cwd.
        let result = dispatch(
            &ctx(tmp.path()),
            "submit_plan",
            &json!({"plan": PLAN}),
            Some(Path::new("/etc/passwd")),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("sandbox"), "{}", result.content);
        assert!(
            !tmp.path().parent().unwrap().join("x.md").exists(),
            "the plan must never land outside cwd"
        );
    }

    #[test]
    fn submit_plan_rejects_empty_and_missing_plan() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(&ctx(tmp.path()), "submit_plan", &json!({"plan": ""}), None);
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("empty"), "{}", result.content);
        let result = dispatch(&ctx(tmp.path()), "submit_plan", &json!({}), None);
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("plan"), "{}", result.content);
    }

    #[test]
    fn submit_plan_without_out_reports_stdout_success() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(&ctx(tmp.path()), "submit_plan", &json!({"plan": PLAN}), None);
        assert!(!result.is_error, "{}", result.content);
    }

    // ---------- the run/chat surface stays clean of submit_plan ----------

    #[test]
    fn submit_plan_is_absent_from_the_run_mode_registry() {
        let schemas = tools::tool_schemas();
        let names: Vec<&str> = schemas
            .iter()
            .filter_map(|s| s.get("name").and_then(Value::as_str))
            .collect();
        assert!(
            !names.contains(&"submit_plan"),
            "run-mode registry must not advertise submit_plan: {names:?}"
        );
    }
}

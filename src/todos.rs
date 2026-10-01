//! T111 — F8 phase 1: the structured todo tool (`todo_add` / `todo_update` /
//! `todo_list`) over `<cwd>/.chug/todos.json`.
//!
//! FEATURES.md F8: the freeform LEDGER edit is the only self-declared plan
//! surface today, and it is unqueryable — a budget-death resume (T63) has to
//! re-derive "what was I doing" from transcript archaeology, and an
//! orchestrator harvesting a child worktree has no jq-able view of the
//! child's self-declared step list. This module gives both a machine-readable
//! home: a JSON array of `{"id","title","status"}` objects the model edits
//! through three driver-visible tools (benchmark: Claude Code's task tools),
//! with the loop twist that the store is cwd-confined so the right semantics
//! fall out by construction — a fresh worktree child boots with an empty
//! list, a resumed child inherits the file in one read.
//!
//! Module shape follows the T70 decisions.rs pattern: ALL logic lives here
//! (schemas, handlers, store I/O, rendering), and src/tools.rs carries only
//! the registration lines (one `schemas()` extend + three dispatch arms) so
//! the tools.rs monolith does not grow. Like `update_ledger` (the
//! bookkeeping-tool precedent) the three tools are run+chat surface and
//! EXCLUDED from plan mode's read-only set: plan.rs filters by name, so the
//! exclusion is structural — a todo call there dies at the plan gate naming
//! the allowed set.
//!
//! Store semantics (spec'd):
//! - ids allocate `t1`, `t2`, … (next = array length + 1; v1 has no removal,
//!   so ids are stable),
//! - missing file = empty list (lazy creation on first `todo_add` only),
//! - corrupt file = every todo tool returns a tool error naming the file
//!   (never panic, never silently discard — the remedy is in the message),
//! - writes go through one compact-rewrite `save()` (the .chug/ append/write
//!   precedent: no fsync ceremony, no new deps).
//!
//! Prompt injection lives in driver.rs (the `## Todos` section after
//! `## Ledger`, run + chat modes, nothing when empty); this module only
//! supplies the rendering.
//!
//! Events: deliberately none (the update_ledger precedent) — every call's
//! `tool_result` preview in `.chug/events.jsonl` IS the audit trail; the
//! durable surface is the store itself.
//!
//! Phase 2 (deferred with a written reason in EVALUATION.md cycle-60 §4):
//! `todo_remove`, blocked-by dependencies, a chat `/todo` command, fork-slot
//! save/restore, per-run reset, MCP exposure, sub-task hierarchies.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::tools::ToolResult;

/// The enforced statuses, named verbatim in the schema description and in
/// every rejection message so the model never has to guess the set.
pub const STATUSES: [&str; 3] = ["pending", "in_progress", "done"];

/// A todo's status. Serde's `snake_case` maps the variants to exactly the
/// three store spellings above; an unknown spelling in the file fails the
/// load (corrupt store — the tools are the only v1 writers, so anything else
/// is foreign data, never silently accepted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Done,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::InProgress => "in_progress",
            Status::Done => "done",
        }
    }

    /// Parse a tool-input status string. The error names the valid set
    /// (T41/T85/T88 remedy doctrine).
    fn parse(s: &str) -> anyhow::Result<Status> {
        match s {
            "pending" => Ok(Status::Pending),
            "in_progress" => Ok(Status::InProgress),
            "done" => Ok(Status::Done),
            other => Err(anyhow!(
                "todo_update: invalid status {other:?} — valid statuses are: {}",
                STATUSES.join(", ")
            )),
        }
    }
}

/// One todo. Field order here IS the stored JSON key order (serde
/// serializes struct fields in declaration order), matching the documented
/// store shape `{"id","title","status"}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub status: Status,
}

/// The store path: `<cwd>/.chug/todos.json` — cwd-confined like every other
/// `.chug/` surface, so worktree children get their own store by construction.
pub fn todos_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("todos.json")
}

/// Load the store. Missing file = empty list (a fresh worktree has no
/// `.chug/` at all). An empty (0-byte / whitespace-only) file bootstraps to
/// an empty list too — `touch` must not brick the surface. Anything else
/// unparseable is CORRUPT: the error names the file and the expected shape,
/// never silently discarded, never a panic.
pub fn load(cwd: &Path) -> anyhow::Result<Vec<Todo>> {
    let path = todos_path(cwd);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(&text).with_context(|| {
        format!(
            "todo store {} is corrupt (expected a JSON array of {{\"id\", \"title\", \
             \"status\"}} objects with status one of: {}); fix or delete the file, then retry",
            path.display(),
            STATUSES.join(", ")
        )
    })
}

/// The one write path: rewrite the store compactly, creating `.chug/` when
/// missing. Compact (no pretty-print) — jq-mineable, diff-friendly, and the
/// smallest bytes on disk. T136: the write is atomic (temp+rename) — this
/// store is live state rewritten on every todo_add/todo_update, and a
/// truncating in-place write destroyed it when a crash landed mid-write.
fn save(cwd: &Path, todos: &[Todo]) -> anyhow::Result<()> {
    let path = todos_path(cwd);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    let text = serde_json::to_string(todos).context("serializing todos")?;
    crate::fsatomic::write_atomic(&path, text.as_bytes())
        .with_context(|| format!("writing {}", path.display()))
}

/// One line per todo: `t3 [in_progress] title` — the rendering shared by
/// `todo_list`, the prompt's `## Todos` section, and any future consumer.
pub fn render(todos: &[Todo]) -> String {
    todos
        .iter()
        .map(|t| format!("{} [{}] {}", t.id, t.status.as_str(), t.title))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The prompt-injection payload: the rendered list, or an EMPTY string when
/// the store is empty or unreadable — the driver only emits a `## Todos`
/// heading for non-empty text, so no empty heading is ever produced. A
/// corrupt store renders nothing here (the todo tools still name the file
/// when called — the error is the surface, not the prompt).
pub fn prompt_text(cwd: &Path) -> String {
    match load(cwd) {
        Ok(todos) if !todos.is_empty() => render(&todos),
        _ => String::new(),
    }
}

/// Schemas for the three todo tools, in registration order, registered in
/// `tools::tool_schemas()` beside `update_ledger` (the bookkeeping group).
/// Lives here (not tools.rs) per the T70 single-source-of-truth pattern: the
/// description the model sees and the validation that enforces it stay in
/// one file.
pub fn schemas() -> Vec<Value> {
    let status_doc = format!("New status: one of {}.", STATUSES.join(", "));
    vec![
        json!({
            "name": "todo_add",
            "description": "Add a todo to `.chug/todos.json` — your structured, queryable step list (jq-mineable by orchestrators; survives a resume). Appends with status `pending` and returns the allocated id (`t1`, `t2`, …). Keep titles short and step-shaped.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "What the step is (one line)"}
                },
                "required": ["title"]
            }
        }),
        json!({
            "name": "todo_update",
            "description": format!(
                "Update one todo in `.chug/todos.json` by id — flip its `status` \
                 (one of {statuses}) and/or retitle it. Unknown id is an error naming the \
                 existing ids; at least one of `status`/`title` is required.",
                statuses = STATUSES.join(", ")
            ),
            "input_schema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "The todo id, e.g. `t3`"},
                    "status": {"type": "string", "enum": STATUSES, "description": status_doc},
                    "title": {"type": "string", "description": "Replacement title"}
                }
            }
        }),
        json!({
            "name": "todo_list",
            "description": "List your todos from `.chug/todos.json` as `t3 [in_progress] title` lines (or `no todos`). The same rendering rides the system prompt's `## Todos` section whenever the list is non-empty.",
            "input_schema": {
                "type": "object",
                "properties": {}
            }
        }),
    ]
}

/// Shared field extraction: a present-and-string field, treating an explicit
/// JSON `null` as absent (the decisions.rs str_field precedent). Returns
/// `None` for absent-or-null, `Some(Err(..))` for a present non-string so
/// the caller can name the field's expected shape.
fn opt_str_field(obj: &Map<String, Value>, key: &str) -> Option<anyhow::Result<String>> {
    match obj.get(key) {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(Ok(s.clone())),
        Some(other) => Some(Err(anyhow!("{key} must be a string, got {}", json_type_name(other)))),
    }
}

/// Human-readable name for a `serde_json` value's type, used by the
/// corrective legs ("got number", "got array") — the decisions.rs local
/// shape (each validation module names its own types; no cross-module
/// coupling).
fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn require_str(input: &Value, key: &str) -> anyhow::Result<String> {
    let Some(obj) = input.as_object() else {
        return Err(anyhow!("missing or non-string field: {key}"));
    };
    match opt_str_field(obj, key) {
        Some(r) => r,
        None => Err(anyhow!("missing or non-string field: {key}")),
    }
}

/// The one title rule (T116): a title being SET — at `todo_add` or at
/// `todo_update` — must be non-empty after trimming. The trim-then-check
/// and the error shape (`{tool}: \`title\` must not be empty — {remedy}`)
/// live in this single helper so the two paths cannot drift again (T111's
/// finding: update accepted what add rejected). Only the remedy wording
/// differs per caller: `todo_add`'s is pinned byte-identical, and
/// `todo_update`'s names the rule.
fn ensure_title_non_empty(tool: &str, remedy: &str, title: &str) -> anyhow::Result<()> {
    if title.trim().is_empty() {
        bail!("{tool}: `title` must not be empty — {remedy}");
    }
    Ok(())
}

/// `todo_add` `{title}`: append with status `pending`, allocate
/// `t<length+1>`, return the id. Lazy-creates `.chug/todos.json` on the
/// first add (a fresh worktree never gets a store it did not ask for).
/// The non-empty-title rule is shared with `todo_update` via
/// `ensure_title_non_empty` (T116).
pub fn todo_add(cwd: &Path, input: &Value) -> anyhow::Result<ToolResult> {
    let title = require_str(input, "title")?;
    ensure_title_non_empty("todo_add", "say what the step is", &title)?;
    let mut todos = load(cwd)?;
    let id = format!("t{}", todos.len() + 1);
    todos.push(Todo {
        id: id.clone(),
        title,
        status: Status::Pending,
    });
    save(cwd, &todos)?;
    let stored = todos.last().expect("just pushed");
    Ok(ToolResult {
        content: format!("added {} (pending): {}", stored.id, stored.title),
        is_error: false,
        images: Vec::new(),
    })
}

/// `todo_update` `{id, status?, title?}`: the enforced-status mutation.
/// Validation diagnoses the whole call in one error (id shape, the
/// at-least-one-field rule, the status set, the non-empty-title rule)
/// BEFORE the id lookup, so a second retry does not whack-a-mole through
/// one-leg-per-call errors; an unknown id names the existing ids.
pub fn todo_update(cwd: &Path, input: &Value) -> anyhow::Result<ToolResult> {
    let obj = input
        .as_object()
        .ok_or_else(|| anyhow!("todo_update expects a JSON object of {{id, status?, title?}}"))?;
    let id = match opt_str_field(obj, "id") {
        Some(r) => r?,
        None => bail!("missing or non-string field: id"),
    };

    let new_status = match opt_str_field(obj, "status") {
        Some(r) => Some(Status::parse(&r?)?),
        None => None,
    };
    let new_title = match opt_str_field(obj, "title") {
        Some(r) => Some(r?),
        None => None,
    };
    if new_status.is_none() && new_title.is_none() {
        bail!(
            "todo_update: nothing to update — pass `status` (one of {}) and/or `title`",
            STATUSES.join(", ")
        );
    }
    if let Some(title) = &new_title {
        ensure_title_non_empty(
            "todo_update",
            "titles must be non-empty, say what the step is",
            title,
        )?;
    }

    let mut todos = load(cwd)?;
    let Some(todo) = todos.iter_mut().find(|t| t.id == id) else {
        bail!(
            "todo_update: unknown id {id:?} — existing ids: {}",
            if todos.is_empty() {
                "(none — the todo list is empty)".to_string()
            } else {
                todos.iter().map(|t| t.id.as_str()).collect::<Vec<_>>().join(", ")
            }
        );
    };
    if let Some(status) = new_status {
        todo.status = status;
    }
    if let Some(title) = new_title {
        todo.title = title;
    }
    let (id, title, status) = (todo.id.clone(), todo.title.clone(), todo.status);
    save(cwd, &todos)?;
    Ok(ToolResult {
        content: format!("updated {id}: {title} [{}]", status.as_str()),
        is_error: false,
        images: Vec::new(),
    })
}

/// `todo_list` `{}`: the compact rendered list, or `no todos` when empty.
pub fn todo_list(cwd: &Path) -> anyhow::Result<ToolResult> {
    let todos = load(cwd)?;
    let content = if todos.is_empty() {
        "no todos".to_string()
    } else {
        render(&todos)
    };
    Ok(ToolResult {
        content,
        is_error: false,
        images: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{dispatch, tool_schemas, ToolCtx};
    use std::time::Duration;
    use tempfile::TempDir;

    fn ctx(cwd: &Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        }
    }

    fn stored(cwd: &Path) -> Vec<Value> {
        let text = fs::read_to_string(todos_path(cwd)).expect("store readable");
        serde_json::from_str(&text).expect("store parses as JSON")
    }

    // ---------- unit: store round-trip / bootstrapping ----------

    #[test]
    fn missing_file_and_missing_chug_dir_load_as_empty_list() {
        let tmp = TempDir::new().unwrap();
        // No .chug/ at all — the fresh-worktree shape.
        assert!(!tmp.path().join(".chug").exists());
        assert_eq!(load(tmp.path()).unwrap(), Vec::new());
        // todo_list on the virgin worktree: `no todos`, and the read must
        // NOT create the store (lazy creation is add-only).
        let result = dispatch(&ctx(tmp.path()), "todo_list", &json!({}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "no todos");
        assert!(!todos_path(tmp.path()).exists(), "todo_list never writes");
    }

    #[test]
    fn empty_file_bootstraps_to_empty_list() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(todos_path(tmp.path()), "").unwrap();
        assert_eq!(load(tmp.path()).unwrap(), Vec::new());
        let result = dispatch(&ctx(tmp.path()), "todo_list", &json!({}));
        assert!(!result.is_error, "{}", result.content);
        assert_eq!(result.content, "no todos");
    }

    #[test]
    fn round_trip_save_then_load_preserves_ids_titles_statuses() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "first"}));
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "second"}));
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t2", "status": "in_progress"}),
        );
        assert!(!r.is_error, "{}", r.content);

        // The on-disk shape is the documented compact array of
        // {"id","title","status"} in key order — checked on the RAW text,
        // because reading it back through a serde_json Map (BTreeMap by
        // default) sorts keys and would hide a struct-field reorder.
        let text = fs::read_to_string(todos_path(tmp.path())).unwrap();
        assert!(!text.contains('\n'), "compact single-line store: {text}");
        let raw: Vec<Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(raw.len(), 2);
        assert!(
            text.contains(r#"{"id":"t2","title":"second","status":"in_progress"}"#),
            "declaration key order on the raw line: {text}"
        );

        // And load() reads the same data back.
        let todos = load(tmp.path()).unwrap();
        assert_eq!(
            todos,
            vec![
                Todo { id: "t1".into(), title: "first".into(), status: Status::Pending },
                Todo { id: "t2".into(), title: "second".into(), status: Status::InProgress },
            ]
        );
    }

    /// T136 class sweep: the todo store is live state rewritten on every
    /// todo_add/todo_update. The save goes through the atomic temp+rename
    /// helper, so a crash mid-write cannot destroy the store, and no temp
    /// sibling lingers after a clean save.
    #[test]
    fn save_is_atomic_no_temp_leftover_and_survives_a_failed_write() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let r = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "first"}));
        assert!(!r.is_error, "{}", r.content);
        let before = fs::read_to_string(todos_path(tmp.path())).unwrap();
        assert!(before.contains("\"first\""));

        // Failure leg: the store path being a directory makes the final
        // rename fail. The save must propagate the error, leave the target
        // untouched, and clean its temp file.
        let store = todos_path(tmp.path());
        let backup = tmp.path().join("todos.json.bak");
        fs::rename(&store, &backup).unwrap();
        fs::create_dir(&store).unwrap();
        let r = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "second"}));
        assert!(r.is_error, "the failed save surfaces as a tool error");
        assert!(store.is_dir(), "the target was never touched by the failed write");
        fs::remove_dir(&store).unwrap();
        fs::rename(&backup, &store).unwrap();

        // Clean save: content replaced, exactly one file in .chug (the
        // store itself — no temp sibling).
        let r = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "third"}));
        assert!(!r.is_error, "{}", r.content);
        let text = fs::read_to_string(todos_path(tmp.path())).unwrap();
        assert!(text.contains("\"third\""), "{text}");
        let entries: Vec<String> = fs::read_dir(tmp.path().join(".chug"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["todos.json".to_string()], "no temp siblings: {entries:?}");
    }

    #[test]
    fn id_allocation_sequence_is_t1_t2_t3_from_array_length_plus_one() {
        let tmp = TempDir::new().unwrap();
        for (n, expected) in ["t1", "t2", "t3"].iter().enumerate() {
            let r = dispatch(
                &ctx(tmp.path()),
                "todo_add",
                &json!({"title": format!("step {}", n + 1)}),
            );
            assert!(!r.is_error, "{}", r.content);
            assert!(
                r.content.contains(expected),
                "add #{} must allocate {expected}: {}",
                n + 1,
                r.content
            );
        }
        let todos = stored(tmp.path());
        let ids: Vec<&str> = todos.iter().filter_map(|t| t["id"].as_str()).collect();
        assert_eq!(ids, vec!["t1", "t2", "t3"]);
        // Every add lands as pending (spec: appends with status `pending`).
        assert!(
            todos.iter().all(|t| t["status"] == "pending"),
            "{}",
            serde_json::to_string(&todos).unwrap()
        );
    }

    #[test]
    fn corrupt_store_is_a_tool_error_naming_the_file_for_every_todo_tool() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(todos_path(tmp.path()), "{{{ not json").unwrap();
        let path_disp = todos_path(tmp.path()).display().to_string();

        let add = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "x"}));
        assert!(add.is_error, "{}", add.content);
        assert!(add.content.contains(&path_disp), "{}", add.content);

        let update = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "done"}),
        );
        assert!(update.is_error, "{}", update.content);
        assert!(update.content.contains(&path_disp), "{}", update.content);

        let list = dispatch(&ctx(tmp.path()), "todo_list", &json!({}));
        assert!(list.is_error, "{}", list.content);
        assert!(list.content.contains(&path_disp), "{}", list.content);

        // The remedy is named (fix or delete), and nothing was overwritten:
        // the corrupt bytes survive byte-identically — never silently
        // discarded.
        assert!(list.content.contains("fix or delete"), "{}", list.content);
        assert_eq!(
            fs::read_to_string(todos_path(tmp.path())).unwrap(),
            "{{{ not json"
        );
    }

    #[test]
    fn semantically_corrupt_store_bad_status_is_also_an_error_naming_the_file() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        // Structurally valid JSON, foreign status: the tools are the only v1
        // writers, so this is foreign data — rejected, not silently accepted.
        fs::write(
            todos_path(tmp.path()),
            r#"[{"id":"t1","title":"x","status":"cancelled"}]"#,
        )
        .unwrap();
        let list = dispatch(&ctx(tmp.path()), "todo_list", &json!({}));
        assert!(list.is_error, "{}", list.content);
        assert!(
            list.content.contains(&todos_path(tmp.path()).display().to_string()),
            "{}",
            list.content
        );
    }

    // ---------- unit: status enforcement ----------

    #[test]
    fn status_enum_accepts_all_three_valid_statuses() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "step"}));
        for status in STATUSES {
            let r = dispatch(
                &ctx(tmp.path()),
                "todo_update",
                &json!({"id": "t1", "status": status}),
            );
            assert!(!r.is_error, "{status}: {}", r.content);
            assert!(
                r.content.contains(&format!("[{status}]")),
                "{status}: {}",
                r.content
            );
        }
        let todos = load(tmp.path()).unwrap();
        assert_eq!(todos[0].status.as_str(), "done", "last write wins");
    }

    #[test]
    fn invalid_status_is_rejected_naming_the_valid_set() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "step"}));
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "cancelled"}),
        );
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("cancelled"), "{}", r.content);
        assert!(
            r.content.contains("pending, in_progress, done"),
            "valid set named: {}",
            r.content
        );
        // The rejection must not have mutated the store.
        assert_eq!(load(tmp.path()).unwrap()[0].status, Status::Pending);
    }

    // ---------- unit: unknown id ----------

    #[test]
    fn unknown_id_error_names_the_existing_ids() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "a"}));
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "b"}));
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t9", "status": "done"}),
        );
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("t9"), "{}", r.content);
        assert!(r.content.contains("t1, t2"), "existing ids named: {}", r.content);

        // Unknown id against an EMPTY list names the emptiness instead of a
        // bare ", " join.
        let empty = TempDir::new().unwrap();
        dispatch(&ctx(empty.path()), "todo_add", &json!({"title": "a"}));
        fs::remove_file(todos_path(empty.path())).unwrap();
        let r = dispatch(
            &ctx(empty.path()),
            "todo_update",
            &json!({"id": "t1", "status": "done"}),
        );
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("empty"), "{}", r.content);
    }

    // ---------- unit: update field rules ----------

    #[test]
    fn todo_update_with_neither_status_nor_title_errors() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "a"}));
        for input in [json!({"id": "t1"}), json!({"id": "t1", "status": null, "title": null})] {
            let r = dispatch(&ctx(tmp.path()), "todo_update", &input);
            assert!(r.is_error, "{input}: {}", r.content);
            assert!(
                r.content.contains("nothing to update") && r.content.contains("status") && r.content.contains("title"),
                "remedy named: {}",
                r.content
            );
        }
        // Explicit nulls count as absent — the store is untouched either way.
        assert_eq!(load(tmp.path()).unwrap()[0].title, "a");
    }

    #[test]
    fn todo_update_non_string_fields_are_named() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "a"}));
        let r = dispatch(&ctx(tmp.path()), "todo_update", &json!({"id": 3, "status": "done"}));
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("id"), "{}", r.content);
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "done", "title": 42}),
        );
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("title"), "{}", r.content);
    }

    #[test]
    fn todo_update_retitle_only_and_status_only_both_work() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "old"}));
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "title": "new"}),
        );
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(load(tmp.path()).unwrap()[0].title, "new");
        assert_eq!(load(tmp.path()).unwrap()[0].status, Status::Pending);
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "done"}),
        );
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(load(tmp.path()).unwrap()[0].title, "new", "title kept");
        assert_eq!(load(tmp.path()).unwrap()[0].status, Status::Done);
    }

    #[test]
    fn todo_update_rejects_empty_and_whitespace_only_titles() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "a"}));
        for title in ["", "   "] {
            let r = dispatch(
                &ctx(tmp.path()),
                "todo_update",
                &json!({"id": "t1", "title": title}),
            );
            assert!(r.is_error, "{title:?}: {}", r.content);
            assert!(
                r.content.contains("titles must be non-empty"),
                "rule named: {}",
                r.content
            );
        }
        // The failed updates never wrote: title and status are untouched.
        let stored = load(tmp.path()).unwrap();
        assert_eq!(stored[0].title, "a");
        assert_eq!(stored[0].status, Status::Pending);

        // An empty title is rejected even alongside a status change, and
        // the whole call is refused before any mutation (validation
        // precedes the id lookup and the save).
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "done", "title": ""}),
        );
        assert!(r.is_error, "{}", r.content);
        assert_eq!(
            load(tmp.path()).unwrap()[0].status,
            Status::Pending,
            "failed update never writes"
        );

        // Trim-then-CHECK, not trim-then-store: a title with surrounding
        // whitespace passes the rule and is stored as-is.
        let r = dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "title": "  spaced  "}),
        );
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(load(tmp.path()).unwrap()[0].title, "  spaced  ");
    }

    // ---------- unit: add validation ----------

    #[test]
    fn todo_add_missing_or_empty_title_is_rejected() {
        let tmp = TempDir::new().unwrap();
        let r = dispatch(&ctx(tmp.path()), "todo_add", &json!({}));
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("title"), "{}", r.content);
        let r = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "   "}));
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("empty"), "{}", r.content);
        assert!(!todos_path(tmp.path()).exists(), "failed adds never write");
    }

    /// T116 symmetry leg: the same two offending titles against BOTH
    /// title-setting paths produce the same error shape —
    /// `{tool}: \`title\` must not be empty — {remedy}` — with
    /// `todo_add`'s bytes pinned byte-identical so future drift is
    /// visible in one diff.
    #[test]
    fn empty_title_errors_have_the_same_shape_on_both_paths() {
        let tmp = TempDir::new().unwrap();
        for title in ["", "   "] {
            let add = dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": title}));
            assert!(add.is_error, "{title:?}: {}", add.content);
            assert_eq!(
                add.content,
                "tool error: todo_add: `title` must not be empty — say what the step is",
                "add's wording is pinned byte-identical (dispatch adds its constant `tool error: ` prefix)"
            );

            // Validation precedes the id lookup: the title rule fires even
            // against an id that does not exist (no todo seeded here).
            let upd = dispatch(
                &ctx(tmp.path()),
                "todo_update",
                &json!({"id": "t9", "title": title}),
            );
            assert!(upd.is_error, "{title:?}: {}", upd.content);
            assert_eq!(
                upd.content,
                "tool error: todo_update: `title` must not be empty — titles must be non-empty, say what the step is",
                "update's wording names the rule"
            );
            assert!(
                add.content.contains("`title` must not be empty")
                    && upd.content.contains("`title` must not be empty"),
                "shared shape segment: {} | {}",
                add.content,
                upd.content
            );
        }
        assert!(!todos_path(tmp.path()).exists(), "failed calls never write");
    }

    // ---------- dispatch: list rendering ----------

    #[test]
    fn todo_list_renders_t_status_title_lines() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "read spec"}));
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "implement"}));
        dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t2", "status": "in_progress"}),
        );
        let r = dispatch(&ctx(tmp.path()), "todo_list", &json!({}));
        assert!(!r.is_error, "{}", r.content);
        assert_eq!(
            r.content,
            "t1 [pending] read spec\nt2 [in_progress] implement"
        );
    }

    // ---------- schema registration + plan-mode exclusion ----------

    /// LIVE `tool_schemas()` carries exactly one schema per todo tool with
    /// the status enum spelled out — registered beside `update_ledger` (the
    /// bookkeeping group), and NOT in the plan-mode surface.
    #[test]
    fn todo_schemas_registered_once_beside_update_ledger_and_absent_from_plan() {
        let schemas = tool_schemas();
        for name in ["todo_add", "todo_update", "todo_list"] {
            let n = schemas
                .iter()
                .filter(|s| s.get("name").and_then(Value::as_str) == Some(name))
                .count();
            assert_eq!(n, 1, "{name} must be registered exactly once");
        }
        // Beside update_ledger: it comes immediately before the todo group.
        let names: Vec<&str> = schemas
            .iter()
            .filter_map(|s| s.get("name").and_then(Value::as_str))
            .collect();
        let ledger_at = names
            .iter()
            .position(|n| *n == "update_ledger")
            .expect("update_ledger registered");
        assert_eq!(
            &names[ledger_at + 1..ledger_at + 4],
            &["todo_add", "todo_update", "todo_list"],
            "todo tools registered directly after update_ledger: {names:?}"
        );
        // Plan mode: the filtered surface excludes all three (structural —
        // plan.rs filters by READ_ONLY_TOOLS by name).
        let plan_schemas = crate::plan::tool_schemas();
        let plan_names: Vec<&str> = plan_schemas
            .iter()
            .filter_map(|s| s.get("name").and_then(Value::as_str))
            .collect();
        for name in ["todo_add", "todo_update", "todo_list"] {
            assert!(
                !plan_names.contains(&name),
                "{name} must not be in the plan surface: {plan_names:?}"
            );
        }
        // And the schema's status enum names the three valid spellings.
        let update = schemas
            .iter()
            .find(|s| s.get("name").and_then(Value::as_str) == Some("todo_update"))
            .unwrap();
        let status_enum: Vec<&str> = update["input_schema"]["properties"]["status"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(status_enum, vec!["pending", "in_progress", "done"]);
    }

    // ---------- prompt-section payload ----------

    #[test]
    fn prompt_text_is_empty_when_list_is_empty_or_store_is_corrupt() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(prompt_text(tmp.path()), "", "virgin worktree: no section");
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "step one"}));
        assert_eq!(prompt_text(tmp.path()), "t1 [pending] step one");
        // Corrupt store: no section (the tools name the file when called).
        fs::write(todos_path(tmp.path()), "garbage{").unwrap();
        assert_eq!(prompt_text(tmp.path()), "");
    }

    /// The full injection composition, end to end without a driver run:
    /// real store writes through the tools → load → render → the driver's
    /// prompt builders. Pins the `## Todos` AFTER `## Ledger` ordering and
    /// the no-empty-heading rule for BOTH modes.
    #[test]
    fn injected_section_sits_after_ledger_in_both_modes_and_vanishes_when_empty() {
        let tmp = TempDir::new().unwrap();
        dispatch(&ctx(tmp.path()), "todo_add", &json!({"title": "ship it"}));
        dispatch(
            &ctx(tmp.path()),
            "todo_update",
            &json!({"id": "t1", "status": "in_progress"}),
        );
        let todos_text = prompt_text(tmp.path());
        assert_eq!(todos_text, "t1 [in_progress] ship it");

        let run = crate::driver::build_system_prompt("SPEC", "goal", "LEDGER BODY", &todos_text);
        let ledger_at = run.find("## Ledger").expect("ledger section");
        let todos_at = run.find("## Todos").expect("todos section present");
        assert!(todos_at > ledger_at, "## Todos after ## Ledger: {run}");
        assert!(run.contains("## Todos\n\nt1 [in_progress] ship it"), "{run}");

        let chat = crate::driver::build_chat_system_prompt(
            Some("CHATSPEC"),
            Some("chat goal"),
            "LEDGER BODY",
            &todos_text,
        );
        let ledger_at = chat.find("## Ledger").expect("ledger section");
        let todos_at = chat.find("## Todos").expect("todos section present");
        assert!(todos_at > ledger_at, "## Todos after ## Ledger: {chat}");

        // Empty list: no heading at all, in either mode.
        let empty = crate::driver::build_system_prompt("SPEC", "goal", "LEDGER BODY", "");
        assert!(!empty.contains("## Todos"), "{empty}");
        let chat_empty =
            crate::driver::build_chat_system_prompt(None, None, "LEDGER BODY", "");
        assert!(!chat_empty.contains("## Todos"), "{chat_empty}");
        // Plan mode renders nothing about todos by construction (its builder
        // takes no todos text) — assert the plan prompt stays todos-free
        // even when the store has content.
        let plan = crate::driver::build_plan_system_prompt(
            Some("PSPEC"),
            "goal",
            "LEDGER BODY",
            crate::plan::PlanKind::Plan,
        );
        assert!(!plan.contains("## Todos"), "{plan}");
    }
}

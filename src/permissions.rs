//! T90 — F4 phase 1: `.chug/permissions.json` deny-list (in-process policy).
//!
//! Policy layer one, ahead of the T83 hooks: declarative per-tool deny rules
//! evaluated in-process BEFORE a tool call reaches the PreToolUse hook
//! check, the plan gate, MCP dispatch, the risk gate, or the tool itself.
//! Hooks are programmable but fail open on any spawn/config problem; an
//! in-process deny has no process to spawn and nothing to fall back
//! through — a match denies, period. The two layers compose: declarative
//! deny for the rules that must not depend on a shell, hooks for everything
//! programmable.
//!
//! Shape (deny-only this phase):
//!
//! ```json
//! {"permissions": {"deny": [
//!   {"tool": "bash", "command": "*rm -rf*"},
//!   {"tool": "write_file", "path": "*.pem"},
//!   {"tool": "web_fetch"}
//! ]}}
//! ```
//!
//! A rule is `{"tool": "<glob>"}` plus at most one arg matcher: `command`
//! (glob against the bash command string), `path` (glob against a file
//! tool's `path` argument), or `url` (glob against web_fetch's url). A rule
//! with no matcher denies the whole named tool. Tool globs are plain
//! `*`/`?` globs — `mcp__*` matches MCP tools by their registered
//! `mcp__<name>__<tool>` name like any other.
//!
//! Failure semantics, both directions pinned:
//! - config problems FAIL OPEN (T83 parity): an unreadable/malformed
//!   config or a malformed rule warns once per run on stderr + one
//!   `permission_error` events line each; a malformed RULE is skipped in
//!   place while valid siblings still load;
//! - a rule match FAILS CLOSED: it always denies.
//!
//! Absent or empty config = zero rules and zero per-call cost. The file is
//! per-checkout (`<cwd>/.chug/`, gitignored): a worktree child has its own
//! and does not inherit the parent's — hooks.json parity.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use serde_json::Value;

use crate::events::{Event, EventSink};
use crate::hooks::glob_matches;

/// The config path: `<cwd>/.chug/permissions.json` (gitignored,
/// per-checkout operator config; a worktree child has its own).
pub fn permissions_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("permissions.json")
}

/// One arg matcher: the input key it reads plus the glob it judges with.
#[derive(Debug, Clone, PartialEq)]
enum ArgMatcher {
    /// Glob against the bash tool's `command` string.
    Command(String),
    /// Glob against a file tool's `path` argument.
    Path(String),
    /// Glob against web_fetch's `url`.
    Url(String),
}

impl ArgMatcher {
    /// The input key this matcher reads.
    fn input_key(&self) -> &'static str {
        match self {
            ArgMatcher::Command(_) => "command",
            ArgMatcher::Path(_) => "path",
            ArgMatcher::Url(_) => "url",
        }
    }

    fn glob(&self) -> &str {
        match self {
            ArgMatcher::Command(glob) | ArgMatcher::Path(glob) | ArgMatcher::Url(glob) => glob,
        }
    }

    /// The builtin tools whose input can carry this key — the load-time
    /// compatibility table behind "does this matcher fit the tool glob".
    fn compatible_tools(&self) -> &'static [&'static str] {
        match self {
            // `command` is bash's; no other builtin takes one.
            ArgMatcher::Command(_) => &["bash"],
            // The file tools: `path` is the read/write/edit target and the
            // optional scope of grep/glob/list_dir/tgrep.
            ArgMatcher::Path(_) => &[
                "read_file",
                "write_file",
                "edit_file",
                "grep",
                "glob",
                "list_dir",
                "tgrep",
            ],
            ArgMatcher::Url(_) => &["web_fetch"],
        }
    }
}

/// One deny rule: a tool-name glob plus at most one arg matcher (`None` =
/// deny the whole tool).
#[derive(Debug, Clone, PartialEq)]
struct DenyRule {
    tool_glob: String,
    matcher: Option<ArgMatcher>,
}

impl DenyRule {
    /// The summary the model sees after `[permission denied] ` and the
    /// `permission_denied` events line carries as `rule` — the rule in its
    /// config shape.
    fn summary(&self) -> String {
        match &self.matcher {
            None => format!("deny {}", self.tool_glob),
            Some(matcher) => {
                format!("deny {} {} \"{}\"", self.tool_glob, matcher.input_key(), matcher.glob())
            }
        }
    }
}

/// The loaded permissions config for one run. Zero rules = the driver's
/// per-call gate is one bool check.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    deny: Vec<DenyRule>,
    /// stderr warn-once latch, per run (config problems + skipped rules).
    warn_count: usize,
}

impl Permissions {
    /// Zero rules (the absent/empty-config leg; also the fail-open landing
    /// spot).
    pub fn empty() -> Self {
        Permissions::default()
    }

    /// Load `.chug/permissions.json` from the run cwd. No search chain, no
    /// CLI flag this phase. Every config problem fails open with a warn
    /// (once per run) + one events.jsonl error line; absent/empty config
    /// returns zero rules with zero events.
    pub fn load(cwd: &Path, sink: &mut dyn EventSink) -> Permissions {
        let path = permissions_path(cwd);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            // Absent file: the normal zero-cost leg.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Permissions::empty(),
            Err(e) => {
                return Self::config_error(
                    &format!("permissions config unreadable {}: {e}", path.display()),
                    sink,
                );
            }
        };
        // Empty file: zero rules, zero cost, no warn.
        if text.trim().is_empty() {
            return Permissions::empty();
        }
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                return Self::config_error(
                    &format!("permissions config malformed {}: {e}", path.display()),
                    sink,
                );
            }
        };
        match Self::parse(&value, sink) {
            Ok((deny, skipped)) => Permissions {
                deny,
                // The skipped-rule warn warned at most once (its own latch).
                warn_count: usize::from(skipped > 0),
            },
            Err(e) => Self::config_error(
                &format!("permissions config invalid {}: {e}", path.display()),
                sink,
            ),
        }
    }

    /// The config-error landing spot: warn once on stderr + one error line,
    /// then run with zero rules (fail-open, T83 parity).
    fn config_error(detail: &str, sink: &mut dyn EventSink) -> Permissions {
        eprintln!("chug: warning: {detail}; running with no deny rules");
        sink.emit(Event::PermissionError {
            detail: detail.to_string(),
        });
        let mut permissions = Permissions::empty();
        permissions.warn_count = 1;
        permissions
    }

    /// Parse the `{"permissions": {"deny": [...]}}` shape. An absent
    /// `permissions` key or absent/empty `deny` list = zero rules. Shape
    /// errors (wrong top-level type, non-array deny) are config errors
    /// (fail-open upstream), never panics; a malformed RULE is skipped in
    /// place — valid siblings still load — with one error line per skip.
    /// Returns the rules plus how many rules were skipped.
    fn parse(
        value: &Value,
        sink: &mut dyn EventSink,
    ) -> anyhow::Result<(Vec<DenyRule>, usize)> {
        let obj = value
            .as_object()
            .ok_or_else(|| anyhow!("expected a JSON object"))?;
        let Some(perms) = obj.get("permissions") else {
            return Ok((Vec::new(), 0));
        };
        let perms = perms
            .as_object()
            .ok_or_else(|| anyhow!("\"permissions\" must be an object"))?;
        let Some(deny) = perms.get("deny") else {
            return Ok((Vec::new(), 0));
        };
        let deny = deny
            .as_array()
            .ok_or_else(|| anyhow!("\"permissions.deny\" must be an array"))?;
        let mut out = Vec::with_capacity(deny.len());
        let mut skipped = 0usize;
        let mut warned = false; // the stderr warn-once latch for skipped rules
        for (i, rule) in deny.iter().enumerate() {
            match parse_rule(rule) {
                Ok(rule) => out.push(rule),
                Err(reason) => {
                    skipped += 1;
                    let detail = format!("permissions.deny[{i}] skipped: {reason}");
                    if !warned {
                        warned = true;
                        eprintln!(
                            "chug: warning: permissions: {detail} (further skipped rules: events.jsonl only)"
                        );
                    }
                    sink.emit(Event::PermissionError { detail });
                }
            }
        }
        Ok((out, skipped))
    }

    /// Zero rules = the driver skips the per-call check entirely.
    pub fn is_empty(&self) -> bool {
        self.deny.is_empty()
    }

    /// The deny check, run per tool call BEFORE the PreToolUse hook check —
    /// a denied call fires no hooks, never reaches the plan gate / MCP /
    /// risk gate, and never executes. `Some(message)` = denied: the tool
    /// does not run and the model receives the tool error (the T83 veto
    /// shape; the loop continues); `None` = allowed.
    pub fn check(&self, tool: &str, input: &Value, sink: &mut dyn EventSink) -> Option<String> {
        let summary = self.matching_rule(tool, input)?;
        sink.emit(Event::PermissionDenied {
            tool: tool.to_string(),
            rule: summary.clone(),
        });
        Some(format!("[permission denied] {summary}"))
    }

    /// First matching rule wins, in config order.
    fn matching_rule(&self, tool: &str, input: &Value) -> Option<String> {
        for rule in &self.deny {
            if !glob_matches(&rule.tool_glob, tool) {
                continue;
            }
            let matched = match &rule.matcher {
                // Whole-tool rule: the tool name matching is the whole
                // judgment.
                None => true,
                // Arg rule: the matcher judges the input arg. A missing or
                // non-string arg (e.g. a bash call without a string
                // `command`) means the rule does NOT match — the matcher had
                // nothing to judge, so fail toward execution.
                Some(matcher) => input
                    .get(matcher.input_key())
                    .and_then(Value::as_str)
                    .is_some_and(|value| glob_matches(matcher.glob(), value)),
            };
            if matched {
                return Some(rule.summary());
            }
        }
        None
    }

    /// Test-only: how many stderr warns fired (asserts the once latch).
    #[cfg(test)]
    pub(crate) fn warn_count(&self) -> usize {
        self.warn_count
    }
}

/// Parse one deny rule: `{"tool": "<glob>"}` plus at most one arg matcher
/// (`command`/`path`/`url`, string globs). A non-object rule, a missing or
/// non-string `tool`, a non-string matcher value, two matchers, an unknown
/// key, or a matcher that cannot fit the tool glob are all malformed — the
/// caller skips the rule with a warning while valid siblings still load.
fn parse_rule(rule: &Value) -> anyhow::Result<DenyRule> {
    let obj = rule
        .as_object()
        .ok_or_else(|| anyhow!("must be an object"))?;
    let tool_glob = obj
        .get("tool")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing or non-string `tool` glob"))?;
    let mut matcher: Option<ArgMatcher> = None;
    for key in ["command", "path", "url"] {
        let Some(value) = obj.get(key) else {
            continue;
        };
        let glob = value
            .as_str()
            .ok_or_else(|| anyhow!("`{key}` must be a string glob"))?;
        if matcher.is_some() {
            return Err(anyhow!("at most one arg matcher (command/path/url), got two"));
        }
        matcher = Some(match key {
            "command" => ArgMatcher::Command(glob.to_string()),
            "path" => ArgMatcher::Path(glob.to_string()),
            _ => ArgMatcher::Url(glob.to_string()),
        });
    }
    for key in obj.keys() {
        if !matches!(key.as_str(), "tool" | "command" | "path" | "url") {
            return Err(anyhow!(
                "unknown key `{key}` (expected `tool` plus at most one of command/path/url)"
            ));
        }
    }
    if let Some(m) = &matcher
        && !matcher_fits(tool_glob, m)
    {
        return Err(anyhow!(
            "`{}` matcher does not fit tool glob `{tool_glob}`",
            m.input_key()
        ));
    }
    Ok(DenyRule {
        tool_glob: tool_glob.to_string(),
        matcher,
    })
}

/// Load-time compatibility: can this tool glob ever name a tool the matcher
/// could judge? A matcher on tools that never carry the argument (e.g.
/// `command` on `read_file`) could never fire — an operator mistake we
/// surface as a malformed rule (skipped + warned), never a dead deny.
///
/// The `mcp__` prefix fits ANY matcher: a glob starting with `mcp__` — even
/// a server-scoped one like `mcp__fs__*` — can only ever name MCP tools,
/// which carry arbitrary args. Globs WITHOUT the prefix (e.g. `mcp*`, `*`)
/// still must be able to match an actual `mcp__` name (the canary string)
/// to ride this rule; everything else is judged by the builtin table.
fn matcher_fits(tool_glob: &str, matcher: &ArgMatcher) -> bool {
    // MCP tools (`mcp__<server>__<tool>`) carry arbitrary args, so a glob
    // that can match an mcp__ name fits any matcher — and a glob with the
    // literal `mcp__` prefix names only MCP tools, so it fits even though
    // it can never match the `mcp__server__…` canary string.
    if tool_glob.starts_with("mcp__") || glob_matches(tool_glob, "mcp__server__tool") {
        return true;
    }
    matcher
        .compatible_tools()
        .iter()
        .any(|tool| glob_matches(tool_glob, tool))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    /// Records emitted events so tests can assert on them.
    #[derive(Default)]
    struct RecordingSink(Vec<Event>);
    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    impl RecordingSink {
        fn denied(&self) -> Vec<(&str, &str)> {
            self.0
                .iter()
                .filter_map(|e| match e {
                    Event::PermissionDenied { tool, rule } => Some((tool.as_str(), rule.as_str())),
                    _ => None,
                })
                .collect()
        }
        fn errors(&self) -> Vec<String> {
            self.0
                .iter()
                .filter_map(|e| match e {
                    Event::PermissionError { detail } => Some(detail.clone()),
                    _ => None,
                })
                .collect()
        }
    }

    fn write_config(cwd: &Path, value: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(permissions_path(cwd), value.to_string()).unwrap();
    }

    fn load(cwd: &Path, sink: &mut RecordingSink) -> Permissions {
        Permissions::load(cwd, sink)
    }

    /// The deny verdict for one scripted tool call against a loaded config.
    fn verdict(permissions: &Permissions, tool: &str, input: Value, sink: &mut RecordingSink) -> Option<String> {
        permissions.check(tool, &input, sink)
    }

    // ---------- config load legs ----------

    #[test]
    fn absent_config_is_zero_rules_and_zero_cost() {
        let tmp = tempfile::tempdir().unwrap();
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty());
        assert!(sink.0.is_empty(), "no events for an absent config");
        assert_eq!(permissions.warn_count(), 0);
    }

    #[test]
    fn empty_config_file_is_zero_rules_without_a_warn() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(permissions_path(tmp.path()), "").unwrap();
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty());
        assert!(sink.0.is_empty(), "empty file: zero cost, no warn");
        assert_eq!(permissions.warn_count(), 0);
    }

    /// Mutant killers (T83 parity): "malformed config aborts the run" and
    /// "malformed config silently ignored with no telemetry" both die —
    /// fail-open with exactly one error line and zero rules.
    #[test]
    fn malformed_config_fails_open_with_one_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(permissions_path(tmp.path()), "{ not json !!!").unwrap();
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty(), "malformed config runs with zero rules");
        let errors = sink.errors();
        assert_eq!(errors.len(), 1, "exactly one error line: {errors:?}");
        assert!(errors[0].contains("malformed"), "{errors:?}");
        assert_eq!(permissions.warn_count(), 1, "stderr warn fired once");
    }

    #[test]
    fn unreadable_config_fails_open_with_one_error_event() {
        let tmp = tempfile::tempdir().unwrap();
        // A DIRECTORY where the file should be: read fails, not NotFound.
        fs::create_dir_all(permissions_path(tmp.path())).unwrap();
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty());
        assert_eq!(sink.errors().len(), 1);
        assert!(sink.errors()[0].contains("unreadable"), "{:?}", sink.errors());
    }

    /// A top-level array is the wrong top-level type: config error,
    /// fail-open to zero rules.
    #[test]
    fn top_level_array_is_a_config_error_not_rules() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), json!([{"tool": "bash"}]));
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty(), "wrong top-level type = zero rules");
        assert_eq!(sink.errors().len(), 1);
        assert_eq!(permissions.warn_count(), 1);
    }

    #[test]
    fn absent_permissions_key_and_empty_deny_are_zero_rules() {
        let tmp = tempfile::tempdir().unwrap();
        for text in ["{}", r#"{"permissions": {}}"#, r#"{"permissions": {"deny": []}}"#] {
            let dir = tmp.path().join(".chug");
            fs::create_dir_all(&dir).unwrap();
            fs::write(permissions_path(tmp.path()), text).unwrap();
            let mut sink = RecordingSink::default();
            let permissions = load(tmp.path(), &mut sink);
            assert!(permissions.is_empty(), "{text}");
            assert!(sink.0.is_empty(), "{text}");
        }
    }

    /// Shape errors are config errors (fail-open), never panics.
    #[test]
    fn wrong_shaped_config_is_an_error_not_a_panic() {
        let tmp = tempfile::tempdir().unwrap();
        for text in [
            r#"{"permissions": "no"}"#,
            r#"{"permissions": {"deny": "no"}}"#,
            r#"{"permissions": {"deny": [42]}}"#,
        ] {
            let dir = tmp.path().join(".chug");
            fs::create_dir_all(&dir).unwrap();
            fs::write(permissions_path(tmp.path()), text).unwrap();
            let mut sink = RecordingSink::default();
            let permissions = load(tmp.path(), &mut sink);
            assert!(permissions.is_empty(), "{text}");
            assert_eq!(sink.errors().len(), 1, "{text}");
        }
    }

    // ---------- rule shape legs ----------

    /// The core malformed-rule semantics: a bad rule is SKIPPED in place
    /// (valid siblings still deny) and each skip surfaces one
    /// `permission_error` line while the stderr warn latches at ONE.
    #[test]
    fn malformed_rules_are_skipped_valid_siblings_still_deny() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [
                "not an object",
                {"tool": "read_file", "command": "*evil*"},
                {"tool": "write_file", "path": "*.pem"}
            ]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        // The two malformed legs were skipped; the valid sibling loaded.
        let errors = sink.errors();
        assert_eq!(errors.len(), 2, "{errors:?}");
        assert!(errors[0].contains("must be an object"), "{errors:?}");
        assert!(
            errors[1].contains("does not fit"),
            "command on read_file is tool-incompatible: {errors:?}"
        );
        assert_eq!(permissions.warn_count(), 1, "stderr warn once per run");
        // The valid sibling still denies.
        assert!(verdict(&permissions, "write_file", json!({"path": "a.pem", "content": "x"}), &mut sink).is_some());
        assert!(verdict(&permissions, "write_file", json!({"path": "a.txt", "content": "x"}), &mut sink).is_none());
        // Run continues: no config error ever aborts.
        assert!(!permissions.is_empty());
    }

    #[test]
    fn every_malformed_rule_leg_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [
                {"tool": "bash", "mystery": "*"},          // unknown matcher key
                {"tool": "bash", "command": "*x*", "path": "*y*"}, // two matchers
                {"tool": "bash", "command": 5},            // non-string matcher value
                {"command": "*x*"},                        // missing tool
                {"tool": 7},                               // non-string tool
                {"tool": "bash", "path": "*y*"}            // path on a non-file tool
            ]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty(), "every leg is malformed: zero rules");
        let errors = sink.errors();
        assert_eq!(errors.len(), 6, "{errors:?}");
        assert!(errors[0].contains("unknown key `mystery`"), "{errors:?}");
        assert!(errors[1].contains("at most one arg matcher"), "{errors:?}");
        assert!(errors[2].contains("`command` must be a string"), "{errors:?}");
        assert!(errors[3].contains("missing or non-string `tool`"), "{errors:?}");
        assert!(errors[4].contains("missing or non-string `tool`"), "{errors:?}");
        assert!(errors[5].contains("does not fit"), "{errors:?}");
        assert_eq!(permissions.warn_count(), 1, "warn-once across all six skips");
    }

    /// Valid rules load in config order (first match wins).
    #[test]
    fn valid_rules_load_in_config_order_and_first_match_wins() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [
                {"tool": "bash", "command": "*specific*"},
                {"tool": "bash"}
            ]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert_eq!(permissions.check("bash", &json!({"command": "specific thing"}), &mut sink).unwrap(),
            "[permission denied] deny bash command \"*specific*\"");
        assert_eq!(permissions.check("bash", &json!({"command": "anything else"}), &mut sink).unwrap(),
            "[permission denied] deny bash");
        // Non-bash tools are untouched by both rules.
        assert!(permissions.check("read_file", &json!({"path": "a"}), &mut sink).is_none());
    }

    /// `mcp__*` tool globs work like any other glob — whole-tool and with
    /// an arg matcher (MCP tools carry arbitrary args, so any matcher fits).
    #[test]
    fn mcp_tool_globs_deny_like_any_other() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [
                {"tool": "mcp__github__create_issue"},
                {"tool": "mcp__*", "path": "*.env"}
            ]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.check("mcp__github__create_issue", &json!({}), &mut sink).is_some());
        assert!(permissions.check("mcp__github__list_issues", &json!({}), &mut sink).is_none());
        assert!(permissions.check("mcp__fs__read", &json!({"path": "prod.env"}), &mut sink).is_some());
        assert!(permissions.check("mcp__fs__read", &json!({"path": "README.md"}), &mut sink).is_none());
    }

    /// A server-scoped `mcp__` glob with an arg matcher loads as a VALID
    /// rule and denies: the `mcp__` prefix itself names only MCP tools,
    /// which carry arbitrary args, so the prefix alone makes any matcher
    /// fit — the canary string's literal `mcp__server__…` can never match a
    /// `mcp__fs__*` glob, so without the prefix leg this rule was skipped
    /// as matcher-that-cannot-fit (both calls allowed, one
    /// `permission_error` line) — the canary was wrong, not the glob.
    #[test]
    fn mcp_server_prefixed_glob_with_arg_matcher_loads_and_denies() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [{"tool": "mcp__fs__*", "path": "*.env"}]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(
            sink.errors().is_empty(),
            "the rule loads VALID, never skipped: {:#?}",
            sink.errors()
        );
        assert_eq!(
            verdict(&permissions, "mcp__fs__read", json!({"path": "prod.env"}), &mut sink).unwrap(),
            "[permission denied] deny mcp__fs__* path \"*.env\""
        );
        assert!(verdict(&permissions, "mcp__fs__read", json!({"path": "ok.txt"}), &mut sink).is_none());
    }

    // ---------- matcher legs (the deny path) ----------

    /// Tool-glob legs through the real matcher: `*`, `?`, exact, prefix —
    /// plus the command/path/url arg-glob legs.
    #[test]
    fn glob_matcher_legs() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [
                {"tool": "bas?", "command": "*rm -rf*"},
                {"tool": "edit_*", "path": "*.pem"},
                {"tool": "*", "url": "*internal.example.com*"}
            ]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        // `bas?` matches bash; the command glob judges the command string.
        assert!(verdict(&permissions, "bash", json!({"command": "rm -rf /tmp/x"}), &mut sink).is_some());
        assert!(verdict(&permissions, "bash", json!({"command": "ls"}), &mut sink).is_none());
        // `edit_*` matches edit_file; path glob judges the path argument.
        assert!(verdict(&permissions, "edit_file", json!({"path": "server.pem", "old": "a", "new": "b"}), &mut sink).is_some());
        assert!(verdict(&permissions, "edit_file", json!({"path": "server.txt", "old": "a", "new": "b"}), &mut sink).is_none());
        // `*` matches everything, url glob judges the url.
        assert!(verdict(&permissions, "web_fetch", json!({"url": "https://internal.example.com/x"}), &mut sink).is_some());
        assert!(verdict(&permissions, "web_fetch", json!({"url": "https://example.com/x"}), &mut sink).is_none());
    }

    /// Whole-tool deny: no arg matcher, the tool name alone denies.
    #[test]
    fn whole_tool_deny() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(tmp.path(), json!({"permissions": {"deny": [{"tool": "web_fetch"}]}}));
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert_eq!(
            verdict(&permissions, "web_fetch", json!({"url": "https://example.com"}), &mut sink).unwrap(),
            "[permission denied] deny web_fetch"
        );
        // Other tools untouched.
        assert!(verdict(&permissions, "bash", json!({"command": "ls"}), &mut sink).is_none());
    }

    /// The non-vacuousness leg: a command-glob rule denies the matching
    /// command AND a non-matching command is untouched (executes normally).
    #[test]
    fn command_glob_denies_matching_but_not_other_commands() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [{"tool": "bash", "command": "*rm -rf*"}]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(verdict(&permissions, "bash", json!({"command": "rm -rf build"}), &mut sink).is_some());
        assert!(verdict(&permissions, "bash", json!({"command": "cargo build"}), &mut sink).is_none());
    }

    /// Path-glob deny on the file tools; a missing or non-string `path`
    /// under a path-rule does NOT match (fail toward execution — the
    /// matcher had nothing to judge).
    #[test]
    fn path_rule_needs_a_string_path_arg() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [{"tool": "write_file", "path": "*.pem"}]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(verdict(&permissions, "write_file", json!({"path": "key.pem", "content": "x"}), &mut sink).is_some());
        // Missing path / non-string path: rule does not match.
        assert!(verdict(&permissions, "write_file", json!({"content": "x"}), &mut sink).is_none());
        assert!(verdict(&permissions, "write_file", json!({"path": 7, "content": "x"}), &mut sink).is_none());
        // A read_file call is untouched by a write_file rule.
        assert!(verdict(&permissions, "read_file", json!({"path": "key.pem"}), &mut sink).is_none());
    }

    /// Deny telemetry shape: one `permission_denied` event per deny with
    /// the tool and the rule summary; none on the allow path.
    #[test]
    fn deny_emits_one_event_per_deny_none_on_allow() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [{"tool": "bash", "command": "*forbidden*"}]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(verdict(&permissions, "bash", json!({"command": "echo forbidden"}), &mut sink).is_some());
        assert!(verdict(&permissions, "bash", json!({"command": "echo fine"}), &mut sink).is_none());
        assert!(verdict(&permissions, "bash", json!({"command": "echo forbidden again"}), &mut sink).is_some());
        assert_eq!(sink.denied().len(), 2, "one line per deny, none on allow");
        assert_eq!(sink.denied()[0], ("bash", "deny bash command \"*forbidden*\""));
        assert_eq!(sink.errors().len(), 0, "no config problems on the clean path");
    }

    /// The `permission_error` events line fires per config problem and the
    /// skipped-rule detail names the rule index.
    #[test]
    fn skipped_rule_error_lines_name_the_index() {
        let tmp = tempfile::tempdir().unwrap();
        write_config(
            tmp.path(),
            json!({"permissions": {"deny": [{"tool": "read_file", "url": "*"}]}}),
        );
        let mut sink = RecordingSink::default();
        let permissions = load(tmp.path(), &mut sink);
        assert!(permissions.is_empty());
        let errors = sink.errors();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].starts_with("permissions.deny[0] skipped:"), "{errors:?}");
        assert!(errors[0].contains("url"), "{errors:?}");
    }
}

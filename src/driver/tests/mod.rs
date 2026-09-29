// T104: the body of driver.rs's `#[cfg(test)] pub(crate) mod tests` (4,259
// lines at the split) lives in this file+directory module pair; driver.rs
// keeps the one-line declaration plus its harness comment. THIS FILE HOLDS
// THE SHARED HARNESS ONLY (T84's one rule): every item below is used by two
// or more family files, or is the `pub(crate)` seam src/trim.rs's tests
// reuse (ctx_for/knobs_with/tool_use_response/RecordingSink — see driver.rs
// beside the declaration). Helpers with callers in a single family moved
// with that family. Moved bytes are byte-identical to the pre-split module
// body and keep their in-module 4-space indent (a move, not a rewrite).
    use super::*;
    use crate::api::{KnownBlock, ScriptedLlm};
    use serde_json::json;

    // One `mod` line per family file; each family's tests live in exactly one
    // file.
    mod budget;
    mod crash;
    mod events;
    mod goal;
    mod hooks_policy;
    mod image;
    mod lock;
    mod mcp;
    mod observability;
    mod permissions_policy;
    mod plan;
    mod preview;
    mod resume;
    mod spec_check_gate;
    mod steering;
    mod stuck;
    mod truncated;
    mod unit;

    #[derive(Default)]
    pub(crate) struct RecordingSink(Vec<Event>);

    impl EventSink for RecordingSink {
        fn emit(&mut self, e: Event) {
            self.0.push(e);
        }
    }

    fn write_spec(tmp: &tempfile::TempDir) -> PathBuf {
        let spec = tmp.path().join("s.md");
        std::fs::write(&spec, "spec text\ncheck: true\n").unwrap();
        spec
    }

    /// Read the events log of a run in `tmp`, asserting every line is JSON.
    fn events_jsonl(tmp: &tempfile::TempDir) -> Vec<Value> {
        let path = tmp.path().join(".chug").join("events.jsonl");
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("events.jsonl readable: {e}"))
            .lines()
            .map(|l| serde_json::from_str(l).expect("every events.jsonl line parses as JSON"))
            .collect()
    }

    // ---------- MCP integration (fake echo server, no network) ----------

    /// LLM double that records the tools array it was offered, like
    /// ScriptedLlm but for the tool schemas (which the driver composes).
    struct ToolRecordingLlm {
        responses: std::collections::VecDeque<Value>,
        recorded_tools: Vec<Vec<Value>>,
    }

    impl ToolRecordingLlm {
        fn new(responses: Vec<Value>) -> Self {
            ToolRecordingLlm {
                responses: responses.into(),
                recorded_tools: Vec::new(),
            }
        }
    }

    impl Llm for ToolRecordingLlm {
        fn complete(
            &mut self,
            _system: &str,
            _messages: &[Message],
            tools: &[Value],
            _obs: &crate::api::ObsCtx<'_>,
        ) -> anyhow::Result<crate::api::Response> {
            self.recorded_tools.push(tools.to_vec());
            self.responses
                .pop_front()
                .map(|body| crate::api::Response { body })
                .ok_or_else(|| anyhow::anyhow!("no scripted response left"))
        }

        fn set_model(&mut self, _model: &str) {}

        fn model(&self) -> &str {
            "tool-recording-model"
        }
    }

    /// Fake MCP echo server (same script family as mcp.rs's tests) configured
    /// via mcp.json in the cwd.
    fn write_echo_server(dir: &Path) {
        let py = dir.join("fake_srv.py");
        std::fs::write(
            &py,
            r#"
import sys, json
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "Echo the arguments back", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}}]}})
    elif m == "tools/call":
        send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(req["params"]["arguments"])}], "isError": False}})
"#,
        )
        .unwrap();
        let cfg = json!({
            "mcpServers": {
                "fake": {"command": "python3", "args": [py.to_string_lossy()]}
            }
        });
        std::fs::write(dir.join("mcp.json"), cfg.to_string()).unwrap();
    }

    pub(crate) fn tool_use_response(name: &str, input: Value) -> Value {
        json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "tool_use", "id": "tu_1", "name": name, "input": input}],
        })
    }

    fn text_only_response(text: &str) -> Value {
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "content": [{"type": "text", "text": text}],
        })
    }

    pub(crate) fn ctx_for<'a>(
        tmp: &'a tempfile::TempDir,
        mode: Mode,
        controls: &'a Controls,
        update_rx: &'a Receiver<SlashUpdate>,
        trace: Option<&'a str>,
        obs: &'a observ::Sink,
    ) -> LoopCtx<'a> {
        LoopCtx {
            cwd: tmp.path(),
            mode,
            controls,
            updates: update_rx,
            bash_timeout: Duration::from_secs(1),
            trace,
            obs,
            plan_out: None,
        }
    }

    pub(crate) fn knobs_with(max_iters: u32) -> TurnKnobs {
        TurnKnobs {
            spec_path: None,
            goal: None,
            check_cmd: None,
            max_iters,
            max_minutes: 120,
            max_tokens: 0,
        }
    }

    fn tool_result_text(messages: &[Message]) -> Option<(String, bool)> {
        messages.iter().rev().find_map(|m| match &m.content[0] {
            ContentBlock::Known(KnownBlock::ToolResult { content, is_error, .. }) => {
                Some((content.as_str().unwrap_or_default().to_string(), *is_error))
            }
            _ => None,
        })
    }

    /// Minimal judge double: always returns the same canned verdict.
    struct CannedJudge(&'static str, f64);

    impl crate::riskgate::Judge for CannedJudge {
        fn judge(&mut self, _command: &str) -> Result<crate::riskgate::Verdict, String> {
            Ok(crate::riskgate::Verdict {
                choice: self.0.to_string(),
                p_destructive: self.1,
            })
        }
    }

    /// Like `ctx_for` but with Mode::Plan and plan_out — the `--out` path
    /// submit_plan writes.
    fn ctx_for_plan<'a>(
        tmp: &'a tempfile::TempDir,
        plan_out: Option<&'a Path>,
        controls: &'a Controls,
        urx: &'a Receiver<SlashUpdate>,
    ) -> LoopCtx<'a> {
        LoopCtx {
            cwd: tmp.path(),
            mode: Mode::Plan,
            controls,
            updates: urx,
            bash_timeout: Duration::from_secs(1),
            trace: None,
            obs: &observ::Sink::Noop,
            plan_out,
        }
    }

    /// Write a permissions.json deny config into `cwd/.chug/` (T90; shared
    /// since T139 — the spec-check gate family denies bash the same way).
    fn write_permissions_json(cwd: &Path, deny: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            permissions::permissions_path(cwd),
            json!({"permissions": {"deny": deny}}).to_string(),
        )
        .unwrap();
    }

    /// Write a hooks.json configuring one PreToolUse entry and one
    /// PostToolUse entry into `cwd/.chug/`.
    fn write_hooks_json(cwd: &Path, pre: Value, post: Value) {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            hooks::hooks_path(cwd),
            json!({"hooks": {"PreToolUse": pre, "PostToolUse": post}}).to_string(),
        )
        .unwrap();
    }

    fn hook_entry(glob: &str, command: &str) -> Value {
        json!({"match": glob, "command": command})
    }


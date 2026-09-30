// T104 family: mcp — MCP schema merge + tool_use routing through the loop (echo server). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
#[test]
fn mcp_schemas_merged_and_mcp_tool_use_routed_to_registry() {
    // T158: the whole scripted attempt (fresh tempdir -> drive -> asserts) is
    // spawn-invalidation retried; red legs embed the observed evidence so a
    // spawn failure under pressure classifies as invalidation.
    drive_attempt_with_spawn_retry(|| {
    let tmp = tempfile::tempdir().unwrap();
    write_echo_server(tmp.path());
    let mut mcp = McpRegistry::new(tmp.path(), false, None).expect("registry with fake server");
    // T138: spawn is deferred — nothing runs before drive_loop loads
    // permissions and starts the allowed servers, so the pre-turn tool
    // list is empty. The post-turn assertions below pin the merge.
    assert!(mcp.tool_schemas().is_empty());

    let mut client = ToolRecordingLlm::new(vec![
        json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "tool_use", "id": "tu_1", "name": "mcp__fake__echo", "input": {"text": "hello mcp"}}]
        }),
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "done"}]
        }),
    ]);
    let mut messages = vec![Message::user(vec![ContentBlock::text_block(
        "call the echo tool".to_string(),
    )])];
    let mut sink = RecordingSink::default();
    let reason = run_turn(
        tmp.path(),
        &mut client,
        &mut None,
        &mut messages,
        &Controls::detached(),
        &mpsc::channel().1,
        &mut knobs_with(5),
        Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        &mut mcp,
        None,
        &observ::Sink::Noop,
        &mut sink,
    )
    .unwrap();
    assert_eq!(reason, TurnEndReason::Completed);

    // The tools array the model saw merges MCP schemas with the built-ins.
    let offered = &client.recorded_tools[0];
    // T158 evidence: the server's own fail-soft start log rides the message,
    // so a `spawning mcp server …` / `failed to start: …` under pressure
    // classifies as spawn invalidation and the attempt retries.
    let mcp_log = mcp_server_log(tmp.path(), "fake");
    assert!(
        offered.iter().any(|t| t["name"] == "mcp__fake__echo"),
        "mcp schema missing: {offered:?}\n--- mcp-fake.log ---\n{mcp_log}"
    );
    assert!(offered.iter().any(|t| t["name"] == "bash"));

    // The mcp__-prefixed tool_use was routed to the registry and the
    // model saw the echoed content.
    let (content, is_error) = tool_result_text(&messages).expect("tool result in transcript");
    assert!(!is_error, "{content}");
    assert!(content.contains(r#""text": "hello mcp""#), "{content}");
    });
}

#[test]
fn empty_registry_is_a_noop_on_the_tools_array() {
    let tmp = tempfile::tempdir().unwrap();
    // mcp_off: guaranteed-empty registry even if the developer's machine
    // has ~/.config/chug/mcp.json.
    let mut mcp = McpRegistry::new(tmp.path(), true, None).unwrap();
    let mut client = ToolRecordingLlm::new(vec![json!({
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 1},
        "content": [{"type": "text", "text": "done"}]
    })]);
    let mut messages = vec![Message::user(vec![ContentBlock::text_block("hi")])];
    let mut sink = RecordingSink::default();
    run_turn(
        tmp.path(),
        &mut client,
        &mut None,
        &mut messages,
        &Controls::detached(),
        &mpsc::channel().1,
        &mut knobs_with(5),
        Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        &mut mcp,
        None,
        &observ::Sink::Noop,
        &mut sink,
    )
    .unwrap();

    let offered = &client.recorded_tools[0];
    let names: Vec<&str> = offered.iter().filter_map(|t| t["name"].as_str()).collect();
    let baseline: Vec<String> = tools::tool_schemas()
        .into_iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    // Byte-identical tool list: nothing added, nothing removed.
    assert_eq!(names, baseline);
    assert!(names.iter().all(|n| !n.starts_with("mcp__")));
}

/// Fake MCP server advertising `resources` and serving a two-entry
/// resources/list + one text resources/read (the T169 e2e stub; same
/// script family as mcp.rs's tests).
fn write_resources_server(dir: &Path) {
    let py = dir.join("fake_res_srv.py");
    std::fs::write(
        &py,
        r#"
import sys, json
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
RESOURCES = [
    {"uri": "mem://greeting", "name": "greeting", "mimeType": "text/plain", "description": "a greeting"},
    {"uri": "mem://bytes", "name": "bytes", "mimeType": "application/octet-stream", "description": "raw bytes"}
]
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}, "resources": {}}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": []}})
    elif m == "resources/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"resources": RESOURCES}})
    elif m == "resources/read":
        uri = req["params"].get("uri")
        send({"jsonrpc": "2.0", "id": i, "result": {"contents": [{"uri": uri, "mimeType": "text/plain", "text": "hello from the stub"}]}})
    else:
        send({"jsonrpc": "2.0", "id": i, "error": {"code": -32601, "message": "method not found: " + m}})
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

/// T169 e2e: the model's `mcp_resource` call routes through the DRIVER
/// dispatch branch (not the mcp__ branch, not tools::inner) to the
/// registry legs — the catalog comes back as the model-facing lines, and
/// the schema rides the live tools array.
#[test]
fn mcp_resource_tool_use_routed_through_driver_to_registry() {
    let tmp = tempfile::tempdir().unwrap();
    write_resources_server(tmp.path());
    let mut mcp = McpRegistry::new(tmp.path(), false, None).expect("registry with fake server");

    let mut client = ToolRecordingLlm::new(vec![
        json!({
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "tool_use", "id": "tu_1", "name": "mcp_resource", "input": {"action": "list"}}]
        }),
        json!({
            "stop_reason": "end_turn",
            "usage": {"input_tokens": 1, "output_tokens": 1},
            "content": [{"type": "text", "text": "done"}]
        }),
    ]);
    let mut messages = vec![Message::user(vec![ContentBlock::text_block(
        "list the mcp resources".to_string(),
    )])];
    let mut sink = RecordingSink::default();
    let reason = run_turn(
        tmp.path(),
        &mut client,
        &mut None,
        &mut messages,
        &Controls::detached(),
        &mpsc::channel().1,
        &mut knobs_with(5),
        Duration::from_secs(tools::BASH_TIMEOUT_SECS),
        &mut mcp,
        None,
        &observ::Sink::Noop,
        &mut sink,
    )
    .unwrap();
    assert_eq!(reason, TurnEndReason::Completed);

    // The builtin schema rode the live tools array beside the mcp__ ones.
    let offered = &client.recorded_tools[0];
    assert!(
        offered.iter().any(|t| t["name"] == "mcp_resource"),
        "mcp_resource schema missing: {offered:?}"
    );

    // The tool_use was routed through the driver branch: the model saw the
    // stub's catalog, not an unknown-tool error.
    let (content, is_error) = tool_result_text(&messages).expect("tool result in transcript");
    assert!(!is_error, "{content}");
    assert!(
        content.contains("fake mem://greeting — a greeting (text/plain)"),
        "{content}"
    );
    assert!(
        content.contains("fake mem://bytes — raw bytes (application/octet-stream)"),
        "{content}"
    );
}

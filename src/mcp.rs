use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::tools::{ToolResult, kill_process_group};

const PROTOCOL_VERSION: &str = "2025-06-18";
const INIT_TIMEOUT: Duration = Duration::from_secs(10);
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);
/// Grace for joining the reader thread on Drop. A reader that still has not
/// seen EOF (an escaped process holding the stdout pipe) is leaked rather
/// than allowed to block Drop forever — same discipline as run_shell.
const READER_JOIN_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug)]
pub struct McpServer {
    name: String,
    child: Option<Child>,
    /// Shared with the reader thread so it can answer server→client requests
    /// (it must never leave the server waiting on a reply that never comes).
    stdin: Arc<Mutex<ChildStdin>>,
    reader_handle: Option<JoinHandle<()>>,
    pending: Arc<Mutex<HashMap<u64, Sender<Value>>>>,
    next_id: Arc<Mutex<u64>>,
    tools: Vec<McpTool>,
    alive: Arc<Mutex<bool>>,
}

/// Lock a mutex without panicking on poisoning: a panic in the reader thread
/// must not take the whole driver down through an unrelated lock site.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Append one line to the per-server mcp log. Best effort: logging must never
/// break the protocol path.
fn log_line(path: &Path, line: &str) {
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn server_down(name: &str) -> ToolResult {
    ToolResult {
        content: format!("mcp server {name} is down"),
        is_error: true,
    }
}

impl McpServer {
    pub fn is_alive(&self) -> bool {
        *lock(&self.alive)
    }

    /// Call an MCP tool (60s budget). Errors are returned as `is_error` tool
    /// results, never as driver-level failures — an MCP server is a peer the
    /// loop must survive.
    pub fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<ToolResult> {
        self.call_with_timeout(tool_name, arguments, CALL_TIMEOUT)
    }

    fn call_with_timeout(
        &mut self,
        tool_name: &str,
        arguments: Value,
        timeout: Duration,
    ) -> anyhow::Result<ToolResult> {
        if !self.is_alive() {
            return Ok(server_down(&self.name));
        }
        let params = json!({ "name": tool_name, "arguments": arguments });
        match self.send_request("tools/call", params, timeout) {
            Ok(resp) => Ok(Self::parse_call_response(resp)),
            Err(e) => {
                // send_request distinguishes the failure modes in its message:
                // "is down" for a broken pipe, "timed out" for a missed reply.
                Ok(ToolResult {
                    content: format!("mcp tool {tool_name} failed: {e:#}"),
                    is_error: true,
                })
            }
        }
    }

    fn parse_call_response(resp: Value) -> ToolResult {
        let json_rpc_error = resp.get("error").map(|e| {
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("unknown mcp error")
                .to_string()
        });
        let is_error_flag = resp
            .get("result")
            .and_then(|r| r.get("isError"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let content_blocks = resp
            .get("result")
            .and_then(|r| r.get("content"))
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        let text = match content_blocks {
            Value::Array(arr) => arr
                .iter()
                .filter_map(|b| {
                    let is_text = b.get("type").and_then(Value::as_str) == Some("text");
                    if is_text {
                        b.get("text").and_then(Value::as_str).map(str::to_string)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        };
        match json_rpc_error {
            Some(err) => ToolResult {
                content: err,
                is_error: true,
            },
            None => ToolResult {
                content: text,
                is_error: is_error_flag,
            },
        }
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        // Kill the WHOLE process group, not just the direct child: a plain
        // `child.kill()` orphans grandchildren that keep the stdout pipe open,
        // the reader never sees EOF, and a naive join wedges the driver
        // forever (same bug class as run_shell's history).
        if let Some(mut child) = self.child.take() {
            kill_process_group(&mut child);
            let _ = child.wait();
        }
        // Bounded join: if the reader has still not seen EOF after the grace
        // period, leak the reader (and its waiter) rather than block Drop.
        if let Some(handle) = self.reader_handle.take() {
            let (tx, rx) = mpsc::channel::<()>();
            let waiter = thread::spawn(move || {
                let _ = handle.join();
                let _ = tx.send(());
            });
            if rx.recv_timeout(READER_JOIN_GRACE).is_err() {
                std::mem::forget(waiter);
            }
        }
    }
}

#[derive(Debug)]
pub struct McpRegistry {
    servers: Vec<McpServer>,
}

impl McpRegistry {
    /// Build the registry for a run/chat session: discover config, spawn every
    /// configured server, complete the handshake, register tools. No config
    /// anywhere, or `mcp_off`, yields an empty registry and zero behavior
    /// change. An explicitly named (`--mcp-config`) config that does not exist
    /// is a user error and fails loudly; discovered-config problems (missing,
    /// unreadable, malformed) fail soft with a warning.
    pub fn new(cwd: &Path, mcp_off: bool, mcp_config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        if mcp_off {
            return Ok(Self { servers: Vec::new() });
        }
        let config_path = match mcp_config_path {
            Some(p) => {
                if !p.exists() {
                    bail!("--mcp-config {}: no such file", p.display());
                }
                Some(p)
            }
            None => find_config(cwd),
        };
        let Some(config_path) = config_path else {
            return Ok(Self { servers: Vec::new() });
        };
        let text = match fs::read_to_string(&config_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "chug: warning: mcp config unreadable {}: {e}",
                    config_path.display()
                );
                return Ok(Self { servers: Vec::new() });
            }
        };
        let config: McpConfig = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "chug: warning: malformed mcp config at {}: {e}",
                    config_path.display()
                );
                return Ok(Self { servers: Vec::new() });
            }
        };
        let mut servers = Vec::new();
        for (name, raw) in config.mcp_servers {
            if !is_valid_name(&name) {
                eprintln!("chug: warning: invalid mcp server name {name}");
                continue;
            }
            let config_error = validate_config(&raw);
            if let Some(msg) = config_error {
                let log = cwd.join(".chug").join(format!("mcp-{name}.log"));
                log_line(&log, &format!("chug: mcp server {name} config error: {msg}"));
                eprintln!("chug: warning: mcp server {name} config error: {msg}");
                continue;
            }
            // Remote HTTP transport is not wired in round 1
            if raw.url.is_some() {
                let log = cwd.join(".chug").join(format!("mcp-{name}.log"));
                log_line(&log, &format!("chug: mcp server {name} is remote; remote http transport lands in round 2"));
                eprintln!("chug: warning: mcp server {name} is remote; remote http transport lands in round 2");
                continue;
            }
            match McpServer::spawn(cwd, &name, raw).and_then(|mut s| {
                s.initialize()?;
                Ok(s)
            }) {
                Ok(srv) => servers.push(srv),
                Err(e) => {
                    let log = cwd.join(".chug").join(format!("mcp-{name}.log"));
                    log_line(&log, &format!("chug: mcp server {name} failed to start: {e:#}"));
                    eprintln!("chug: warning: mcp server {name} failed to start: {e:#}");
                }
            }
        }
        Ok(Self { servers })
    }

    /// MCP tool schemas merged into the tools array. NOTE: MCP tools bypass
    /// the laya risk gate (it judges bash commands only).
    pub fn tool_schemas(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for srv in &self.servers {
            for tool in &srv.tools {
                out.push(json!({
                    "name": format!("mcp__{}__{}", srv.name, tool.name),
                    "description": tool.description,
                    "input_schema": tool.input_schema
                }));
            }
        }
        out
    }

    /// Dispatch a `mcp__<server>__<tool>` tool_use. Always returns a
    /// `ToolResult`; unknown server/tool names become error results the model
    /// can see and recover from.
    pub fn dispatch(&mut self, name: &str, arguments: Value) -> ToolResult {
        let Some((srv_name, tool_name)) = parse_mcp_tool_name(name) else {
            return ToolResult {
                content: format!("unknown tool: {name}"),
                is_error: true,
            };
        };
        let Some(srv) = self.servers.iter_mut().find(|s| s.name == srv_name) else {
            return ToolResult {
                content: format!("mcp server {srv_name} not found"),
                is_error: true,
            };
        };
        match srv.call(&tool_name, arguments) {
            Ok(res) => res,
            Err(e) => ToolResult {
                content: format!("mcp server {srv_name} is down: {e:#}"),
                is_error: true,
            },
        }
    }
}

fn find_config(cwd: &Path) -> Option<PathBuf> {
    let cwd_cfg = cwd.join("mcp.json");
    if cwd_cfg.exists() {
        return Some(cwd_cfg);
    }
    if let Ok(home) = std::env::var("HOME") {
        let home_cfg = PathBuf::from(home).join(".config/chug/mcp.json");
        if home_cfg.exists() {
            return Some(home_cfg);
        }
    }
    None
}

fn is_valid_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn expand_env_vars(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut missing = false;
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next();
            let mut var = String::new();
            while let Some(&ch) = chars.peek() {
                if ch == '}' {
                    chars.next();
                    break;
                }
                var.push(ch);
                chars.next();
            }
            if let Ok(val) = std::env::var(&var) {
                out.push_str(&val);
            } else {
                missing = true;
                break;
            }
        } else {
            out.push(c);
        }
    }
    if missing { None } else { Some(out) }
}

fn validate_config(raw: &McpServerConfigRaw) -> Option<String> {
    let is_remote = raw.url.is_some();
    let is_stdio = raw.command.is_some();
    if is_remote && is_stdio {
        return Some("server cannot be both remote and stdio".into());
    }
    if is_remote {
        if let Some(t) = &raw.transport {
            if t != "http" {
                return Some(format!("unsupported transport: {t}"));
            }
        }
        if let Some(headers) = &raw.headers {
            for (k, v) in headers {
                if expand_env_vars(v).is_none() {
                    return Some(format!("missing env var in header {k}"));
                }
            }
        }
        return None;
    }
    if is_stdio {
        return None;
    }
    Some("stdio server requires command".into())
}

#[derive(Debug, serde::Deserialize)]
struct McpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfigRaw>,
}

#[derive(Debug, serde::Deserialize)]
struct McpServerConfigRaw {
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Option<Vec<String>>,
    #[serde(default)]
    env: Option<HashMap<String, String>>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    transport: Option<String>,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
}

impl McpServer {
    fn spawn(cwd: &Path, name: &str, cfg_raw: McpServerConfigRaw) -> anyhow::Result<Self> {
        let log_path = cwd.join(".chug").join(format!("mcp-{}.log", name));
        if let Some(parent) = log_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let command = cfg_raw.command.as_deref().context("command required for stdio server")?;
        let mut cmd = Command::new(command);
        if let Some(args) = cfg_raw.args {
            cmd.args(args);
        }
        if let Some(env) = cfg_raw.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }
        cmd.current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawning mcp server {name}"))?;
        let stdin = child.stdin.take().context("capturing mcp server stdin")?;
        let stdout = child.stdout.take().context("capturing mcp server stdout")?;
        let stderr = child.stderr.take().context("capturing mcp server stderr")?;
        let stdin = Arc::new(Mutex::new(stdin));

        // stderr → <cwd>/.chug/mcp-<name>.log
        let log_for_stderr = log_path.clone();
        thread::spawn(move || {
            let Ok(mut file) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_for_stderr)
            else {
                return;
            };
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = writeln!(file, "{line}");
            }
        });

        let pending = Arc::new(Mutex::new(HashMap::<u64, Sender<Value>>::new()));
        let next_id = Arc::new(Mutex::new(0u64));
        let alive = Arc::new(Mutex::new(true));
        let (pending_clone, alive_clone, stdin_for_reader, log_for_reader) =
            (Arc::clone(&pending), Arc::clone(&alive), Arc::clone(&stdin), log_path.clone());
        let reader_handle = thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(&line) else {
                    continue;
                };
                let is_request = v.get("method").is_some();
                match v.get("id").and_then(Value::as_u64) {
                    // Server→client request: v1 implements nothing on the
                    // server-initiated surface, but the server MUST get a
                    // JSON-RPC reply or it waits forever — send "method not
                    // found" instead of silently dropping.
                    Some(id) if is_request => {
                        let reply = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": {"code": -32601, "message": "method not found"}
                        });
                        if let Ok(reply_line) = serde_json::to_string(&reply) {
                            let mut stdin = lock(&stdin_for_reader);
                            let _ = stdin.write_all(reply_line.as_bytes());
                            let _ = stdin.write_all(b"\n");
                        }
                    }
                    // Response to one of our requests: demux by id.
                    Some(id) => {
                        if let Some(tx) = lock(&pending_clone).remove(&id) {
                            let _ = tx.send(v);
                        }
                    }
                    // Notification (no id): log and drop.
                    None => log_line(&log_for_reader, &line),
                }
            }
            *lock(&alive_clone) = false;
        });

        Ok(Self {
            name: name.to_string(),
            child: Some(child),
            stdin,
            reader_handle: Some(reader_handle),
            pending,
            next_id,
            tools: Vec::new(),
            alive,
        })
    }

    /// Send one JSON-RPC request and await its response. The pending entry is
    /// registered BEFORE the request is written (a fast server can reply
    /// before the write returns — a post-write insert races and loses the
    /// response) and removed on write failure, so the map never leaks an
    /// entry that no response can ever match.
    fn send_request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> anyhow::Result<Value> {
        let request_id = {
            let mut id = lock(&self.next_id);
            *id += 1;
            *id
        };
        let req = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });
        let line = serde_json::to_string(&req).context("serializing mcp request")?;
        let (tx, rx) = mpsc::channel();
        {
            let mut pending = lock(&self.pending);
            pending.insert(request_id, tx);
        }
        let write_result = (|| -> anyhow::Result<()> {
            let mut stdin = lock(&self.stdin);
            stdin
                .write_all(line.as_bytes())
                .context("write to mcp stdin")?;
            stdin
                .write_all(b"\n")
                .context("write newline to mcp stdin")?;
            Ok(())
        })();
        if let Err(e) = write_result {
            lock(&self.pending).remove(&request_id);
            *lock(&self.alive) = false;
            return Err(e.context(format!("mcp server {} is down", self.name)));
        }
        match rx.recv_timeout(timeout) {
            Ok(resp) => Ok(resp),
            // A timeout is NOT a dead server: remove the pending entry so it
            // cannot shadow a late response's id, and say so.
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&request_id);
                bail!(
                    "mcp {} request timed out after {}s",
                    method,
                    timeout.as_secs()
                );
            }
            Err(RecvTimeoutError::Disconnected) => {
                bail!("mcp response channel closed unexpectedly");
            }
        }
    }

    /// `initialize` → `notifications/initialized` → `tools/list`. Any failure
    /// aborts the handshake; the caller drops the server (fail-soft).
    fn initialize(&mut self) -> anyhow::Result<()> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "chug", "version": "0.1.0" }
        });
        self.send_request("initialize", params, INIT_TIMEOUT)?;
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        if let Ok(line) = serde_json::to_string(&notif) {
            let mut stdin = lock(&self.stdin);
            let _ = stdin.write_all(line.as_bytes());
            let _ = stdin.write_all(b"\n");
        }
        let list_resp = self.send_request("tools/list", json!({}), LIST_TIMEOUT)?;
        let tools = list_resp
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(Value::as_array)
            .with_context(|| format!("mcp server {}: tools/list returned no tools array", self.name))?;
        self.tools = tools
            .iter()
            .filter_map(|t| {
                let name = t.get("name")?.as_str()?.to_string();
                let description = t
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let input_schema = t.get("inputSchema").cloned().unwrap_or(json!({}));
                Some(McpTool { name, description, input_schema })
            })
            .collect();
        Ok(())
    }
}

/// Parse `mcp__<server>__<tool>`. The tool name may itself contain `__`, so
/// split off the prefix, then split server/tool ONCE.
fn parse_mcp_tool_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    if server.is_empty() || tool.is_empty() {
        return None;
    }
    Some((server.to_string(), tool.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ---------- fake servers ----------

    /// Standard echo server: initialize, tools/list (one `echo` tool),
    /// tools/call (echoes arguments as text). Returns the script body.
    fn echo_server_body() -> &'static str {
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
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "0.0.1"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "Echo the arguments back", "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}, "required": ["text"]}}]}})
    elif m == "tools/call":
        if req["params"].get("name") != "echo":
            send({"jsonrpc": "2.0", "id": i, "error": {"code": -32602, "message": "unknown tool: " + req["params"]["name"]}})
        else:
            send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(req["params"]["arguments"])}], "isError": False}})
"#
    }

    /// Write a python script and build a spawn config for it.
    fn server_config(dir: &Path, name: &str, body: &str) -> McpServerConfigRaw {
        let py = dir.join(format!("{name}_srv.py"));
        fs::write(&py, body).unwrap();
        McpServerConfigRaw {
            command: Some("python3".to_string()),
            args: Some(vec![py.to_string_lossy().into_owned()]),
            env: None,
            url: None,
            transport: None,
            headers: None,
        }
    }

    /// Spawn + fully initialize an echo server.
    fn spawn_echo(tmp: &Path) -> McpServer {
        let mut srv = McpServer::spawn(tmp, "fake", server_config(tmp, "fake", echo_server_body()))
            .unwrap();
        srv.initialize().unwrap();
        srv
    }

    /// Write an mcp.json into `dir` configuring one server.
    fn write_mcp_json(dir: &Path, name: &str, body: &str) {
        let py = dir.join(format!("{name}_srv.py"));
        fs::write(&py, body).unwrap();
        let cfg_json = json!({
            "mcpServers": {
                name: {
                    "command": "python3",
                    "args": [py.to_string_lossy()]
                }
            }
        });
        fs::write(dir.join("mcp.json"), serde_json::to_string(&cfg_json).unwrap()).unwrap();
    }

    // ---------- unit: naming / config discovery ----------

    #[test]
    fn config_discovery_cwd_then_home() {
        let tmp = TempDir::new().unwrap();
        let cwd_cfg = tmp.path().join("mcp.json");
        std::fs::write(&cwd_cfg, r#"{"mcpServers":{"a":{"command":"echo"}}}"#).unwrap();
        // cwd config found without a flag
        assert_eq!(find_config(tmp.path()), Some(cwd_cfg));
        // Nothing anywhere → None.
        let empty = TempDir::new().unwrap();
        assert_eq!(find_config(empty.path()), None);
    }

    /// An explicit `--mcp-config` wins over cwd discovery.
    #[test]
    fn flag_config_wins_over_cwd_config() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "cwdserver", echo_server_body());
        let flag_py = tmp.path().join("flagserver_srv.py");
        fs::write(&flag_py, echo_server_body()).unwrap();
        let flag_cfg = json!({
            "mcpServers": {
                "flagserver": {"command": "python3", "args": [flag_py.to_string_lossy()]}
            }
        });
        let flag_path = tmp.path().join("flag-mcp.json");
        fs::write(&flag_path, flag_cfg.to_string()).unwrap();

        let reg = McpRegistry::new(tmp.path(), false, Some(flag_path)).unwrap();
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name, "flagserver");
        assert!(reg.tool_schemas()[0]["name"] == "mcp__flagserver__echo");
    }

    #[test]
    fn invalid_server_name_rejected() {
        assert!(!is_valid_name("BadName"));
        assert!(!is_valid_name("has_underscore"));
        assert!(!is_valid_name(""));
        assert!(is_valid_name("good-name-1"));
    }

    #[test]
    fn parse_mcp_tool_name_splits_once_and_allows_underscores_in_tool() {
        assert_eq!(
            parse_mcp_tool_name("mcp__fake__echo"),
            Some(("fake".into(), "echo".into()))
        );
        // Tool names containing `__` must survive (regression: 3-way split
        // dropped every tool whose name contained a double underscore).
        assert_eq!(
            parse_mcp_tool_name("mcp__fake__do__thing"),
            Some(("fake".into(), "do__thing".into()))
        );
        assert_eq!(parse_mcp_tool_name("mcp__fake"), None);
        assert_eq!(parse_mcp_tool_name("mcp__fake__"), None);
        assert_eq!(parse_mcp_tool_name("mcp____echo"), None);
        assert_eq!(parse_mcp_tool_name("bash"), None);
        assert_eq!(parse_mcp_tool_name("mcp"), None);
    }

    #[test]
    fn nonexistent_mcp_config_flag_errors_loudly() {
        // Regression: `--mcp-config /no/such/file` used to silently disable
        // MCP. It must be a loud user error instead.
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("no-such-mcp.json");
        let err = McpRegistry::new(tmp.path(), false, Some(missing)).unwrap_err();
        assert!(err.to_string().contains("no such file"), "{err}");
    }

    #[test]
    fn malformed_config_disables_mcp_softly() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("mcp.json"), "{ not json !!!").unwrap();
        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert!(reg.servers.is_empty());
    }

    #[test]
    fn mcp_off_yields_empty_registry_even_with_config() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let reg = McpRegistry::new(tmp.path(), true, None).unwrap();
        assert!(reg.servers.is_empty());
        assert!(reg.tool_schemas().is_empty());
    }

    // ---------- full path: spawn → handshake → list → call ----------

    #[test]
    fn fake_server_full_path_handshake_list_call() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert_eq!(reg.servers.len(), 1);

        let schemas = reg.tool_schemas();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0]["name"], "mcp__fake__echo");
        assert_eq!(schemas[0]["description"], "Echo the arguments back");
        assert_eq!(schemas[0]["input_schema"]["type"], "object");

        let res = reg.dispatch(
            "mcp__fake__echo",
            json!({"text": "hello world"}),
        );
        assert!(!res.is_error, "{}", res.content);
        assert_eq!(res.content, r#"echo: {"text": "hello world"}"#);
        // Dropping the registry kills the server (Drop path exercised).
        drop(reg);
    }

    #[test]
    fn dispatch_unknown_server_and_tool_are_error_results() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();

        let res = reg.dispatch("mcp__nosuch__echo", json!({}));
        assert!(res.is_error);
        assert!(res.content.contains("not found"), "{}", res.content);

        let res = reg.dispatch("mcp__fake__nosuchtool", json!({}));
        assert!(res.is_error);
        assert!(!res.content.contains("not found"), "{}", res.content);

        let res = reg.dispatch("bash", json!({}));
        assert!(res.is_error);
        assert!(res.content.contains("unknown tool"), "{}", res.content);
    }

    // ---------- fix (a): pending entry registered before write ----------

    /// Storm of calls against a fast echo server: with the old order (write
    /// first, insert after), a server that replies before the client inserts
    /// its pending entry loses responses. Every call must match its response.
    #[test]
    fn fast_echo_storm_all_responses_matched() {
        let tmp = TempDir::new().unwrap();
        let mut srv = spawn_echo(tmp.path());
        for i in 0..30 {
            let res = srv
                .call("echo", json!({"text": format!("n{i}")}))
                .unwrap();
            assert!(!res.is_error, "call {i} lost: {}", res.content);
            assert!(
                res.content.contains(&format!(r#""text": "n{i}""#)),
                "call {i} got a foreign response: {}",
                res.content
            );
        }
    }

    /// A failed write must remove the pending entry it inserted, leaving the
    /// map clean (no shadowed ids, no leaked senders).
    #[test]
    fn write_failure_removes_pending_entry() {
        let tmp = TempDir::new().unwrap();
        let mut srv = spawn_echo(tmp.path());
        // Kill the child so the stdin pipe breaks; the reader may or may not
        // have noticed EOF yet, but either way the pending map must end empty.
        let mut child = srv.child.take().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(100));

        let res = srv.call("echo", json!({"text": "x"})).unwrap();
        assert!(res.is_error, "{}", res.content);
        assert!(srv.pending.lock().unwrap().is_empty());
    }

    // ---------- fix (d): timeout says timeout, not down ----------

    #[test]
    fn timeout_removes_pending_entry_and_says_timeout() {
        let tmp = TempDir::new().unwrap();
        // A server that answers the handshake but never answers tools/call.
        let body = r#"
import sys, json, time
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
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "slow", "description": "never answers", "inputSchema": {"type": "object"}}]}})
    elif m == "tools/call":
        time.sleep(300)
"#;
        let mut srv = McpServer::spawn(tmp.path(), "fake", server_config(tmp.path(), "fake", body)).unwrap();
        srv.initialize().unwrap();

        let res = srv
            .call_with_timeout("slow", json!({}), Duration::from_millis(300))
            .unwrap();
        assert!(res.is_error);
        // Regression: this used to read "server is down", which is wrong —
        // the server is alive, just slow.
        assert!(res.content.contains("timed out after"), "{}", res.content);
        assert!(!res.content.contains("is down"), "{}", res.content);
        assert!(srv.is_alive());
        // The pending entry was removed, not left to shadow the id.
        assert!(srv.pending.lock().unwrap().is_empty());
    }

    // ---------- fix (f): server→client requests get a reply ----------

    #[test]
    fn server_request_gets_method_not_found_reply() {
        let tmp = TempDir::new().unwrap();
        // On tools/call this server first asks the client to sample, and only
        // answers the call after receiving ANY reply for its request id. If
        // the client dropped the server→client request, the call would time
        // out instead of returning the echo.
        let body = r#"
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
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}})
    elif m == "tools/call":
        send({"jsonrpc": "2.0", "id": 100, "method": "sampling/createMessage", "params": {}})
        for line2 in sys.stdin:
            r2 = json.loads(line2)
            if r2.get("id") == 100:
                send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(r2)}], "isError": False}})
                break
"#;
        let mut srv = McpServer::spawn(tmp.path(), "fake", server_config(tmp.path(), "fake", body)).unwrap();
        srv.initialize().unwrap();

        // Short timeout so a regression (silently dropped request → server
        // never answers) fails in seconds, not after 60s.
        let res = srv
            .call_with_timeout("echo", json!({}), Duration::from_secs(5))
            .unwrap();
        assert!(!res.is_error, "{}", res.content);
        // The echoed server-request reply must be the JSON-RPC error reply.
        assert!(
            res.content.contains(r#""code": -32601"#) && res.content.contains("method not found"),
            "client reply was not a method-not-found error: {}",
            res.content
        );
    }

    // ---------- fix (b): Drop kills the group and never wedges ----------

    /// Regression: the MCP server spawns a background `sleep` in its own
    /// process group that inherits the stdout pipe. Before the fix, Drop
    /// killed only the direct child; the orphan sleep kept the pipe open, the
    /// reader never saw EOF, and the unbounded join blocked Drop forever.
    /// Drop must kill the whole group and return promptly.
    #[test]
    fn drop_kills_process_group_and_does_not_wedge() {
        let tmp = TempDir::new().unwrap();
        let py = tmp.path().join("fake_srv.py");
        fs::write(&py, echo_server_body()).unwrap();
        let cfg = McpServerConfigRaw {
            command: Some("sh".to_string()),
            args: Some(vec![
                "-c".to_string(),
                format!("sleep 300 & exec python3 {}", py.to_string_lossy()),
            ]),
            env: None,
            url: None,
            transport: None,
            headers: None,
        };
        let mut srv = McpServer::spawn(tmp.path(), "fake", cfg).unwrap();
        srv.initialize().unwrap();

        let start = std::time::Instant::now();
        drop(srv);
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(8),
            "McpServer::drop blocked for {elapsed:?} (process-group kill failed)"
        );
    }

    // ---------- spec: framing / dead server / misc ----------

    /// A notification interleaved between request and result must not break
    /// response matching: the result is still matched by id.
    #[test]
    fn notification_interleaved_between_request_and_result() {
        let tmp = TempDir::new().unwrap();
        let body = r#"
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
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "fake", "version": "0"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}})
    elif m == "tools/call":
        send({"jsonrpc": "2.0", "method": "notifications/progress", "params": {"progress": 1}})
        send({"jsonrpc": "2.0", "id": i, "result": {"content": [{"type": "text", "text": "echo: " + json.dumps(req["params"]["arguments"])}], "isError": False}})
"#;
        let mut srv = McpServer::spawn(tmp.path(), "fake", server_config(tmp.path(), "fake", body)).unwrap();
        srv.initialize().unwrap();

        let res = srv.call("echo", json!({"text": "hi"})).unwrap();
        assert!(!res.is_error, "{}", res.content);
        assert!(res.content.contains("hi"), "{}", res.content);
    }

    /// Server killed mid-run (SIGKILL): subsequent calls return the spec'd
    /// `mcp server <name> is down` error result — no respawn in v1, no panic.
    #[test]
    fn dead_server_reports_down_and_run_continues() {
        let tmp = TempDir::new().unwrap();
        let mut srv = spawn_echo(tmp.path());
        let mut child = srv.child.take().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        // Give the reader thread a beat to observe EOF.
        for _ in 0..50 {
            if !srv.is_alive() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!srv.is_alive());

        let res = srv.call("echo", json!({"text": "x"})).unwrap();
        assert!(res.is_error);
        assert_eq!(res.content, "mcp server fake is down");
    }

    #[test]
    fn json_rpc_error_reply_maps_to_error_result() {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32602, "message": "unknown tool: nope"}
        });
        let res = McpServer::parse_call_response(resp);
        assert!(res.is_error);
        assert_eq!(res.content, "unknown tool: nope");
    }

    #[test]
    fn is_error_flag_and_multi_text_blocks_map_correctly() {
        let resp = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "content": [
                    {"type": "text", "text": "line one"},
                    {"type": "image", "data": "..."},
                    {"type": "text", "text": "line two"}
                ],
                "isError": true
            }
        });
        let res = McpServer::parse_call_response(resp);
        assert!(res.is_error);
        assert_eq!(res.content, "line one\nline two");
    }

    // ---------- spec-9 config extension ----------

    #[test]
    fn remote_entry_parses_and_validates_success() {
        use std::collections::HashMap;
        unsafe { std::env::set_var("REMOTE_TOKEN", "abc123"); }
        let raw = McpServerConfigRaw {
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            transport: Some("http".to_string()),
            headers: Some({
                let mut m = HashMap::new();
                m.insert("Authorization".to_string(), "Bearer ${REMOTE_TOKEN}".to_string());
                m
            }),
        };
        assert_eq!(validate_config(&raw), None);
    }

    #[test]
    fn missing_env_var_skips_only_that_server() {
        let tmp = TempDir::new().unwrap();
        // Create a stdio server
        let py = tmp.path().join("std_srv.py");
        fs::write(&py, echo_server_body()).unwrap();
        let cfg_json = json!({
            "mcpServers": {
                "stdio-srv": {"command": "python3", "args": [py.to_string_lossy()]},
                "remote-srv": {
                    "url": "https://example.com/mcp",
                    "transport": "http",
                    "headers": {"Authorization": "Bearer ${MISSING_VAR}"}
                }
            }
        });
        fs::write(tmp.path().join("mcp.json"), serde_json::to_string(&cfg_json).unwrap()).unwrap();

        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        // remote skipped due to missing env var, stdio loaded
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name, "stdio-srv");
    }

    #[test]
    fn bad_transport_skips_only_that_server() {
        let tmp = TempDir::new().unwrap();
        let py = tmp.path().join("std_srv.py");
        fs::write(&py, echo_server_body()).unwrap();
        let cfg_json = json!({
            "mcpServers": {
                "stdio-srv": {"command": "python3", "args": [py.to_string_lossy()]},
                "remote-srv": {
                    "url": "https://example.com/mcp",
                    "transport": "ws",
                    "headers": {}
                }
            }
        });
        fs::write(tmp.path().join("mcp.json"), serde_json::to_string(&cfg_json).unwrap()).unwrap();

        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name, "stdio-srv");
    }

    #[test]
    fn stdio_only_config_unchanged() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert_eq!(reg.servers.len(), 1);
    }
}

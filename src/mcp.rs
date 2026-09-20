use anyhow::{Context, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const PROTOCOL_VERSION: &str = "2025-06-18";
const INIT_TIMEOUT: Duration = Duration::from_secs(10);
const LIST_TIMEOUT: Duration = Duration::from_secs(10);
const CALL_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

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
    stdin: Option<std::sync::Mutex<std::process::ChildStdin>>,
    reader_handle: Option<thread::JoinHandle<()>>,
    pending: Arc<Mutex<HashMap<u64, std::sync::mpsc::Sender<Value>>>>,
    next_id: Arc<Mutex<u64>>,
    tools: Vec<McpTool>,
    log_path: PathBuf,
    alive: Arc<Mutex<bool>>,
}

impl McpServer {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn tools(&self) -> &[McpTool] {
        &self.tools
    }

    pub fn is_alive(&self) -> bool {
        *self.alive.lock().unwrap()
    }

    pub fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<CallResult> {
        if !self.is_alive() {
            return Ok(CallResult {
                content: format!("mcp server {} is down", self.name),
                is_error: true,
            });
        }
        let request_id = {
            let mut id = self.next_id.lock().unwrap();
            *id += 1;
            *id
        };
        let req = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });
        let (tx, rx) = std::sync::mpsc::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(request_id, tx);
        }
        let line = serde_json::to_string(&req).unwrap();
        let mut stdin = self.stdin.as_ref().unwrap().lock().unwrap();
        if let Err(e) = writeln!(stdin, "{}", line) {
            return Ok(CallResult {
                content: format!("mcp server {} is down", self.name),
                is_error: true,
            });
        }
        // wait for response
        match rx.recv_timeout(CALL_TIMEOUT) {
            Ok(resp) => {
                let is_error = resp.get("error").is_some()
                    || resp.get("result").and_then(|r| r.get("isError")).and_then(|v| v.as_bool()).unwrap_or(false);
                let content_blocks = resp.get("result").and_then(|r| r.get("content")).cloned().unwrap_or(Value::Array(vec![]));
                let text = match content_blocks {
                    Value::Array(arr) => arr.iter().filter_map(|b| b.get("type").and_then(|t| t.as_str()).filter(|t| *t == "text").and_then(|_| b.get("text")).and_then(|t| t.as_str()).map(|s| s.to_string())).collect::<Vec<_>>().join("\n"),
                    _ => String::new(),
                };
                Ok(CallResult { content: text, is_error })
            }
            Err(_) => Ok(CallResult {
                content: format!("mcp server {} is down", self.name),
                is_error: true,
            }),
        }
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

#[derive(Debug)]
pub struct CallResult {
    pub content: String,
    pub is_error: bool,
}

pub struct McpRegistry {
    servers: Vec<McpServer>,
}

impl McpRegistry {
    pub fn new(cwd: &Path, mcp_off: bool, mcp_config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        if mcp_off {
            return Ok(Self { servers: Vec::new() });
        }
        let config_path = find_config(cwd, mcp_config_path);
        let config = match config_path {
            Some(p) => match fs::read_to_string(&p) {
                Ok(s) => match serde_json::from_str::<McpConfig>(&s) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("chug: warning: malformed mcp config at {}: {}", p.display(), e);
                        return Ok(Self { servers: Vec::new() });
                    }
                },
                Err(e) => {
                    eprintln!("chug: warning: mcp config unreadable {}: {}", p.display(), e);
                    return Ok(Self { servers: Vec::new() });
                }
            },
            None => return Ok(Self { servers: Vec::new() }),
        };
        let mut servers = Vec::new();
        for (name, cfg) in config.mcp_servers {
            if !is_valid_name(&name) {
                eprintln!("chug: warning: invalid mcp server name {}", name);
                continue;
            }
            let server = McpServer::spawn(cwd, &name, cfg);
            match server {
                Ok(mut srv) => {
                    if srv.initialize() {
                        servers.push(srv);
                    }
                }
                Err(e) => {
                    eprintln!("chug: warning: mcp server {} failed to start: {}", name, e);
                }
            }
        }
        Ok(Self { servers })
    }

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

    pub fn dispatch(&mut self, name: &str, arguments: Value) -> CallResult {
        if let Some((srv_name, tool_name)) = parse_mcp_tool_name(name) {
            for srv in &mut self.servers {
                if srv.name == srv_name {
                    match srv.call(&tool_name, arguments) {
                        Ok(res) => return res,
                        Err(_) => {
                            return CallResult {
                                content: format!("mcp server {} is down", srv_name),
                                is_error: true,
                            }
                        }
                    }
                }
            }
            return CallResult {
                content: format!("mcp server {} not found", srv_name),
                is_error: true,
            };
        }
        CallResult {
            content: "unknown tool".to_string(),
            is_error: true,
        }
    }

    pub fn kill_all(&mut self) {
        self.servers.clear();
    }
}

fn find_config(cwd: &Path, flag_path: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = flag_path {
        if p.exists() { return Some(p); }
        return None;
    }
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

#[derive(Debug, serde::Deserialize)]
struct McpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, McpServerConfigRaw>,
}

#[derive(Debug, serde::Deserialize)]
struct McpServerConfigRaw {
    command: String,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
}

impl McpServer {
    fn spawn(cwd: &Path, name: &str, cfg_raw: McpServerConfigRaw) -> anyhow::Result<Self> {
        let log_path = cwd.join(".chug").join(format!("mcp-{}.log", name));
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut cmd = Command::new(&cfg_raw.command);
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
        let mut child = cmd.spawn().with_context(|| format!("spawning mcp server {}", name))?;
        let stdin = child.stdin.take().context("stdin")?;
        let stdout = child.stdout.take().context("stdout")?;
        let stderr = child.stderr.take().context("stderr")?;

        // log stderr
        let log_path_clone = log_path.clone();
        thread::spawn(move || {
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&log_path_clone).unwrap_or_else(|_| std::fs::File::create(&log_path_clone).unwrap());
            let mut reader = BufReader::new(stderr);
            let mut buf = String::new();
            loop {
                buf.clear();
                if reader.read_line(&mut buf).unwrap_or(0) == 0 { break; }
                let _ = file.write_all(buf.as_bytes());
            }
        });

        let pending = Arc::new(Mutex::new(HashMap::<u64, std::sync::mpsc::Sender<Value>>::new()));
        let next_id = Arc::new(Mutex::new(0u64));
        let pending_clone = Arc::clone(&pending);
        let alive = Arc::new(Mutex::new(true));
        let alive_clone = Arc::clone(&alive);
        let reader_handle = thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    if line.trim().is_empty() { continue; }
                    if let Ok(v) = serde_json::from_str::<Value>(&line) {
                        if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                            let mut pending_map = pending_clone.lock().unwrap();
                            if let Some(tx) = pending_map.remove(&id) {
                                let _ = tx.send(v);
                            }
                        } else {
                            // notification, drop
                        }
                    }
                } else {
                    break;
                }
            }
            *alive_clone.lock().unwrap() = false;
        });

        Ok(Self {
            name: name.to_string(),
            child: Some(child),
            stdin: Some(std::sync::Mutex::new(stdin)),
            reader_handle: Some(reader_handle),
            pending,
            next_id,
            tools: Vec::new(),
            log_path,
            alive,
        })
    }

    fn send_request(&mut self, method: &str, params: Value, timeout: Duration) -> anyhow::Result<Value> {
        let request_id = {
            let mut id = self.next_id.lock().unwrap();
            *id += 1;
            *id
        };
        let req = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });
        let line = serde_json::to_string(&req).unwrap();
        let mut stdin = self.stdin.as_ref().unwrap().lock().unwrap();
        stdin.write_all(line.as_bytes()).context("write to mcp stdin")?;
        stdin.write_all(b"\n").context("write newline")?;

        let (tx, rx) = std::sync::mpsc::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(request_id, tx);
        }
        match rx.recv_timeout(timeout) {
            Ok(resp) => Ok(resp),
            Err(_) => bail!("mcp request timeout"),
        }
    }

    fn initialize(&mut self) -> bool {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "chug", "version": "0.1.0" }
        });
        match self.send_request("initialize", params, INIT_TIMEOUT) {
            Ok(resp) => {
                // send initialized notification
                let notif = json!({
                    "jsonrpc": "2.0",
                    "method": "notifications/initialized",
                    "params": {}
                });
                if let Ok(line) = serde_json::to_string(&notif) {
                    if let Ok(mut stdin) = self.stdin.as_ref().unwrap().lock() {
                        let _ = writeln!(stdin, "{}", line);
                    }
                }
                // list tools
                match self.send_request("tools/list", json!({}), LIST_TIMEOUT) {
                    Ok(list_resp) => {
                        if let Some(tools) = list_resp.get("result").and_then(|r| r.get("tools")).and_then(|t| t.as_array()) {
                            self.tools = tools.iter().filter_map(|t| {
                                let name = t.get("name")?.as_str()?.to_string();
                                let description = t.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                let input_schema = t.get("inputSchema").cloned().unwrap_or(json!({}));
                                Some(McpTool { name, description, input_schema })
                            }).collect();
                            true
                        } else {
                            false
                        }
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }
}

fn parse_mcp_tool_name(name: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = name.split("__").collect();
    if parts.len() == 3 && parts[0] == "mcp" {
        Some((parts[1].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn config_discovery_precedence() {
        let tmp = TempDir::new().unwrap();
        let cwd_cfg = tmp.path().join("mcp.json");
        std::fs::write(&cwd_cfg, r#"{"mcpServers":{"a":{"command":"echo"}}}"#).unwrap();
        let home_cfg = std::env::var("HOME").unwrap();
        // discovery uses flag first, then cwd, then home
        assert!(find_config(tmp.path(), Some(cwd_cfg.clone())).unwrap() == cwd_cfg);
        assert!(find_config(tmp.path(), None).unwrap() == cwd_cfg);
    }

    #[test]
    fn invalid_server_name_rejected() {
        assert!(!is_valid_name("BadName"));
        assert!(is_valid_name("good-name-1"));
    }
}

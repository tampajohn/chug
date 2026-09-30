use anyhow::{anyhow, Context, bail};
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

use crate::permissions::Permissions;
use crate::tools::{ToolResult, kill_process_group};

pub(crate) const PROTOCOL_VERSION: &str = "2025-06-18";
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

/// Defensive cap on registered resources per server (mirror of the HTTP
/// transport's MAX_MCP_TOOLS: a broken server must not flood the registry).
pub(crate) const MAX_MCP_RESOURCES: usize = 200;

/// Server capabilities advertised at `initialize` (F11 phase 1a). One
/// shared shape for every transport — stdio parses it now, the HTTP
/// transport captures the same struct (no per-transport fork) and phase 1b
/// reads the prompts/listChanged flags from it. Presence, per the MCP spec,
/// is the capability KEY existing (an empty `{}` object advertises the
/// capability with no options); `listChanged` is its optional flag.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[allow(dead_code)] // the catalog is kept COMPLETE (all advertised flags), not just the flags phase 1a reads; phase 1b reads the rest
pub(crate) struct McpCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub tools_list_changed: bool,
    pub resources_list_changed: bool,
    pub prompts_list_changed: bool,
}

impl McpCapabilities {
    /// Parse the `capabilities` object out of an initialize result. An
    /// absent object (or absent per-capability entries) is simply false —
    /// a server advertising nothing supports nothing, and a server without
    /// `resources` is never queried for them.
    pub(crate) fn parse(capabilities: Option<&Value>) -> Self {
        let Some(caps) = capabilities.and_then(Value::as_object) else {
            return Self::default();
        };
        let flag = |key: &str| caps.contains_key(key);
        let list_changed = |key: &str| {
            caps.get(key)
                .and_then(|c| c.get("listChanged"))
                .and_then(Value::as_bool)
                .unwrap_or(false)
        };
        Self {
            tools: flag("tools"),
            resources: flag("resources"),
            prompts: flag("prompts"),
            tools_list_changed: list_changed("tools"),
            resources_list_changed: list_changed("resources"),
            prompts_list_changed: list_changed("prompts"),
        }
    }
}

/// One advertised MCP resource (a `resources/list` entry) — metadata only;
/// the content itself comes from `resources/read`.
#[derive(Debug, Clone)]
#[allow(dead_code)] // registry surface is internal until phase 1b wires a model-facing consumer
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// One content block from `resources/read`: exactly one of `text`/`blob`
/// carries the payload (`blob` is the server's base64 string, kept verbatim
/// — decoding is the consumer's job, phase 1b).
#[derive(Debug, Clone)]
#[allow(dead_code)] // registry surface is internal until phase 1b wires a model-facing consumer
pub struct McpResourceContents {
    pub uri: String,
    pub mime_type: String,
    pub text: Option<String>,
    pub blob: Option<String>,
}

/// Shared `resources/list` response mapping (stdio phase 1a; the HTTP
/// transport reuses it in phase 1b). Entries without a `uri` are skipped
/// (same filter_map discipline as tools/list). Over-cap advertisements are
/// warn-and-capped: the first `max` are kept and the warning line is
/// returned for the caller to log where its transport logs.
pub(crate) fn parse_resources_list(
    server: &str,
    resp: &Value,
    max: usize,
) -> anyhow::Result<(Vec<McpResource>, Option<String>)> {
    // A JSON-RPC error reply carries the server's own message — surface it
    // (same discipline as parse_call_response) so a refused list names why.
    if let Some(err) = resp
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
    {
        bail!("mcp server {server}: resources/list failed: {err}");
    }
    let resources = resp
        .get("result")
        .and_then(|r| r.get("resources"))
        .and_then(Value::as_array)
        .with_context(|| format!("mcp server {server}: resources/list returned no resources array"))?;
    let mapped: Vec<McpResource> = resources
        .iter()
        .filter_map(|r| {
            let uri = r.get("uri")?.as_str()?.to_string();
            Some(McpResource {
                uri,
                name: r.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                description: r
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                mime_type: r
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            })
        })
        .collect();
    if mapped.len() > max {
        let warning =
            format!("resources/list advertised {} resources; capped at {max}", mapped.len());
        return Ok((mapped.into_iter().take(max).collect(), Some(warning)));
    }
    Ok((mapped, None))
}

/// Shared `resources/read` response mapping: `result.contents` text and
/// blob blocks become [`McpResourceContents`] values.
pub(crate) fn parse_resource_contents(server: &str, resp: &Value) -> anyhow::Result<Vec<McpResourceContents>> {
    // Same error mapping as resources/list: the server's message names the
    // failure (e.g. an unknown uri), not just the missing contents array.
    if let Some(err) = resp
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
    {
        bail!("mcp server {server}: resources/read failed: {err}");
    }
    let contents = resp
        .get("result")
        .and_then(|r| r.get("contents"))
        .and_then(Value::as_array)
        .with_context(|| format!("mcp server {server}: resources/read returned no contents array"))?;
    Ok(contents
        .iter()
        .filter_map(|c| {
            let uri = c.get("uri")?.as_str()?.to_string();
            Some(McpResourceContents {
                uri,
                mime_type: c
                    .get("mimeType")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                text: c.get("text").and_then(Value::as_str).map(str::to_string),
                blob: c.get("blob").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect())
}

/// Transport-agnostic view of one MCP server (stdio child process or remote
/// streamable-HTTP endpoint). The registry routes purely by server name; the
/// transport is an implementation detail of each backend.
pub trait McpBackend {
    fn name(&self) -> &str;
    /// Part of the backend contract (both transports implement it); the
    /// registry itself routes by name and lets `call` handle down servers.
    #[allow(dead_code)]
    fn is_alive(&self) -> bool;
    fn tools(&self) -> &[McpTool];
    fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<ToolResult>;
    /// Capabilities the server advertised at initialize (shared
    /// [`McpCapabilities`] shape — both transports capture it).
    #[allow(dead_code)] // phase 1b wires a model-facing surface; the registry methods below read it
    fn capabilities(&self) -> &McpCapabilities;
    /// `resources/list`. Default: this transport does not speak resources
    /// yet (F11 phase 1b) — the stdio transport overrides.
    #[allow(dead_code)]
    fn list_resources(&mut self) -> anyhow::Result<Vec<McpResource>> {
        bail!(
            "mcp server {}: resources are not supported over this transport yet (phase 1b)",
            self.name()
        )
    }
    /// `resources/read` for one uri. Default: same phase-1b story as
    /// [`McpBackend::list_resources`].
    #[allow(dead_code)]
    fn read_resource(&mut self, _uri: &str) -> anyhow::Result<Vec<McpResourceContents>> {
        bail!(
            "mcp server {}: resources are not supported over this transport yet (phase 1b)",
            self.name()
        )
    }
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
    /// Capabilities the server advertised at initialize (F11 phase 1a):
    /// a server without the `resources` flag is never sent resources legs.
    capabilities: McpCapabilities,
    /// Per-server log file (.chug/mcp-<name>.log): stderr, dropped
    /// notifications, and the resources warn-and-cap note land here.
    log_path: PathBuf,
    alive: Arc<Mutex<bool>>,
}

/// Lock a mutex without panicking on poisoning: a panic in the reader thread
/// must not take the whole driver down through an unrelated lock site.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Append one line to the per-server mcp log. Best effort: logging must never
/// break the protocol path. Shared with the HTTP transport (remote servers
/// log to the same .chug/mcp-<name>.log file).
pub(crate) fn log_line(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}

fn server_down(name: &str) -> ToolResult {
    ToolResult {
        content: format!("mcp server {name} is down"),
        is_error: true,
        images: Vec::new(),
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
                    images: Vec::new(),
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
                images: Vec::new(),
            },
            None => ToolResult {
                content: text,
                is_error: is_error_flag,
                images: Vec::new(),
            },
        }
    }

    /// `resources/list` (LIST_TIMEOUT budget, same framing discipline as
    /// tools/list). The response mapping is shared with the HTTP transport
    /// (phase 1b reuses `parse_resources_list`); the warn-and-cap note goes
    /// to the per-server log, mirroring the HTTP tools cap.
    fn resources_list(&mut self) -> anyhow::Result<Vec<McpResource>> {
        if !self.is_alive() {
            bail!("mcp server {} is down", self.name);
        }
        let resp = self.send_request("resources/list", json!({}), LIST_TIMEOUT)?;
        let (resources, warning) = parse_resources_list(&self.name, &resp, MAX_MCP_RESOURCES)?;
        if let Some(warning) = warning {
            log_line(
                &self.log_path,
                &format!("chug: warning: mcp server {}: {warning}", self.name),
            );
        }
        Ok(resources)
    }

    /// `resources/read` for one uri (CALL_TIMEOUT budget: a read can be as
    /// expensive as a tool call). Text and blob contents map into
    /// [`McpResourceContents`]; JSON-RPC errors become named errors.
    fn resource_read(&mut self, uri: &str) -> anyhow::Result<Vec<McpResourceContents>> {
        if !self.is_alive() {
            bail!("mcp server {} is down", self.name);
        }
        let resp = self.send_request("resources/read", json!({ "uri": uri }), CALL_TIMEOUT)?;
        parse_resource_contents(&self.name, &resp)
    }
}

impl McpBackend for McpServer {
    fn name(&self) -> &str {
        &self.name
    }
    fn is_alive(&self) -> bool {
        McpServer::is_alive(self)
    }
    fn tools(&self) -> &[McpTool] {
        &self.tools
    }
    fn call(&mut self, tool_name: &str, arguments: Value) -> anyhow::Result<ToolResult> {
        // Delegate to the inherent method: identical wire behavior.
        McpServer::call(self, tool_name, arguments)
    }
    fn capabilities(&self) -> &McpCapabilities {
        &self.capabilities
    }
    fn list_resources(&mut self) -> anyhow::Result<Vec<McpResource>> {
        // Delegate to the inherent method: identical wire behavior.
        McpServer::resources_list(self)
    }
    fn read_resource(&mut self, uri: &str) -> anyhow::Result<Vec<McpResourceContents>> {
        // Delegate to the inherent method: identical wire behavior.
        McpServer::resource_read(self, uri)
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

pub struct McpRegistry {
    servers: Vec<Box<dyn McpBackend>>,
    /// T138: entries parsed from the config but NOT yet spawned. Spawning
    /// waits for [`McpRegistry::start`], which the driver calls only after
    /// `.chug/permissions.json` has loaded and only for servers the deny
    /// rules allow to start — a repository-controlled mcp.json must not
    /// execute its command before permission enforcement.
    pending: Vec<PendingServer>,
}

/// One parsed-and-validated config entry awaiting its spawn (T138).
struct PendingServer {
    cwd: PathBuf,
    name: String,
    raw: McpServerConfigRaw,
}

/// One server's outcome from [`McpRegistry::list_resources`]: every started
/// server appears exactly once; a server that cannot serve resources (not
/// advertised, down, timed out) carries the named error instead — partial
/// failure is data, never a panic and never a wire request the server did
/// not advertise.
#[derive(Debug)]
#[allow(dead_code)] // phase 1b wires a model-facing surface over this result
pub struct McpServerResources {
    pub server: String,
    /// This server's resources, or the error that failed its leg (the
    /// message already names the server).
    pub resources: anyhow::Result<Vec<McpResource>>,
}

impl McpRegistry {
    /// Build the registry for a run/chat session: discover config, read and
    /// validate it — and NOTHING else. No process is spawned, no network
    /// connection is opened: that happens in [`McpRegistry::start`], which
    /// the driver calls after permissions load (T138: a repo-controlled
    /// mcp.json used to spawn its command before the policy layer existed).
    /// No config anywhere, or `mcp_off`, yields an empty registry and zero
    /// behavior change. An explicitly named (`--mcp-config`) config that
    /// does not exist is a user error and fails loudly; discovered-config
    /// problems (missing, unreadable, malformed) fail soft with a warning.
    pub fn new(cwd: &Path, mcp_off: bool, mcp_config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        if mcp_off {
            return Ok(Self { servers: Vec::new(), pending: Vec::new() });
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
            return Ok(Self { servers: Vec::new(), pending: Vec::new() });
        };
        let text = match fs::read_to_string(&config_path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!(
                    "chug: warning: mcp config unreadable {}: {e}",
                    config_path.display()
                );
                return Ok(Self { servers: Vec::new(), pending: Vec::new() });
            }
        };
        let config: McpConfig = match serde_json::from_str(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "chug: warning: malformed mcp config at {}: {e}",
                    config_path.display()
                );
                return Ok(Self { servers: Vec::new(), pending: Vec::new() });
            }
        };
        let mut pending: Vec<PendingServer> = Vec::new();
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
            pending.push(PendingServer { cwd: cwd.to_path_buf(), name, raw });
        }
        Ok(Self { servers: Vec::new(), pending })
    }

    /// T138: spawn + handshake every entry parsed at `new` time — called by
    /// the driver AFTER `.chug/permissions.json` has loaded, so a
    /// repository-controlled mcp.json never executes its command before
    /// permission enforcement. Each spawn is gated by
    /// [`Permissions::mcp_spawn_block_reason`]: a whole-tool `bash` deny or
    /// an `mcp__*`/`mcp__<server>__*` deny skips the spawn (fail-soft warn,
    /// like any other spawn failure). Fail-soft per server: any spawn or
    /// handshake error skips that server; the run continues.
    pub fn start(&mut self, permissions: &Permissions) {
        for PendingServer { cwd, name, raw } in std::mem::take(&mut self.pending) {
            let is_stdio = raw.url.is_none();
            if let Some(reason) = permissions.mcp_spawn_block_reason(&name, is_stdio) {
                let log = cwd.join(".chug").join(format!("mcp-{}.log", name));
                let detail =
                    format!("chug: mcp server {name} not started: denied by permissions ({reason})");
                log_line(&log, &detail);
                eprintln!("chug: warning: {detail}");
                continue;
            }
            // Remote entry: streamable-HTTP transport. Fail-soft like stdio:
            // any handshake error skips this server, the run continues.
            if !is_stdio {
                match spawn_remote(&cwd, &name, &raw) {
                    Ok(srv) => self.servers.push(Box::new(srv)),
                    Err(e) => {
                        let log = cwd.join(".chug").join(format!("mcp-{name}.log"));
                        log_line(&log, &format!("chug: mcp server {name} failed to start: {e:#}"));
                        eprintln!("chug: warning: mcp server {name} failed to start: {e:#}");
                    }
                }
                continue;
            }
            match McpServer::spawn(&cwd, &name, raw).and_then(|mut s| {
                s.initialize()?;
                Ok(s)
            }) {
                Ok(srv) => self.servers.push(Box::new(srv)),
                Err(e) => {
                    let log = cwd.join(".chug").join(format!("mcp-{name}.log"));
                    log_line(&log, &format!("chug: mcp server {name} failed to start: {e:#}"));
                    eprintln!("chug: warning: mcp server {name} failed to start: {e:#}");
                }
            }
        }
    }

    /// MCP tool schemas merged into the tools array. NOTE: MCP tools bypass
    /// the laya risk gate (it judges bash commands only).
    pub fn tool_schemas(&self) -> Vec<Value> {
        let mut out = Vec::new();
        for srv in &self.servers {
            for tool in srv.tools() {
                out.push(json!({
                    "name": format!("mcp__{}__{}", srv.name(), tool.name),
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
                images: Vec::new(),
            };
        };
        let Some(srv) = self.servers.iter_mut().find(|s| s.name() == srv_name) else {
            return ToolResult {
                content: format!("mcp server {srv_name} not found"),
                is_error: true,
                images: Vec::new(),
            };
        };
        match srv.call(&tool_name, arguments) {
            Ok(res) => res,
            Err(e) => ToolResult {
                content: format!("mcp server {srv_name} is down: {e:#}"),
                is_error: true,
                images: Vec::new(),
            },
        }
    }

    /// F11 phase 1a: list resources from every started server. The
    /// capability catalog gates the wire: a server that did not advertise
    /// `resources` is NEVER sent resources/list — its entry carries the
    /// named error instead. Dead or timed-out servers degrade to a named
    /// error in their own entry (the run continues); capable servers reuse
    /// the LIST timeout.
    #[allow(dead_code)] // internal surface until phase 1b wires a model-facing tool
    pub fn list_resources(&mut self) -> Vec<McpServerResources> {
        self.servers
            .iter_mut()
            .map(|srv| {
                let server = srv.name().to_string();
                // The capability gate IS the (e) contract: a server that did
                // not advertise `resources` is never sent resources/list —
                // RED-proven (the always-query mutant made the stub see the
                // request) and now enforced here.
                let resources = if srv.capabilities().resources {
                    srv.list_resources()
                } else {
                    Err(anyhow!("mcp server {server} does not advertise resources"))
                };
                McpServerResources { server, resources }
            })
            .collect()
    }

    /// F11 phase 1a: read one resource from the named server. Unknown
    /// server, missing capability, and dead server all produce named
    /// errors — never a panic, never a hang (the leg reuses the CALL
    /// timeout).
    #[allow(dead_code)] // internal surface until phase 1b wires a model-facing tool
    pub fn read_resource(
        &mut self,
        server: &str,
        uri: &str,
    ) -> anyhow::Result<Vec<McpResourceContents>> {
        let Some(srv) = self.servers.iter_mut().find(|s| s.name() == server) else {
            bail!("mcp server {server} not found");
        };
        if !srv.capabilities().resources {
            bail!("mcp server {server} does not advertise resources; not sending resources/read");
        }
        srv.read_resource(uri)
    }
}

/// Build and handshake a remote (streamable-HTTP) MCP server from an already
/// validated config: `url` is present, `transport` is "http", and every
/// `${VAR}` in the headers expands (validate_config ran first). Fail-soft:
/// any error here skips the server; the caller logs and continues the run.
fn spawn_remote(cwd: &Path, name: &str, raw: &McpServerConfigRaw) -> anyhow::Result<crate::mcp_http::HttpMcpServer> {
    let url = raw.url.clone().context("remote server requires url")?;
    let mut headers = Vec::new();
    if let Some(hdrs) = &raw.headers {
        for (k, v) in hdrs {
            let expanded = expand_env_vars(v)
                .with_context(|| format!("missing env var in header {k}"))?;
            headers.push((k.clone(), expanded));
        }
    }
    let mut srv = crate::mcp_http::HttpMcpServer::new(name.to_string(), url, headers)
        .context("building http mcp client")?;
    // Same per-server log file as stdio servers: listen-stream notes
    // (dropped notifications, reconnects, 405) land there too.
    srv.set_log_path(cwd.join(".chug").join(format!("mcp-{name}.log")));
    srv.initialize()?;
    Ok(srv)
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
        if let Some(t) = &raw.transport
            && t != "http"
        {
            return Some(format!("unsupported transport: {t}"));
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
pub struct McpServerConfigRaw {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

/// T138 (credentials leg): the environment a spawned stdio MCP server
/// inherits — a fixed key allowlist read from the parent, NOT the parent
/// environment itself. The parent env carries credentials (API keys,
/// tokens); a repository-controlled mcp.json must not receive them just by
/// naming a command. The config entry's `env` map opts specific variables
/// back in per server. Keys are chosen so `"command": "python3"` resolves
/// and interpreters with a profile-dir expectation still run.
fn baseline_env() -> Vec<(&'static str, std::ffi::OsString)> {
    #[cfg(windows)]
    const BASELINE_KEYS: &[&str] = &[
        "PATH", "HOME", "TMPDIR", "LANG", "LC_ALL", "SystemRoot", "TEMP", "TMP",
        "USERPROFILE", "APPDATA", "LOCALAPPDATA",
    ];
    #[cfg(not(windows))]
    const BASELINE_KEYS: &[&str] = &["PATH", "HOME", "TMPDIR", "LANG", "LC_ALL"];
    BASELINE_KEYS
        .iter()
        .filter_map(|k| std::env::var_os(k).map(|v| (*k, v)))
        .collect()
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
        // T138 (credentials leg): a repository-controlled mcp.json must not
        // receive the parent environment by default — it carries API keys
        // and tokens the spawned process could exfiltrate. Spawn with a
        // fixed minimal baseline (PATH/HOME/... so interpreters resolve)
        // plus exactly the `env` map the entry configures; everything else
        // stays out unless the config names it explicitly.
        cmd.env_clear();
        for (key, value) in baseline_env() {
            cmd.env(key, value);
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
                // v1 limitation: a STRING id fails as_u64 and falls to the
                // notification arm (dropped without a -32601 reply).
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
            capabilities: McpCapabilities::default(),
            log_path,
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
            // cannot shadow a late response's id, and say so. The message
            // names the server: registry-level resource errors must name it
            // (F11 phase 1a), and the tools path benefits equally.
            Err(RecvTimeoutError::Timeout) => {
                lock(&self.pending).remove(&request_id);
                bail!(
                    "mcp server {} request {method} timed out after {}s",
                    self.name,
                    timeout.as_secs()
                );
            }
            Err(RecvTimeoutError::Disconnected) => {
                bail!("mcp response channel closed unexpectedly");
            }
        }
    }

    /// `initialize` → `notifications/initialized` → `tools/list`. Any failure
    /// aborts the handshake; the caller drops the server (fail-soft). The
    /// initialize response's `capabilities` object is parsed into the shared
    /// [`McpCapabilities`] catalog (F11 phase 1a): it decides whether this
    /// server is ever sent resources legs.
    fn initialize(&mut self) -> anyhow::Result<()> {
        let params = json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": { "name": "chug", "version": "0.1.0" }
        });
        let init_resp = self.send_request("initialize", params, INIT_TIMEOUT)?;
        self.capabilities = McpCapabilities::parse(
            init_resp
                .get("result")
                .and_then(|r| r.get("capabilities")),
        );
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

    /// Script body = fail-fast alarm prelude + the fake's own body. The
    /// alarm far outlives any test (each finishes in seconds and Drop kills
    /// the child); it exists so a STUCK child can never hold a gate
    /// hostage — at the alarm it dies, the client sees EOF, and the
    /// affected test fails loudly instead of hanging.
    fn fake_server_script(body: &str) -> String {
        format!("import signal\nsignal.alarm(120)  # T6: a wedged fake must die, not hang the suite\n{body}")
    }

    /// Write a python script and build a spawn config for it. Every fake
    /// gets an overall self-termination alarm (T6): a wedged child dies on
    /// its own — the pipe EOF marks it down and the test fails fast —
    /// instead of blocking the suite on a silent pipe forever.
    fn server_config(dir: &Path, name: &str, body: &str) -> McpServerConfigRaw {
        let py = dir.join(format!("{name}_srv.py"));
        fs::write(&py, fake_server_script(body)).unwrap();
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
        fs::write(&py, fake_server_script(body)).unwrap();
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

        let mut reg = McpRegistry::new(tmp.path(), false, Some(flag_path)).unwrap();
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name(), "flagserver");
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
        match McpRegistry::new(tmp.path(), false, Some(missing)) {
            Ok(_) => panic!("missing explicit config must be a loud error"),
            Err(err) => assert!(err.to_string().contains("no such file"), "{err}"),
        }
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
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());
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
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());

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
        fs::write(&py, fake_server_script(echo_server_body())).unwrap();
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

    // ---------- F11 phase 1a (T162): resources consume legs ----------

    /// Resources stub: advertises tools + resources{listChanged} + prompts,
    /// serves resources/list (250 entries — over MAX_MCP_RESOURCES) and
    /// resources/read (a text resource, a blob resource, JSON-RPC error for
    /// unknown uris).
    fn resources_server_body() -> &'static str {
        r#"
import sys, json
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
RESOURCES = [{"uri": "mem://r%d" % i, "name": "r%d" % i, "mimeType": "text/plain", "description": "resource %d" % i} for i in range(250)]
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({"jsonrpc": "2.0", "id": i, "result": {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}, "resources": {"listChanged": True}, "prompts": {}}, "serverInfo": {"name": "fake", "version": "0.0.1"}}})
    elif m == "tools/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}})
    elif m == "resources/list":
        send({"jsonrpc": "2.0", "id": i, "result": {"resources": RESOURCES}})
    elif m == "resources/read":
        uri = req["params"].get("uri")
        if uri == "mem://greeting":
            send({"jsonrpc": "2.0", "id": i, "result": {"contents": [{"uri": uri, "mimeType": "text/plain", "text": "hello from the stub"}]}})
        elif uri == "mem://bytes":
            send({"jsonrpc": "2.0", "id": i, "result": {"contents": [{"uri": uri, "mimeType": "application/octet-stream", "blob": "aGVsbG8="}]}})
        else:
            send({"jsonrpc": "2.0", "id": i, "error": {"code": -32602, "message": "unknown resource: " + str(uri)}})
    else:
        send({"jsonrpc": "2.0", "id": i, "error": {"code": -32601, "message": "method not found: " + m}})
"#
    }

    /// Tools-only stub (echo capabilities) that appends EVERY received
    /// method name to a log file — the (e) witness: the test reads the file
    /// at the stub side and asserts zero resources/list requests.
    fn method_log_body(requests: &Path) -> String {
        format!(
            r#"
import sys, json, pathlib
LOG = pathlib.Path({requests:?})
def send(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    if "method" in req:
        with LOG.open("a") as f:
            f.write(req["method"] + "\n")
    if "method" not in req or "id" not in req:
        continue
    m, i = req["method"], req["id"]
    if m == "initialize":
        send({{"jsonrpc": "2.0", "id": i, "result": {{"protocolVersion": "2025-06-18", "capabilities": {{"tools": {{}}}}, "serverInfo": {{"name": "fake", "version": "0.0.1"}}}}}})
    elif m == "tools/list":
        send({{"jsonrpc": "2.0", "id": i, "result": {{"tools": []}}}})
    else:
        send({{"jsonrpc": "2.0", "id": i, "error": {{"code": -32601, "message": "method not found: " + m}}}})
"#,
            requests = requests
        )
    }

    /// (a) The capability catalog captures what the server advertised —
    /// and stays all-false for a server advertising only tools.
    #[test]
    fn capabilities_captured_when_advertised_and_absent_when_not() {
        let tmp = TempDir::new().unwrap();
        let mut capable = McpServer::spawn(
            tmp.path(),
            "capable",
            server_config(tmp.path(), "capable", resources_server_body()),
        )
        .unwrap();
        capable.initialize().unwrap();
        let caps = capable.capabilities();
        assert!(caps.tools, "{caps:?}");
        assert!(caps.resources, "{caps:?}");
        assert!(caps.prompts, "{caps:?}");
        assert!(caps.resources_list_changed, "{caps:?}");
        assert!(!caps.prompts_list_changed, "{caps:?}");
        assert!(!caps.tools_list_changed, "{caps:?}");
        drop(capable);

        // The plain echo server advertises {"tools": {}} — resources and
        // prompts absent means false.
        let srv = spawn_echo(tmp.path());
        let caps = srv.capabilities();
        assert!(caps.tools);
        assert!(!caps.resources);
        assert!(!caps.prompts);
    }

    /// (a, unit) Parse rules: presence is the key existing (even `{}`);
    /// absent object → all false; listChanged only when the flag is there.
    #[test]
    fn capabilities_parse_presence_and_list_changed_flags() {
        assert_eq!(McpCapabilities::parse(None), McpCapabilities::default());
        assert_eq!(
            McpCapabilities::parse(Some(&json!({}))),
            McpCapabilities::default()
        );
        let caps = McpCapabilities::parse(Some(&json!({
            "tools": {"listChanged": true},
            "resources": {},
            "prompts": {"listChanged": true}
        })));
        assert!(caps.tools && caps.tools_list_changed);
        assert!(caps.resources && !caps.resources_list_changed);
        assert!(caps.prompts && caps.prompts_list_changed);
    }

    /// (b) resources/list parses into McpResource values and is capped at
    /// MAX_MCP_RESOURCES with the warn-and-cap note in the per-server log
    /// (same semantics as the HTTP MAX_MCP_TOOLS cap).
    #[test]
    fn resources_list_parses_and_caps_at_max_with_warning() {
        let tmp = TempDir::new().unwrap();
        let mut srv = McpServer::spawn(
            tmp.path(),
            "fake",
            server_config(tmp.path(), "fake", resources_server_body()),
        )
        .unwrap();
        srv.initialize().unwrap();
        let resources = McpBackend::list_resources(&mut srv).unwrap();
        assert_eq!(
            resources.len(),
            MAX_MCP_RESOURCES,
            "the stub advertises 250; the registry must cap at {MAX_MCP_RESOURCES}"
        );
        assert_eq!(resources[0].uri, "mem://r0");
        assert_eq!(resources[0].name, "r0");
        assert_eq!(resources[0].description, "resource 0");
        assert_eq!(resources[0].mime_type, "text/plain");
        // warn-and-cap: the cap note lands in the per-server log.
        let log =
            fs::read_to_string(tmp.path().join(".chug").join("mcp-fake.log")).unwrap_or_default();
        assert!(
            log.contains("capped at"),
            "expected the cap warning in the server log, got: {log}"
        );
        assert!(log.contains("250 resources"), "{log}");
    }

    /// (c) resources/read maps text AND blob contents.
    #[test]
    fn resources_read_maps_text_and_blob_contents() {
        let tmp = TempDir::new().unwrap();
        let mut srv = McpServer::spawn(
            tmp.path(),
            "fake",
            server_config(tmp.path(), "fake", resources_server_body()),
        )
        .unwrap();
        srv.initialize().unwrap();
        let contents = McpBackend::read_resource(&mut srv, "mem://greeting").unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].uri, "mem://greeting");
        assert_eq!(contents[0].mime_type, "text/plain");
        assert_eq!(contents[0].text.as_deref(), Some("hello from the stub"));
        assert!(contents[0].blob.is_none());

        let contents = McpBackend::read_resource(&mut srv, "mem://bytes").unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0].blob.as_deref(), Some("aGVsbG8="));
        assert!(contents[0].text.is_none());
    }

    /// (c, unit) The shared read mapping: text and blob blocks, empty
    /// mimeType when absent, and a named error when the contents array is
    /// missing.
    #[test]
    fn resource_contents_parse_maps_blocks_and_errors() {
        let resp = json!({"result": {"contents": [
            {"uri": "mem://a", "mimeType": "text/plain", "text": "hi"},
            {"uri": "mem://b", "blob": "aGVsbG8="}
        ]}});
        let contents = parse_resource_contents("fake", &resp).unwrap();
        assert_eq!(contents[0].text.as_deref(), Some("hi"));
        assert!(contents[0].blob.is_none());
        assert_eq!(contents[1].blob.as_deref(), Some("aGVsbG8="));
        assert!(contents[1].text.is_none());
        assert_eq!(contents[1].mime_type, "");
        // Missing contents array: the error names the server.
        let err = parse_resource_contents("fake", &json!({"result": {}})).unwrap_err();
        assert!(err.to_string().contains("fake"), "{err}");
        assert!(err.to_string().contains("contents"), "{err}");
        // A JSON-RPC error reply surfaces the server's own message.
        let err = parse_resource_contents(
            "fake",
            &json!({"error": {"code": -32602, "message": "unknown resource: mem://x"}}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("unknown resource: mem://x"), "{err}");
    }

    /// (d) An unknown uri and a dead server both produce errors that name
    /// the server — never a panic, never a hang.
    #[test]
    fn resource_read_unknown_uri_and_dead_server_are_named_errors() {
        let tmp = TempDir::new().unwrap();
        let mut srv = McpServer::spawn(
            tmp.path(),
            "fake",
            server_config(tmp.path(), "fake", resources_server_body()),
        )
        .unwrap();
        srv.initialize().unwrap();
        let err = McpBackend::read_resource(&mut srv, "mem://nope").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("fake"), "{msg}");
        assert!(msg.contains("unknown resource: mem://nope"), "{msg}");

        // Kill the child (SIGKILL): the read degrades to the named
        // "is down" error.
        let mut child = srv.child.take().unwrap();
        let _ = child.kill();
        let _ = child.wait();
        for _ in 0..50 {
            if !srv.is_alive() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!srv.is_alive());
        let err = McpBackend::read_resource(&mut srv, "mem://greeting").unwrap_err();
        assert!(err.to_string().contains("mcp server fake is down"), "{err}");
    }

    /// (d, registry) Unknown server name and non-capable server read: named
    /// errors from the registry surface.
    #[test]
    fn registry_read_resource_names_unknown_and_noncapable_servers() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
        let err = reg.read_resource("nosuch", "mem://x").unwrap_err();
        assert!(err.to_string().contains("mcp server nosuch not found"), "{err}");
        let err = reg.read_resource("fake", "mem://x").unwrap_err();
        assert!(
            err.to_string().contains("mcp server fake does not advertise resources"),
            "{err}"
        );
    }

    /// (e) THE CAPABILITY GATE: a server WITHOUT the resources capability
    /// is never sent resources/list — asserted at the stub (zero such
    /// requests in its method log) and in the registry result (its entry
    /// carries the named error instead).
    #[test]
    fn resources_never_queried_without_capability() {
        let tmp = TempDir::new().unwrap();
        let requests = tmp.path().join("requests.log");
        write_mcp_json(tmp.path(), "fake", &method_log_body(&requests));
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);

        let out = reg.list_resources();
        assert_eq!(out.len(), 1, "every started server appears exactly once");
        assert_eq!(out[0].server, "fake");
        let err = out[0].resources.as_ref().unwrap_err();
        assert!(
            err.to_string().contains("mcp server fake does not advertise resources"),
            "{err}"
        );
        // A targeted read is gated the same way.
        let err = reg.read_resource("fake", "mem://x").unwrap_err();
        assert!(err.to_string().contains("does not advertise resources"), "{err}");

        // The stub-side witness: tools were spoken, resources never were.
        let seen = fs::read_to_string(&requests).unwrap_or_default();
        assert!(seen.contains("initialize"), "{seen}");
        assert!(seen.contains("tools/list"), "{seen}");
        assert!(
            !seen.contains("resources/list"),
            "a server without the resources capability was queried: {seen}"
        );
    }

    /// Registry routing end-to-end on a CAPABLE server: list_resources()
    /// aggregates the capped list, read_resource(server, uri) returns the
    /// text contents — errors name the server on both paths.
    #[test]
    fn registry_lists_and_reads_capable_server() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", resources_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);

        let out = reg.list_resources();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].server, "fake");
        let resources = out[0].resources.as_ref().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(resources.len(), MAX_MCP_RESOURCES);
        assert_eq!(resources[0].uri, "mem://r0");

        let contents = reg.read_resource("fake", "mem://greeting").unwrap();
        assert_eq!(contents[0].text.as_deref(), Some("hello from the stub"));
    }

    /// Phase-1b honesty is covered where the HTTP backend lives: the
    /// transport captures the shared capability catalog but speaks no
    /// resource legs yet (see mcp_http.rs tests).
    #[test]
    fn capabilities_parse_is_the_shared_shape_between_transports() {
        // The HTTP transport parses the SAME struct (mcp_http.rs initialize);
        // pin the parse shape once here so a transport fork fails loudly.
        let caps = McpCapabilities::parse(Some(&json!({"resources": {"listChanged": true}})));
        assert!(caps.resources && caps.resources_list_changed);
        assert!(!caps.tools && !caps.prompts);
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
    fn validate_config_missing_env_var() {
        use std::collections::HashMap;
        let raw = McpServerConfigRaw {
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            transport: Some("http".to_string()),
            headers: Some({
                let mut m = HashMap::new();
                m.insert("Authorization".to_string(), "Bearer ${MISSING_VAR}".to_string());
                m
            }),
        };
        let err = validate_config(&raw).expect("should error");
        assert!(err.contains("missing env var in header Authorization"));
    }

    #[test]
    fn validate_config_bad_transport() {
        let raw = McpServerConfigRaw {
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            transport: Some("ws".to_string()),
            headers: None,
        };
        let err = validate_config(&raw).expect("should error");
        assert_eq!(err, "unsupported transport: ws");
    }

    #[test]
    fn validate_config_url_and_command_conflict() {
        let raw = McpServerConfigRaw {
            command: Some("python3".to_string()),
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            transport: None,
            headers: None,
        };
        let err = validate_config(&raw).expect("should error");
        assert_eq!(err, "server cannot be both remote and stdio");
    }

    #[test]
    fn validate_config_neither_url_nor_command() {
        let raw = McpServerConfigRaw {
            command: None,
            args: None,
            env: None,
            url: None,
            transport: None,
            headers: None,
        };
        let err = validate_config(&raw).expect("should error");
        assert_eq!(err, "stdio server requires command");
    }

    #[test]
    fn expand_env_vars_direct() {
        unsafe { std::env::set_var("X", "abc123"); }
        assert_eq!(expand_env_vars("Bearer ${X}"), Some("Bearer abc123".to_string()));
        unsafe { std::env::set_var("A", "1"); std::env::set_var("B", "2"); }
        assert_eq!(expand_env_vars("${A}-${B}"), Some("1-2".to_string()));
        assert_eq!(expand_env_vars("literal $ not braces"), Some("literal $ not braces".to_string()));
        assert_eq!(expand_env_vars("${MISSING}"), None);
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

        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());
        // remote skipped due to missing env var, stdio loaded
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name(), "stdio-srv");
        // verify log contains config error
        let log_path = tmp.path().join(".chug").join("mcp-remote-srv.log");
        let content = fs::read_to_string(&log_path).unwrap_or_default();
        assert!(content.contains("missing env var in header Authorization"));
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

        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name(), "stdio-srv");
        let log_path = tmp.path().join(".chug").join("mcp-remote-srv.log");
        let content = fs::read_to_string(&log_path).unwrap_or_default();
        assert!(content.contains("unsupported transport: ws"));
    }

    #[test]
    fn stdio_only_config_unchanged() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        // T138: spawn is deferred to start(), gated on permissions.
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
    }

    // ---------- T138: spawn gating + env baseline ----------

    struct NoSink;
    impl crate::events::EventSink for NoSink {
        fn emit(&mut self, _e: crate::events::Event) {}
    }

    /// Write a permissions.json deny config into `cwd/.chug/` (same shape
    /// the driver loads) and return the loaded Permissions.
    fn load_denies(cwd: &Path, deny: Value) -> Permissions {
        let dir = cwd.join(".chug");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("permissions.json"),
            json!({"permissions": {"deny": deny}}).to_string(),
        )
        .unwrap();
        Permissions::load(cwd, &mut NoSink)
    }

    /// Echo server body prefixed with a flag write: the flag file appears
    /// the instant the COMMAND executes, before any handshake.
    fn flag_body(flag: &Path) -> String {
        format!(
            "import pathlib\npathlib.Path({flag:?}).write_text(\"ran\")\n{}",
            echo_server_body()
        )
    }

    /// THE DEFERRAL PIN: `new()` alone must not execute anything (no flag,
    /// no server); `start()` with zero deny rules then spawns + handshakes
    /// normally (flag exists, schemas registered).
    #[test]
    fn t138_new_defers_execution_and_start_spawns_when_allowed() {
        let tmp = TempDir::new().unwrap();
        let flag = tmp.path().join("t138-flag");
        write_mcp_json(tmp.path(), "fake", &flag_body(&flag));
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        assert!(reg.servers.is_empty(), "new() must not spawn");
        assert!(!flag.exists(), "new() must not execute the command");
        assert!(reg.tool_schemas().is_empty());

        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
        assert!(flag.exists(), "start() with no deny rules must spawn");
        assert_eq!(reg.tool_schemas()[0]["name"], "mcp__fake__echo");
    }

    /// A whole-tool bash deny prevents the startup execution: the command
    /// never runs, no server registers.
    #[test]
    fn t138_deny_bash_blocks_stdio_spawn() {
        let tmp = TempDir::new().unwrap();
        let flag = tmp.path().join("t138-flag");
        write_mcp_json(tmp.path(), "fake", &flag_body(&flag));
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&load_denies(tmp.path(), json!([{"tool": "bash"}])));
        assert!(!flag.exists(), "a bash deny must prevent the mcp.json command");
        assert!(reg.servers.is_empty());
        assert!(reg.tool_schemas().is_empty());
    }

    /// A whole-namespace `mcp__*` deny prevents the startup execution too.
    #[test]
    fn t138_deny_mcp_star_blocks_stdio_spawn() {
        let tmp = TempDir::new().unwrap();
        let flag = tmp.path().join("t138-flag");
        write_mcp_json(tmp.path(), "fake", &flag_body(&flag));
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&load_denies(tmp.path(), json!([{"tool": "mcp__*"}])));
        assert!(!flag.exists(), "an mcp__* deny must prevent the mcp.json command");
        assert!(reg.servers.is_empty());
        assert!(reg.tool_schemas().is_empty());
    }

    /// A server-scoped deny (`mcp__fake__*`) blocks only that server: the
    /// sibling server (different name, same config file) still spawns.
    #[test]
    fn t138_deny_server_scoped_blocks_only_that_server() {
        let tmp = TempDir::new().unwrap();
        let fake_flag = tmp.path().join("t138-fake-flag");
        let other_flag = tmp.path().join("t138-other-flag");
        let py_fake = tmp.path().join("fake_srv.py");
        let py_other = tmp.path().join("other_srv.py");
        fs::write(&py_fake, fake_server_script(&flag_body(&fake_flag))).unwrap();
        fs::write(&py_other, fake_server_script(&flag_body(&other_flag))).unwrap();
        let cfg_json = json!({
            "mcpServers": {
                "fake": {"command": "python3", "args": [py_fake.to_string_lossy()]},
                "other": {"command": "python3", "args": [py_other.to_string_lossy()]},
            }
        });
        fs::write(
            tmp.path().join("mcp.json"),
            serde_json::to_string(&cfg_json).unwrap(),
        )
        .unwrap();
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&load_denies(tmp.path(), json!([{"tool": "mcp__fake__*"}])));
        assert!(!fake_flag.exists(), "the denied server must not have run");
        assert!(other_flag.exists(), "the sibling server must still run");
        assert_eq!(reg.servers.len(), 1);
        assert_eq!(reg.servers[0].name(), "other");
    }

    /// start() is idempotent: a second call is a no-op (the pending list is
    /// consumed by the first), never a double spawn.
    #[test]
    fn t138_start_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        write_mcp_json(tmp.path(), "fake", echo_server_body());
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&Permissions::empty());
        reg.start(&Permissions::empty());
        assert_eq!(reg.servers.len(), 1);
    }

    /// THE CREDENTIALS LEG: a spawned stdio server must NOT inherit the
    /// parent environment (API keys et al.). It sees the baseline (PATH) and
    /// exactly the `env` map the entry configures — nothing else.
    #[test]
    fn t138_stdio_spawn_does_not_inherit_parent_env() {
        let tmp = TempDir::new().unwrap();
        let dump = tmp.path().join("env-dump.json");
        let py = tmp.path().join("env_srv.py");
        let body = format!(
            "import os, pathlib, sys, json\npathlib.Path({dump:?}).write_text(json.dumps({{k: os.environ.get(k) for k in (\"T138_SECRET\", \"T138_OK\", \"PATH\")}}))\n{}",
            echo_server_body()
        );
        fs::write(&py, fake_server_script(&body)).unwrap();
        let cfg_json = json!({
            "mcpServers": {
                "envy": {
                    "command": "python3",
                    "args": [py.to_string_lossy()],
                    "env": {"T138_OK": "via-config"}
                }
            }
        });
        fs::write(
            tmp.path().join("mcp.json"),
            serde_json::to_string(&cfg_json).unwrap(),
        )
        .unwrap();
        unsafe { std::env::set_var("T138_SECRET", "hunter2") };
        let mut reg = McpRegistry::new(tmp.path(), false, None).unwrap();
        reg.start(&Permissions::empty());
        unsafe { std::env::remove_var("T138_SECRET") };
        assert_eq!(reg.servers.len(), 1, "server must spawn for the env dump");
        let seen: HashMap<String, Option<String>> =
            serde_json::from_str(&fs::read_to_string(&dump).unwrap()).unwrap();
        assert_eq!(seen["T138_SECRET"], None, "credentials must not leak: {seen:?}");
        assert_eq!(seen["T138_OK"].as_deref(), Some("via-config"), "config env must pass: {seen:?}");
        assert!(seen["PATH"].is_some(), "baseline PATH must survive: {seen:?}");
    }
}

use std::thread;
use std::time::Duration;

use anyhow::{Context, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MAX_TOKENS: u32 = 8192;
const READ_TIMEOUT_SECS: u64 = 600;
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Exponential backoff between failed attempts: 1s, 2s, 4s, 8s (4 attempts total).
const RETRY_DELAYS_SECS: [u64; 4] = [1, 2, 4, 8];

/// One content block of a message.
///
/// Known block types are deserialized with `#[serde(tag = "type")]`; anything
/// else (or a known type carrying unexpected extra fields) falls through to
/// [`ContentBlock::Other`], which preserves the raw JSON verbatim. This keeps
/// multi-turn echo valid for providers that interleave `thinking` or
/// `redacted_thinking` blocks with `text` and `tool_use`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ContentBlock {
    Known(KnownBlock),
    Other(Value),
}

/// Internally tagged ("type") variants we understand structurally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum KnownBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        #[serde(default)]
        data: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        #[serde(default)]
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        #[serde(default)]
        content: Value,
        #[serde(default)]
        is_error: bool,
    },
}

impl ContentBlock {
    /// Returns `(tool_use_id, name, input)` if this block requests a tool call.
    /// All other block types (text, thinking, unknown) are ignored.
    pub fn tool_use(&self) -> Option<(&str, &str, &Value)> {
        match self {
            ContentBlock::Known(KnownBlock::ToolUse { id, name, input }) => {
                Some((id, name, input))
            }
            _ => None,
        }
    }

    /// Returns the text payload if this is a plain text block.
    pub fn text(&self) -> Option<&str> {
        match self {
            ContentBlock::Known(KnownBlock::Text { text }) => Some(text),
            _ => None,
        }
    }

    /// Convenience constructor for a plain text block.
    pub fn text_block(text: impl Into<String>) -> Self {
        ContentBlock::Known(KnownBlock::Text { text: text.into() })
    }

    /// Convenience constructor for a tool_result block.
    pub fn tool_result_block(tool_use_id: &str, content: String, is_error: bool) -> Self {
        ContentBlock::Known(KnownBlock::ToolResult {
            tool_use_id: tool_use_id.to_string(),
            content: Value::String(content),
            is_error,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

impl Message {
    pub fn user(content: Vec<ContentBlock>) -> Self {
        Message {
            role: "user".to_string(),
            content,
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Message {
            role: "assistant".to_string(),
            content,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Client {
    http: reqwest::blocking::Client,
    base_url: String,
    api_key: Option<String>,
    auth_token: Option<String>,
    model: String,
}

/// The driver-facing slice of the Messages API client. A trait so driver and
/// chat tests can script responses without any network. `&mut self` so test
/// doubles can consume scripted responses.
pub trait Llm {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> anyhow::Result<Response>;
    fn set_model(&mut self, model: &str);
}

impl Llm for Client {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> anyhow::Result<Response> {
        Client::complete(self, system, messages, tools)
    }

    fn set_model(&mut self, model: &str) {
        Client::set_model(self, model);
    }
}

#[derive(Debug)]
pub struct Response {
    pub body: Value,
}

impl Response {
    /// Parsed content blocks. Unknown block types come back as
    /// [`ContentBlock::Other`] with their raw JSON intact.
    pub fn content_blocks(&self) -> Vec<ContentBlock> {
        match self.body.get("content") {
            Some(v) => serde_json::from_value::<Vec<ContentBlock>>(v.clone())
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// Stop reason (`end_turn`, `tool_use`, ...). Used by tests and diagnostics;
    /// the driver loop itself keys off tool_use blocks rather than this field.
    #[allow(dead_code)]
    pub fn stop_reason(&self) -> Option<String> {
        self.body
            .get("stop_reason")
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    pub fn text(&self) -> String {
        self.content_blocks()
            .iter()
            .filter_map(ContentBlock::text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Client {
    pub fn new(model: &str) -> anyhow::Result<Self> {
        // Endpoint + credentials: process env first, then the `env` block of
        // ~/.claude/settings.json, then the api.anthropic.com default
        // (base URL only). Resolution lives in auth.rs.
        let ep = crate::auth::resolve_endpoint()?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(READ_TIMEOUT_SECS))
            .use_rustls_tls()
            .build()
            .context("building HTTP client")?;
        Ok(Self {
            http,
            base_url: ep.base_url,
            api_key: ep.api_key,
            auth_token: ep.auth_token,
            model: model.to_string(),
        })
    }

    /// Test-only constructor that skips credential checks (no network is ever
    /// attempted by the constructor itself).
    #[cfg(test)]
    pub fn new_without_credentials(model: &str) -> anyhow::Result<Self> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(READ_TIMEOUT_SECS))
                .use_rustls_tls()
                .build()
                .context("building HTTP client")?,
            base_url: crate::auth::DEFAULT_BASE_URL.to_string(),
            api_key: None,
            auth_token: None,
            model: model.to_string(),
        })
    }

    /// Swap the model id used by subsequent `complete` calls (chat `/model`).
    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    #[cfg(test)]
    pub fn model(&self) -> &str {
        &self.model
    }

    /// One non-streaming Messages API call with retry/backoff on non-200.
    pub fn complete(
        &self,
        system: &str,
        messages: &[Message],
        tools: &[Value],
    ) -> anyhow::Result<Response> {
        let url = format!("{}/v1/messages", self.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": system,
            "messages": messages,
            "tools": tools,
            "tool_choice": { "type": "auto" },
        });

        let mut last_error = String::new();
        let total_attempts = RETRY_DELAYS_SECS.len();
        for (attempt, backoff) in RETRY_DELAYS_SECS.iter().enumerate() {
            let resp = self.post(&url, &body)?;
            let status = resp.status();
            if status.is_success() {
                let text = resp.text().context("reading response body")?;
                let parsed: Value = serde_json::from_str(&text)
                    .with_context(|| format!("parsing response JSON: {}", preview(&text, 500)))?;
                return Ok(Response { body: parsed });
            }
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok());
            let err_text = resp.text().unwrap_or_default();
            last_error = format!("HTTP {status}: {}", preview(&err_text, 1000));
            if attempt == total_attempts - 1 {
                break;
            }
            let delay = retry_after.unwrap_or(*backoff);
            thread::sleep(Duration::from_secs(delay));
        }
        bail!(
            "LLM request failed after {} attempts; last error: {last_error}",
            RETRY_DELAYS_SECS.len()
        );
    }

    fn post(&self, url: &str, body: &Value) -> anyhow::Result<reqwest::blocking::Response> {
        let mut req = self
            .http
            .post(url)
            .header("anthropic-version", ANTHROPIC_VERSION);
        if let Some(key) = &self.api_key {
            req = req.header("x-api-key", key);
        }
        if let Some(token) = &self.auth_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        Ok(req.json(body).send()?)
    }
}

fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

/// Test double: replays scripted response bodies and records every call, so
/// driver/chat state-machine tests run without network.
#[cfg(test)]
pub struct ScriptedLlm {
    pub responses: std::collections::VecDeque<Value>,
    pub calls: Vec<(String, Vec<Message>)>,
    pub model: String,
    /// When set, every `complete` call raises this flag before returning —
    /// lets tests trigger a deterministic mid-turn operator interrupt.
    pub abort_on_call: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
}

#[cfg(test)]
impl ScriptedLlm {
    pub fn new(responses: Vec<Value>) -> Self {
        ScriptedLlm {
            responses: responses.into(),
            calls: Vec::new(),
            model: "scripted-model".to_string(),
            abort_on_call: None,
        }
    }
}

#[cfg(test)]
impl Llm for ScriptedLlm {
    fn complete(
        &mut self,
        system: &str,
        messages: &[Message],
        _tools: &[Value],
    ) -> anyhow::Result<Response> {
        self.calls.push((system.to_string(), messages.to_vec()));
        if let Some(flag) = &self.abort_on_call {
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let body = self
            .responses
            .pop_front()
            .ok_or_else(|| anyhow::anyhow!("ScriptedLlm: no scripted response left"))?;
        Ok(Response { body })
    }

    fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_parses_tool_use_ignoring_other_blocks() {
        let resp = Response {
            body: json!({
                "stop_reason": "tool_use",
                "content": [
                    {"type": "thinking", "thinking": "let me look", "signature": "sig1"},
                    {"type": "text", "text": "hello"},
                    {"type": "tool_use", "id": "tu_1", "name": "bash", "input": {"command": "ls"}},
                    {"type": "brand_new_future_block", "payload": {"a": 1}}
                ]
            }),
        };
        let blocks = resp.content_blocks();
        assert_eq!(blocks.len(), 4);
        // thinking block round-trips verbatim enough to re-serialize with its signature
        match &blocks[0] {
            ContentBlock::Known(KnownBlock::Thinking { thinking, signature }) => {
                assert_eq!(thinking, "let me look");
                assert_eq!(signature.as_deref(), Some("sig1"));
            }
            other => panic!("expected thinking block, got {other:?}"),
        }
        // unknown block type falls back to raw JSON, never errors
        match &blocks[3] {
            ContentBlock::Other(v) => {
                assert_eq!(v["payload"]["a"], 1);
            }
            other => panic!("expected raw fallback, got {other:?}"),
        }
        // only the tool_use block is surfaced for dispatch
        let tool_uses: Vec<_> = blocks.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(tool_uses.len(), 1);
        assert_eq!(tool_uses[0].0, "tu_1");
        assert_eq!(tool_uses[0].1, "bash");
        assert_eq!(tool_uses[0].2["command"], "ls");
        assert_eq!(resp.text(), "hello");
        assert_eq!(resp.stop_reason().as_deref(), Some("tool_use"));
    }

    #[test]
    fn set_model_swaps_the_model_id() {
        let mut client = Client::new_without_credentials("first-model").unwrap();
        assert_eq!(client.model(), "first-model");
        client.set_model("second-model");
        assert_eq!(client.model(), "second-model");
    }

    #[test]
    fn message_serialization_round_trips() {
        let msg = Message::user(vec![
            ContentBlock::text_block("kick"),
            ContentBlock::tool_result_block("tu_1", "output".into(), true),
            ContentBlock::Known(KnownBlock::RedactedThinking {
                data: "xyz".into(),
            }),
            ContentBlock::Other(json!({"type": "brand_new_block", "payload": [1]})),
        ]);
        let text = serde_json::to_string(&msg).unwrap();
        assert_eq!(serde_json::from_str::<Message>(&text).unwrap(), msg);
        assert!(text.contains("\"type\":\"tool_result\""));
        assert!(text.contains("\"tool_use_id\":\"tu_1\""));
        assert!(text.contains("\"is_error\":true"));
    }
}

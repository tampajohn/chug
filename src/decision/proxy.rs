//! T332 — the PROXY decision provider: the company tools-proxy's hosted
//! System One surface (`/v1/systemone`), implemented against the VERIFIED
//! live contract in `docs/decision-providers.md` (probed 2026-11-07) — not
//! a guess. The wire shape is a drop-in for the daemon's `/judge`: same
//! `{state, questions}` body plus a REQUIRED `model` field, same
//! `{answers:{…}}` envelope, plus per-call `usage.cost`.

use std::time::Duration;

use serde_json::{Value, json};

use super::{PROXY_TOKEN_ENV, PROXY_URL_ENV, DecisionProvider, log_decision_call};

/// The proxy call bound (the recorded contract's 10s; the local daemon
/// keeps its own 2s — the two transports have different latencies).
pub const PROXY_TIMEOUT_SECS: u64 = 10;
/// The hosted decision model the proxy exposes (the operator's name for
/// it; the canonical id also works per the recorded contract). A `model`
/// field the caller already set is honored verbatim.
pub const PROXY_MODEL: &str = "jev";

/// The `usage` figures the proxy envelope carries (the recorded shape:
/// `{"usage":{"input_tokens":N,"output_tokens":N,"cost":<dollars>,…}}`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProxyUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    /// The per-call dollar figure — the Langfuse `decision`-tag legibility
    /// number. Absent when the endpoint omits it.
    pub cost: Option<f64>,
}

/// Extract [`ProxyUsage`] from a response envelope; `None` when the
/// envelope carries no readable usage block (logging stays best-effort).
pub fn proxy_usage_of(envelope: &Value) -> Option<ProxyUsage> {
    let usage = envelope.get("usage")?;
    Some(ProxyUsage {
        input_tokens: usage.get("input_tokens").and_then(Value::as_u64)?,
        output_tokens: usage.get("output_tokens").and_then(Value::as_u64)?,
        cost: usage.get("cost").and_then(Value::as_f64),
    })
}

/// The hosted proxy provider (the second provider). Constructed from
/// `CHUG_DECISION_PROXY_URL` / `CHUG_DECISION_PROXY_TOKEN`; construction
/// NEVER fails on missing config — per the recorded contract, a proxy call
/// without a configured endpoint/token fails the CALL SITE's existing
/// degrade path (T259 fail-open, T326 fail-closed, risk gate fail-open per
/// command), not the run.
pub struct ProxySystemOne {
    /// The base URL (`<base>/v1/systemone` is posted). `None` = not
    /// configured: every call fails with the named fix.
    url: Option<String>,
    /// The Bearer credential. `None` is legal at the wire level (the
    /// endpoint answers 401 = the call site's degrade path).
    token: Option<String>,
    http: reqwest::blocking::Client,
}

impl ProxySystemOne {
    /// Read the config env ONCE (the one env read — `judge_envelope` never
    /// re-reads process-global env, the mcp_serve.rs env rule).
    pub fn from_env() -> Self {
        let url = std::env::var(PROXY_URL_ENV)
            .ok()
            .map(|u| u.trim().to_string())
            .filter(|u| !u.is_empty());
        let token = std::env::var(PROXY_TOKEN_ENV)
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        Self::new(url, token)
    }

    /// Explicit construction (tests + explicit operator wiring).
    pub fn new(url: Option<String>, token: Option<String>) -> Self {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(PROXY_TIMEOUT_SECS))
            .build()
            .unwrap_or_default();
        Self { url, token, http }
    }

    /// True when a proxy endpoint is configured (the auto matrix's "proxy
    /// when configured" test — the token is NOT required to count).
    pub fn configured() -> bool {
        std::env::var(PROXY_URL_ENV)
            .ok()
            .map(|u| !u.trim().is_empty())
            .unwrap_or(false)
    }

    /// The configured endpoint (status lines name it; never the token).
    pub fn endpoint(&self) -> Option<String> {
        self.url.clone()
    }
}

impl DecisionProvider for ProxySystemOne {
    fn judge_envelope(&mut self, body: &Value) -> Result<Value, String> {
        let Some(url) = &self.url else {
            return Err(format!(
                "decision proxy not configured (set {PROXY_URL_ENV})"
            ));
        };
        // The proxy contract REQUIRES the model field; add the hosted
        // decision model when the caller did not name one (the daemon
        // never sees this field — LocalDaemon posts the body as-is).
        let mut wire = body.clone();
        if wire.get("model").and_then(Value::as_str).unwrap_or_default().is_empty() {
            wire["model"] = json!(PROXY_MODEL);
        }
        let mut req = self
            .http
            .post(format!("{}/v1/systemone", url.trim_end_matches('/')))
            .header("Content-Type", "application/json")
            .json(&wire);
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let resp = req
            .send()
            .map_err(|e| format!("decision proxy request failed: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(format!("decision proxy returned HTTP {}", status.as_u16()));
        }
        let envelope: Value = resp
            .json()
            .map_err(|e| format!("decision proxy response is not valid JSON: {e}"))?;
        // Cost legibility (T332 req 4): one best-effort Langfuse trace
        // tagged `decision` per successful call — the T327 dashboard sees
        // the spend; this never gates and is a no-op with Langfuse off.
        log_decision_call("proxy", PROXY_MODEL, proxy_usage_of(&envelope).as_ref());
        Ok(envelope)
    }

    fn describe(&self) -> String {
        match &self.url {
            Some(url) => format!("proxy {url}/v1/systemone"),
            None => "proxy (unconfigured)".into(),
        }
    }
}

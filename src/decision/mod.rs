//! T332 — the decision-model provider abstraction: proxy Jev alongside
//! local Laya, so a pilot box needs no daemon before the loop gets its
//! cheapest superpowers.
//!
//! Two providers speak the same wire shape (docs/decision-providers.md —
//! the VERIFIED live contract; the proxy's `/v1/systemone` is a drop-in for
//! the daemon's `/judge`): [`LocalDaemon`] (the existing chugd unix-socket
//! transport, reused unchanged via [`crate::daemon`]) and
//! [`ProxySystemOne`] (the company tools-proxy over HTTPS, `model` field +
//! Bearer auth + the 10s bound). Selection is `CHUG_DECISION_PROVIDER`
//! (`auto|local|proxy`, the T330 profile key) with the proxy endpoint/token
//! on `CHUG_DECISION_PROXY_URL` / `CHUG_DECISION_PROXY_TOKEN`:
//!
//! * `auto` (the default) — the local daemon socket when it answers
//!   `/healthz`, else the proxy when configured, else the call site's
//!   existing no-provider behavior;
//! * `local` — the daemon only; `proxy` — the proxy only (a proxy call
//!   without a configured endpoint/token fails the call site's existing
//!   degrade path, per the recorded contract).
//!
//! The provider never decides policy: it returns the response envelope or
//! an error, and each call site keeps its own degrade semantics (the T259
//! triage fail-opens, the T326 noul gate fails closed, the risk gate
//! fail-opens per command). The shell + ideate.py call sites mirror this
//! exact table (loopd.sh `decision_provider` / scripts/ideate.py
//! `noul_provider`) — one contract, three transports.
//!
//! Cost legibility: every successful proxy call emits ONE Langfuse trace
//! tagged `decision` ([`decision_trace_body`]) carrying `usage.cost` — the
//! T327 dashboard sees the spend; no free invisible cost.

mod local;
mod proxy;

pub use local::LocalDaemon;
pub use proxy::{ProxySystemOne, ProxyUsage};

use serde_json::{Value, json};

/// `CHUG_DECISION_PROVIDER` — the decision-layer selection key
/// (`auto|local|proxy`; unset/blank = `auto`, the documented default).
pub const PROVIDER_ENV: &str = "CHUG_DECISION_PROVIDER";
/// `CHUG_DECISION_PROXY_URL` — the proxy base URL (`<base>/v1/systemone` is
/// posted). The T330 profile key; never committed (the T249 lint + scrub).
pub const PROXY_URL_ENV: &str = "CHUG_DECISION_PROXY_URL";
/// `CHUG_DECISION_PROXY_TOKEN` — the proxy credential (`Authorization:
/// Bearer …`). Optional at the wire level (a tokenless proxy is a 401 per
/// call = the call site's degrade path, never a construction error).
pub const PROXY_TOKEN_ENV: &str = "CHUG_DECISION_PROXY_TOKEN";

/// Which provider [`decision_mode`] selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionMode {
    /// The auto matrix: healthy local socket, else configured proxy, else
    /// the call site's existing no-provider behavior.
    Auto,
    /// The local laya daemon only (free/offline stays).
    Local,
    /// The hosted proxy only.
    Proxy,
}

/// The PURE `CHUG_DECISION_PROVIDER` selection table ([`provider_from_env`]
/// and the call sites compose from it; the risk gate's own `CHUG_JUDGE`
/// table stays the risk gate's). Unset, empty, or blank defaults to
/// [`DecisionMode::Auto`]; values are case/whitespace tolerant (matching
/// the `CHUG_JUDGE` trim+lowercase rule). Any other value is a HARD ERROR
/// naming the valid modes — an unknown mode must refuse rather than
/// silently pick a provider the operator did not ask for (the `CHUG_JUDGE`
/// garbage rule).
pub fn decision_mode(raw: Option<&str>) -> Result<DecisionMode, String> {
    let normalized = raw
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase());
    match normalized.as_deref() {
        None | Some("auto") => Ok(DecisionMode::Auto),
        Some("local") => Ok(DecisionMode::Local),
        Some("proxy") => Ok(DecisionMode::Proxy),
        Some(other) => Err(format!(
            "unknown {PROVIDER_ENV} mode {other:?} (expected auto|local|proxy)"
        )),
    }
}

/// One decision-model call: POST a `{state, questions}` body, get the full
/// response envelope (`answers` + `usage`) back. `Err` is the provider
/// failing — the CALL SITE owns the degrade (fail-open, fail-closed, or
/// off, exactly as before this module existed).
pub trait DecisionProvider: Send {
    fn judge_envelope(&mut self, body: &Value) -> Result<Value, String>;
    /// A short human name for status lines and logs.
    fn describe(&self) -> String;
}

/// Resolve the configured provider per [`decision_mode`] (the auto matrix
/// included). `Ok(None)` is NO provider — every call site's existing
/// no-provider behavior stands (the T259 triage routes kimi, the T326 gate
/// rejects, the risk gate is not built this way — it composes its own
/// clients, see [`crate::riskgate::judge_from_env`]).
///
/// Auto resolves ONLY when the proxy is actually configured: with no proxy
/// endpoint set, auto collapses to the local daemon without probing
/// (today's behavior, byte for byte — the provider change must be
/// invisible in every fixture that does not configure a proxy).
// The production consult path is `judge_from_env` (riskgate), which
// carries this same table against the `Judge` trait; `provider_from_env`
// is the pure table carrier for the pinned matrix test (and mirrors
// `status_line`'s Auto branch). Dead in non-test builds by construction —
// hence the allow.
#[allow(dead_code)]
pub fn provider_from_env() -> anyhow::Result<Option<Box<dyn DecisionProvider>>> {
    let raw = std::env::var(PROVIDER_ENV).ok();
    match decision_mode(raw.as_deref()).map_err(|e| anyhow::anyhow!(e))? {
        DecisionMode::Local => Ok(Some(Box::new(LocalDaemon::from_env()?))),
        DecisionMode::Proxy => Ok(Some(Box::new(ProxySystemOne::from_env()))),
        DecisionMode::Auto => {
            if !ProxySystemOne::configured() {
                // No proxy configured: auto IS the local daemon (today's
                // default shape), no health probe, nothing observable.
                return Ok(Some(Box::new(LocalDaemon::from_env()?)));
            }
            let local = LocalDaemon::from_env()?;
            if local.healthz_ok().is_ok() {
                Ok(Some(Box::new(local)))
            } else {
                Ok(Some(Box::new(ProxySystemOne::from_env())))
            }
        }
    }
}

/// The `chug daemon --status` decision line: which provider the selection
/// table resolves to RIGHT NOW (best-effort, never gates anything — a
/// status report names the wiring, it does not probe the proxy).
pub fn status_line() -> String {
    let raw = std::env::var(PROVIDER_ENV).ok();
    let mode = match decision_mode(raw.as_deref()) {
        Ok(mode) => mode,
        Err(e) => return format!("invalid ({e})"),
    };
    match mode {
        DecisionMode::Local => match LocalDaemon::from_env() {
            Ok(local) => format!("local ({})", local.describe()),
            Err(e) => format!("local (socket unresolvable: {e:#})"),
        },
        DecisionMode::Proxy => match ProxySystemOne::from_env().endpoint() {
            Some(url) => format!("proxy ({url})"),
            None => format!("proxy (unconfigured — set {PROXY_URL_ENV})"),
        },
        DecisionMode::Auto => {
            if !ProxySystemOne::configured() {
                return "auto -> local (default; no proxy configured)".into();
            }
            match LocalDaemon::from_env() {
                Ok(local) if local.healthz_ok().is_ok() => {
                    format!("auto -> local ({})", local.describe())
                }
                _ => format!(
                    "auto -> proxy ({})",
                    ProxySystemOne::from_env().endpoint().unwrap_or_default()
                ),
            }
        }
    }
}

/// The Langfuse trace body for ONE proxy decision call: tagged
/// `["chug", "decision"]` (the T327 dashboard's filter), named after the
/// provider, carrying the model and the recorded `usage` figures
/// (`usage.cost` is the legibility figure — tiny per call, never invisible).
pub(crate) fn decision_trace_body(
    trace_id: &str,
    provider: &str,
    model: &str,
    usage: Option<&ProxyUsage>,
) -> Value {
    let mut metadata = json!({ "model": model, "provider": provider });
    if let Some(u) = usage {
        metadata["input_tokens"] = json!(u.input_tokens);
        metadata["output_tokens"] = json!(u.output_tokens);
        if let Some(cost) = u.cost {
            metadata["cost"] = json!(cost);
        }
    }
    json!({
        "id": trace_id,
        "name": format!("decision-proxy ({provider})"),
        "metadata": metadata,
        "tags": ["chug", "decision"],
        "sessionId": trace_id,
    })
}

/// Best-effort emission of [`decision_trace_body`] — a no-op unless
/// Langfuse is configured (the sink is off otherwise; this must never
/// gate, never log, never fail a decision call).
pub(crate) fn log_decision_call(provider: &str, model: &str, usage: Option<&ProxyUsage>) {
    let obs = crate::observ::global();
    if !obs.enabled() {
        return;
    }
    // A standalone trace per call (Langfuse derives the parent from the id;
    // no run-trace plumbing is threaded into the provider).
    let trace_id = format!("dec-{}", short_id());
    obs.emit(crate::observ::envelope(
        &trace_id,
        &crate::observ::now_rfc3339(),
        "trace-create",
        decision_trace_body(&trace_id, provider, model, usage),
    ));
}

/// 16 lowercase hex chars from time ⊕ pid ⊕ a process-wide counter — the
/// observ id shape, locally re-derived so the telemetry legibility leg
/// touches nothing outside this module (no new dependencies).
fn short_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut seed = nanos
        ^ (u64::from(std::process::id()) << 32)
        ^ COUNTER
            .fetch_add(1, Ordering::SeqCst)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
    // splitmix64 finalizer (the observ.rs id primitive, same constants).
    seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = seed;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    format!("{:016x}", z ^ (z >> 31))
}

#[cfg(test)]
pub(crate) mod tests;

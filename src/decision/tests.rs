//! The T332 provider pins: the PURE selection table, the recorded wire
//! contract round-trips (canned responses over a LOCAL mock — never a live
//! network call, the proxy endpoint/token never appear in any tracked
//! file), and the auto-selection matrix.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;

use serde_json::{Value, json};

use super::proxy::proxy_usage_of;
use super::*;

/// The recorded contract's response shapes (docs/decision-providers.md):
/// choice / noul / score all share the `{"answers":{…},"usage":{…}}`
/// envelope.
fn envelope(answers: &str) -> String {
    format!(
        "{{\"model\":\"typesafe/fixture\",\"answers\":{answers},\
         \"usage\":{{\"input_tokens\":42,\"output_tokens\":7,\"cost\":0.000042,\
         \"is_byok\":false}},\"id\":\"gen-dec-fixture\",\"provider\":\"TypeSafe\"}}"
    )
}

/// One canned HTTP response over a TCP socket (the mock proxy).
fn http_resp(status_line: &str, body: &str) -> String {
    format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    )
}

/// A local mock /v1/systemone server: serves `response` to every POST,
/// captures the raw request bytes, returns the bound address. Binds
/// 127.0.0.1:0 — an ephemeral LOCAL port passed via env at runtime; the
/// proxy endpoint and any token NEVER appear in a tracked file.
fn serve_proxy(response: &'static str) -> (String, std::sync::Arc<std::sync::Mutex<Vec<u8>>>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock proxy");
    let addr = listener.local_addr().expect("local addr").to_string();
    let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let cap = captured.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf);
                        if let Some(head_end) = text.find("\r\n\r\n") {
                            let want = text
                                .lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if buf.len() >= head_end + 4 + want {
                                break;
                            }
                        }
                    }
                }
            }
            *cap.lock().expect("captured map") = buf.clone();
            let resp = if response.starts_with("HTTP/") {
                response.to_string()
            } else {
                http_resp("HTTP/1.1 200 OK", response)
            };
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
    (addr, captured)
}

fn captured_string(cap: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>) -> String {
    String::from_utf8_lossy(&cap.lock().expect("captured map")).to_string()
}

/// A healthy local mock daemon socket: 200 `{"status":"ok"}` to every GET
/// /healthz (the model loads before the socket binds — healthz never lies).
pub(crate) fn serve_healthz(sock: &Path) {
    let _ = std::fs::remove_file(sock);
    let listener = UnixListener::bind(sock).expect("bind mock daemon");
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf);
            let body = "{\"status\":\"ok\"}";
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
}

/// SAFETY: set/remove one env var — serialized by [`DELEGATE_ENV_LOCK`]
/// (the crate's ONE process-global env lock, T129), saved value restored
/// by the caller's guard.
fn set_env(var: &str, value: Option<&std::ffi::OsStr>) {
    unsafe {
        match value {
            Some(v) => std::env::set_var(var, v),
            None => std::env::remove_var(var),
        }
    }
}

/// Save/restore the decision env while HOLDING the crate's one env lock
/// for the guard's whole lifetime (the T129 rule: env mutation is
/// serialized across the binary's tests, and restored on drop).
struct EnvGuard(
    // Held for its Drop side effect (the lock, not a value anyone reads).
    #[allow(dead_code)]
    std::sync::MutexGuard<'static, ()>,
    Vec<(&'static str, Option<std::ffi::OsString>)>,
);
impl EnvGuard {
    fn take() -> Self {
        let guard = crate::delegate::tests::DELEGATE_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        Self(
            guard,
            vec![
                (PROVIDER_ENV, std::env::var_os(PROVIDER_ENV)),
                (PROXY_URL_ENV, std::env::var_os(PROXY_URL_ENV)),
                (PROXY_TOKEN_ENV, std::env::var_os(PROXY_TOKEN_ENV)),
                (
                    crate::daemon::SOCK_ENV,
                    std::env::var_os(crate::daemon::SOCK_ENV),
                ),
            ],
        )
    }
    fn set(&self, var: &str, value: &str) {
        set_env(var, Some(value.as_ref()));
    }
    fn remove(&self, var: &str) {
        set_env(var, None);
    }
}
impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (var, value) in &self.1 {
            set_env(var, value.as_deref());
        }
    }
}

// --- the PURE selection table -------------------------------------------------

#[test]
fn decision_mode_table_is_pinned() {
    // Unset / empty / blank / explicit auto — all select Auto (the
    // documented default), case- and whitespace-tolerant.
    for raw in [None, Some(""), Some("   "), Some("auto"), Some("AUTO"), Some(" Auto ")] {
        assert_eq!(decision_mode(raw), Ok(DecisionMode::Auto), "raw {raw:?}");
    }
    for raw in [Some("local"), Some("LOCAL"), Some(" local ")] {
        assert_eq!(decision_mode(raw), Ok(DecisionMode::Local), "raw {raw:?}");
    }
    for raw in [Some("proxy"), Some("PROXY"), Some("Proxy ")] {
        assert_eq!(decision_mode(raw), Ok(DecisionMode::Proxy), "raw {raw:?}");
    }
    // Garbage is a HARD ERROR naming the valid modes (never a silent pick).
    for garbage in ["garbage", "daemons", "0", "auto,local", "GARBAGE"] {
        let err = decision_mode(Some(garbage)).expect_err(garbage);
        assert!(
            err.contains("unknown CHUG_DECISION_PROVIDER mode"),
            "{garbage}: {err}"
        );
        assert!(err.contains("expected auto|local|proxy"), "{garbage}: {err}");
    }
    let err = decision_mode(Some("  Bogus  ")).expect_err("bogus");
    assert!(err.contains("bogus"), "the error names the value: {err}");
}

// --- the recorded wire contract, round-tripped over the LOCAL mock ------------

#[test]
fn proxy_round_trips_the_recorded_choice_contract() {
    let _env = EnvGuard::take();
    let (addr, captured) = serve_proxy(Box::leak(
        envelope(r#"{"risk":{"type":"choice","choice":"safe","probabilities":{"destructive":0.07,"risky":0.1,"safe":0.83},"confidence":0.9}}"#)
            .into_boxed_str(),
    ));
    let mut proxy = ProxySystemOne::new(Some(format!("http://{addr}")), Some("fx-token-42".into()));
    let body = json!({
        "state": {"context": "ctx", "command": "ls"},
        "questions": {"risk": {"type": "choice", "criteria": {"safe": "s", "risky": "r", "destructive": "d"}}}
    });
    let env = proxy.judge_envelope(&body).expect("the proxy answers");
    // The envelope is returned WHOLE (answers + usage) — call sites parse it.
    assert_eq!(
        env.pointer("/answers/risk/choice").and_then(Value::as_str),
        Some("safe")
    );
    assert_eq!(
        env.pointer("/usage/cost").and_then(Value::as_f64),
        Some(0.000042)
    );
    let sent = captured_string(&captured).to_ascii_lowercase();
    assert!(sent.contains("post /v1/systemone"), "the recorded path: {sent}");
    assert!(
        sent.contains("authorization: bearer fx-token-42"),
        "bearer auth (any header case): {sent}"
    );
    assert!(sent.contains("\"model\":\"jev\""), "the REQUIRED model field: {sent}");
    assert!(sent.contains("\"state\""), "the /judge body as-is: {sent}");
}

#[test]
fn proxy_round_trips_noul_and_score_shapes_and_honors_a_set_model() {
    let _env = EnvGuard::take();
    let (addr, captured) = serve_proxy(Box::leak(
        envelope(r#"{"gate":{"type":"noul","noul":0.12}}"#).into_boxed_str(),
    ));
    let mut proxy = ProxySystemOne::new(Some(format!("http://{addr}")), None);
    let body = json!({
        "model": "jev-canonical-id",
        "state": {"proposal_title": "t", "proposal_stub": "s"},
        "questions": {"gate": {"type": "noul", "criteria": {"no": "outward", "yes": "meta"}}}
    });
    let env = proxy.judge_envelope(&body).expect("the proxy answers");
    assert_eq!(env.pointer("/answers/gate/noul").and_then(Value::as_f64), Some(0.12));
    let sent = captured_string(&captured);
    assert!(
        sent.contains("\"model\":\"jev-canonical-id\""),
        "a caller-set model is honored verbatim: {sent}"
    );
    // No token configured -> NO Authorization header at all (a tokenless
    // proxy is legal at the wire level; the 401 is the call site's degrade).
    assert!(!sent.contains("Authorization:"), "no token, no header: {sent}");

    // The score shape (criteria an ARRAY of exactly two entries) round-trips
    // too — the provider is envelope-agnostic, the doc's shapes all pass.
    let (addr2, _) = serve_proxy(Box::leak(
        envelope(r#"{"s":{"type":"score","score":0.25,"legend":{"0":"low","1":"high"},"probabilities":{"0":0.75,"1":0.25},"confidence":0.8}}"#)
            .into_boxed_str(),
    ));
    let mut proxy2 = ProxySystemOne::new(Some(format!("http://{addr2}")), None);
    let score_body = json!({
        "state": {}, "questions": {"s": {"type": "score", "criteria": ["low", "high"]}}
    });
    let env2 = proxy2.judge_envelope(&score_body).expect("score answers");
    assert_eq!(env2.pointer("/answers/s/score").and_then(Value::as_f64), Some(0.25));
}

#[test]
fn proxy_errors_are_named_and_fail_per_call_never_at_construction() {
    let _env = EnvGuard::take();
    // Not configured: construction SUCCEEDS (the recorded contract — the
    // call site's degrade path owns it) and every CALL is a named error.
    let mut proxy = ProxySystemOne::new(None, None);
    let err = proxy
        .judge_envelope(&json!({"state": {}, "questions": {}}))
        .expect_err("unconfigured");
    assert!(
        err.contains("decision proxy not configured (set CHUG_DECISION_PROXY_URL)"),
        "{err}"
    );

    // Non-2xx (the recorded 401/400/500 shapes) -> a named error.
    for (status_line, needle) in [
        ("HTTP/1.1 401 Unauthorized", "HTTP 401"),
        ("HTTP/1.1 400 Bad Request", "HTTP 400"),
        ("HTTP/1.1 500 Internal Server Error", "HTTP 500"),
    ] {
        let leaked: &'static str = Box::leak(
            format!("{status_line}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .into_boxed_str(),
        );
        let (addr, _) = serve_proxy(leaked);
        let mut proxy = ProxySystemOne::new(Some(format!("http://{addr}")), Some("t".into()));
        let err = proxy
            .judge_envelope(&json!({"state": {}, "questions": {}}))
            .expect_err(status_line);
        assert!(err.contains(needle), "{status_line}: {err}");
    }

    // Garbage JSON -> a named error (never a panic).
    let leaked: &'static str = Box::leak(http_resp("HTTP/1.1 200 OK", "not json").into_boxed_str());
    let (addr, _) = serve_proxy(leaked);
    let mut proxy = ProxySystemOne::new(Some(format!("http://{addr}")), None);
    assert!(proxy.judge_envelope(&json!({})).is_err());

    // Unreachable endpoint -> a named transport error.
    let mut proxy = ProxySystemOne::new(Some("http://127.0.0.1:1".into()), None);
    let err = proxy
        .judge_envelope(&json!({"state": {}, "questions": {}}))
        .expect_err("dead endpoint");
    assert!(err.contains("decision proxy request failed"), "{err}");
}

#[test]
fn configured_reads_only_the_url_and_usage_parses_tolerantly() {
    let _env = EnvGuard::take();
    _env.remove(PROXY_URL_ENV);
    assert!(!ProxySystemOne::configured(), "unset URL -> not configured");
    _env.set(PROXY_URL_ENV, "   ");
    assert!(!ProxySystemOne::configured(), "blank URL -> not configured");
    _env.set(PROXY_URL_ENV, "https://proxy.example.test");
    assert!(ProxySystemOne::configured(), "a URL alone configures (token optional)");

    // usage extraction: present, partial (no cost), and absent.
    let full = json!({"usage": {"input_tokens": 42, "output_tokens": 7, "cost": 0.5}});
    assert_eq!(
        proxy_usage_of(&full),
        Some(ProxyUsage { input_tokens: 42, output_tokens: 7, cost: Some(0.5) })
    );
    let no_cost = json!({"usage": {"input_tokens": 1, "output_tokens": 2}});
    assert_eq!(
        proxy_usage_of(&no_cost),
        Some(ProxyUsage { input_tokens: 1, output_tokens: 2, cost: None })
    );
    assert_eq!(proxy_usage_of(&json!({})), None, "no usage -> None (best-effort)");
}

// --- the auto-selection matrix ------------------------------------------------

/// The doc's matrix: (socket present+healthy / present+unhealthy / absent)
/// x (proxy configured / absent). Unhealthy + configured proxy -> proxy;
/// healthy -> local; absent + no proxy -> local (today's default shape);
/// and the mode table's explicit legs bypass the probe entirely.
#[test]
fn the_auto_selection_matrix_is_pinned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let _env = EnvGuard::take();
    let sock = tmp.path().join("matrix.sock");

    // absent socket + no proxy -> local (the daemon client's own shape).
    _env.remove(PROXY_URL_ENV);
    _env.set(crate::daemon::SOCK_ENV, sock.to_str().expect("sock str"));
    let provider = provider_from_env().expect("auto constructs");
    assert!(
        provider.expect("a provider").describe().contains("sock"),
        "no proxy configured: auto IS the local daemon"
    );

    // absent socket + proxy configured -> proxy.
    _env.set(PROXY_URL_ENV, "https://proxy.example.test");
    let provider = provider_from_env().expect("auto constructs");
    assert!(
        provider.expect("a provider").describe().contains("proxy"),
        "dead socket + configured proxy routes to the proxy"
    );

    // present + healthy + proxy configured -> local (the socket wins).
    serve_healthz(&sock);
    let provider = provider_from_env().expect("auto constructs");
    let described = provider.expect("a provider").describe();
    assert!(
        described.contains("sock") && !described.contains("proxy"),
        "a healthy socket beats a configured proxy: {described}"
    );

    // explicit local/proxy bypass the probe: local resolves the daemon
    // client even with a proxy configured.
    _env.set(PROVIDER_ENV, "local");
    let provider = provider_from_env().expect("local constructs");
    assert!(
        provider.expect("a provider").describe().contains("sock"),
        "explicit local never consults the proxy"
    );
    _env.set(PROVIDER_ENV, "proxy");
    let provider = provider_from_env().expect("proxy constructs");
    assert!(
        provider.expect("a provider").describe().contains("proxy"),
        "explicit proxy never probes the socket"
    );

    // garbage mode: a HARD error (never a silent provider pick).
    _env.set(PROVIDER_ENV, "garbage");
    let err = match provider_from_env() {
        Ok(_) => panic!("garbage mode must refuse, not pick"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("unknown CHUG_DECISION_PROVIDER mode"), "{err}");
}

// --- the cost-legibility trace body -------------------------------------------

#[test]
fn decision_trace_body_carries_the_decision_tag_and_cost() {
    let body = decision_trace_body(
        "dec-fixture",
        "proxy jev",
        "jev",
        Some(&ProxyUsage { input_tokens: 42, output_tokens: 7, cost: Some(0.000042) }),
    );
    assert_eq!(body["tags"][0], "chug");
    assert_eq!(body["tags"][1], "decision", "the T327 dashboard's tag");
    assert_eq!(body["id"], "dec-fixture");
    assert_eq!(body["metadata"]["model"], "jev");
    assert_eq!(body["metadata"]["cost"], 0.000042);
    assert_eq!(body["metadata"]["input_tokens"], 42);

    // No usage recorded: the metadata carries the model, no cost figures.
    let bare = decision_trace_body("dec-fixture", "proxy jev", "jev", None);
    assert_eq!(bare["tags"][1], "decision");
    assert!(bare["metadata"].get("cost").is_none());
}

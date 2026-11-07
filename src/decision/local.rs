//! T332 — the LOCAL decision provider: the existing chugd laya daemon over
//! its 0600 unix socket, reused UNCHANGED. The transport is
//! [`crate::daemon::uds_request`] (the hand-rolled HTTP/1.1 client the
//! `DaemonJudge` has always used) and the socket resolution is
//! [`crate::daemon::sock_path`] (`$CHUG_DAEMON_SOCK`, then `$CHUG_HOME`,
//! then `~/.chug/daemon.sock`) — nothing about the daemon or its client
//! moves; this module only NAMES the provider so the selection table has a
//! local leg and the proxy has a sibling to fall back from.

use std::path::PathBuf;

use serde_json::Value;

use super::DecisionProvider;

/// The local laya daemon (the first provider, free + offline, staying).
pub struct LocalDaemon {
    sock: PathBuf,
}

impl LocalDaemon {
    /// Resolve the daemon socket the same way every existing consumer does
    /// (the T204 resolution order, via [`crate::daemon::sock_path`]).
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            sock: crate::daemon::sock_path()?,
        })
    }

    /// Test seam: a provider pinned to one explicit socket path.
    #[allow(dead_code)]
    pub fn at(sock: PathBuf) -> Self {
        Self { sock }
    }

    /// The auto-selection health probe: `GET /healthz` over the socket; 200
    /// means a warm daemon (the model loads BEFORE the socket binds, so
    /// healthz never lies about readiness — [`crate::daemon`]'s own
    /// contract, probed through the same transport).
    pub fn healthz_ok(&self) -> Result<String, String> {
        let (status, body) = crate::daemon::uds_request(&self.sock, "GET", "/healthz", None)?;
        if status == 200 {
            Ok(body)
        } else {
            Err(format!("healthz returned HTTP {status}"))
        }
    }

    /// Test seam: the resolved socket path (fixture assertions).
    #[allow(dead_code)]
    pub fn socket(&self) -> &std::path::Path {
        &self.sock
    }
}
impl DecisionProvider for LocalDaemon {
    fn judge_envelope(&mut self, body: &Value) -> Result<Value, String> {
        let raw = serde_json::to_string(body)
            .map_err(|e| format!("serializing the judge request: {e}"))?;
        let (status, resp) = crate::daemon::uds_request(&self.sock, "POST", "/judge", Some(&raw))?;
        if status != 200 {
            return Err(format!("judge daemon returned HTTP {status}"));
        }
        serde_json::from_str(&resp)
            .map_err(|e| format!("judge daemon response is not valid JSON: {e}"))
    }

    fn describe(&self) -> String {
        format!("sock {}", self.sock.display())
    }
}

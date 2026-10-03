//! T205: the chug-side HF consumption path for laya checkpoints — the
//! endpoint-agnostic, token-honest half of the daemon's checkpoint fetch.
//! Compiled in EVERY build (unlike `judge_model`, which is `daemon`-feature
//! gated): the revision parsing, endpoint/token resolution and auth-error
//! classification here are pure env/string logic, so their unit pins run in
//! the plain `cargo test` gate.
//!
//! What lives here and why:
//!   * `parse_hub_ref` — `CHUG_LAYA_CHECKPOINT` accepts `org/model@REV`
//!     (a pinned revision: reproducibility for a policy-affecting artifact).
//!   * `hub_endpoint` / `hub_token` — verified against hf-hub 0.4.3: the
//!     crate reads NEITHER `HF_TOKEN` from the env (only the cached token
//!     file, `Cache::token`) NOR `HF_ENDPOINT` from `Api::new()` (only
//!     `ApiBuilder::from_env`, which `Api::new()` never uses). Both
//!     passthroughs are therefore OURS, thin and explicit.
//!   * `map_hub_fetch_error` — the T205 honesty gate: an auth-shaped hub
//!     failure (HTTP 401/403, the ureq Display shape) produces ONE clear
//!     stderr line naming the fix (HF_TOKEN missing vs insufficient) plus
//!     ONE `judge_checkpoint_auth_error` note in events.jsonl — never a
//!     silent retry storm (the fetch itself is one attempt: `ApiBuilder`
//!     max_retries is 0, and the daemon bails on the first failed file, so
//!     the client latches and the loop fails open, T190 shape).
//!
//! Hygiene: the token VALUE never crosses this module's outputs — the
//! events note records `token_set` (a boolean), the fix lines name the VAR,
//! and tests/no_secret_spill.rs pins the repo to the same rule.

// In a plain build (no `daemon` feature) this module carries TEST-ONLY
// surfaces: the production caller is the feature-gated judge_model fetch.
// The pins still run in the plain `cargo test` gate — that is the point of
// the ungated split — so dead-code is expected exactly there.
#![cfg_attr(not(feature = "daemon"), allow(dead_code))]

use std::path::Path;

#[cfg(feature = "daemon")]
use anyhow::{Context, Result};
use serde_json::json;

/// The private/gated-repo token: read by [`hub_token`] and passed to the
/// hf-hub builder (the crate does NOT read it itself — see the module doc).
pub const HF_TOKEN_ENV: &str = "HF_TOKEN";
/// The STANDARD HF endpoint knob, honored as the fallback target.
pub const HF_ENDPOINT_ENV: &str = "HF_ENDPOINT";
/// The chug-specific endpoint override: wins over [`HF_ENDPOINT_ENV`] so a
/// loopd env file can point the daemon at Artifactory (or any
/// HF-compatible host) without touching operators' global HF settings.
pub const CHUG_HF_ENDPOINT_ENV: &str = "CHUG_HF_ENDPOINT";
/// The public default — public chug must never require a token, or every
/// public user hits an auth wall on the shipped base model.
pub const DEFAULT_ENDPOINT: &str = "https://huggingface.co";
/// The hub branch used when `CHUG_LAYA_CHECKPOINT` names no `@REV`.
pub const DEFAULT_REVISION: &str = "main";

// ---------------------------------------------------------------------------
// Checkpoint reference parsing
// ---------------------------------------------------------------------------

/// Split a hub checkpoint value into `(repo, revision)`. `org/model` pins
/// nothing (resolves at [`DEFAULT_REVISION`]); `org/model@REV` pins REV (a
/// sha or tag — the cache keys off the revision, so a pin never resolves
/// under a moved branch). Anything without a non-empty `@`-split is the
/// whole value at the default revision.
pub fn parse_hub_ref(value: &str) -> (String, String) {
    match value.rsplit_once('@') {
        Some((repo, rev)) if !repo.is_empty() && !rev.is_empty() => {
            (repo.to_string(), rev.to_string())
        }
        _ => (value.to_string(), DEFAULT_REVISION.to_string()),
    }
}

/// The resolved shape of one `CHUG_LAYA_CHECKPOINT` value — the ONE
/// dir-vs-hub decision, single-homed here so the pins below guard the real
/// branch (the feature-gated `judge_model::CheckpointSpec` maps this 1:1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointRef {
    /// An existing local directory (wins over any hub id — T204 req 2).
    Dir(std::path::PathBuf),
    /// A hub repo, optionally revision-pinned (`org/model@REV`).
    Hub { repo: String, revision: String },
}

/// Resolve a checkpoint value: an EXISTING LOCAL DIRECTORY wins over any
/// hub id (T204 req 2 unchanged — so a dir named `x@y` is still a dir);
/// anything else parses as `org/model[@REV]` via [`parse_hub_ref`].
pub fn resolve_ref(value: &str) -> CheckpointRef {
    if Path::new(value).is_dir() {
        CheckpointRef::Dir(std::path::PathBuf::from(value))
    } else {
        let (repo, revision) = parse_hub_ref(value);
        CheckpointRef::Hub { repo, revision }
    }
}

// ---------------------------------------------------------------------------
// Endpoint + token resolution (the hf-hub passthroughs)
// ---------------------------------------------------------------------------

/// The hub base URL the daemon downloads from: `CHUG_HF_ENDPOINT` wins,
/// then the standard `HF_ENDPOINT` (ecosystem parity — the publish
/// contract's HF clients use it for Artifactory), then the public default.
/// A trailing `/` is trimmed so `…//org/model` URLs never form.
pub fn hub_endpoint() -> String {
    for key in [CHUG_HF_ENDPOINT_ENV, HF_ENDPOINT_ENV] {
        if let Ok(v) = std::env::var(key) {
            let v = v.trim().trim_end_matches('/').to_string();
            if !v.is_empty() {
                return v;
            }
        }
    }
    DEFAULT_ENDPOINT.to_string()
}

/// The hub token: a non-empty `HF_TOKEN` (trimmed — an env file may carry
/// trailing whitespace), else None and the hf-hub builder falls back to the
/// cached token file (`huggingface-cli login`), which is hf-hub 0.4.3's own
/// default. Env wins over the file, matching huggingface_hub semantics.
pub fn hub_token() -> Option<String> {
    std::env::var(HF_TOKEN_ENV)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

// ---------------------------------------------------------------------------
// Auth-failure honesty (the T205 gate)
// ---------------------------------------------------------------------------

/// Classify a FORMATTED error chain as auth-shaped: `Some(status)` for
/// HTTP 401 (missing/rejected token) or 403 (token valid, access denied —
/// private-org repos answer either depending on the auth state). ureq
/// 2.12's `Error::Status` Display is `"{url}: status code {status}"`, which
/// survives into the anyhow chain through hf-hub's
/// `ApiError::RequestError(Box<ureq::Error>)`.
pub fn auth_status_in(chain: &str) -> Option<u16> {
    if chain.contains("status code 401") {
        Some(401)
    } else if chain.contains("status code 403") {
        Some(403)
    } else {
        None
    }
}

/// The ONE clear stderr line for an auth-shaped failure — it NAMES the fix
/// and nothing else has to be inferred. Missing token → say to set
/// `HF_TOKEN` (read-scoped, with access to the repo); token present but
/// rejected → say the token is insufficient (scope/expiry/membership).
/// Never includes the token VALUE (there is none here by construction —
/// only `token_set` reaches this far).
/// The actionable clause of the auth fix — ONE source of truth shared by
/// the stderr line ([`auth_fix_line`]) and the error-chain context (so the
/// fix language lands everywhere the error travels: daemon.log, transcripts,
/// test assertions), never the token value.
fn auth_fix_clause(repo: &str, _status: u16, token_set: bool) -> String {
    if token_set {
        format!(
            "HF_TOKEN is set but was rejected: check its scope/expiry and org membership \
             (it needs read access to the private repo {repo})"
        )
    } else {
        format!(
            "the repo is private or gated: set HF_TOKEN to a read-scoped token with access \
             to {repo} (or point CHUG_LAYA_CHECKPOINT at a local checkpoint dir)"
        )
    }
}

/// The ONE stderr line for an auth failure: names the repo, revision, status
/// and the fix — never the token value.
pub fn auth_fix_line(repo: &str, revision: &str, status: u16, token_set: bool) -> String {
    format!(
        "chug: judge checkpoint {repo}@{revision}: HF hub auth failed (HTTP {status}) — {}",
        auth_fix_clause(repo, status, token_set)
    )
}

/// Map one hub-file fetch failure into the error the daemon surfaces.
/// Non-auth failures keep the T204 offline hint untouched. Auth failures
/// (401/403) get the T205 honesty gate: ONE stderr fix line + ONE
/// events.jsonl note, then the error carries the same classification so the
/// daemon log records it where the stderr line may only live in daemon.log.
/// ONE attempt per load by construction — `resolve` bails on the first
/// failed file — and the client latches the dead daemon, so the run
/// continues fail-open without a retry storm.
pub fn map_hub_fetch_error(
    err: anyhow::Error,
    repo: &str,
    revision: &str,
    file: &str,
    token_set: bool,
    note_cwd: Option<&Path>,
) -> anyhow::Error {
    let chain = format!("{err:#}");
    let Some(status) = auth_status_in(&chain) else {
        return err.context("offline? pre-stage the cache with the standard HF tooling");
    };
    let line = auth_fix_line(repo, revision, status, token_set);
    eprintln!("{line}");
    note_auth_error(note_cwd, repo, revision, file, status, token_set);
    err.context(format!(
        "HF hub auth failed (HTTP {status}) for {repo}@{revision} — {}",
        auth_fix_clause(repo, status, token_set)
    ))
}

/// The ONE events.jsonl note for the auth failure (best-effort, T190
/// fail-open shape: telemetry never aborts the daemon or the run). Records
/// `token_set` as a BOOLEAN — the token value never enters the log.
fn note_auth_error(
    note_cwd: Option<&Path>,
    repo: &str,
    revision: &str,
    file: &str,
    status: u16,
    token_set: bool,
) {
    let dir = match note_cwd {
        Some(p) => p.to_path_buf(),
        None => match std::env::current_dir() {
            Ok(d) => d,
            Err(_) => return, // nowhere to note; the stderr line already fired
        },
    };
    crate::eventlog::append_line(
        &dir,
        json!({
            "type": "judge_checkpoint_auth_error",
            "ts": crate::observ::now_rfc3339(),
            "repo": repo,
            "revision": revision,
            "file": file,
            "status": status,
            "token_set": token_set,
        }),
    );
}

/// Build the hf-hub client the daemon fetches through, with the T205
/// passthroughs applied. Feature-gated (hf-hub is an optional dep) but
/// defined HERE so the endpoint/token rules have ONE home; the plain-build
/// pins above exercise `hub_endpoint`/`hub_token` directly.
#[cfg(feature = "daemon")]
pub(crate) fn build_hub_api() -> Result<hf_hub::api::sync::Api> {
    let mut builder = hf_hub::api::sync::ApiBuilder::new();
    if let Some(token) = hub_token() {
        builder = builder.with_token(Some(token));
    }
    builder = builder.with_endpoint(hub_endpoint());
    builder.build().context("initializing the HF hub client")
}

// ---------------------------------------------------------------------------
// Test scaffolding shared across the binary (cfg(test) only)
// ---------------------------------------------------------------------------

/// ONE env lock shared by every test in the binary that touches the T205
/// keys (hf_hosting AND the feature-gated judge_model pins): parallel test
/// threads share the process env, so both modules serialize on this.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A scoped env override: `set`/`unset` under the lock, and the key is
/// ALWAYS unset on drop (the restore-to-absent choice is safe because no
/// other test in this binary reads the T205 keys).
#[cfg(test)]
pub(crate) struct EnvVar(&'static str);

#[cfg(test)]
impl EnvVar {
    pub(crate) fn set(&self, v: &str) {
        unsafe { std::env::set_var(self.0, v) };
    }
    pub(crate) fn unset(&self) {
        unsafe { std::env::remove_var(self.0) };
    }
}

#[cfg(test)]
impl Drop for EnvVar {
    fn drop(&mut self) {
        self.unset();
    }
}

/// A guard that starts the var ABSENT and unsets it again on drop — for
/// tests that need a known-missing key.
#[cfg(test)]
pub(crate) fn env_guard(key: &'static str) -> EnvVar {
    let g = EnvVar(key);
    g.unset();
    g
}

// ---------------------------------------------------------------------------
// Pins (ungated: plain `cargo test` runs these)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- revision parsing (spec Tests pin 1) ---------------------------------

    #[test]
    fn parse_hub_ref_pins_revision_sha() {
        assert_eq!(
            parse_hub_ref("videoamp/laya-stop-judge@9c6af39cdce45b570f0b7f8fad2b311c96019804"),
            (
                "videoamp/laya-stop-judge".to_string(),
                "9c6af39cdce45b570f0b7f8fad2b311c96019804".to_string()
            )
        );
        // tags too — a tag may contain `/` (e.g. refs/...); the LAST @ splits
        assert_eq!(
            parse_hub_ref("org/model@release-1.2"),
            ("org/model".to_string(), "release-1.2".to_string())
        );
    }

    #[test]
    fn parse_hub_ref_bare_repo_defaults_to_main() {
        assert_eq!(
            parse_hub_ref("convaiinnovations/laya"),
            ("convaiinnovations/laya".to_string(), "main".to_string())
        );
        // degenerate splits stay unpinned rather than half-parsing
        assert_eq!(
            parse_hub_ref("org/model@"),
            ("org/model@".to_string(), "main".to_string())
        );
        assert_eq!(
            parse_hub_ref("@revision"),
            ("@revision".to_string(), "main".to_string())
        );
    }

    #[test]
    fn local_dir_wins_and_at_values_pin_through_resolve_ref() {
        // the T204 contract (existing dir wins) lives in resolve_ref — pin
        // the REAL decision branch, including an `@` inside a dir name.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weird@name");
        std::fs::create_dir(&path).unwrap();
        assert_eq!(
            resolve_ref(path.to_str().unwrap()),
            CheckpointRef::Dir(path.to_path_buf())
        );
        assert_eq!(
            resolve_ref(dir.path().to_str().unwrap()),
            CheckpointRef::Dir(dir.path().to_path_buf())
        );
        // a NON-dir value with `@` goes to the hub, pinned
        assert_eq!(
            resolve_ref("org/model@abc123"),
            CheckpointRef::Hub {
                repo: "org/model".to_string(),
                revision: "abc123".to_string()
            }
        );
        assert_eq!(
            resolve_ref("convaiinnovations/laya"),
            CheckpointRef::Hub {
                repo: "convaiinnovations/laya".to_string(),
                revision: "main".to_string()
            }
        );
    }

    // --- endpoint + token resolution ------------------------------------------

    #[test]
    fn chug_endpoint_wins_over_standard_hf_endpoint() {
        let _env = ENV_LOCK.lock().unwrap();
        let chug = EnvVar(CHUG_HF_ENDPOINT_ENV);
        let std_ep = EnvVar(HF_ENDPOINT_ENV);
        chug.set("https://artifactory.example.com/hf");
        std_ep.set("https://other.example.com");
        assert_eq!(hub_endpoint(), "https://artifactory.example.com/hf");
        // standard knob honored when the chug one is absent/empty
        chug.unset();
        assert_eq!(hub_endpoint(), "https://other.example.com");
        chug.set("   ");
        assert_eq!(hub_endpoint(), "https://other.example.com");
        // neither set -> the public default (public chug stays public)
        std_ep.unset();
        chug.unset();
        assert_eq!(hub_endpoint(), DEFAULT_ENDPOINT);
    }

    #[test]
    fn endpoint_trailing_slash_is_trimmed() {
        let _env = ENV_LOCK.lock().unwrap();
        let chug = EnvVar(CHUG_HF_ENDPOINT_ENV);
        chug.set("https://artifactory.example.com/hf/");
        assert_eq!(hub_endpoint(), "https://artifactory.example.com/hf");
    }

    #[test]
    fn token_env_is_trimmed_and_empty_means_absent() {
        let _env = ENV_LOCK.lock().unwrap();
        let tok = EnvVar(HF_TOKEN_ENV);
        tok.unset();
        assert_eq!(hub_token(), None);
        tok.set("   ");
        assert_eq!(hub_token(), None);
        tok.set("  hf_dummy_placeholder_value  ");
        assert_eq!(hub_token().as_deref(), Some("hf_dummy_placeholder_value"));
    }

    // --- auth-failure honesty (spec Tests pin 2) ------------------------------

    #[test]
    fn auth_status_classifies_ureq_chain_shapes() {
        let url = "https://huggingface.co/org/model/resolve/main/model.safetensors";
        assert_eq!(
            auth_status_in(&format!(
                "checkpoint org/model: fetching model.safetensors from the HF hub: \
                 request error: {url}: status code 401"
            )),
            Some(401)
        );
        assert_eq!(
            auth_status_in(&format!("request error: {url}: status code 403")),
            Some(403)
        );
        // transport/offline/not-found shapes are NOT auth
        assert_eq!(auth_status_in("request error: Dns Failed: dns error"), None);
        assert_eq!(
            auth_status_in(&format!("request error: {url}: status code 404")),
            None
        );
    }

    #[test]
    fn fix_line_names_the_fix_and_never_a_token_value() {
        let missing = auth_fix_line("videoamp/laya-judge", "abc123", 401, false);
        assert!(missing.contains("set HF_TOKEN"), "names the fix: {missing}");
        assert!(missing.contains("read-scoped"), "names the scope: {missing}");
        assert!(missing.contains("videoamp/laya-judge"), "names the repo");
        assert!(missing.contains("401"));
        assert!(!missing.contains('\n'), "ONE line: {missing}");

        let rejected = auth_fix_line("videoamp/laya-judge", "abc123", 403, true);
        assert!(rejected.contains("HF_TOKEN is set but was rejected"));
        assert!(rejected.contains("scope/expiry"));
        assert!(!rejected.contains('\n'), "ONE line: {rejected}");
    }

    #[test]
    fn mapped_auth_error_names_fix_and_notes_once_in_events() {
        let tmp = tempfile::tempdir().unwrap();
        let synthetic = anyhow::anyhow!(
            "request error: https://huggingface.co/v/resolve/main/model.safetensors: \
             status code 401"
        );
        let err = map_hub_fetch_error(
            synthetic,
            "videoamp/laya-judge",
            "abc123",
            F_SAFETENSORS_PLACEHOLDER,
            false,
            Some(tmp.path()),
        );
        let text = format!("{err:#}");
        assert!(text.contains("HF_TOKEN"), "names the fix: {text}");
        assert!(text.contains("401"));

        // exactly ONE events note, typed, boolean token_set, no token value
        let log = tmp.path().join(".chug/events.jsonl");
        let lines = std::fs::read_to_string(&log).unwrap();
        let notes: Vec<&str> = lines.lines().collect();
        assert_eq!(notes.len(), 1, "one note per fetch failure");
        let note: serde_json::Value = serde_json::from_str(notes[0]).unwrap();
        assert_eq!(note["type"], "judge_checkpoint_auth_error");
        assert_eq!(note["repo"], "videoamp/laya-judge");
        assert_eq!(note["status"], 401);
        assert_eq!(note["token_set"], false);
        assert!(!lines.contains("hf_dummy"), "no token value in the log");

        // a second mapping of the same failure (a fresh attempt, e.g. the
        // next daemon spawn) notes again — ONE note per ATTEMPT, never a
        // per-file flood (resolve bails after the first failed file).
        let err2 = map_hub_fetch_error(
            anyhow::anyhow!("request error: https://h/resolve/main/x: status code 403"),
            "videoamp/laya-judge",
            "abc123",
            F_SAFETENSORS_PLACEHOLDER,
            true,
            Some(tmp.path()),
        );
        assert!(format!("{err2:#}").contains("403"));
        let lines = std::fs::read_to_string(&log).unwrap();
        assert_eq!(lines.lines().count(), 2);
    }

    /// The name of the first fetched layout file, kept as a const so the
    /// pins above don't depend on the feature-gated judge_model consts.
    const F_SAFETENSORS_PLACEHOLDER: &str = "model.safetensors";
}

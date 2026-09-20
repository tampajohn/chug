//! Endpoint + credential resolution for the Messages API client.
//!
//! Resolution order for each of base URL and credentials — first non-empty
//! wins:
//!
//! 1. Process env: `ANTHROPIC_BASE_URL` / `ANTHROPIC_AUTH_TOKEN` /
//!    `ANTHROPIC_API_KEY`.
//! 2. The top-level `env` object of `~/.claude/settings.json` (same keys).
//! 3. Built-in default for base URL only: `https://api.anthropic.com`.
//!
//! The settings file is read LAZILY on first client construction and cached
//! for the process lifetime. A missing file, unparseable JSON, or missing
//! keys are treated as absent — never an error. Credential values are never
//! logged.
//!
//! The pure core [`resolve_endpoint_with`] takes an injected env getter and
//! settings loader so tests need no real `$HOME` and no env mutation.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::bail;
use serde_json::Value;

pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Names BOTH sources so the operator knows every place a credential may live.
pub const NO_CREDENTIALS_MSG: &str =
    "no credentials: set ANTHROPIC_AUTH_TOKEN or ANTHROPIC_API_KEY (env or ~/.claude/settings.json env block)";

const BASE_URL_VAR: &str = "ANTHROPIC_BASE_URL";
const API_KEY_VAR: &str = "ANTHROPIC_API_KEY";
const AUTH_TOKEN_VAR: &str = "ANTHROPIC_AUTH_TOKEN";

/// Resolved endpoint + credentials for one API client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub base_url: String,
    pub api_key: Option<String>,
    pub auth_token: Option<String>,
}

/// The `env` block of `~/.claude/settings.json`, narrowed to the keys chug
/// uses. Absent file / keys / unparseable JSON all yield the all-absent
/// default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsEnv {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub auth_token: Option<String>,
}

impl SettingsEnv {
    fn get(&self, var: &str) -> Option<&str> {
        match var {
            BASE_URL_VAR => self.base_url.as_deref(),
            API_KEY_VAR => self.api_key.as_deref(),
            AUTH_TOKEN_VAR => self.auth_token.as_deref(),
            _ => None,
        }
    }
}

/// Resolve endpoint + credentials from an injected env getter and settings
/// loader. The loader is consulted AT MOST ONCE per call; wrap it in a cache
/// (see [`cached`]) to avoid re-reading the file across resolutions.
///
/// Values from either source are trimmed; empty-after-trim counts as absent.
pub fn resolve_endpoint_with(
    env: &dyn Fn(&str) -> Option<String>,
    settings: &dyn Fn() -> SettingsEnv,
) -> anyhow::Result<Endpoint> {
    // Exactly one loader call per resolution; the file is read lazily here.
    let file = settings();
    let pick = |var: &str| -> Option<String> {
        env(var)
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .or_else(|| {
                file.get(var)
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            })
    };

    let base_url = pick(BASE_URL_VAR).unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
    let api_key = pick(API_KEY_VAR);
    let auth_token = pick(AUTH_TOKEN_VAR);

    if api_key.is_none() && auth_token.is_none() {
        bail!("{NO_CREDENTIALS_MSG}");
    }

    Ok(Endpoint {
        base_url,
        api_key,
        auth_token,
    })
}

/// Production resolution: real process env + the lazily loaded, process-cached
/// settings file. This is what `Client::new` consumes.
pub fn resolve_endpoint() -> anyhow::Result<Endpoint> {
    static SETTINGS: OnceLock<SettingsEnv> = OnceLock::new();
    let env_fn = |name: &str| std::env::var(name).ok();
    let settings = || cached(&SETTINGS, || load_settings_env(&settings_path(&home_dir())));
    resolve_endpoint_with(&env_fn, &settings)
}

/// `get_or_init` semantics behind a helper so tests can inject a counting
/// loader and prove the file is read at most once.
fn cached<T: Clone>(cell: &OnceLock<T>, load: impl FnOnce() -> T) -> T {
    cell.get_or_init(load).clone()
}

/// `$HOME/.claude/settings.json`. Unix resolves home from `HOME`, windows
/// from `USERPROFILE`; no extra dependency for this alone.
fn settings_path(home: &Option<PathBuf>) -> PathBuf {
    let mut path = home.clone().unwrap_or_else(|| PathBuf::from("/"));
    path.push(".claude");
    path.push("settings.json");
    path
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(unix)]
    let var = "HOME";
    #[cfg(windows)]
    let var = "USERPROFILE";
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

/// Read + parse the settings file. ANY failure (missing file, unreadable,
/// unparseable JSON, missing `env` object or keys) yields the all-absent
/// default — the file itself is never an error. File values are trimmed;
/// empty-after-trim counts as absent.
fn load_settings_env(path: &Path) -> SettingsEnv {
    let Ok(text) = std::fs::read_to_string(path) else {
        return SettingsEnv::default();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
        return SettingsEnv::default();
    };
    let Some(env_obj) = parsed.get("env").and_then(Value::as_object) else {
        return SettingsEnv::default();
    };
    let trimmed = |key: &str| -> Option<String> {
        env_obj
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    };
    SettingsEnv {
        base_url: trimmed(BASE_URL_VAR),
        api_key: trimmed(API_KEY_VAR),
        auth_token: trimmed(AUTH_TOKEN_VAR),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    /// Env getter over a map (no process-env mutation — edition 2024 makes
    /// that unsafe, and parallel tests would race anyway).
    fn map_env(map: &[(&str, &str)]) -> impl for<'a> Fn(&'a str) -> Option<String> {
        let owned: HashMap<String, String> = map
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| owned.get(name).cloned()
    }

    fn no_settings() -> SettingsEnv {
        SettingsEnv::default()
    }

    fn settings_from(tmp: &tempfile::TempDir, json: &str) -> impl Fn() -> SettingsEnv {
        let path = tmp.path().join("settings.json");
        fs::write(&path, json).unwrap();
        move || load_settings_env(&path)
    }

    #[test]
    fn env_only_resolves() {
        let env = map_env(&[
            (BASE_URL_VAR, "https://proxy.example.com"),
            (AUTH_TOKEN_VAR, " tok "),
        ]);
        let ep = resolve_endpoint_with(&env, &no_settings).unwrap();
        assert_eq!(ep.base_url, "https://proxy.example.com");
        assert_eq!(ep.auth_token.as_deref(), Some("tok"));
        assert_eq!(ep.api_key, None);
    }

    #[test]
    fn file_only_resolves_without_env() {
        let tmp = tempfile::tempdir().unwrap();
        let loader = settings_from(
            &tmp,
            r#"{"env": {"ANTHROPIC_BASE_URL": "https://proxy.example.com",
                        "ANTHROPIC_AUTH_TOKEN": "file-token"}}"#,
        );
        let env = map_env(&[]);
        let ep = resolve_endpoint_with(&env, &loader).unwrap();
        assert_eq!(ep.base_url, "https://proxy.example.com");
        assert_eq!(ep.auth_token.as_deref(), Some("file-token"));
    }

    #[test]
    fn env_wins_over_file() {
        let tmp = tempfile::tempdir().unwrap();
        let loader = settings_from(
            &tmp,
            r#"{"env": {"ANTHROPIC_BASE_URL": "https://from-file",
                        "ANTHROPIC_AUTH_TOKEN": "file-token",
                        "ANTHROPIC_API_KEY": "file-key"}}"#,
        );
        let env = map_env(&[(AUTH_TOKEN_VAR, "env-token")]);
        let ep = resolve_endpoint_with(&env, &loader).unwrap();
        // Env token wins; file-only values still fill the gaps.
        assert_eq!(ep.auth_token.as_deref(), Some("env-token"));
        assert_eq!(ep.api_key.as_deref(), Some("file-key"));
        assert_eq!(ep.base_url, "https://from-file");
    }

    #[test]
    fn file_base_url_wins_over_builtin_default() {
        let tmp = tempfile::tempdir().unwrap();
        let loader =
            settings_from(&tmp, r#"{"env": {"ANTHROPIC_BASE_URL": " https://from-file "}}"#);
        let env = map_env(&[(AUTH_TOKEN_VAR, "env-token")]);
        let ep = resolve_endpoint_with(&env, &loader).unwrap();
        assert_eq!(ep.base_url, "https://from-file");
    }

    #[test]
    fn default_base_url_when_no_source_has_it() {
        let env = map_env(&[(AUTH_TOKEN_VAR, "tok")]);
        let ep = resolve_endpoint_with(&env, &no_settings).unwrap();
        assert_eq!(ep.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn missing_file_and_malformed_json_are_absent_not_errors() {
        let env = map_env(&[(AUTH_TOKEN_VAR, "tok")]);

        // Missing file.
        assert_eq!(load_settings_env(Path::new("/nonexistent/chug/settings.json")), SettingsEnv::default());
        let missing = || load_settings_env(Path::new("/nonexistent/chug/settings.json"));
        let ep = resolve_endpoint_with(&env, &missing).unwrap();
        assert_eq!(ep.base_url, DEFAULT_BASE_URL);

        // Malformed JSON.
        let tmp = tempfile::tempdir().unwrap();
        let loader = settings_from(&tmp, "{not json");
        let ep = resolve_endpoint_with(&env, &loader).unwrap();
        assert_eq!(ep.base_url, DEFAULT_BASE_URL);
        assert_eq!(ep.auth_token.as_deref(), Some("tok"));
    }

    #[test]
    fn empty_values_are_treated_as_absent() {
        let tmp = tempfile::tempdir().unwrap();
        // File has empty/whitespace values; env has an empty token.
        let loader = settings_from(
            &tmp,
            r#"{"env": {"ANTHROPIC_BASE_URL": "   ",
                        "ANTHROPIC_AUTH_TOKEN": "",
                        "ANTHROPIC_API_KEY": "file-key"}}"#,
        );
        let env = map_env(&[(AUTH_TOKEN_VAR, "  ")]);
        let ep = resolve_endpoint_with(&env, &loader).unwrap();
        // Empty env token falls through to the file; empty file values are
        // skipped; the API key survives.
        assert_eq!(ep.auth_token, None);
        assert_eq!(ep.api_key.as_deref(), Some("file-key"));
        assert_eq!(ep.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn no_credentials_error_names_both_sources() {
        let env = map_env(&[]);
        let err = resolve_endpoint_with(&env, &no_settings).unwrap_err().to_string();
        assert_eq!(err, NO_CREDENTIALS_MSG);
        assert!(err.contains("env or ~/.claude/settings.json env block"));
    }

    #[test]
    fn caching_reads_the_file_at_most_once() {
        let cell: OnceLock<SettingsEnv> = OnceLock::new();
        let loads = std::cell::Cell::new(0);
        let load = || {
            loads.set(loads.get() + 1);
            SettingsEnv {
                base_url: Some("https://from-file".into()),
                auth_token: Some("file-token".into()),
                api_key: None,
            }
        };
        let settings = || cached(&cell, load);
        let env = map_env(&[]);
        let first = resolve_endpoint_with(&env, &settings).unwrap();
        let second = resolve_endpoint_with(&env, &settings).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.base_url, "https://from-file");
        // Second resolve hit the cache; the loader ran exactly once.
        assert_eq!(loads.get(), 1);
    }

    #[test]
    fn settings_path_lands_at_home_dot_claude() {
        let home = Some(PathBuf::from("/Users/someone"));
        assert_eq!(
            settings_path(&home),
            PathBuf::from("/Users/someone/.claude/settings.json")
        );
        // No HOME: falls back to root — file won't exist, treated as absent.
        assert_eq!(settings_path(&None), PathBuf::from("/.claude/settings.json"));
    }
}

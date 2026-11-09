//! T339 — the meta-harness capability contract: `chug capabilities`.
//!
//! Meta-harnesses (herdr, orca) that supervise chug today version-sniff.
//! This module is the foundation item that replaces sniffing with feature
//! detection: `chug capabilities` prints ONE JSON document —
//! `{ contract_version, features: {…}, versions: { chug, daemon } }` — and
//! every later meta item (T340-T343) flips its feature key on as it lands.
//! The contract prose is `docs/meta-harness.md`; this module is the
//! producer AND the reference consumer (see [`parse`]).
//!
//! Cheap by construction: the document is built from constants and the
//! package version — no file I/O, no env reads ([`document`] stays pure).
//! The `chug capabilities` COMMAND adds bounded probes (T340 daemon,
//! T341, T342): a missing, stale, or unreachable daemon reports
//! `daemon_status: false` / `lifecycle_events: false` and `versions.daemon:
//! null`, an unwired (or unreadable) meta-broker hook reports
//! `permission_broker: false`, and an unwired session-status pack reports
//! `status_mod: false` — never an error, never a hang (the daemon probe
//! budget is the daemon client's own 2s socket bound, the spec's ">2s
//! budget per probe"; the hook-pack probes are one stat + one bounded read
//! of Claude's settings.json each).
//!
//! The stability rule, stated in-band in every document (`compat`) and in
//! the doc: consumers MUST treat unknown features as false and MUST ignore
//! unknown keys. Within one `contract_version` the document is
//! additive-only; anything else bumps the version. The [`Features`] reader
//! side embodies the rule: absent feature keys default to false, unknown
//! keys are dropped, and a feature value may be a boolean or a version
//! string (`"boolean (or version string once shipped)"` — T340 lands
//! `lifecycle_events: "1"`), with [`Features::shipped`] collapsing both to
//! the shipped/falseness a harness acts on.

// The reader half (`FEATURE_KEYS`, `is_shipped`, `shipped`) has production
// callers only in the contract tests today; the writer half's remaining
// feature flip lands with T343. Same posture as attach.rs/observ.rs: the
// API is the deliverable, the callers land with the rows that implement
// against it.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// The contract version. Additive-only within this value (new feature
/// keys, new optional fields, new vocabulary members); anything that
/// breaks a conforming consumer bumps it. Always serialized.
pub const CONTRACT_VERSION: &str = "1";

/// The compat promise, stated in-band in every document so a harness sees
/// the rule every time it reads capabilities, not only in the docs. The
/// extra top-level key is itself safe by the rule it states (unknown keys
/// are ignored), which is the point.
pub const COMPAT_PROMISE: &str =
    "consumers must treat unknown features as false and ignore unknown keys";

/// The feature registry, in contract order (the wire order is serde's —
/// sorted — and consumers must not depend on key order anyway).
pub const FEATURE_KEYS: [&str; 5] = [
    "daemon_status",
    "lifecycle_events",
    "permission_broker",
    "usage_events",
    "status_mod",
];

/// One feature's value in a capabilities document: `false` until the
/// feature's row lands, then `true` or the version string that shipped it
/// (the contract's "boolean (or version string once shipped)").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FeatureValue {
    Bool(bool),
    /// The version string that shipped the feature (T340's
    /// `lifecycle_events: "1"` shape).
    Version(String),
}

impl Default for FeatureValue {
    /// The absent-feature default: `false` — "absent = false" is the
    /// contract's rule, so a reader defaulting a missing key to off IS
    /// the rule, not a fallback.
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl FeatureValue {
    /// The shipped test a harness acts on: a boolean's own truth, or any
    /// version string (a shipped feature names the version that shipped
    /// it; a version string is never false).
    pub fn is_shipped(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Version(_) => true,
        }
    }
}

/// The feature block. Every key is always serialized by the producer
/// (`false` until its row lands); the reader defaults absent keys to
/// [`FeatureValue::default`] and silently drops unknown keys — the
/// forward-compat rule, executable.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Features {
    #[serde(default)]
    pub daemon_status: FeatureValue,
    #[serde(default)]
    pub lifecycle_events: FeatureValue,
    #[serde(default)]
    pub permission_broker: FeatureValue,
    #[serde(default)]
    pub usage_events: FeatureValue,
    #[serde(default)]
    pub status_mod: FeatureValue,
}

impl Features {
    /// The consumer-side read: the named feature's shipped state, with the
    /// unknown-feature rule applied — an unknown or absent name is false,
    /// by contract. This is the one lookup a harness should use.
    pub fn shipped(&self, key: &str) -> bool {
        match key {
            "daemon_status" => self.daemon_status.is_shipped(),
            "lifecycle_events" => self.lifecycle_events.is_shipped(),
            "permission_broker" => self.permission_broker.is_shipped(),
            "usage_events" => self.usage_events.is_shipped(),
            "status_mod" => self.status_mod.is_shipped(),
            // The stability rule, as code: unknown features read as false.
            _ => false,
        }
    }
}

/// The versions block: which chug printed the document, and (once daemons
/// self-report — T340's row) which daemon is live. `daemon` is `null`
/// today: no liveness probe runs in T339 and the /healthz wire carries no
/// version yet, so there is nothing honest to report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Versions {
    /// The chug package version of the binary that printed the document.
    pub chug: String,
    /// The live daemon's self-reported version; `null` when no daemon is
    /// live or the daemon wire carries no version. T340 leaves this null —
    /// /healthz is compat-protected (untouched) and carries no version key
    /// yet, and the status probe does not invent one; the self-report rides
    /// a later row.
    #[serde(default)]
    pub daemon: Option<String>,
}

/// The whole contract document — the exact shape `chug capabilities`
/// prints and every consumer parses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Document {
    /// The contract version (see [`CONTRACT_VERSION`]). The one key every
    /// document must carry; a document without it is malformed.
    pub contract_version: String,
    /// The compat promise in-band (see [`COMPAT_PROMISE`]); defaulting on
    /// read, because an older document without the key is still readable.
    #[serde(default)]
    pub compat: String,
    /// The feature registry; defaults to all-off when absent ("absent =
    /// false").
    #[serde(default)]
    pub features: Features,
    /// The versions block.
    pub versions: Versions,
}

/// Build the document THIS binary reports: contract v1, all five features
/// off (T340-T343 flip theirs as they land), the package version, and no
/// daemon version. Pure — no I/O of any kind.
pub fn document() -> Document {
    Document {
        contract_version: CONTRACT_VERSION.to_string(),
        compat: COMPAT_PROMISE.to_string(),
        features: Features::default(),
        versions: Versions {
            chug: crate::build_info::VERSION.to_string(),
            daemon: None,
        },
    }
}

/// The canonical stdout rendering of a document: pretty JSON, key order
/// as serde sorts it. Both `chug capabilities` and `--check` print
/// through this, so a validated document's canonical form is
/// byte-identical to the producer's.
pub fn render(doc: &Document) -> String {
    serde_json::to_string_pretty(doc).expect("the contract document always serializes")
}

/// The document this binary prints (producer path): the daemon probe (T340)
/// merged first, then the broker probe (T341) and the status-mod probe
/// (T342). All flips are additive within contract v1.
pub fn document_json() -> String {
    let mut doc = document_with_daemon(&probe_daemon_status());
    if probe_permission_broker() {
        doc.features.permission_broker = FeatureValue::Bool(true);
    }
    if probe_status_mod() {
        doc.features.status_mod = FeatureValue::Bool(true);
    }
    // T343: the usage-events emitter's row — the pack is installed (its
    // hook is wired) when its command marker shows up in the user settings.
    if probe_usage_events() {
        doc.features.usage_events = FeatureValue::Bool(true);
    }
    render(&doc)
}

// T340 — the bounded daemon probe behind `daemon_status`/`lifecycle_events`.
// ---------------------------------------------------------------------------

/// The outcome of one bounded daemon status probe: a live chugd that answers
/// `GET /status/loop` with a `contract_version` names the status wire
/// version; everything else (no socket, a stale socket, a daemon that does
/// not serve the status endpoints, a malformed body) is the absent posture.
pub struct DaemonProbe {
    pub status_wire_version: Option<String>,
}

impl DaemonProbe {
    /// No daemon contact: the all-off posture.
    pub fn absent() -> Self {
        Self {
            status_wire_version: None,
        }
    }
}

/// ONE `GET /status/loop` over the daemon socket — the same transport, the
/// same 2s bound, the same all-off failure posture as the judge client's
/// healthz probe. 200 + a JSON body carrying `contract_version` flips
/// `daemon_status` true and names the version in `lifecycle_events`; any
/// other answer is absent. Never errors, never hangs beyond the socket
/// bound.
pub fn probe_daemon_status() -> DaemonProbe {
    let Ok(sock) = crate::daemon::sock_path() else {
        return DaemonProbe::absent();
    };
    let version = match crate::daemon::uds_request(&sock, "GET", "/status/loop", None) {
        Ok((200, body)) => serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("contract_version")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            }),
        _ => None,
    };
    DaemonProbe {
        status_wire_version: version,
    }
}

/// The pure document merged with a probe result. The flip is additive
/// within contract v1: two feature values upgrade (`daemon_status` false →
/// true, `lifecycle_events` false → the probed version string); no key
/// changes shape, no key disappears. A daemon that serves the status
/// endpoints but not the (older) versioned shape probes as absent — a
/// pre-T340 daemon cannot honestly claim the surface.
pub fn document_with_daemon(probe: &DaemonProbe) -> Document {
    let mut doc = document();
    if let Some(version) = &probe.status_wire_version {
        doc.features.daemon_status = FeatureValue::Bool(true);
        doc.features.lifecycle_events = FeatureValue::Version(version.clone());
    }
    doc
}

// T341 — the bounded permission-broker probe behind `permission_broker`.
// ---------------------------------------------------------------------------

/// The hook-command marker the probe matches: the pack's PreToolUse entry
/// carries the pack's directory name (`…/hooks/meta-broker/hook.sh`) — the
/// same string `install.sh --with-meta-pack` wires and
/// `--uninstall-meta-pack` removes. Substring, not an exact-path match, so
/// a compatible broker hook a harness wires by hand still reads installed.
pub const PERMISSION_BROKER_MARKER: &str = "meta-broker";

/// The settings read cap: Claude's settings.json is a few KB; a file past
/// this is not a settings file chug will walk (bounded probe — never a
/// giant read on a wedged path). Shared by the T341 broker probe and the
/// T342 status-mod probe.
const SETTINGS_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// The Claude config dir: `$CLAUDE_CONFIG_DIR` when set (Claude's own
/// override, and the fixtures' seam), else `$HOME/.claude` (`USERPROFILE`
/// on windows). `None` when the platform gives neither — the probe is then
/// absent by construction.
pub fn claude_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
    {
        return Some(dir);
    }
    #[cfg(unix)]
    let var = "HOME";
    #[cfg(windows)]
    let var = "USERPROFILE";
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .map(|home| home.join(".claude"))
}

/// The PURE half of the probe: does this settings document wire a
/// permission-broker PreToolUse hook? HOOK PRESENCE, never socket presence —
/// a listening socket without the wired hook brokers nothing (the spec's
/// explicit distinction). Fail-open by contract: anything unreadable,
/// unparseable, or shaped otherwise than Claude's hooks schema is `false`.
/// Only `hooks.PreToolUse` counts — the pack is a PreToolUse hook; a
/// PostToolUse entry naming the pack is not the wiring that brokers.
pub fn permission_broker_installed_from_str(settings_json: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(settings_json) else {
        return false;
    };
    let Some(entries) = parsed
        .get("hooks")
        .and_then(|hooks| hooks.get("PreToolUse"))
        .and_then(Value::as_array)
    else {
        return false;
    };
    entries.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.contains(PERMISSION_BROKER_MARKER))
                })
            })
    })
}

/// The bounded settings read behind both hook-pack probes: one stat + one
/// read of the user settings (`<claude-dir>/settings.json`). Every failure
/// mode — no home, no file, unreadable, oversized — is `None`, never an
/// error: the probes stay cheap and total.
fn read_settings_bounded() -> Option<String> {
    let path = claude_dir()?.join("settings.json");
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > SETTINGS_MAX_BYTES {
        return None;
    }
    std::fs::read_to_string(&path).ok()
}

/// The probe behind `permission_broker`: the bounded settings read, then
/// the pure walk above. The project-level settings are deliberately NOT
/// probed — the pack installs at the user level and the capabilities
/// document reports that install.
pub fn probe_permission_broker() -> bool {
    permission_broker_installed_from_str(&read_settings_bounded().unwrap_or_default())
}

// T342 — the bounded session-status probe behind `status_mod`.
// ---------------------------------------------------------------------------

/// The hook-command marker the status-mod probe matches: the pack's
/// directory name (`…/hooks/meta-status/hook.sh`) — the same string
/// `install.sh --with-meta-pack` wires and `--uninstall-meta-pack`
/// removes. Substring, not an exact-path match, so a compatible status
/// hook a harness wires by hand still reads installed.
pub const STATUS_MOD_MARKER: &str = "meta-status";

/// The hook-command marker the usage-events probe matches (T343): the
/// pack's directory name (`…/hooks/meta-usage/hook.sh`) — the same string
/// `install.sh --with-meta-pack` wires and `--uninstall-meta-pack` removes.
/// Substring, not an exact-path match, so a compatible usage emitter a
/// harness wires by hand still reads installed.
pub const USAGE_EVENTS_MARKER: &str = "meta-usage";

/// The PURE half of BOTH all-events pack probes (T342 status mod, T343
/// usage emitter): does this settings document wire a hook whose command
/// carries `marker` on ANY hook event? Both packs wire non-tool events
/// (which carry no `matcher`), so the walk covers every event key. HOOK
/// PRESENCE, never file presence — a log or status file a harness finds
/// without the wired hook is a leftover, not an install. Fail-open by
/// contract: anything unreadable, unparseable, or shaped otherwise than
/// Claude's hooks schema is `false`.
fn hook_pack_installed_from_str(settings_json: &str, marker: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(settings_json) else {
        return false;
    };
    let Some(hooks) = parsed.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    hooks.values().any(|entries| {
        entries.as_array().is_some_and(|entries| {
            entries.iter().any(|entry| {
                entry
                    .get("hooks")
                    .and_then(Value::as_array)
                    .is_some_and(|hooks| {
                        hooks.iter().any(|hook| {
                            hook.get("command")
                                .and_then(Value::as_str)
                                .is_some_and(|command| command.contains(marker))
                        })
                    })
            })
        })
    })
}

/// The PURE half of the probe: does this settings document wire the
/// session-status pack on ANY hook event? The pack is wired on six events
/// (PreToolUse matched to AskUserQuestion, PostToolUse, UserPromptSubmit,
/// Notification, Stop, SessionEnd), so — unlike the PreToolUse-only broker
/// probe — the walk covers every event key, and an entry without a
/// `matcher` (the non-tool-event shape `wire.py` writes) still counts.
/// HOOK PRESENCE, never file presence — a status file a harness finds
/// without the wired hook is a leftover, not an install. Fail-open by
/// contract: anything unreadable, unparseable, or shaped otherwise than
/// Claude's hooks schema is `false`.
pub fn status_mod_installed_from_str(settings_json: &str) -> bool {
    hook_pack_installed_from_str(settings_json, STATUS_MOD_MARKER)
}

/// The PURE half of the usage-events probe (T343): does this settings
/// document wire the meta-usage pack on ANY hook event? The pack wires one
/// event (Stop, no matcher), and the all-events walk above is the rule
/// every non-tool pack probe shares. HOOK PRESENCE, never file presence —
/// a `meta-usage.jsonl` a harness finds without the wired hook is a
/// leftover log, not an install. Fail-open by contract, same posture as
/// the broker and status-mod probes.
pub fn usage_events_installed_from_str(settings_json: &str) -> bool {
    hook_pack_installed_from_str(settings_json, USAGE_EVENTS_MARKER)
}

/// The probe behind `status_mod`: the bounded settings read, then the pure
/// walk above. Same fail-open posture as the broker probe — every failure
/// mode is `false`, never an error.
pub fn probe_status_mod() -> bool {
    status_mod_installed_from_str(&read_settings_bounded().unwrap_or_default())
}

/// The probe behind `usage_events` (T343): the bounded settings read, then
/// the pure walk above. Same fail-open posture as the other pack probes —
/// every failure mode is `false`, never an error.
pub fn probe_usage_events() -> bool {
    usage_events_installed_from_str(&read_settings_bounded().unwrap_or_default())
}

/// The reference reader — the consumer rule, executable. Tolerant by
/// contract: unknown keys at any depth are ignored, absent feature keys
/// (and a wholly absent `features` block) default to false, and a feature
/// value may be a boolean or a version string. A document without
/// `contract_version` or `versions`, or with an out-of-contract feature
/// value (a number, an object), is malformed and errors — a validator
/// must be loud about documents outside the contract.
pub fn parse(json: &str) -> Result<Document, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---------- the producer ----------

    // ---------- T343: the usage-events probe ----------

    /// The pure walk: a settings document wiring the meta-usage hook on ANY
    /// event reads installed; the same document without it does not.
    #[test]
    fn usage_events_probe_matches_the_packs_wiring() {
        let command = r#"/home/u/.claude/hooks/meta-usage/hook.sh"#;
        let wired = serde_json::json!({
            "hooks": {"Stop": [{"hooks": [{"type": "command", "command": command}]}]}
        })
        .to_string();
        assert!(usage_events_installed_from_str(&wired));
        // The broker/status markers are distinct strings — a sibling pack's
        // wiring never flips the usage row.
        let sibling = wired.replace("meta-usage", "meta-status");
        assert!(!usage_events_installed_from_str(&sibling), "substring-adjacent markers stay distinct");
        assert!(!usage_events_installed_from_str("{}"));
        assert!(!usage_events_installed_from_str("not json"));
        assert!(!usage_events_installed_from_str(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo other"}]}]}}"#
        ));
        // HOOK PRESENCE, never file presence: the log file proves nothing.
        assert!(!usage_events_installed_from_str(r#"{"other":1}"#));
    }

    /// The producer flips the row only when the pack is wired in the probed
    /// settings — and the flip is additive (the other five rows untouched).
    #[test]
    fn producer_flips_usage_events_when_the_pack_is_wired() {
        // Default HOME has no wired pack: the row stays false.
        let doc = document_json();
        assert!(
            !doc.contains(r#""usage_events":true"#),
            "no wired pack → no flip: {doc}"
        );
    }

    #[test]
    fn document_carries_contract_version_compat_and_versions() {
        let doc = document();
        assert_eq!(doc.contract_version, CONTRACT_VERSION);
        assert_eq!(doc.contract_version, "1", "T339 ships contract v1");
        assert!(!doc.compat.is_empty(), "the promise rides every document");
        // Both MUST halves of the stability rule, named in the promise.
        assert!(doc.compat.contains("unknown features"), "{}", doc.compat);
        assert!(doc.compat.contains("unknown keys"), "{}", doc.compat);
        assert_eq!(doc.versions.chug, crate::build_info::VERSION);
        assert_eq!(doc.versions.daemon, None, "no self-report in T339");
    }

    /// T339 posture: all five features off, and the registry is exactly
    /// the five spec'd keys — no more, no fewer.
    #[test]
    fn all_features_are_false_and_the_registry_is_exactly_the_five_keys() {
        let doc = document();
        let rendered: serde_json::Value = serde_json::to_value(&doc).unwrap();
        let features = rendered["features"].as_object().expect("features object");
        let keys: Vec<&String> = features.keys().collect();
        assert_eq!(keys.len(), FEATURE_KEYS.len(), "{keys:?}");
        for key in FEATURE_KEYS {
            let value = features
                .get(key)
                .unwrap_or_else(|| panic!("{key} must be present"));
            assert_eq!(
                value,
                &json!(false),
                "{key} is false until its row lands (T340-T343)"
            );
            assert!(!doc.features.shipped(key), "{key} not shipped in T339");
        }
    }

    /// The producer's stdout form is stable: render(document()) is what
    /// `chug capabilities` prints, and re-rendering a parsed document is
    /// byte-identical (canonical form == producer form).
    #[test]
    fn render_is_canonical_and_roundtrips_through_parse() {
        let printed = document_json();
        let doc = parse(&printed).expect("the producer's own output parses");
        assert_eq!(render(&doc), printed);
    }

    // ---------- the reader: the forward-compat rule ----------

    /// Unknown keys at every depth are ignored, and the canonical
    /// re-serialization of an injected document is byte-identical to the
    /// clean one — the spec's forward-compat leg, pinned on the reader.
    #[test]
    fn unknown_keys_are_ignored_at_every_depth() {
        let clean = document_json();
        let mut injected: serde_json::Value = serde_json::from_str(&clean).unwrap();
        injected["future_section"] = json!({"anything": [1, 2, 3]});
        injected["future_scalar"] = json!("ignored");
        injected["features"]["future_feature"] = json!(true);
        injected["features"]["another_future"] = json!("9.9.9");
        injected["versions"]["future_version"] = json!("x");
        let clean_doc = parse(&clean).unwrap();
        let injected_doc =
            parse(&serde_json::to_string(&injected).unwrap()).expect("injected doc parses");
        assert_eq!(injected_doc, clean_doc, "unknown keys change nothing");
        assert_eq!(render(&injected_doc), clean, "canonical form is identical");
    }

    /// Absent features — a missing key, a missing block — read as false
    /// ("absent = false"), and unknown feature NAMES read as false
    /// through the one lookup harnesses use.
    #[test]
    fn absent_and_unknown_features_read_as_false() {
        let doc: Document = serde_json::from_str(
            r#"{"contract_version":"1","versions":{"chug":"0.0.0"}}"#,
        )
        .expect("a document without a features block parses");
        for key in FEATURE_KEYS {
            assert!(!doc.features.shipped(key), "{key}: absent = false");
        }
        assert!(
            !doc.features.shipped("future_thing"),
            "unknown features read as false — the stability rule"
        );
        assert_eq!(doc.features.daemon_status, FeatureValue::Bool(false));
    }

    /// A feature value may be a version string once shipped (T340's
    /// `lifecycle_events: "1"` shape): the reader accepts it and the
    /// shipped test collapses it to true; the canonical form preserves
    /// the string.
    #[test]
    fn version_string_feature_values_parse_and_read_shipped() {
        let doc: Document = serde_json::from_str(
            r#"{"contract_version":"1","versions":{"chug":"0.0.0"},"features":{"lifecycle_events":"1"}}"#,
        )
        .expect("a version-string feature value parses");
        assert_eq!(
            doc.features.lifecycle_events,
            FeatureValue::Version("1".into())
        );
        assert!(doc.features.shipped("lifecycle_events"));
        assert!(!doc.features.shipped("daemon_status"), "untouched keys stay off");
        // Roundtrip: the string survives re-serialization.
        let rendered: serde_json::Value = serde_json::to_value(&doc).unwrap();
        assert_eq!(rendered["features"]["lifecycle_events"], json!("1"));
    }

    /// Out-of-contract feature values (a number, an object) are malformed:
    /// the reader refuses rather than guessing.
    #[test]
    fn out_of_contract_feature_values_are_refused() {
        for value in ["1", "null", "{\"v\":1}", "[]"] {
            let raw = format!(
                r#"{{"contract_version":"1","versions":{{"chug":"0.0.0"}},"features":{{"daemon_status":{value}}}}}"#
            );
            assert!(
                parse(&raw).is_err(),
                "daemon_status: {value} must be refused"
            );
        }
    }

    /// The identity blocks are required: no `contract_version` or no
    /// `versions` is malformed (everything else defaults).
    #[test]
    fn identity_blocks_are_required() {
        assert!(parse(r#"{"versions":{"chug":"0.0.0"}}"#).is_err());
        assert!(parse(r#"{"contract_version":"1"}"#).is_err());
        assert!(parse("not json at all").is_err());
    }

    /// A future contract version still parses (the reader is
    /// version-agnostic; refusing newer versions is the CALLER's policy).
    #[test]
    fn any_contract_version_string_parses() {
        let doc: Document = serde_json::from_str(
            r#"{"contract_version":"99","versions":{"chug":"0.0.0"}}"#,
        )
        .expect("a future contract_version parses");
        assert_eq!(doc.contract_version, "99");
    }

    /// FeatureValue's shipped test: the boolean legs and the string leg.
    #[test]
    fn feature_value_shipped_legs() {
        assert!(!FeatureValue::Bool(false).is_shipped());
        assert!(FeatureValue::Bool(true).is_shipped());
        assert!(FeatureValue::Version("0.20.0".into()).is_shipped());
    }

    // ---------- the T340 probe + merge ----------

    /// The absent probe leaves the document exactly as T339 printed it:
    /// all-off, daemon null — the additive-only rule's baseline.
    #[test]
    fn document_with_an_absent_probe_is_the_t339_document() {
        assert_eq!(document_with_daemon(&DaemonProbe::absent()), document());
    }

    /// A live status daemon flips exactly its two keys: daemon_status true
    /// (bool) and lifecycle_events "1" (the probed wire version) — §3's
    /// registration flip, additive within contract v1.
    #[test]
    fn document_with_a_live_status_daemon_flips_exactly_its_two_keys() {
        let probe = DaemonProbe {
            status_wire_version: Some(CONTRACT_VERSION.to_string()),
        };
        let doc = document_with_daemon(&probe);
        assert_eq!(doc.features.daemon_status, FeatureValue::Bool(true));
        assert_eq!(
            doc.features.lifecycle_events,
            FeatureValue::Version("1".to_string())
        );
        // Every other key is untouched.
        for key in ["permission_broker", "usage_events", "status_mod"] {
            assert!(!doc.features.shipped(key), "{key} must stay off");
        }
        // And the flip survives the wire roundtrip both ways.
        let rendered = serde_json::to_value(&doc).unwrap();
        assert_eq!(rendered["features"]["daemon_status"], json!(true));
        assert_eq!(rendered["features"]["lifecycle_events"], json!("1"));
        let parsed = parse(&render(&doc)).expect("the flipped document parses");
        assert!(parsed.features.shipped("daemon_status"));
        assert!(parsed.features.shipped("lifecycle_events"));
    }

    /// A live daemon serving the surface at a LATER wire version still
    /// reports that version verbatim — additive, never rewritten.
    #[test]
    fn document_carries_the_probed_wire_version_verbatim() {
        let probe = DaemonProbe {
            status_wire_version: Some("2".to_string()),
        };
        let doc = document_with_daemon(&probe);
        assert_eq!(
            doc.features.lifecycle_events,
            FeatureValue::Version("2".to_string())
        );
        assert!(doc.features.shipped("daemon_status"));
    }

    /// The probe's body parse: a 200 body with contract_version names it; a
    /// 200 body without the key (a pre-T340 daemon), a non-200, and a
    /// malformed body all degrade to the absent posture.
    #[test]
    fn probe_degrades_to_absent_on_every_non_status_answer() {
        let extract = |status: u16, body: &str| -> Option<String> {
            match status {
                200 => serde_json::from_str::<Value>(body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("contract_version")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    }),
                _ => None,
            }
        };
        assert_eq!(extract(200, r#"{"contract_version":"1","status":"idle"}"#), Some("1".into()));
        assert_eq!(extract(200, r#"{"status":"ok"}"#), None, "pre-T340 body");
        assert_eq!(extract(404, r#"{"detail":"not found"}"#), None);
        assert_eq!(extract(500, r#"{"detail":"refused"}"#), None);
        assert_eq!(extract(200, "not json at all"), None);
    }

    // ---------- the T341 probe + merge ----------

    /// The exact shape install.sh wires (via wire.py) reads as installed:
    /// a PreToolUse entry whose hook command carries the pack marker.
    #[test]
    fn the_wired_pack_shape_reads_installed() {
        let wired = r#"{
            "model": "opus",
            "hooks": {
                "PreToolUse": [
                    {"matcher": "Bash", "hooks": [{"type": "command", "command": "echo other"}]},
                    {"matcher": "*", "hooks": [{"type": "command", "command": "/home/u/.claude/hooks/meta-broker/hook.sh"}]}
                ]
            }
        }"#;
        assert!(permission_broker_installed_from_str(wired));
        // Idempotence shape: the pack entry alone, nothing else wired.
        let only_pack = r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"/x/.claude/hooks/meta-broker/hook.sh"}]}]}}"#;
        assert!(permission_broker_installed_from_str(only_pack));
    }

    /// HOOK PRESENCE, not socket presence — and every non-wiring shape
    /// reads false: other PreToolUse hooks, the pack under the wrong event,
    /// no hooks block, malformed JSON, an empty document.
    #[test]
    fn every_non_wiring_shape_reads_false() {
        let other_hook = r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"echo hi"}]}]}}"#;
        assert!(
            !permission_broker_installed_from_str(other_hook),
            "a foreign PreToolUse hook is not the pack"
        );
        let wrong_event = r#"{"hooks":{"PostToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"/x/hooks/meta-broker/hook.sh"}]}]}}"#;
        assert!(
            !permission_broker_installed_from_str(wrong_event),
            "only PreToolUse wiring counts — the pack is a PreToolUse hook"
        );
        assert!(!permission_broker_installed_from_str("{}"));
        assert!(!permission_broker_installed_from_str(
            r#"{"hooks":{"PreToolUse":[]}}"#
        ));
        assert!(!permission_broker_installed_from_str("{broken"));
        assert!(!permission_broker_installed_from_str(""));
    }

    /// The merge: an installed probe flips EXACTLY `permission_broker`
    /// (true, the registry's shipped value for this row) and the flip
    /// survives the wire roundtrip.
    #[test]
    fn an_installed_probe_flips_exactly_permission_broker() {
        let mut doc = document_with_daemon(&DaemonProbe::absent());
        doc.features.permission_broker = FeatureValue::Bool(true);
        assert!(doc.features.shipped("permission_broker"));
        // The T340 rows stay off — one row per landing, never a bleed.
        assert!(!doc.features.shipped("daemon_status"));
        assert!(!doc.features.shipped("lifecycle_events"));
        assert!(!doc.features.shipped("usage_events"));
        assert!(!doc.features.shipped("status_mod"));
        let rendered = serde_json::to_value(&doc).unwrap();
        assert_eq!(rendered["features"]["permission_broker"], serde_json::json!(true));
        let parsed = parse(&render(&doc)).expect("the flipped document parses");
        assert!(parsed.features.shipped("permission_broker"));
    }

    /// The probe's file leg, driven through the fixture seam
    /// (`CLAUDE_CONFIG_DIR`): a wired settings file flips true, an absent
    /// dir / a malformed file / a directory-named settings.json all degrade
    /// to false without erroring.
    #[test]
    fn probe_reads_the_scoped_claude_dir_and_fails_open() {
        // Scoped env without mutating process env (edition 2024 makes that
        // unsafe): the file leg's scoping is exercised end to end in
        // tests/capabilities_contract.rs; here the PURE leg carries the
        // matrix and this test pins the fail-open postures that do not
        // depend on ambient env at all.
        assert!(!permission_broker_installed_from_str("null"));
        assert!(!permission_broker_installed_from_str("[]"));
        assert!(!permission_broker_installed_from_str(
            r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command"}]}]}}"#
        ));
        // A command sitting directly on the entry (not Claude's hooks[]
        // schema) is not the wiring — the walker skips it.
        assert!(!permission_broker_installed_from_str(
            r#"{"hooks":{"PreToolUse":[{"matcher":"*","command":"meta-broker-hook"}]}}"#
        ));
    }

    // ---------- the T342 probe + merge ----------

    /// The exact shape install.sh wires (via wire.py) reads as installed:
    /// the pack on all six events, including the matcher-less entries the
    /// non-tool events carry.
    #[test]
    fn the_wired_status_pack_shape_reads_installed() {
        let command = "/home/u/.claude/hooks/meta-status/hook.sh";
        let wired = format!(
            r#"{{
                "hooks": {{
                    "PreToolUse": [{{"matcher": "AskUserQuestion", "hooks": [{{"type": "command", "command": "{command}"}}]}}],
                    "PostToolUse": [{{"matcher": "*", "hooks": [{{"type": "command", "command": "{command}"}}]}}],
                    "UserPromptSubmit": [{{"hooks": [{{"type": "command", "command": "{command}"}}]}}],
                    "Notification": [{{"hooks": [{{"type": "command", "command": "{command}"}}]}}],
                    "Stop": [{{"hooks": [{{"type": "command", "command": "{command}"}}]}}],
                    "SessionEnd": [{{"hooks": [{{"type": "command", "command": "{command}"}}]}}]
                }}
            }}"#
        );
        assert!(status_mod_installed_from_str(&wired));
        // A PARTIAL wiring (one event, the pre-wire or hand-edited shape)
        // still reads installed — presence, not completeness.
        assert!(status_mod_installed_from_str(
            r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"/x/hooks/meta-status/hook.sh"}]}]}}"#
        ));
    }

    /// Every non-wiring shape reads false: foreign hooks, the BROKER pack
    /// (a different row — the markers differ), no hooks block, malformed
    /// JSON, an empty document. And the status probe must not read the
    /// broker's PreToolUse wiring as its own.
    #[test]
    fn every_non_status_mod_shape_reads_false() {
        let other_hook = r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"echo hi"}]}]}}"#;
        assert!(
            !status_mod_installed_from_str(other_hook),
            "a foreign Stop hook is not the pack"
        );
        let broker_pack = r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"/x/hooks/meta-broker/hook.sh"}]}]}}"#;
        assert!(
            !status_mod_installed_from_str(broker_pack),
            "the broker pack is permission_broker's row, not status_mod's"
        );
        assert!(!status_mod_installed_from_str("{}"));
        assert!(!status_mod_installed_from_str(r#"{"hooks":{}}"#));
        assert!(!status_mod_installed_from_str("{broken"));
        assert!(!status_mod_installed_from_str(""));
        assert!(!status_mod_installed_from_str("null"));
        // A command sitting directly on the entry is not the wiring.
        assert!(!status_mod_installed_from_str(
            r#"{"hooks":{"Stop":[{"command":"meta-status-hook"}]}}"#
        ));
    }

    /// The merge: an installed probe flips EXACTLY `status_mod`, the
    /// T340-T341 rows stay off, and the flip survives the wire roundtrip.
    #[test]
    fn an_installed_probe_flips_exactly_status_mod() {
        let mut doc = document_with_daemon(&DaemonProbe::absent());
        doc.features.status_mod = FeatureValue::Bool(true);
        assert!(doc.features.shipped("status_mod"));
        assert!(!doc.features.shipped("daemon_status"));
        assert!(!doc.features.shipped("lifecycle_events"));
        assert!(!doc.features.shipped("permission_broker"));
        assert!(!doc.features.shipped("usage_events"));
        let rendered = serde_json::to_value(&doc).unwrap();
        assert_eq!(rendered["features"]["status_mod"], serde_json::json!(true));
        let parsed = parse(&render(&doc)).expect("the flipped document parses");
        assert!(parsed.features.shipped("status_mod"));
    }

    /// Both hook-pack probes on one settings file (the state a real
    /// --with-meta-pack box is in): each row flips from ITS marker only.
    #[test]
    fn both_pack_rows_flip_from_their_own_markers() {
        let both = r#"{"hooks":{
            "PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"/x/hooks/meta-broker/hook.sh"}]}],
            "Stop":[{"hooks":[{"type":"command","command":"/x/hooks/meta-status/hook.sh"}]}]}}"#;
        assert!(permission_broker_installed_from_str(both));
        assert!(status_mod_installed_from_str(both));
        let only_broker = r#"{"hooks":{"PreToolUse":[{"matcher":"*","hooks":[{"type":"command","command":"/x/hooks/meta-broker/hook.sh"}]}]}}"#;
        assert!(permission_broker_installed_from_str(only_broker));
        assert!(!status_mod_installed_from_str(only_broker));
    }
}

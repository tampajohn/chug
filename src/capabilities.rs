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
//! package version — no daemon contact, no file I/O, no env reads. A
//! missing daemon is reported false/null, never an error and never a hang;
//! the <50ms budget is structural, not a tuning target. (The liveness
//! probe behind `daemon_status` lands with T340's row, which owns the
//! bounded-probe budget; T339 ships the contract and the reader.)
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

// The reader half (`FEATURE_KEYS`, `is_shipped`, `shipped`) has no
// production caller until T340-T343 wire their flips; the contract tests
// (src unit tests + tests/capabilities_contract.rs through `--check`)
// consume it today. Same posture as attach.rs/observ.rs: the API is the
// deliverable, the callers land with the rows that implement against it.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

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
    /// live or the daemon wire carries no version (T340 lands the report).
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

/// The document this binary prints (producer path).
pub fn document_json() -> String {
    render(&document())
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
}

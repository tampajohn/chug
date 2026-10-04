//! T222: the kev checkpoint's pure consumption logic — the pins, the
//! adapter-config parse, the LoRA scale and the base-architecture
//! classification — compiled in EVERY build (like `hf_hosting`) so its unit
//! pins run in the plain `cargo test` gate. The daemon-feature-gated half
//! (file resolution, hub probing, tensor loading) lives in `kev_model`.
//!
//! Impl-time resolution of the CONFIRMED contestant (the spec's instruction:
//! "the adapter's adapter_config.json names its base — resolve it at impl
//! time and pin BOTH the adapter sha and the base revision"), probed on the
//! HF API 2026-10-04:
//!
//! * `jaredpalmer/kev-0.8b` @ `bf75a6a8848ea6960ff2ed108d9ed44c2941174f`
//!   resolves 200 with the kev layout: adapter_config.json +
//!   adapter_model.safetensors + head.pt + tokenizer.json +
//!   tokenizer_config.json + provenance.json.
//! * The base is **`Qwen/Qwen3.5-0.8B-Base`** @
//!   `dc7cdfe2ee4154fa7e30f5b51ca41bfa40174e68` (provenance.json; the card's
//!   `dc7cdfe2` prefix agrees) — `model_type: "qwen3_5"`,
//!   Qwen3_5ForConditionalGeneration: **24 layers = 18 Gated DeltaNet
//!   (linear attention) + 6 full attention**, and the LoRA target_modules
//!   name the DeltaNet projections (`in_proj_qkv`, `in_proj_a`,
//!   `in_proj_b`, `in_proj_z`, `out_proj`) alongside the usual q/k/v/o/gate/
//!   up/down. The T220 research note's "kev is prefill-only qwen3" premise
//!   is therefore defective for this checkpoint in the same class as its
//!   verdict-2.0 repo id: the confirmed 0.8B contestant is in the spec's own
//!   "DeltaNet, candle-blocked" category (the line written for kev-4b+).
//!   candle-transformers 0.11 — the newest release on crates.io — ships no
//!   qwen3_5 model, so nothing here substitutes an architecture for it; the
//!   loader classifies the base and REFUSES with that exact reason
//!   (`kev_model`), leaving the gated-DeltaNet port as T223's first blocker.
//!
//! Card facts carried into the harness (T222 tests + T223's run): a single
//! fitted temperature (T = 2.35, stored in `head.pt`, applied at load — no
//! per-qtype/per-cardinality table like the RLAgent config); the pointer
//! head is two projections scoring each option's closing token against the
//! question's final token; the adapter is merged into the base at load
//! time; option order can flip answers (the harness pins a both-orderings
//! fixture); small-kev OOD is weak.
#![cfg_attr(not(feature = "daemon"), allow(dead_code))]

use anyhow::{bail, Result};
use serde::Deserialize;

/// The confirmed kev contestant (T220's probe; the only one this row
/// delivers against).
pub const KEV_REPO: &str = "jaredpalmer/kev-0.8b";
/// The adapter repo's pinned revision — `org/model@REV` on
/// CHUG_LAYA_CHECKPOINT (T205 discipline: a policy-affecting artifact).
pub const KEV_REVISION: &str = "bf75a6a8848ea6960ff2ed108d9ed44c2941174f";
/// The LoRA base repo, resolved at impl time from adapter_config.json.
pub const KEV_BASE_REPO: &str = "Qwen/Qwen3.5-0.8B-Base";
/// The base revision the checkpoint was trained against (provenance.json;
/// the card's `dc7cdfe2` prefix agrees).
pub const KEV_BASE_REVISION: &str = "dc7cdfe2ee4154fa7e30f5b51ca41bfa40174e68";
/// The card's single fitted temperature (stored in head.pt, applied at
/// load). The kev family has NO per-qtype/per-cardinality table — that
/// RLAgent machinery does not apply.
pub const KEV_CARD_TEMPERATURE: f64 = 2.35;

// The kev layout's file names (siblings of the pinned repo; the tokenizer
// sits at the ROOT — unlike the RLAgent layout's tokenizer/ subdirectory).
pub const F_ADAPTER_CONFIG: &str = "adapter_config.json";
pub const F_ADAPTER_MODEL: &str = "adapter_model.safetensors";
pub const F_HEAD: &str = "head.pt";
pub const F_TOKENIZER: &str = "tokenizer.json";
pub const F_TOKENIZER_CONFIG: &str = "tokenizer_config.json";
pub const F_PROVENANCE: &str = "provenance.json";

// ---------------------------------------------------------------------------
// adapter_config.json (the PEFT LoRA config)
// ---------------------------------------------------------------------------

/// The fields of `adapter_config.json` the loader consumes. The pinned
/// checkpoint's values: peft_type LORA, r 16, lora_alpha 32, base
/// Qwen/Qwen3.5-0.8B-Base, 12 target modules incl. the DeltaNet set, no
/// rslora, no DoRA, fan_in_fan_out false.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KevAdapterConfig {
    pub peft_type: String,
    pub base_model_name_or_path: String,
    pub r: usize,
    pub lora_alpha: f64,
    #[serde(default)]
    pub target_modules: Vec<String>,
    #[serde(default)]
    pub use_rslora: bool,
    #[serde(default)]
    pub use_dora: bool,
    #[serde(default)]
    pub fan_in_fan_out: bool,
}

impl KevAdapterConfig {
    pub fn parse(raw: &str) -> Result<Self> {
        let cfg: Self = serde_json::from_str(raw)
            .map_err(|e| anyhow::anyhow!("parsing {F_ADAPTER_CONFIG}: {e}"))?;
        if cfg.peft_type != "LORA" {
            bail!(
                "{F_ADAPTER_CONFIG}: peft_type {:?} is not LORA — the kev loader ports LoRA only",
                cfg.peft_type
            );
        }
        if cfg.r == 0 {
            bail!("{F_ADAPTER_CONFIG}: r must be positive");
        }
        if cfg.lora_alpha <= 0.0 {
            bail!("{F_ADAPTER_CONFIG}: lora_alpha must be positive");
        }
        if cfg.use_dora {
            bail!("{F_ADAPTER_CONFIG}: use_dora=true is not supported (DoRA merge)");
        }
        if cfg.use_rslora {
            bail!("{F_ADAPTER_CONFIG}: use_rslora=true is not supported (rank-scaled alpha)");
        }
        if cfg.target_modules.is_empty() {
            bail!("{F_ADAPTER_CONFIG}: target_modules is empty");
        }
        if cfg.fan_in_fan_out {
            // PEFT's transpose flag for Conv1d-style bases; the qwen family
            // is all Linear. Refuse rather than guess a transposed merge.
            bail!("{F_ADAPTER_CONFIG}: fan_in_fan_out=true is not supported");
        }
        Ok(cfg)
    }

    /// The merge scale for `W' = W + scale * B * A` — PEFT's
    /// `lora_alpha / r` (rslora's `alpha / sqrt(r)` is refused at parse).
    /// The pinned checkpoint: 32 / 16 = 2.0.
    pub fn lora_scale(&self) -> f64 {
        self.lora_alpha / self.r as f64
    }

    /// The base architecture the adapter targets, classified from the
    /// target_modules signature.
    pub fn base_architecture(&self) -> BaseArch {
        classify_target_modules(&self.target_modules)
    }
}

// ---------------------------------------------------------------------------
// Base-architecture classification (the candle-blocked gate's input)
// ---------------------------------------------------------------------------

/// What the LoRA target_modules say the base is. The gate in `kev_model`
/// refuses everything but [`BaseArch::PlainQwen3`] — and that only as far as
/// T222's loader reaches (the forward assembly itself is T223's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseArch {
    /// Standard qwen-family dense attention: the target set is a subset of
    /// {q,k,v,o,gate,up,down}_proj. candle-transformers has this family.
    PlainQwen3,
    /// Gated DeltaNet / linear-attention layers are targeted: the base is a
    /// hybrid (qwen3_5: 18 DeltaNet + 6 full attention for the confirmed
    /// 0.8B). candle-transformers has NO such model — candle-blocked, the
    /// spec's kev-4b+ category, which the 0.8B turns out to share.
    DeltaNetFamily,
    /// Recognizably neither — refuse with the unknown-module list rather
    /// than guess (no architecture substitution).
    Unknown(Vec<String>),
}

/// The gated-DeltaNet projection names (Qwen3.5's linear attention). A
/// plain qwen3 attention has NO `in_proj_*` — its projections are
/// q/k/v_proj — so any `in_proj_*` target is a DeltaNet marker.
const DELTANET_TARGETS: [&str; 6] = [
    "in_proj_qkv",
    "in_proj_z",
    "in_proj_a",
    "in_proj_b",
    "in_proj_qkvz",
    "in_proj_ba",
];
/// The plain qwen3 target set (attention + MLP).
const PLAIN_QWEN3_TARGETS: [&str; 7] = [
    "q_proj",
    "k_proj",
    "v_proj",
    "o_proj",
    "gate_proj",
    "up_proj",
    "down_proj",
];

pub fn classify_target_modules(targets: &[String]) -> BaseArch {
    let mut unknown: Vec<String> = Vec::new();
    let mut deltanet = false;
    for t in targets {
        if DELTANET_TARGETS.contains(&t.as_str()) {
            deltanet = true;
        } else if PLAIN_QWEN3_TARGETS.contains(&t.as_str()) {
            // plain-attention / MLP projection
        } else if t == "out_proj" || t == "conv1d" {
            // DeltaNet's output projection / short conv — attention in a
            // qwen-family base is `o_proj`, so these never appear on a
            // plain-qwen3 base.
            deltanet = true;
        } else {
            unknown.push(t.clone());
        }
    }
    if !unknown.is_empty() {
        return BaseArch::Unknown(unknown);
    }
    if deltanet {
        BaseArch::DeltaNetFamily
    } else {
        BaseArch::PlainQwen3
    }
}

// ---------------------------------------------------------------------------
// provenance.json (the T205-style base pin, verified against the consts)
// ---------------------------------------------------------------------------

/// The fields of `provenance.json` the loader verifies: WHERE the weights
/// came from (base repo + revision) and the measured artifact hashes (the
/// live leg re-hashes the downloaded files against them).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KevProvenance {
    pub config: KevProvenanceConfig,
    #[serde(default)]
    pub measured_checkpoint: Option<KevMeasuredCheckpoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KevProvenanceConfig {
    pub base: String,
    #[serde(default)]
    pub base_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KevMeasuredCheckpoint {
    #[serde(default)]
    pub adapter_sha256: Option<String>,
    #[serde(default)]
    pub head_sha256: Option<String>,
}

impl KevProvenance {
    pub fn parse(raw: &str) -> Result<Self> {
        serde_json::from_str(raw).map_err(|e| anyhow::anyhow!("parsing {F_PROVENANCE}: {e}"))
    }

    /// The base pin: the recorded base repo + revision must equal the
    /// impl-time consts EXACTLY. A moved base is a policy-affecting artifact
    /// change — the checkpoint would be a different judge — so a mismatch
    /// refuses with both sides named (T205's honesty gate, kev side).
    pub fn verify_base_pin(&self) -> Result<()> {
        let rev = self.config.base_revision.as_deref().unwrap_or("<absent>");
        if self.config.base != KEV_BASE_REPO || rev != KEV_BASE_REVISION {
            bail!(
                "{F_PROVENANCE}: base pin mismatch — recorded {}@{}, pinned {KEV_BASE_REPO}@\
                 {KEV_BASE_REVISION}; the checkpoint's base moved, so it is not the pinned \
                 judge (re-pin deliberately or refuse)",
                self.config.base,
                rev
            );
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// The pinned checkpoint's adapter_config.json, verbatim values from the
    /// 2026-10-04 probe (irrelevant PEFT knobs elided — serde defaults
    /// cover their absence here, and unknown keys are ignored by parse).
    #[test]
    fn pinned_adapter_config_parses_and_scales() {
        let raw = r#"{
            "base_model_name_or_path": "Qwen/Qwen3.5-0.8B-Base",
            "peft_type": "LORA",
            "r": 16,
            "lora_alpha": 32,
            "use_rslora": false,
            "use_dora": false,
            "fan_in_fan_out": false,
            "target_modules": [
                "in_proj_b", "in_proj_qkv", "gate_proj", "v_proj", "o_proj",
                "k_proj", "up_proj", "in_proj_z", "in_proj_a", "out_proj",
                "down_proj", "q_proj"
            ]
        }"#;
        let cfg = KevAdapterConfig::parse(raw).expect("the pinned adapter config parses");
        assert_eq!(cfg.base_model_name_or_path, KEV_BASE_REPO);
        assert_eq!(cfg.r, 16);
        assert_eq!(cfg.lora_alpha, 32.0);
        assert_eq!(cfg.lora_scale(), 2.0, "PEFT scale = alpha / r");
        assert_eq!(cfg.target_modules.len(), 12);
        // The confirmed contestant's base is the DeltaNet hybrid — the gate
        // MUST classify it candle-blocked, never plain-qwen3.
        assert_eq!(cfg.base_architecture(), BaseArch::DeltaNetFamily);
    }

    #[test]
    fn adapter_config_rejects_unsupported_variants() {
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"LORA","base_model_name_or_path":"m","r":0,"lora_alpha":16,"target_modules":["q_proj"]}"#).is_err());
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"LORA","base_model_name_or_path":"m","r":8,"lora_alpha":0,"target_modules":["q_proj"]}"#).is_err());
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"LORA","base_model_name_or_path":"m","r":8,"lora_alpha":16,"target_modules":[],"use_dora":true}"#).is_err());
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"LORA","base_model_name_or_path":"m","r":8,"lora_alpha":16,"target_modules":["q_proj"],"use_rslora":true}"#).is_err());
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"LORA","base_model_name_or_path":"m","r":8,"lora_alpha":16,"target_modules":["q_proj"],"fan_in_fan_out":true}"#).is_err());
        assert!(KevAdapterConfig::parse(r#"{"peft_type":"IA3","base_model_name_or_path":"m","r":8,"lora_alpha":16,"target_modules":["q_proj"]}"#).is_err());
        assert!(KevAdapterConfig::parse("not json").is_err());
    }

    /// The classifier: the pinned checkpoint's DeltaNet set, a plain-qwen3
    /// set, and unknown modules (refuse, never guess).
    #[test]
    fn target_modules_classify_the_base_family() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            classify_target_modules(&v(&["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"])),
            BaseArch::PlainQwen3
        );
        // DeltaNet markers fire alone or alongside plain modules.
        assert_eq!(classify_target_modules(&v(&["in_proj_qkv"])), BaseArch::DeltaNetFamily);
        assert_eq!(classify_target_modules(&v(&["out_proj"])), BaseArch::DeltaNetFamily);
        assert_eq!(classify_target_modules(&v(&["conv1d"])), BaseArch::DeltaNetFamily);
        assert_eq!(
            classify_target_modules(&v(&["q_proj", "in_proj_z", "down_proj"])),
            BaseArch::DeltaNetFamily
        );
        // Unknown modules are returned for the refusal message.
        match classify_target_modules(&v(&["q_proj", "some_new_thing"])) {
            BaseArch::Unknown(u) => assert_eq!(u, vec!["some_new_thing".to_string()]),
            other => panic!("expected Unknown, got {other:?}"),
        }
        assert_eq!(classify_target_modules(&[]), BaseArch::PlainQwen3);
    }

    /// The base pin: the recorded provenance must equal the impl-time consts
    /// exactly; either side drifting is a named refusal.
    #[test]
    fn provenance_base_pin_verifies_against_the_consts() {
        let good = format!(
            r#"{{"config":{{"base":"{KEV_BASE_REPO}","base_revision":"{KEV_BASE_REVISION}"}}}}"#
        );
        let prov = KevProvenance::parse(&good).expect("provenance parses");
        assert_eq!(prov.config.base, KEV_BASE_REPO);
        assert_eq!(prov.config.base_revision.as_deref(), Some(KEV_BASE_REVISION));
        prov.verify_base_pin().expect("the pinned base verifies");

        // 40 chars, but NOT the pinned revision.
        let moved = format!(
            r#"{{"config":{{"base":"{KEV_BASE_REPO}","base_revision":"{}"}}}}"#,
            "0".repeat(40)
        );
        let err = KevProvenance::parse(&moved)
            .expect("parses")
            .verify_base_pin()
            .expect_err("a moved base refuses");
        let msg = format!("{err:#}");
        assert!(msg.contains("base pin mismatch"), "{msg}");
        assert!(msg.contains(KEV_BASE_REVISION), "{msg}");

        // A different base repo refuses too.
        let other = r#"{"config":{"base":"Qwen/Qwen3-0.6B","base_revision":"abc"}}"#;
        assert!(KevProvenance::parse(other).expect("parses").verify_base_pin().is_err());
    }

    /// The measured hashes ride provenance for the live leg's re-hash pin.
    #[test]
    fn provenance_measured_checkpoint_parses() {
        let raw = r#"{"config":{"base":"b","base_revision":"r"},
            "measured_checkpoint":{"adapter_sha256":"aa","head_sha256":"bb"}}"#;
        let prov = KevProvenance::parse(raw).unwrap();
        let m = prov.measured_checkpoint.expect("measured checkpoint present");
        assert_eq!(m.adapter_sha256.as_deref(), Some("aa"));
        assert_eq!(m.head_sha256.as_deref(), Some("bb"));
    }

    /// The pins themselves are well-formed 40-hex revisions (a typo here
    /// would send the daemon hunting a nonexistent revision).
    #[test]
    fn revision_pins_are_40_hex() {
        for pin in [KEV_REVISION, KEV_BASE_REVISION] {
            assert_eq!(pin.len(), 40, "{pin} must be a full git sha");
            assert!(pin.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        }
        assert_eq!(KEV_REPO, "jaredpalmer/kev-0.8b");
        assert_eq!(KEV_BASE_REPO, "Qwen/Qwen3.5-0.8B-Base");
        assert_eq!(KEV_CARD_TEMPERATURE, 2.35);
    }
}

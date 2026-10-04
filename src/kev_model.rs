//! T222: the kev-layout checkpoint loader — layout discrimination, file
//! resolution through the same T205 machinery as the RLAgent path, pin
//! verification, and the base-architecture gate. Compiled ONLY under the
//! `daemon` feature (like `judge_model`): the feature-off build stays
//! candle-free (tests/daemon_feature_off.rs).
//!
//! ## What this row delivers, honestly
//!
//! The spec's premise — "kev is prefill-only qwen3, candle-transformers HAS
//! qwen3" — is defective for the CONFIRMED contestant: impl-time resolution
//! (see `kev_config`) shows `jaredpalmer/kev-0.8b` sits on
//! **Qwen/Qwen3.5-0.8B-Base**, a hybrid of 18 Gated DeltaNet (linear
//! attention) + 6 full-attention layers. candle-transformers 0.11 (the
//! newest crates.io release) ships no qwen3_5 / gated-DeltaNet model, so
//! the checkpoint is in the spec's own "DeltaNet, candle-blocked" category.
//! Loading it would need exactly the port this row's scope walls off
//! (T204-class work); substituting candle's dense qwen3 would serve a
//! DIFFERENT model — never done here.
//!
//! So the loader goes as far as honesty allows: it resolves the six-file
//! kev layout (small files first — the gate fires BEFORE any multi-hundred-MB
//! fetch), parses and validates the PEFT LoRA config, verifies the
//! provenance base pin against the impl-time consts, classifies the base
//! architecture from the target_modules signature, and then REFUSES with
//! the precise reason (see [`refusal`]). The daemon surfaces that refusal
//! exactly like any other failed model load — the process exits with the
//! message and risk-gate/notify clients fail open — so pointing
//! CHUG_LAYA_CHECKPOINT at the confirmed kev checkpoint produces a clear,
//! actionable error naming the base revision and the missing candle port,
//! never a mis-built or mislabeled model. Gating behavior is unchanged and
//! no live traffic flows (SPEC-3 stands).
//!
//! T223 (the run + verdict half) inherits, in order: (1) the gated-DeltaNet
//! prefill port for candle (or a candle-blocked verdict documented against
//! the real suite), (2) the kev input assembly — each question is its own
//! row continuing the shared state, and the pointer head scores each
//! option's closing token against the question's final token, both of which
//! need pinning from the kev suite's own artifacts — and (3) the single
//! fitted temperature T = 2.35 from head.pt. The wire seam T223 plugs into
//! is already pinned: `judge_pack::answers_payload` now takes the model
//! name and an optional act-head probability, and tests/kev_loader.rs pins
//! the kev /judge response shape (no rl_agent extension — kev has no act
//! head, and nothing is fabricated).
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::hf_hosting::{build_hub_api, map_hub_fetch_error};
use crate::judge_model::CheckpointSpec;
use crate::kev_config::{
    BaseArch, KevAdapterConfig, KevProvenance, F_ADAPTER_CONFIG, F_ADAPTER_MODEL, F_HEAD,
    F_PROVENANCE, F_TOKENIZER, F_TOKENIZER_CONFIG, KEV_CARD_TEMPERATURE, KEV_REPO, KEV_REVISION,
};

/// A resolved kev-layout checkpoint: the hub repo + pinned revision or a
/// local directory, plus the six file paths (paths are resolved lazily for
/// the hub arm — see [`KevPaths::resolve_dir`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KevPaths {
    pub adapter_config: PathBuf,
    pub adapter_model: PathBuf,
    pub head: PathBuf,
    pub tokenizer: PathBuf,
    pub tokenizer_config: PathBuf,
    pub provenance: PathBuf,
}

impl KevPaths {
    /// The Dir arm: all six files must exist on disk (nothing is fetched,
    /// so probing everything is free).
    pub fn resolve_dir(root: &Path) -> Result<Self> {
        let paths = Self {
            adapter_config: root.join(F_ADAPTER_CONFIG),
            adapter_model: root.join(F_ADAPTER_MODEL),
            head: root.join(F_HEAD),
            tokenizer: root.join(F_TOKENIZER),
            tokenizer_config: root.join(F_TOKENIZER_CONFIG),
            provenance: root.join(F_PROVENANCE),
        };
        for f in [&paths.adapter_config, &paths.head, &paths.provenance] {
            if !f.is_file() {
                bail!("kev layout: {} missing under {}", f.display(), root.display());
            }
        }
        Ok(paths)
    }
}

/// One hub fetch, cache-first through the standard HF cache (the T205
/// machinery's own shape: api → ApiRepo pinned to the revision).
pub struct HubFetcher {
    repo_id: String,
    revision: String,
    repo: hf_hub::Repo,
    api: hf_hub::api::sync::Api,
}

impl HubFetcher {
    pub fn new(repo_id: &str, revision: &str) -> Result<Self> {
        let api = build_hub_api()?;
        Ok(Self {
            repo: hf_hub::Repo::with_revision(
                repo_id.to_string(),
                hf_hub::RepoType::Model,
                revision.to_string(),
            ),
            repo_id: repo_id.to_string(),
            revision: revision.to_string(),
            api,
        })
    }

    /// The cache-first get, with the T205 error mapping (auth status named,
    /// offline hinted — the same shape the RLAgent loader produces).
    pub fn get(&self, rel: &str) -> Result<PathBuf> {
        let api_repo = self.api.repo(self.repo.clone());
        let token_set = crate::hf_hosting::hub_token().is_some();
        api_repo.get(rel).map_err(|e| {
            map_hub_fetch_error(
                anyhow::Error::new(e).context(format!(
                    "kev checkpoint {}@{}: fetching {rel} from the HF hub",
                    self.repo_id, self.revision
                )),
                &self.repo_id,
                &self.revision,
                rel,
                token_set,
                None,
            )
        })
    }
}

/// Cheap layout discrimination BEFORE any big download: is this spec the
/// kev layout (adapter + head) or the RLAgent layout?
///
/// * Dir — pure existence checks (adapter_config.json AND head.pt).
/// * Hub — one metadata call (`info()`) reading the sibling list; if the
///   hub is unreachable, a cache-first double probe decides (a warm cache
///   keeps both layouts working offline; a cold one refuses with BOTH
///   probes named instead of the RLAgent loader's misleading
///   "model.safetensors" error).
///
/// The default laya hub path pays one extra small request per daemon start
/// (the metadata call); its warm/offline behavior is unchanged.
pub fn is_kev_layout(spec: &CheckpointSpec) -> Result<bool> {
    match spec {
        CheckpointSpec::Dir(root) => {
            Ok(root.join(F_ADAPTER_CONFIG).is_file() && root.join(F_HEAD).is_file())
        }
        CheckpointSpec::Hub { repo, revision, .. } => {
            let fetcher = HubFetcher::new(repo, revision)
                .with_context(|| format!("hub client for {repo}@{revision}"))?;
            let api_repo = fetcher.api.repo(fetcher.repo.clone());
            match api_repo.info() {
                Ok(info) => {
                    let names: Vec<&str> =
                        info.siblings.iter().map(|s| s.rfilename.as_str()).collect();
                    let kev = names.contains(&F_ADAPTER_CONFIG) && names.contains(&F_HEAD);
                    let rl = names.contains(&"model.safetensors");
                    match (kev, rl) {
                        (true, _) => Ok(true),
                        (false, true) => Ok(false),
                        // A repo with neither layout is refused up front,
                        // naming what the two loaders expected.
                        (false, false) => bail!(
                            "{repo}@{revision}: neither the kev layout ({F_ADAPTER_CONFIG} + \
                             {F_HEAD}) nor the RLAgent layout (model.safetensors) is present"
                        ),
                    }
                }
                Err(info_err) => {
                    // Hub unreachable: decide cache-first, so a warm cache
                    // (the offline daemon case) still routes correctly.
                    let kev_probe = fetcher.get(F_ADAPTER_CONFIG);
                    if kev_probe.is_ok() {
                        return Ok(true);
                    }
                    let rl_probe = fetcher.get("model.safetensors");
                    match (kev_probe, rl_probe) {
                        (_, Ok(_)) => Ok(false),
                        (kev_err, rl_err) => bail!(
                            "cannot determine the layout of {repo}@{revision} (hub unreachable: \
                             {info_err}); kev probe ({F_ADAPTER_CONFIG}): {} | RLAgent probe \
                             (model.safetensors): {}",
                            kev_err.unwrap_err(),
                            rl_err.unwrap_err()
                        ),
                    }
                }
            }
        }
    }
}

/// The inspected (parsed + pin-verified) kev checkpoint, as far as T222's
/// loader reaches: the validated adapter config and the classified base
/// architecture. `spec`/`paths` are carried for the T223 continuation (it
/// loads the tensors from exactly these locations); the T222 daemon reads
/// only the classified adapter + arch — hence the scoped allow.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct KevInspection {
    pub spec: CheckpointSpec,
    pub paths: Option<KevPaths>,
    pub adapter: KevAdapterConfig,
    pub arch: BaseArch,
}

/// Inspect a kev-layout checkpoint: resolve the small files, parse and
/// validate adapter_config.json, verify the provenance base pin, and
/// classify the base. Deliberately fetches NOTHING heavy — the gate must
/// fire before a multi-hundred-MB adapter download.
pub fn inspect(spec: &CheckpointSpec) -> Result<KevInspection> {
    let (adapter_config_raw, provenance_raw, paths) = match spec {
        CheckpointSpec::Dir(root) => {
            let paths = KevPaths::resolve_dir(root)?;
            (
                std::fs::read_to_string(&paths.adapter_config)
                    .with_context(|| format!("reading {}", paths.adapter_config.display()))?,
                std::fs::read_to_string(&paths.provenance)
                    .with_context(|| format!("reading {}", paths.provenance.display()))?,
                Some(paths),
            )
        }
        CheckpointSpec::Hub { repo, revision, .. } => {
            let fetcher = HubFetcher::new(repo, revision)?;
            let read = |rel: &str| -> Result<String> {
                let path = fetcher.get(rel)?;
                std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
            };
            (
                read(F_ADAPTER_CONFIG)?,
                read(F_PROVENANCE)?,
                None, // the remaining hub paths are T223's (fetched only once a base can serve)
            )
        }
    };

    let adapter = KevAdapterConfig::parse(&adapter_config_raw)?;
    let provenance = KevProvenance::parse(&provenance_raw)?;
    provenance.verify_base_pin()?;
    // Internal consistency: the adapter config and the provenance record
    // must name the SAME base (an artifact disagreeing with its own
    // provenance is not loadable, whatever the base is).
    if adapter.base_model_name_or_path != provenance.config.base {
        bail!(
            "{F_PROVENANCE} and {F_ADAPTER_CONFIG} disagree on the base: {:?} vs {:?} — \
             internally inconsistent artifact, refusing",
            provenance.config.base,
            adapter.base_model_name_or_path
        );
    }
    let arch = adapter.base_architecture();
    Ok(KevInspection { spec: spec.clone(), paths, adapter, arch })
}

/// The classified refusal: why this kev checkpoint cannot serve TODAY.
/// Pure (no I/O) so the tests pin the exact reasons; the daemon prints it
/// and exits, leaving clients fail-open.
pub fn refusal(inspection: &KevInspection) -> String {
    let base = &inspection.adapter.base_model_name_or_path;
    let scale = inspection.adapter.lora_scale();
    match &inspection.arch {
        BaseArch::DeltaNetFamily => format!(
            "candle-blocked: the LoRA base {base} is a Gated DeltaNet hybrid (qwen3_5: linear-\
             attention projections in_proj_*/out_proj among the targets, merge scale {scale}) — \
             candle-transformers 0.11 (the newest crates.io release) has no qwen3_5 model, and \
             substituting the dense qwen3 would serve a different architecture. Porting \
             gated-DeltaNet prefill is T223's first blocker (the spec's \"DeltaNet, \
             candle-blocked\" category, which the confirmed 0.8B {KEV_REPO}@{KEV_REVISION} \
             shares with kev-4b+)."
        ),
        BaseArch::PlainQwen3 => format!(
            "not built yet: the base {base} is a plain qwen3 (candle has this family), but the \
             kev input assembly — per-question rows continuing the shared state, pointer head \
             over each option's closing token vs the question's final token, single fitted \
             temperature {KEV_CARD_TEMPERATURE} — is not pinned from the kev suite yet (T223). \
             The adapter merge scale here would be {scale}."
        ),
        BaseArch::Unknown(modules) => format!(
            "unrecognized base family: target modules {modules:?} match neither the plain \
             qwen3 set nor the Gated DeltaNet signature — refusing rather than guessing an \
             architecture"
        ),
    }
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kev_config::{KEV_BASE_REPO, KEV_BASE_REVISION};

    /// A minimal kev-layout directory built on the fly: the two files the
    /// discriminator + inspector need, with the pinned checkpoint's real
    /// config values. Nothing heavy is ever fetched.
    fn write_fixture_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(F_ADAPTER_CONFIG),
            format!(
                r#"{{"peft_type":"LORA","base_model_name_or_path":"{KEV_BASE_REPO}","r":16,
                    "lora_alpha":32,"target_modules":["q_proj","in_proj_qkv","out_proj"]}}"#
            ),
        )
        .unwrap();
        std::fs::write(
            dir.path().join(F_PROVENANCE),
            format!(
                r#"{{"config":{{"base":"{KEV_BASE_REPO}","base_revision":"{KEV_BASE_REVISION}"}}}}"#
            ),
        )
        .unwrap();
        std::fs::write(dir.path().join(F_HEAD), b"stub-head").unwrap();
        dir
    }

    #[test]
    fn dir_layout_discriminates_and_inspects_to_deltanet() {
        let dir = write_fixture_dir();
        let spec = CheckpointSpec::Dir(dir.path().to_path_buf());
        assert!(matches!(spec, CheckpointSpec::Dir(_)), "a local path is the Dir arm");
        assert!(is_kev_layout(&spec).expect("discriminates"), "adapter + head present");
        let insp = inspect(&spec).expect("inspects");
        assert_eq!(insp.adapter.lora_scale(), 2.0);
        assert_eq!(insp.arch, BaseArch::DeltaNetFamily);
        let msg = refusal(&insp);
        assert!(msg.contains("candle-blocked"), "{msg}");
        assert!(msg.contains(KEV_BASE_REPO), "{msg}");
        assert!(msg.contains("T223"), "{msg}");
    }

    #[test]
    fn dir_layout_refuses_on_missing_kev_files() {
        let dir = write_fixture_dir();
        // No head.pt → NOT the kev layout (the RLAgent loader then reports
        // its own, correctly-named layout error).
        std::fs::remove_file(dir.path().join(F_HEAD)).unwrap();
        let spec = CheckpointSpec::Dir(dir.path().to_path_buf());
        assert!(!is_kev_layout(&spec).expect("discriminates"));

        // With head.pt back but the provenance record removed, inspection
        // refuses naming the file — the pin record is not optional.
        std::fs::write(dir.path().join(F_HEAD), b"stub").unwrap();
        std::fs::remove_file(dir.path().join(F_PROVENANCE)).unwrap();
        let err = inspect(&spec).expect_err("missing provenance refuses");
        assert!(format!("{err:#}").contains(F_PROVENANCE), "{err:#}");
    }

    /// A kev directory whose provenance base pin disagrees with the consts:
    /// the inspection refuses BEFORE any classification (a moved base is a
    /// policy-affecting artifact change, the T205 discipline).
    #[test]
    fn dir_layout_refuses_on_a_moved_base_before_classifying() {
        let dir = write_fixture_dir();
        std::fs::write(
            dir.path().join(F_PROVENANCE),
            format!(r#"{{"config":{{"base":"{KEV_BASE_REPO}","base_revision":"{}"}}}}"#, "f".repeat(40)),
        )
        .unwrap();
        let spec = CheckpointSpec::Dir(dir.path().to_path_buf());
        let err = inspect(&spec).expect_err("moved base refuses");
        let msg = format!("{err:#}");
        assert!(msg.contains("base pin mismatch"), "{msg}");
    }

    /// Internally inconsistent artifact: the adapter config names a
    /// different base than its own provenance record.
    #[test]
    fn dir_layout_refuses_on_disagreeing_base_records() {
        let dir = write_fixture_dir();
        std::fs::write(
            dir.path().join(F_ADAPTER_CONFIG),
            r#"{"peft_type":"LORA","base_model_name_or_path":"Qwen/Qwen3-0.6B","r":16,
                "lora_alpha":32,"target_modules":["q_proj"]}"#,
        )
        .unwrap();
        let spec = CheckpointSpec::Dir(dir.path().to_path_buf());
        let err = inspect(&spec).expect_err("disagreeing records refuse");
        let msg = format!("{err:#}");
        assert!(msg.contains("disagree on the base"), "{msg}");
    }

    /// The plain-qwen3 arm's refusal: a future kev checkpoint on a dense
    /// qwen3 base is RECOGNIZED but still cannot serve until T223 pins the
    /// assembly — and the message says exactly that (no fake "loaded").
    #[test]
    fn plain_qwen3_base_is_recognized_but_still_refuses() {
        let dir = write_fixture_dir();
        std::fs::write(
            dir.path().join(F_ADAPTER_CONFIG),
            format!(
                r#"{{"peft_type":"LORA","base_model_name_or_path":"{KEV_BASE_REPO}","r":16,
                    "lora_alpha":32,"target_modules":["q_proj","k_proj","v_proj","o_proj"]}}"#
            ),
        )
        .unwrap();
        let spec = CheckpointSpec::Dir(dir.path().to_path_buf());
        let insp = inspect(&spec).expect("inspects");
        assert_eq!(insp.arch, BaseArch::PlainQwen3);
        let msg = refusal(&insp);
        assert!(msg.contains("not built yet"), "{msg}");
        assert!(msg.contains("T223"), "{msg}");
        // And a fully unknown family refuses without guessing (the base
        // record stays consistent so only the family is unknown).
        std::fs::write(
            dir.path().join(F_ADAPTER_CONFIG),
            format!(
                r#"{{"peft_type":"LORA","base_model_name_or_path":"{KEV_BASE_REPO}","r":16,
                    "lora_alpha":32,"target_modules":["w1","w2"]}}"#
            ),
        )
        .unwrap();
        let insp = inspect(&spec).expect("inspects");
        assert!(matches!(refusal(&insp), m if m.contains("unrecognized base family")));
    }
}

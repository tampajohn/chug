//! T204 phase 1 (F15): the laya inference core — the RLAgent-layout
//! checkpoint loader + ModernBERT-large encoder forward + decision head, all
//! in candle, behind the `daemon` cargo feature (off by default, so the hot
//! `chug run` path never compiles candle — pinned by
//! tests/daemon_feature_off.rs). The socket server, lifecycle and
//! CHUG_JUDGE client that host THIS core are the next T204 slice.
//!
//! Ground truth is the Python SDK (rl_agent_api.py + rl_common.py): a
//! 421M-param ModernBERT-large encoder (`answerdotai/ModernBERT-large`,
//! fused-Wqkv/Wi checkpoint naming) feeding a from-scratch decision head —
//! a 2-layer norm-first TransformerEncoder (relu FFN), a per-option [MASK]
//! scorer (LayerNorm→Linear→GELU→Linear) and an escalation act head
//! (Linear(1028→256)→GELU→Linear(256→n_act)) over the CLS pool plus four
//! scalar features of the detached answer distribution. candle-transformers
//! ships ModernBERT; the head is ported here (candle-nn has no
//! PyTorch-compatible TransformerEncoderLayer). NO architecture
//! substitution — SPEC-3 carries over verbatim: classification only, never
//! completion/stuck/verdict-final judgments.
//!
//! Parity chain, weightless → live:
//!   * judge_pack.rs pins serialization, sequence assembly and answer shape
//!     against committed token fixtures (always on);
//!   * the LIVE test (CHUG_LAYA_LIVE_PARITY=1) runs the committed golden
//!     vectors (tests/fixtures/laya/golden-vectors.json, probabilities
//!     within 1e-3, incl. the billing case) through tokenizer → assembly →
//!     forward → temperature scaling, checkpoint-by-checkpoint
//!     (convaiinnovations/laya and the tampajohn RLAgent fine-tune), plus
//!     token-id parity vs the same fixtures' recorded ids.
//!
//! Checkpoints: RLAgent fine-tunes share one layout (model.safetensors +
//! rl_agent_config.json + tokenizer/ + encoder/config.json), resolved from
//! a local dir or the HF hub — cache-first into the standard HF cache
//! (~/.cache/huggingface), downloading on a miss, clear error offline.
//! CHUG_LAYA_CHECKPOINT overrides the default (convaiinnovations/laya).
//!
//! Phase-1 slice note: the daemon slice wires the served path; until it
//! lands this surface is test-only, hence the scoped allow.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use candle_core::{DType, Device, IndexOp, Tensor, D};
use candle_nn::ops::softmax;
use candle_nn::{embedding, layer_norm, linear, Embedding, LayerNorm, Linear, Module, VarBuilder};
use candle_transformers::models::modernbert::{Config as EncoderConfig, ModernBert};
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::judge_pack::{
    answers_payload, build_sequence, parse_ordered, qtype_index, serialize_state, to_internal,
    Inference, InternalQ, OValue, Temperatures,
};

/// The default judge checkpoint (the shipped base model).
pub const DEFAULT_CHECKPOINT: &str = "convaiinnovations/laya";
/// Operator override for the default checkpoint: a local RLAgent-layout dir
/// or a hub repo id. (Checked per-load so the daemon slice can resolve once
/// at startup.)
pub const CHECKPOINT_ENV: &str = "CHUG_LAYA_CHECKPOINT";
/// Env gate for the live (weights-loaded) parity tests — default runs never
/// touch weights or the network.
pub const LIVE_PARITY_ENV: &str = "CHUG_LAYA_LIVE_PARITY";

const F_SAFETENSORS: &str = "model.safetensors";
const F_RL_CONFIG: &str = "rl_agent_config.json";
const F_TOKENIZER: &str = "tokenizer/tokenizer.json";
const F_TOKENIZER_CONFIG: &str = "tokenizer/tokenizer_config.json";
const F_ENCODER_CONFIG: &str = "encoder/config.json";

// ---------------------------------------------------------------------------
// Checkpoint resolution
// ---------------------------------------------------------------------------

/// Where the checkpoint comes from: a local RLAgent-layout directory or an
/// HF hub repo id (cache-first, standard HF cache).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointSpec {
    Dir(PathBuf),
    Hub(String),
}

impl CheckpointSpec {
    /// The daemon's default checkpoint: CHUG_LAYA_CHECKPOINT when set (an
    /// existing directory wins over a repo id), else the shipped base model.
    pub fn resolve() -> Self {
        match std::env::var(CHECKPOINT_ENV) {
            Ok(v) if !v.trim().is_empty() => Self::from_user(v.trim()),
            _ => Self::Hub(DEFAULT_CHECKPOINT.to_string()),
        }
    }

    fn from_user(v: &str) -> Self {
        if Path::new(v).is_dir() {
            Self::Dir(PathBuf::from(v))
        } else {
            Self::Hub(v.to_string())
        }
    }

    fn hub(repo: &str) -> Self {
        Self::Hub(repo.to_string())
    }
}

/// The five files of the RLAgent layout, resolved to concrete paths.
struct CheckpointFiles {
    safetensors: PathBuf,
    rl_config: PathBuf,
    tokenizer: PathBuf,
    tokenizer_config: PathBuf,
    encoder_config: PathBuf,
}

impl CheckpointFiles {
    fn resolve(spec: &CheckpointSpec) -> Result<Self> {
        const LAYOUT: &str = "expected the RLAgent layout: model.safetensors, \
             rl_agent_config.json, tokenizer/tokenizer.json, \
             tokenizer/tokenizer_config.json, encoder/config.json";
        match spec {
            CheckpointSpec::Dir(root) => {
                let join = |rel: &str| -> Result<PathBuf> {
                    let p = root.join(rel);
                    if p.exists() {
                        Ok(p)
                    } else {
                        bail!(
                            "checkpoint dir {}: missing {rel} ({LAYOUT})",
                            root.display()
                        )
                    }
                };
                Ok(Self {
                    safetensors: join(F_SAFETENSORS)?,
                    rl_config: join(F_RL_CONFIG)?,
                    tokenizer: join(F_TOKENIZER)?,
                    tokenizer_config: join(F_TOKENIZER_CONFIG)?,
                    encoder_config: join(F_ENCODER_CONFIG)?,
                })
            }
            CheckpointSpec::Hub(repo) => {
                // hf-hub's ApiRepo::get is cache-first: a warm HF cache
                // (the spec's standard ~/.cache/huggingface) never touches
                // the network; a miss downloads; offline + cold is a clear
                // error the daemon surfaces (clients fail-open as today).
                let api = hf_hub::api::sync::Api::new()
                    .context("initializing the HF hub client")?;
                let repo_api = api.model(repo.clone());
                let get = |rel: &str| -> Result<PathBuf> {
                    repo_api.get(rel).with_context(|| {
                        format!(
                            "checkpoint {repo}: fetching {rel} from the HF hub \
                             (offline? pre-stage the cache with the standard HF tooling)"
                        )
                    })
                };
                Ok(Self {
                    safetensors: get(F_SAFETENSORS)?,
                    rl_config: get(F_RL_CONFIG)?,
                    tokenizer: get(F_TOKENIZER)?,
                    tokenizer_config: get(F_TOKENIZER_CONFIG)?,
                    encoder_config: get(F_ENCODER_CONFIG)?,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Config files
// ---------------------------------------------------------------------------

/// rl_agent_config.json — the fields inference consumes. Temperatures fall
/// back to 1.0 like the SDK (`cfg.get("temperature", [1., 1., 1.])`).
#[derive(Debug, Deserialize)]
struct RLAgentConfig {
    #[serde(default)]
    #[allow(dead_code)] // kept for diagnostics; inference keys off the tensors
    encoder: String,
    head_layers: usize,
    max_len: usize,
    head_max_len: usize,
    #[serde(default = "unit_temperatures")]
    temperature: [f64; 3],
    #[serde(default)]
    temperature_by_options: HashMap<String, f64>,
    #[serde(default)]
    act_costs: HashMap<String, f64>,
}

fn unit_temperatures() -> [f64; 3] {
    [1.0, 1.0, 1.0]
}

impl RLAgentConfig {
    fn load(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", path.display()))?;
        if cfg.max_len == 0 || cfg.head_max_len == 0 {
            bail!("{}: max_len and head_max_len must be positive", path.display());
        }
        if cfg.head_layers == 0 {
            bail!("{}: head_layers must be positive", path.display());
        }
        Ok(cfg)
    }

    fn temperatures(&self) -> Temperatures {
        Temperatures {
            per_qtype: self.temperature,
            by_options: self.temperature_by_options.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        }
    }
}

/// encoder/config.json -> candle's ModernBERT config. The SDK checkpoints
/// ship the transformers-5.x shape (rope thetas nested under
/// `rope_parameters.<layer_type>.rope_theta`); the ModernBERT defaults
/// (160000 global / 10000 local) back the fallback.
fn encoder_config(v: &serde_json::Value) -> Result<EncoderConfig> {
    let get = |k: &str| -> Result<&serde_json::Value> {
        v.get(k).with_context(|| format!("encoder config: missing {k}"))
    };
    let get_num = |k: &str| -> Result<f64> {
        get(k)?
            .as_f64()
            .with_context(|| format!("encoder config: {k} not a number"))
    };
    let get_usize = |k: &str| -> Result<usize> {
        Ok(get_num(k)? as usize)
    };
    let rope_theta = |layer_type: &str, dflt: f64| -> f64 {
        v.get("rope_parameters")
            .and_then(|rp| rp.get(layer_type))
            .and_then(|e| e.get("rope_theta"))
            .and_then(|t| t.as_f64())
            .unwrap_or(dflt)
    };
    Ok(EncoderConfig {
        vocab_size: get_usize("vocab_size")?,
        hidden_size: get_usize("hidden_size")?,
        num_hidden_layers: get_usize("num_hidden_layers")?,
        num_attention_heads: get_usize("num_attention_heads")?,
        intermediate_size: get_usize("intermediate_size")?,
        max_position_embeddings: get_usize("max_position_embeddings")?,
        layer_norm_eps: get_num("layer_norm_eps")?,
        pad_token_id: get_usize("pad_token_id")? as u32,
        global_attn_every_n_layers: get_usize("global_attn_every_n_layers")?,
        global_rope_theta: rope_theta("full_attention", 160000.0),
        local_attention: get_usize("local_attention")?,
        local_rope_theta: rope_theta("sliding_attention", 10000.0),
        classifier_config: None,
    })
}

/// Special-token ids: cls/sep/pad from the encoder config (the model's own
/// ids), the [MASK] string from the tokenizer config (build_sequence
/// mask-masks every text through it).
struct SpecialTokens {
    cls: u32,
    sep: u32,
    pad: u32,
    mask: u32,
    mask_text: String,
}

fn special_tokens(
    enc_cfg: &serde_json::Value,
    tok_cfg: &serde_json::Value,
    tok: &Tokenizer,
) -> Result<SpecialTokens> {
    let id = |key: &str| -> Result<u32> {
        enc_cfg
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .with_context(|| format!("encoder config: missing {key}"))
    };
    let mask_text = match tok_cfg.get("mask_token") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(o) if o.is_object() => o
            .get("content")
            .and_then(|c| c.as_str())
            .with_context(|| "tokenizer config: mask_token object without content")?
            .to_string(),
        _ => bail!("tokenizer config: missing mask_token"),
    };
    let mask = tok
        .token_to_id(&mask_text)
        .with_context(|| format!("tokenizer: no id for mask token {mask_text:?}"))?;
    Ok(SpecialTokens {
        cls: id("cls_token_id")?,
        sep: id("sep_token_id")?,
        pad: id("pad_token_id")?,
        mask,
        mask_text,
    })
}

// ---------------------------------------------------------------------------
// The decision head (rl_common.DecisionModel's torch pieces, ported)
// ---------------------------------------------------------------------------

/// One `nn.TransformerEncoderLayer(d, nhead, 4d, dropout, batch_first=True,
/// norm_first=True)` — dropout is training-only (the SDK runs eval()), so
/// the port is the eval path only: pre-norm self-attention + pre-norm relu
/// FFN, both residual.
struct HeadLayer {
    in_proj: Linear,  // fused QKV [3d, d] (torch MultiheadAttention in_proj)
    out_proj: Linear, // [d, d]
    linear1: Linear,  // FFN up [4d, d]
    linear2: Linear,  // FFN down [d, 4d]
    norm1: LayerNorm,
    norm2: LayerNorm,
    n_heads: usize,
    head_dim: usize,
    scale: f64,
}

impl HeadLayer {
    fn load(vb: VarBuilder, hidden: usize, eps: f64) -> Result<Self> {
        // Python: nhead = max(1, d // 64)
        let n_heads = (hidden / 64).max(1);
        let head_dim = hidden / n_heads;
        // torch MultiheadAttention stores the fused QKV projection flat
        // (self_attn.in_proj_weight/in_proj_bias) while out_proj is a
        // submodule (self_attn.out_proj.{weight,bias}) — load accordingly.
        let vb_attn = vb.pp("self_attn");
        let in_proj = Linear::new(
            vb_attn.get((3 * hidden, hidden), "in_proj_weight")?,
            Some(vb_attn.get(3 * hidden, "in_proj_bias")?),
        );
        Ok(Self {
            in_proj,
            out_proj: linear(hidden, hidden, vb_attn.pp("out_proj"))?,
            linear1: linear(hidden, 4 * hidden, vb.pp("linear1"))?,
            linear2: linear(4 * hidden, hidden, vb.pp("linear2"))?,
            norm1: layer_norm(hidden, eps, vb.pp("norm1"))?,
            norm2: layer_norm(hidden, eps, vb.pp("norm2"))?,
            n_heads,
            head_dim,
            scale: (head_dim as f64).powf(-0.5),
        })
    }

    fn forward(&self, xs: &Tensor, pad_mask: &Tensor) -> Result<Tensor> {
        let residual = xs;
        // --- pre-norm multi-head self-attention (torch _sa_block) ---
        let x = xs.apply(&self.norm1)?;
        let (b, seq, hidden) = x.dims3()?;
        let qkv = x
            .apply(&self.in_proj)?
            .reshape((b, seq, 3, self.n_heads, self.head_dim))?
            .permute((2, 0, 3, 1, 4))?;
        let q = qkv.get(0)?;
        let k = qkv.get(1)?;
        let v = qkv.get(2)?;
        let q = (q * self.scale)?; // torch MHA scales q by head_dim**-0.5
        let att = q.matmul(&k.t()?)?; // (b, h, seq, seq)
        let att = att.broadcast_add(pad_mask)?;
        let att = softmax(&att, D::Minus1)?;
        let o = att
            .matmul(&v)?
            .transpose(1, 2)?
            .reshape((b, seq, hidden))?;
        let x = (residual + o.apply(&self.out_proj)?)?;
        // --- pre-norm relu FFN (torch _ff_block) ---
        let residual = &x;
        let f = x
            .apply(&self.norm2)?
            .apply(&self.linear1)?
            .relu()?
            .apply(&self.linear2)?;
        Ok((residual + f)?)
    }
}

/// The tensors of one collated judge request (system_one's `collate_items`
/// output, host-side): one row per question, padded to the batch max.
struct Batch {
    input_ids: Vec<u32>,
    attention: Vec<f32>,
    qtype: Vec<u32>,
    marker_pos: Vec<u32>,
    marker_mask: Vec<u8>,
    n_tokens: usize,
    b: usize,
    l: usize,
    kmax: usize,
}

/// One question's packed sequence (`collate_items`' item: ids, [MASK] marker
/// positions, the qtype embedding index).
struct Item {
    ids: Vec<u32>,
    markers: Vec<usize>,
    qtype: usize,
}

/// collate_items for a single request group: right-pad input_ids with the
/// tokenizer's pad id (attention 0), zero-fill marker positions past a
/// row's option count (marker_mask 0).
fn collate(items: &[Item], pad: u32) -> Batch {
    let b = items.len();
    let l = items.iter().map(|it| it.ids.len()).max().unwrap_or(0);
    let kmax = items.iter().map(|it| it.markers.len()).max().unwrap_or(0);
    let mut input_ids = vec![pad; b * l];
    let mut attention = vec![0f32; b * l];
    let mut marker_pos = vec![0u32; b * kmax];
    let mut marker_mask = vec![0u8; b * kmax];
    let mut n_tokens = 0usize;
    for (r, it) in items.iter().enumerate() {
        input_ids[r * l..r * l + it.ids.len()].copy_from_slice(&it.ids);
        attention[r * l..r * l + it.ids.len()].fill(1.0);
        n_tokens += it.ids.len();
        for (j, &m) in it.markers.iter().enumerate() {
            marker_pos[r * kmax + j] = m as u32;
            marker_mask[r * kmax + j] = 1;
        }
    }
    Batch { input_ids, attention, qtype: items.iter().map(|it| it.qtype as u32).collect(), marker_pos, marker_mask, n_tokens, b, l, kmax }
}

// ---------------------------------------------------------------------------
// JudgeModel
// ---------------------------------------------------------------------------

/// The loaded judge: tokenizer + config + weights, warm for the daemon's
/// lifetime (one load per host, shared by every request — the
/// DAEMON-NOT-IN-PROCESS doctrine; the per-run hot path stays candle-free).
pub struct JudgeModel {
    tokenizer: Tokenizer,
    specials: SpecialTokens,
    max_len: usize,
    head_max_len: usize,
    temperatures: Temperatures,
    encoder: ModernBert,
    head: Vec<HeadLayer>,
    type_emb: Embedding,
    scorer_norm: LayerNorm,
    scorer_mid: Linear,
    scorer_out: Linear,
    act_mid: Linear,
    act_out: Linear,
    hidden: usize,
    device: Device,
    spec: CheckpointSpec,
}

impl JudgeModel {
    /// Load the daemon's default checkpoint (CHUG_LAYA_CHECKPOINT-aware).
    pub fn load_default() -> Result<Self> {
        Self::load(&CheckpointSpec::resolve())
    }

    /// Load one checkpoint: local dir or hub repo, cache-first.
    pub fn load(spec: &CheckpointSpec) -> Result<Self> {
        let files = CheckpointFiles::resolve(spec)?;
        let rl = RLAgentConfig::load(&files.rl_config)?;

        let enc_json: serde_json::Value = parse_json_file(&files.encoder_config)?;
        let mb_cfg = encoder_config(&enc_json)?;
        let tok_json: serde_json::Value = parse_json_file(&files.tokenizer_config)?;

        let mut tokenizer = Tokenizer::from_file(&files.tokenizer)
            .map_err(|e| anyhow!("loading {}: {e}", files.tokenizer.display()))?;
        // Parity-critical: the SDK tokenizes raw pieces with no implicit
        // specials/padding (build_sequence adds CLS/SEP itself).
        tokenizer
            .with_truncation(None)
            .map_err(|e| anyhow!("disabling tokenizer truncation: {e}"))?;
        tokenizer.with_padding(None);

        let device = default_device()?;
        let specials = special_tokens(&enc_json, &tok_json, &tokenizer)?;

        // Safetensors: fp16 on disk -> fp32 compute (the SDK's fp32 CPU
        // goldens are the parity target; python's load_state_dict casts the
        // same way). Drained one tensor at a time so the fp16 map frees as
        // the fp32 map grows.
        let raw = candle_core::safetensors::load(&files.safetensors, &Device::Cpu)
            .with_context(|| format!("reading {}", files.safetensors.display()))?;
        let mut tensors: HashMap<String, Tensor> = HashMap::with_capacity(raw.len());
        for (k, v) in raw {
            tensors.insert(k, v.to_dtype(DType::F32)?);
        }
        validate_layout(&tensors, &rl)?;

        // The encoder lives under checkpoint prefix "encoder."; candle's
        // ModernBERT expects "model." — one rename pass, no tensor rewrite.
        let encoder_map: HashMap<String, Tensor> = tensors
            .iter()
            .filter(|(k, _)| k.starts_with("encoder."))
            .map(|(k, v)| (format!("model.{}", &k["encoder.".len()..]), v.clone()))
            .collect();
        let encoder = ModernBert::load(
            VarBuilder::from_tensors(encoder_map, DType::F32, &device),
            &mb_cfg,
        )?;

        let vb = VarBuilder::from_tensors(tensors, DType::F32, &device);
        let hidden = mb_cfg.hidden_size;
        let head = (0..rl.head_layers)
            .map(|i| HeadLayer::load(vb.pp("head").pp("layers").pp(i), hidden, 1e-5))
            .collect::<Result<Vec<_>>>()?;
        // DecisionModel's fixed eps: nn.LayerNorm/nn.Linear defaults (1e-5);
        // the ENCODER's eps comes from encoder config (already wired above).
        let type_emb = embedding(3, hidden, vb.pp("type_emb"))?;
        let scorer_norm = layer_norm(hidden, 1e-5, vb.pp("scorer").pp(0))?;
        let scorer_mid = linear(hidden, hidden, vb.pp("scorer").pp(1))?;
        let scorer_out = linear(hidden, 1, vb.pp("scorer").pp(3))?;
        let act_mid = linear(hidden + 4, 256, vb.pp("act_head").pp(0))?;
        let n_act = rl.act_costs.len() + 1;
        let act_out = linear(256, n_act, vb.pp("act_head").pp(2))?;
        let (out_dim, _) = act_out.weight().dims2()?;
        if out_dim != n_act {
            bail!(
                "checkpoint {}: act head width {out_dim} disagrees with act_costs ({n_act})",
                files.safetensors.display()
            );
        }

        Ok(Self {
            tokenizer,
            specials,
            max_len: rl.max_len,
            head_max_len: rl.head_max_len,
            temperatures: rl.temperatures(),
            encoder,
            head,
            type_emb,
            scorer_norm,
            scorer_mid,
            scorer_out,
            act_mid,
            act_out,
            hidden,
            device,
            spec: spec.clone(),
        })
    }

    pub fn spec(&self) -> &CheckpointSpec {
        &self.spec
    }

    /// add_special_tokens=False encode — the SDK tokenizes raw pieces;
    /// build_sequence adds CLS/SEP itself.
    fn encode(&self, text: &str) -> Result<Vec<u32>> {
        Ok(self
            .tokenizer
            .encode(text, false)
            .map_err(|e| anyhow!("tokenization failed: {e}"))?
            .get_ids()
            .to_vec())
    }

    /// system_one: a /judge request body ({state, questions}) -> the layad
    /// response payload ({model, answers, usage}). Questions in request
    /// order; one batched forward for the whole request.
    pub fn judge(&self, state: &OValue, questions: &OValue) -> Result<serde_json::Value> {
        let qdefs = match questions {
            OValue::Obj(entries) => entries,
            _ => bail!("questions must be a JSON object (id -> definition)"),
        };
        if qdefs.is_empty() {
            bail!("questions must not be empty");
        }
        let state_text = serialize_state(state).replace(&self.specials.mask_text, " ");
        let state_ids = self.encode(&state_text)?;

        let mut qs: Vec<(String, InternalQ)> = Vec::with_capacity(qdefs.len());
        let mut items: Vec<Item> = Vec::with_capacity(qdefs.len());
        for (qid, qdef) in qdefs {
            let q = to_internal(qdef)
                .map_err(|e| anyhow!("question {qid:?}: {e}"))?;
            let qt = qtype_index(&q.t)
                .ok_or_else(|| anyhow!("question {qid:?}: unknown type {:?}", q.t))?;
            let opts = q.options();
            // head text: "<type> question: <instructions>"; options get one
            // token each of lead-in space; every text is mask-masked first.
            let head_text = format!(
                "{} question: {}",
                q.t,
                q.ins.replace(&self.specials.mask_text, " ")
            );
            let head_ids = self.encode(&head_text)?;
            let mut opt_ids = Vec::with_capacity(opts.len());
            for o in &opts {
                let text = format!(" {}", o.replace(&self.specials.mask_text, " "));
                let mut ids = Vec::with_capacity(49);
                ids.push(self.specials.mask);
                ids.extend(self.encode(&text)?.into_iter().take(48));
                opt_ids.push(ids);
            }
            let (ids, markers) = build_sequence(
                &head_ids,
                &opt_ids,
                &state_ids,
                self.max_len,
                self.head_max_len,
                false,
                self.specials.cls,
                self.specials.sep,
            );
            if markers.len() != opts.len() {
                bail!(
                    "question {qid:?}: options do not fit in head_max_len={} tokens",
                    self.head_max_len
                );
            }
            qs.push((qid.clone(), q));
            items.push(Item { ids, markers, qtype: qt });
        }

        let batch = collate(&items, self.specials.pad);
        let (logits_rows, act) = self.forward(&batch)?;
        let infs: Vec<Inference> = items
            .iter()
            .enumerate()
            .map(|(r, it)| Inference {
                logits: logits_rows[r][..it.markers.len()].to_vec(),
                act_probability: act[r],
            })
            .collect();
        Ok(answers_payload(&qs, &infs, batch.n_tokens, &self.temperatures))
    }

    /// Parse a raw /judge request body (order-preserving) and judge it —
    /// the daemon handler's whole per-request job, minus transport.
    pub fn judge_request(&self, raw: &str) -> Result<serde_json::Value> {
        let v = parse_ordered(raw).map_err(|e| anyhow!("invalid request JSON: {e}"))?;
        let state = v
            .field_pub("state")
            .ok_or_else(|| anyhow!("request missing \"state\""))?;
        let questions = v
            .field_pub("questions")
            .ok_or_else(|| anyhow!("request missing \"questions\""))?;
        self.judge(state, questions)
    }

    /// DecisionModel.forward: encoder -> +type_emb -> head layers -> [MASK]
    /// gather -> scorer logits (masked_fill -1e4) + act-head probability.
    /// Returns (logits per row [kmax], act_probability per row).
    fn forward(&self, batch: &Batch) -> Result<(Vec<Vec<f32>>, Vec<f32>)> {
        let dev = &self.device;
        let input_ids = Tensor::from_vec(batch.input_ids.clone(), (batch.b, batch.l), dev)?;
        let attention = Tensor::from_vec(batch.attention.clone(), (batch.b, batch.l), dev)?;

        // Encoder (ModernBERT handles rope, the sliding-window mask and the
        // padding mask internally; fp32 throughout).
        let mut h = self.encoder.forward(&input_ids, &attention)?;

        // + per-row qtype embedding, broadcast over positions.
        let qtype = Tensor::from_vec(batch.qtype.clone(), (batch.b,), dev)?;
        let te = self.type_emb.forward(&qtype)?.unsqueeze(1)?;
        h = h.broadcast_add(&te)?;

        // Head attention mask: torch's key_padding_mask as the float form
        // it builds internally — 0.0 real, -inf padded (b, 1, 1, l).
        let mut pad = vec![0f32; batch.b * batch.l];
        for (i, &a) in batch.attention.iter().enumerate() {
            if a == 0.0 {
                pad[i] = f32::NEG_INFINITY;
            }
        }
        let pad_mask = Tensor::from_vec(pad, (batch.b, 1, 1, batch.l), dev)?;
        for layer in &self.head {
            h = layer.forward(&h, &pad_mask)?;
        }

        // Gather the [MASK] positions per row (padded slots clamp to 0 like
        // torch's marker_pos.clamp(min=0); they are masked out below).
        let mut rows = Vec::with_capacity(batch.b);
        for r in 0..batch.b {
            let hr = h.i((r, .., ..))?;
            let pos = batch.marker_pos[r * batch.kmax..(r + 1) * batch.kmax].to_vec();
            let idx = Tensor::from_vec(pos, (batch.kmax,), dev)?;
            rows.push(hr.index_select(&idx, 0)?);
        }
        let m = Tensor::cat(&rows, 0)?.reshape((batch.b, batch.kmax, self.hidden))?;

        // scorer: LayerNorm -> Linear -> GELU -> Linear(->1)
        let logits = m
            .apply(&self.scorer_norm)?
            .apply(&self.scorer_mid)?
            .gelu_erf()?
            .apply(&self.scorer_out)?
            .squeeze(D::Minus1)?; // (b, kmax)
        let cond = Tensor::from_vec(batch.marker_mask.clone(), (batch.b, batch.kmax), dev)?;
        let neg = Tensor::full(-1e4f32, (batch.b, batch.kmax), dev)?;
        let logits = cond.where_cond(&logits, &neg)?; // masked_fill(~mask, -1e4)

        // act-head features from the DETACHED distribution (eval: no autograd
        // to detach, values identical): softmax over the FULL kmax row with
        // the -1e4 fills (their prob underflows to exact 0, like torch).
        let p = softmax(&logits, D::Minus1)?;
        let am = p.argmax(D::Minus1)?; // (b,)
        let onehot = am
            .unsqueeze(1)? // (b, 1)
            .broadcast_as((batch.b, batch.kmax))?
            .eq(
                &Tensor::arange(0u32, batch.kmax as u32, dev)?
                    .unsqueeze(0)? // (1, kmax)
                    .broadcast_as((batch.b, batch.kmax))?,
            )?
            .to_dtype(DType::F32)?; // (b, kmax)
        let rest = (&p * &onehot.affine(-1.0, 1.0)?)?; // argmax zeroed out
        let max1 = p.max(D::Minus1)?.unsqueeze(1)?;
        let max2 = rest.max(D::Minus1)?.unsqueeze(1)?;
        // p in [0,1], so clamp(1e-9, 1.0) == torch's clamp_min(1e-9)
        let ent = (&p * &p.clamp(1e-9f32, 1.0f32)?.log()?)?
            .sum_keepdim(D::Minus1)?
            .neg()?; // (b, 1)
        // k = marker_mask.sum().clamp(min=2) -> log(k) in f32 (torch), plus
        // the k/255 feature, both host-computed like the mask itself.
        let mut log_k = vec![0f32; batch.b];
        let mut k255 = vec![0f32; batch.b];
        for r in 0..batch.b {
            let ki: u32 = batch.marker_mask[r * batch.kmax..(r + 1) * batch.kmax]
                .iter()
                .map(|&m| u32::from(m != 0))
                .sum();
            let k = ki.max(2) as f32;
            log_k[r] = k.ln();
            k255[r] = k / 255.0;
        }
        let log_k = Tensor::from_vec(log_k, (batch.b, 1), dev)?;
        let k255 = Tensor::from_vec(k255, (batch.b, 1), dev)?;
        let ent = ent.broadcast_div(&log_k)?;
        let top2_gap = (&max1 - &max2)?; // top2[0] - top2[1]
        let feats = Tensor::cat(&[max1, top2_gap, ent, k255], D::Minus1)?; // (b, 4)

        let pooled = h.i((.., 0, ..))?.contiguous()?; // CLS position (b, d)
        let act_in = Tensor::cat(&[pooled, feats], D::Minus1)?; // (b, d+4)
        let act_logits = act_in
            .apply(&self.act_mid)?
            .gelu_erf()?
            .apply(&self.act_out)?; // (b, n_act)
        let act_probs = softmax(&act_logits, D::Minus1)?;

        let logits_rows = logits.to_vec2::<f32>()?;
        let act_rows = act_probs.to_vec2::<f32>()?;
        Ok((
            logits_rows,
            act_rows.iter().map(|row| row[0]).collect(),
        ))
    }
}

fn parse_json_file(path: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parsing {}", path.display()))
}

/// The RLAgent tensor-layout pin: fail with a layout-naming error (not a
/// deep candle shape error) when the checkpoint is not the expected shape.
fn validate_layout(t: &HashMap<String, Tensor>, rl: &RLAgentConfig) -> Result<()> {
    const LAYOUT: &str = "expected RLAgent naming: encoder.*, head.layers.N.*, \
         scorer.{0,1,3}.*, act_head.{0,2}.*, type_emb.weight, temperature";
    let required = [
        "encoder.embeddings.tok_embeddings.weight",
        "encoder.final_norm.weight",
        "head.layers.0.self_attn.in_proj_weight",
        "head.layers.0.self_attn.out_proj.weight",
        "head.layers.0.linear1.weight",
        "head.layers.0.norm1.weight",
        "scorer.0.weight",
        "scorer.1.weight",
        "scorer.3.weight",
        "act_head.0.weight",
        "act_head.2.weight",
        "type_emb.weight",
        "temperature",
    ];
    for key in required {
        if !t.contains_key(key) {
            bail!("checkpoint is not the RLAgent layout: missing tensor {key:?} ({LAYOUT})");
        }
    }
    for i in 0..rl.head_layers {
        for key in [
            format!("head.layers.{i}.linear2.weight"),
            format!("head.layers.{i}.norm2.weight"),
        ] {
            if !t.contains_key(&key) {
                bail!("checkpoint is not the RLAgent layout: missing tensor {key:?} ({LAYOUT})");
            }
        }
    }
    Ok(())
}

/// CPU for phase 1. Metal is compiled in on Apple Silicon (the Cargo.toml
/// release leg), but candle 0.11's rotary-emb custom op has no Metal
/// kernel — `ModernBert::forward` on the GPU dies with "no metal
/// implementation for rotary-emb" — and CPU fp32 is also the reference
/// device the parity goldens were generated on. Revisit GPU execution when
/// candle ships a metal rope (or the encoder loop is ported); the
/// classification layer is warm-model-bound anyway.
fn default_device() -> Result<Device> {
    Ok(Device::Cpu)
}

// ---------------------------------------------------------------------------

impl OValue {
    /// Field access without the crate-private judge_pack helper (the test
    /// module walks fixture trees; the daemon handler uses parse_ordered +
    /// judge_pack::to_internal directly).
    fn field_pub(&self, key: &str) -> Option<&OValue> {
        match self {
            OValue::Obj(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live() -> bool {
        std::env::var(LIVE_PARITY_ENV).ok().as_deref() == Some("1")
    }

    // ------------------------------------------------------------------
    // Weightless: config parsing + spec resolution (no weights, no net).
    // ------------------------------------------------------------------

    #[test]
    fn rl_agent_config_parses_and_defaults() {
        let raw = r#"{
            "encoder": "answerdotai/ModernBERT-large",
            "head_layers": 2, "max_len": 512, "head_max_len": 192,
            "act_costs": {"escalate": 0.5},
            "temperature": [1.6369030475616455, 1.2514300346374512, 1.983399510383606],
            "temperature_by_options": {"choice:3-5": 1.7601518630981445, "noul:2": 1.983399510383606}
        }"#;
        let cfg: RLAgentConfig = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.max_len, 512);
        assert_eq!(cfg.head_max_len, 192);
        assert_eq!(cfg.head_layers, 2);
        assert_eq!(cfg.act_costs.len() + 1, 2);
        let temps = cfg.temperatures();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
        assert!(close(temps.lookup(0, 3), 1.7601518630981445));
        assert!(close(temps.lookup(2, 2), 1.983399510383606));
        assert!(close(temps.lookup(1, 5), 1.2514300346374512));

        // defaults: absent temperature -> 1.0s, absent maps -> empty
        let cfg: RLAgentConfig =
            serde_json::from_str(r#"{"head_layers": 1, "max_len": 512, "head_max_len": 192}"#)
                .unwrap();
        assert_eq!(cfg.temperature, [1.0, 1.0, 1.0]);
        assert!(cfg.temperature_by_options.is_empty());
        assert_eq!(cfg.act_costs.len() + 1, 1);

        // zero max_len/head_layers pass serde (usize) and are rejected by
        // the loader's load() — the guard lives in RLAgentConfig::load.
        let cfg = serde_json::from_str::<RLAgentConfig>(
            r#"{"head_layers": 1, "max_len": 0, "head_max_len": 192}"#,
        )
        .unwrap();
        assert_eq!(cfg.max_len, 0);
        let err = RLAgentConfig::load(Path::new("/nonexistent/rl_agent_config.json"));
        assert!(err.is_err());
    }

    #[test]
    fn encoder_config_maps_transformers5_shape() {
        let v: serde_json::Value = serde_json::from_str(
            r#"{
            "vocab_size": 50368, "hidden_size": 1024, "num_hidden_layers": 28,
            "num_attention_heads": 16, "intermediate_size": 2624,
            "max_position_embeddings": 8192, "layer_norm_eps": 1e-05,
            "pad_token_id": 50283, "global_attn_every_n_layers": 3,
            "local_attention": 128,
            "rope_parameters": {"full_attention": {"rope_theta": 160000.0},
                                "sliding_attention": {"rope_theta": 10000.0}}
        }"#,
        )
        .unwrap();
        let cfg = encoder_config(&v).unwrap();
        assert_eq!(cfg.vocab_size, 50368);
        assert_eq!(cfg.hidden_size, 1024);
        assert_eq!(cfg.pad_token_id, 50283);
        assert_eq!(cfg.global_rope_theta, 160000.0);
        assert_eq!(cfg.local_rope_theta, 10000.0);
        assert_eq!(cfg.local_attention, 128);

        // missing rope_parameters falls back to the ModernBERT thetas; a
        // missing required key is a clear error.
        let v2: serde_json::Value = serde_json::from_str(
            r#"{"vocab_size": 1, "hidden_size": 2, "num_hidden_layers": 3,
                "num_attention_heads": 1, "intermediate_size": 4,
                "max_position_embeddings": 8, "layer_norm_eps": 1e-5,
                "pad_token_id": 0, "global_attn_every_n_layers": 1,
                "local_attention": 8}"#,
        )
        .unwrap();
        let cfg = encoder_config(&v2).unwrap();
        assert_eq!(cfg.global_rope_theta, 160000.0);
        assert_eq!(cfg.local_rope_theta, 10000.0);
        let v3: serde_json::Value = serde_json::from_str(r#"{"vocab_size": 1}"#).unwrap();
        assert!(encoder_config(&v3).is_err());
    }

    #[test]
    fn checkpoint_spec_resolves_env_override() {
        // unset -> shipped base model
        let saved = std::env::var(CHECKPOINT_ENV).ok();
        unsafe { std::env::remove_var(CHECKPOINT_ENV) };
        assert_eq!(CheckpointSpec::resolve(), CheckpointSpec::Hub(DEFAULT_CHECKPOINT.into()));

        // set to an existing dir -> Dir; set to anything else -> Hub repo id
        let dir = tempfile::tempdir().unwrap();
        unsafe { std::env::set_var(CHECKPOINT_ENV, dir.path()) };
        assert_eq!(CheckpointSpec::resolve(), CheckpointSpec::Dir(dir.path().to_path_buf()));
        unsafe { std::env::set_var(CHECKPOINT_ENV, "tampajohn/laya-stop-completion-judge") };
        assert_eq!(
            CheckpointSpec::resolve(),
            CheckpointSpec::Hub("tampajohn/laya-stop-completion-judge".into())
        );
        unsafe { std::env::set_var(CHECKPOINT_ENV, "") };
        assert_eq!(CheckpointSpec::resolve(), CheckpointSpec::Hub(DEFAULT_CHECKPOINT.into()));

        match saved {
            Some(v) => unsafe { std::env::set_var(CHECKPOINT_ENV, v) },
            None => unsafe { std::env::remove_var(CHECKPOINT_ENV) },
        }
    }

    // ------------------------------------------------------------------
    // Live: CHUG_LAYA_LIVE_PARITY=1 — the committed goldens through the
    // FULL pipeline (tokenizer -> assembly -> forward -> temperature
    // scaling), checkpoint-by-checkpoint. Default runs never load weights.
    // ------------------------------------------------------------------

    fn ov_arr_ids(v: &OValue) -> Vec<u32> {
        match v {
            OValue::Arr(items) => items
                .iter()
                .map(|i| match i {
                    OValue::Int(n) => *n as u32,
                    other => panic!("non-int in id array: {other:?}"),
                })
                .collect(),
            other => panic!("expected an array, got {other:?}"),
        }
    }

    fn ov_str(v: &OValue) -> &str {
        match v {
            OValue::Str(s) => s,
            other => panic!("expected a string, got {other:?}"),
        }
    }

    fn ov_to_json(v: &OValue) -> serde_json::Value {
        match v {
            OValue::Null => serde_json::Value::Null,
            OValue::Bool(b) => serde_json::Value::Bool(*b),
            OValue::Int(i) => serde_json::json!(i),
            OValue::Float(f) => serde_json::json!(f),
            OValue::Str(s) => serde_json::json!(s),
            OValue::Arr(items) => serde_json::Value::Array(items.iter().map(ov_to_json).collect()),
            OValue::Obj(entries) => {
                let mut m = serde_json::Map::new();
                for (k, v) in entries {
                    m.insert(k.clone(), ov_to_json(v));
                }
                serde_json::Value::Object(m)
            }
        }
    }

    /// Structural equality with a float tolerance (numbers within tol,
    /// everything else exact) — object key ORDER is not a shape concern.
    fn assert_value_close(got: &serde_json::Value, want: &serde_json::Value, tol: f64, name: &str) {
        match (got, want) {
            (serde_json::Value::Object(g), serde_json::Value::Object(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: object field count");
                for (k, wv) in w {
                    let gv = g.get(k).unwrap_or_else(|| panic!("{name}: missing field {k}"));
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (serde_json::Value::Array(g), serde_json::Value::Array(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: array length");
                for (gv, wv) in g.iter().zip(w.iter()) {
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (serde_json::Value::Number(g), serde_json::Value::Number(w)) => {
                let (g, w) = (g.as_f64().unwrap(), w.as_f64().unwrap());
                assert!((g - w).abs() <= tol, "{name}: numeric drift {g} vs {w} (tol {tol})");
            }
            _ => assert_eq!(got, want, "{name}: scalar/shape mismatch"),
        }
    }

    /// The full live parity: for every committed golden fixture, the Rust
    /// pipeline must reproduce (a) the SDK's recorded tokenization, (b) the
    /// SDK's recorded assembled sequence, and (c) the SDK's probabilities
    /// within 1e-3 — on the same checkpoints the goldens were generated
    /// from (convaiinnovations/laya + tampajohn/laya-stop-completion-judge,
    /// both cache-first from the standard HF cache).
    #[test]
    fn live_parity_golden_vectors() {
        if !live() {
            eprintln!(
                "skipping: set {LIVE_PARITY_ENV}=1 to run the weights-loaded golden parity"
            );
            return;
        }
        let golden_raw = std::fs::read_to_string("tests/fixtures/laya/golden-vectors.json").unwrap();
        let token_raw = std::fs::read_to_string("tests/fixtures/laya/token-fixtures.json").unwrap();
        let golden = parse_ordered(&golden_raw).unwrap();
        let token = parse_ordered(&token_raw).unwrap();
        let g_fixtures = match golden.field_pub("fixtures") {
            Some(OValue::Arr(a)) => a,
            other => panic!("golden fixtures array, got {other:?}"),
        };
        let t_fixtures = match token.field_pub("fixtures") {
            Some(OValue::Obj(o)) => o.clone(),
            other => panic!("token fixtures object, got {other:?}"),
        };
        assert_eq!(g_fixtures.len(), t_fixtures.len(), "fixture count drift");
        assert!(g_fixtures.len() >= 8);

        // one warm model per distinct checkpoint, loaded in file order and
        // dropped before the next (each load is ~1.7GB fp32).
        let mut current: Option<(String, JudgeModel)> = None;
        for (i, fx) in g_fixtures.iter().enumerate() {
            let name = ov_str(golden_field(fx, "name"));
            let ckpt = ov_str(golden_field(fx, "checkpoint"));
            if current.as_ref().map(|(c, _)| c != ckpt).unwrap_or(true) {
                // the default checkpoint honors CHUG_LAYA_CHECKPOINT (a
                // local-dir override of the base model); fine-tunes hub-load
                // cache-first.
                let spec = if ckpt == DEFAULT_CHECKPOINT {
                    CheckpointSpec::resolve()
                } else {
                    CheckpointSpec::hub(ckpt)
                };
                let model = JudgeModel::load(&spec)
                    .unwrap_or_else(|e| panic!("loading checkpoint {ckpt}: {e:#}"));
                current = Some((ckpt.to_string(), model));
            }
            let model = &current.as_ref().unwrap().1;

            let t_fx = &t_fixtures[i].1;
            let state = golden_field(fx, "state");
            let questions = golden_field(fx, "questions");

            // (a) state serialization byte-parity vs the SDK's recorded text
            let state_text = serialize_state(state);
            assert_eq!(
                state_text,
                ov_str(golden_field(t_fx, "state_text")),
                "{name}: state text drifted from the SDK recording"
            );
            // state_ids is recorded per-question in the token fixtures
            // (identical across a fixture's questions — same state text).
            let state_ids = model
                .encode(&state_text.replace(&model.specials.mask_text, " "))
                .unwrap();

            // (b) per-question: tokenization + assembly parity
            let qdefs = match questions {
                OValue::Obj(o) => o,
                other => panic!("{name}: questions object, got {other:?}"),
            };
            let t_questions = match golden_field(t_fx, "questions") {
                OValue::Arr(a) => a,
                other => panic!("{name}: token-fixture questions array, got {other:?}"),
            };
            assert_eq!(qdefs.len(), t_questions.len(), "{name}: question count");
            for (r, (qid, qdef)) in qdefs.iter().enumerate() {
                let q = to_internal(qdef).unwrap_or_else(|e| panic!("{name}: {qid}: {e}"));
                let t_q = &t_questions[r];
                assert_eq!(qid, ov_str(golden_field(t_q, "id")), "{name}: question order");
                let head_text = format!(
                    "{} question: {}",
                    q.t,
                    q.ins.replace(&model.specials.mask_text, " ")
                );
                assert_eq!(
                    model.encode(&head_text).unwrap(),
                    ov_arr_ids(golden_field(t_q, "head_ids")),
                    "{name}/{qid}: head tokenization drifted"
                );
                assert_eq!(
                    state_ids,
                    ov_arr_ids(golden_field(t_q, "state_ids")),
                    "{name}/{qid}: state tokenization drifted"
                );
                let opts = q.options();
                let mut opt_ids = Vec::with_capacity(opts.len());
                for o in &opts {
                    let text = format!(" {}", o.replace(&model.specials.mask_text, " "));
                    let mut ids = Vec::with_capacity(49);
                    ids.push(model.specials.mask);
                    ids.extend(model.encode(&text).unwrap().into_iter().take(48));
                    opt_ids.push(ids);
                }
                let want_opt: Vec<Vec<u32>> = match golden_field(t_q, "opt_ids") {
                    OValue::Arr(a) => a.iter().map(ov_arr_ids).collect(),
                    other => panic!("{name}: opt_ids array, got {other:?}"),
                };
                assert_eq!(opt_ids, want_opt, "{name}/{qid}: option tokenization drifted");
                let (ids, markers) = build_sequence(
                    &model.encode(&head_text).unwrap(),
                    &opt_ids,
                    &state_ids,
                    model.max_len,
                    model.head_max_len,
                    false,
                    model.specials.cls,
                    model.specials.sep,
                );
                assert_eq!(
                    ids,
                    ov_arr_ids(golden_field(t_q, "ids")),
                    "{name}/{qid}: assembled ids drifted"
                );
                let want_markers: Vec<usize> = match golden_field(t_q, "markers") {
                    OValue::Arr(a) => a
                        .iter()
                        .map(|v| match v {
                            OValue::Int(n) => *n as usize,
                            other => panic!("marker not an int: {other:?}"),
                        })
                        .collect(),
                    other => panic!("{name}: markers array, got {other:?}"),
                };
                assert_eq!(markers, want_markers, "{name}/{qid}: markers drifted");
            }

            // (c) the probability goldens: full pipeline vs the SDK's own
            // system_one response, within 1e-3.
            let got = model.judge(state, questions).unwrap();
            let want = ov_to_json(golden_field(fx, "expected"));
            assert_value_close(&got, &want, 1e-3, name);
        }
    }

    fn golden_field<'a>(v: &'a OValue, key: &str) -> &'a OValue {
        v.field_pub(key)
            .unwrap_or_else(|| panic!("fixture missing {key:?}"))
    }
}

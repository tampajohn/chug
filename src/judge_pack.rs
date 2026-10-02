//! T204 phase 1 (F15): the laya packing + answer-shape port — the weightless
//! half of the baked-in judge. Ported line-for-line from the Python SDK refs
//! (rl_common.py `serialize_state`/`render_options`/`build_sequence`/
//! `temp_bucket`/`confidence_from_probs` + rl_agent_api.py `system_one`'s
//! answer assembly) so the Rust daemon produces byte-identical sequences and
//! drop-in /judge responses. What is NOT here (yet): the model itself — the
//! checkpoint loader + candle forward live behind the `daemon` feature
//! (src/laya_model.rs), the socket server in src/daemon.rs.
//!
//! Parity is pinned two ways, both weightless and always-running under plain
//! `cargo test`:
//!   * committed token-id fixtures (tests/fixtures/laya/token-fixtures.json,
//!     generated from the Python SDK by scripts/gen_laya_token_fixtures.py) —
//!     state serialization byte-exact, sequence assembly token-exact,
//!     response shape + values vs the SDK's own responses;
//!   * the live goldens (tests/fixtures/laya/golden-vectors.json,
//!     probabilities within 1e-3) are asserted by the daemon-feature test
//!     once the model lands (CHUG_LAYA_LIVE=1).
//!
//! The one deliberate deviation from naive serde: Python dicts preserve
//! insertion order and `json.dumps` emits it; serde_json's Map sorts keys.
//! The daemon must re-serialize request states byte-identically, so the
//! ordered representation here is a pair-list (`OValue::Obj`), built in this
//! slice from the fixtures' `*_pairs` encoding; the raw-request parser joins
//! with the daemon slice.
//!
//! Phase-1 slice note: the daemon slice wires the served path; until it
//! lands this surface is test-only, hence the scoped allow.
#![allow(dead_code)]

use serde_json::{Value, json};

/// Laya question types, in the SDK's canonical order (`QTYPES` in rl_common.py).
pub const QTYPES: [(&str, usize); 3] = [("choice", 0), ("score", 1), ("noul", 2)];

pub fn qtype_index(name: &str) -> Option<usize> {
    QTYPES.iter().position(|(n, _)| *n == name)
}

fn qtype_name(i: usize) -> &'static str {
    QTYPES[i].0
}

// ---------------------------------------------------------------------------
// Ordered JSON value + Python-json.dumps-compatible serialization
// ---------------------------------------------------------------------------

/// Insertion-ordered JSON value. `Obj` keeps key order — the property
/// `json.dumps(state, ensure_ascii=False)` relies on and serde_json's map
/// discards.
#[derive(Debug, Clone, PartialEq)]
pub enum OValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<OValue>),
    Obj(Vec<(String, OValue)>),
}

impl OValue {
    /// Rebuild an ordered value from the fixtures' pair encoding
    /// (`to_pairs` in scripts/gen_laya_token_fixtures.py): JSON objects are
    /// encoded as `[["key", value], …]` pair arrays, everything else passes
    /// through (strings/numbers/bools/null; arrays as arrays).
    pub fn from_pairs(v: &Value) -> OValue {
        match v {
            Value::Null => OValue::Null,
            Value::Bool(b) => OValue::Bool(*b),
            Value::Number(n) => match n.as_i64() {
                Some(i) => OValue::Int(i),
                None => OValue::Float(n.as_f64().unwrap_or_default()),
            },
            Value::String(s) => OValue::Str(s.clone()),
            Value::Array(items) => {
                let is_obj = items
                    .iter()
                    .all(|i| i.as_array().is_some_and(|kv| kv.len() == 2 && kv[0].is_string()));
                if is_obj {
                    OValue::Obj(
                        items
                            .iter()
                            .map(|i| {
                                let kv = i.as_array().unwrap();
                                (kv[0].as_str().unwrap().to_string(), OValue::from_pairs(&kv[1]))
                            })
                            .collect(),
                    )
                } else {
                    OValue::Arr(items.iter().map(OValue::from_pairs).collect())
                }
            }
            _ => OValue::Null, // unreachable from the fixture encoding
        }
    }

    /// `json.dumps(self, ensure_ascii=False)` — object keys in insertion
    /// order, `, `/`: ` separators, Python string escaping, Python float
    /// repr. `ensure_ascii=True` (the `_to_internal` non-string
    /// instructions path) additionally escapes every non-ASCII codepoint.
    pub fn dumps(&self, ensure_ascii: bool) -> String {
        let mut out = String::new();
        self.write(&mut out, ensure_ascii);
        out
    }

    fn write(&self, out: &mut String, ensure_ascii: bool) {
        match self {
            OValue::Null => out.push_str("null"),
            OValue::Bool(true) => out.push_str("true"),
            OValue::Bool(false) => out.push_str("false"),
            OValue::Int(i) => out.push_str(&i.to_string()),
            OValue::Float(f) => out.push_str(&py_float_repr(*f)),
            OValue::Str(s) => write_py_string(s, ensure_ascii, out),
            OValue::Arr(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out, ensure_ascii);
                }
                out.push(']');
            }
            OValue::Obj(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    write_py_string(k, ensure_ascii, out);
                    out.push_str(": ");
                    v.write(out, ensure_ascii);
                }
                out.push('}');
            }
        }
    }
}

/// rl_common.serialize_state: strings pass through, everything else is
/// `json.dumps(…, ensure_ascii=False)`.
pub fn serialize_state(state: &OValue) -> String {
    match state {
        OValue::Str(s) => s.clone(),
        other => other.dumps(false),
    }
}

fn write_py_string(s: &str, ensure_ascii: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c if ensure_ascii && (c as u32) > 0x7e => {
                // Python's ensure_ascii: BMP -> \uXXXX, astral -> surrogate pair.
                if (c as u32) <= 0xffff {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                } else {
                    let cp = c as u32 - 0x1_0000;
                    let hi = 0xd800 + (cp >> 10);
                    let lo = 0xdc00 + (cp & 0x3ff);
                    out.push_str(&format!("\\u{hi:04x}\\u{lo:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Python `repr(float)` shape: shortest round-trip digits, exponent form
/// below 1e-4 / from 1e16, two-digit exponents with an explicit sign,
/// integral floats keep `.0`. States chug serves are string-valued, so the
/// exponent corners are best-effort (documented approximation).
fn py_float_repr(x: f64) -> String {
    if !x.is_finite() {
        // Python's json module emits Infinity/NaN (allow_nan by default).
        return if x.is_nan() {
            "NaN".into()
        } else if x > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let abs = x.abs();
    if abs >= 1e16 || abs < 1e-4 {
        // Exponent form: Rust's {:e} normalizes to one leading digit; Python
        // wants `e±NN` with a two-digit exponent.
        let s = format!("{:e}", x);
        if let Some((mantissa, exponent)) = s.split_once('e') {
            let exp: i32 = exponent.parse().unwrap_or(0);
            let sign = if exp < 0 { '-' } else { '+' };
            return format!("{mantissa}e{sign}{:02}", exp.abs());
        }
        s
    } else if x.fract() == 0.0 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}

// ---------------------------------------------------------------------------
// Question definitions + option rendering (rl_agent_api._to_internal +
// rl_common.render_options)
// ---------------------------------------------------------------------------

/// Ordered criteria, exactly the shapes `render_options` branches on.
#[derive(Debug, Clone, PartialEq)]
pub enum Crit {
    /// choice with an object: ordered (label, description-or-absent) pairs.
    Choice(Vec<(String, Option<String>)>),
    /// score: ordered level descriptions.
    Score(Vec<String>),
    /// noul: at most the "false"/"true" descriptions (absent -> defaults).
    Noul {
        false_: Option<String>,
        true_: Option<String>,
    },
}

/// `_to_internal`'s internal question: type tag, instructions text, criteria.
#[derive(Debug, Clone, PartialEq)]
pub struct InternalQ {
    pub t: String,
    pub ins: String,
    pub crit: Crit,
}

impl Crit {
    /// Rendered option texts in label-index order (noul is always
    /// [false, true] so p[1] == noul).
    pub fn render_options(&self) -> Vec<String> {
        match self {
            Crit::Choice(pairs) => pairs
                .iter()
                .map(|(k, v)| match v {
                    None => k.clone(),
                    Some(v) => format!("{k}: {v}"),
                })
                .collect(),
            Crit::Score(levels) => levels
                .iter()
                .enumerate()
                .map(|(i, c)| format!("level {i}: {c}"))
                .collect(),
            Crit::Noul { false_, true_ } => vec![
                format!(
                    "false: {}",
                    false_.clone().unwrap_or_else(|| "no, the statement does not hold".into())
                ),
                format!(
                    "true: {}",
                    true_.clone().unwrap_or_else(|| "yes, the statement holds".into())
                ),
            ],
        }
    }
}

impl InternalQ {
    pub fn options(&self) -> Vec<String> {
        self.crit.render_options()
    }

    /// The ordered answer keys for a choice question (system_one's
    /// `list(q["crit"].keys())`).
    pub fn choice_keys(&self) -> Vec<String> {
        match &self.crit {
            Crit::Choice(pairs) => pairs.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Sequence assembly (rl_common.build_sequence, from token-id pieces)
// ---------------------------------------------------------------------------

/// Assemble `[CLS] <type> instructions [SEP] [MASK] opt0 [MASK] opt1 … [SEP]
/// state [SEP]` from the per-piece token ids (head text ids, per-option
/// `[mask]+ids` lists post-[:48], FULL state ids). Tokenization itself is the
/// daemon's job (behind the feature); the budget/truncation math — the part
/// that silently mis-truncates — is pinned here, token-exact, weightless.
///
/// Returns (input_ids, marker positions in the given option order).
/// mask token id (50284) is NOT consumed by assembly — options arrive with
/// their marker already prepended, and the state text is mask-masked before
/// tokenization — it stays part of the daemon loader's token map.
#[allow(clippy::too_many_arguments)] // the SDK signature, kept 1:1 for the parity audit
pub fn build_sequence(
    head_ids: &[u32],
    opt_ids: &[Vec<u32>],
    state_ids: &[u32],
    max_len: usize,
    head_max_len: usize,
    truncate_left: bool,
    cls: u32,
    sep: u32,
) -> (Vec<u32>, Vec<usize>) {
    let mut opt_ids: Vec<Vec<u32>> = opt_ids.to_vec();
    let opt_budget = head_max_len.saturating_sub(opt_ids.iter().map(Vec::len).sum::<usize>());
    if opt_budget < 16 {
        // too many / too long options: shrink every option text evenly
        let per = 4usize.max((head_max_len - 16) / opt_ids.len().max(1));
        for o in opt_ids.iter_mut() {
            o.truncate(per);
        }
    }
    let opt_budget = head_max_len.saturating_sub(opt_ids.iter().map(Vec::len).sum::<usize>());
    let head: Vec<u32> = head_ids.iter().copied().take(8.max(opt_budget)).collect();

    let mut ids = Vec::with_capacity(max_len.min(head.len() + state_ids.len() + opt_budget + 16));
    ids.push(cls);
    ids.extend(head);
    ids.push(sep);
    let mut markers = Vec::with_capacity(opt_ids.len());
    for o in &opt_ids {
        markers.push(ids.len());
        ids.extend(o.iter().copied());
    }
    ids.push(sep);
    let room = max_len.saturating_sub(ids.len() + 1);
    let st: Vec<u32> = if truncate_left {
        let start = state_ids.len().saturating_sub(room);
        state_ids[start..].to_vec()
    } else {
        state_ids.iter().copied().take(room).collect()
    };
    ids.extend(st);
    ids.push(sep);
    ids.truncate(max_len);
    let markers = markers.into_iter().filter(|m| *m < max_len).collect();
    (ids, markers)
}

/// rl_common.temp_bucket: per-cardinality temperature key.
pub fn temp_bucket(qtype: usize, k: usize) -> String {
    let size = if k <= 2 {
        "2"
    } else if k <= 5 {
        "3-5"
    } else if k <= 10 {
        "6-10"
    } else {
        "11+"
    };
    format!("{}:{}", qtype_name(qtype), size)
}

/// Fitted temperatures (rl_agent_config.json): per-qtype fallback + the
/// per-cardinality overrides.
#[derive(Debug, Clone, PartialEq)]
pub struct Temperatures {
    pub per_qtype: [f64; 3],
    pub by_options: Vec<(String, f64)>,
}

impl Temperatures {
    pub fn lookup(&self, qtype: usize, k: usize) -> f64 {
        let key = temp_bucket(qtype, k);
        self.by_options
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, v)| *v)
            .unwrap_or(self.per_qtype[qtype])
    }
}

// ---------------------------------------------------------------------------
// Answer assembly (rl_agent_api.system_one's response loop)
// ---------------------------------------------------------------------------

/// Python `round(v, 4)` for the non-tie case: correctly-rounded 4-decimal
/// formatting of the binary value (ties are not representable f64s except at
/// dyadic points like 0.03125, where format's half-even agrees with Python).
fn round4(v: f64) -> f64 {
    format!("{v:.4}").parse().unwrap_or(v)
}

/// f32 softmax over temperature-scaled logits (system_one's numpy math,
/// float32 throughout: z = logits/temp; p = exp(z - max); p /= sum).
pub fn softmax_scaled(z: &[f32], temp: f64) -> Vec<f32> {
    let temp = temp as f32;
    let z: Vec<f32> = z.iter().map(|v| v / temp).collect();
    let max = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut p: Vec<f32> = z.iter().map(|v| (v - max).exp()).collect();
    let sum: f32 = p.iter().sum();
    for v in p.iter_mut() {
        *v /= sum;
    }
    p
}

/// rl_common.confidence_from_probs: 1 - normalized entropy of the answer
/// distribution (float32 accumulation like numpy's small-array sum, widened
/// at the division exactly as the numpy float32/float64 mix does).
pub fn confidence_from_probs(p: &[f32], k: usize) -> f64 {
    if k < 2 {
        return 1.0;
    }
    let p = &p[..k.min(p.len())];
    let mut ent = 0.0f32;
    for &v in p {
        let v = v.clamp(1e-12, 1.0);
        ent -= v * v.ln();
    }
    1.0 - (ent as f64) / (k as f64).ln()
}

/// One question's inference payload (the daemon slices these out of the
/// model output; the tests feed the fixture-recorded logits).
pub struct Inference {
    pub logits: Vec<f32>,
    /// softmax(act_logits)[0] — the rl_agent escalation-probability ext.
    pub act_probability: f32,
}

/// system_one's response: `{"model": "rl-agent", "answers": {…}, "usage":
/// {"input_tokens": n, "output_tokens": 0}}`, answers in the request's
/// question order. Field sets, types and rounding are drop-in pinned against
/// the fixture-recorded SDK responses (tests/… parity below).
pub fn answers_payload(
    qs: &[(String, InternalQ)],
    inf: &[Inference],
    n_tokens: usize,
    temps: &Temperatures,
) -> Value {
    let mut answers = serde_json::Map::new();
    for (r, (qid, q)) in qs.iter().enumerate() {
        let qt = qtype_index(&q.t).unwrap_or(0);
        let k = inf[r].logits.len();
        let p = softmax_scaled(&inf[r].logits, temps.lookup(qt, k));
        let ext = json!({"act_probability": inf[r].act_probability as f64});
        let answer = match &q.crit {
            Crit::Choice(_) => {
                let keys = q.choice_keys();
                let best = p
                    .iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let mut probabilities = serde_json::Map::new();
                for (kk, v) in keys.iter().zip(p.iter()) {
                    probabilities.insert(kk.clone(), json!(round4(*v as f64)));
                }
                json!({
                    "type": "choice",
                    "choice": keys[best],
                    "probabilities": Value::Object(probabilities),
                    "confidence": round4(confidence_from_probs(&p, k)),
                    "rl_agent": ext,
                })
            }
            Crit::Score(levels) => {
                let score: f64 = p
                    .iter()
                    .enumerate()
                    .map(|(i, v)| i as f64 * *v as f64)
                    .sum();
                let mut probabilities = serde_json::Map::new();
                for (i, v) in p.iter().enumerate() {
                    probabilities.insert(i.to_string(), json!(round4(*v as f64)));
                }
                let mut legend = serde_json::Map::new();
                for (i, c) in levels.iter().enumerate() {
                    legend.insert(i.to_string(), json!(c));
                }
                json!({
                    "type": "score",
                    "score": round4(score),
                    "legend": Value::Object(legend),
                    "probabilities": Value::Object(probabilities),
                    "confidence": round4(confidence_from_probs(&p, k)),
                    "rl_agent": ext,
                })
            }
            Crit::Noul { .. } => json!({
                "type": "noul",
                "noul": round4(p[1] as f64),
                "rl_agent": ext,
            }),
        };
        answers.insert(qid.clone(), answer);
    }
    json!({
        "model": "rl-agent",
        "answers": Value::Object(answers),
        "usage": {"input_tokens": n_tokens, "output_tokens": 0},
    })
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        // cargo runs test binaries with cwd = package root (the T48 doctrine —
        // no compile-time CARGO_MANIFEST_DIR).
        PathBuf::from("tests/fixtures/laya/token-fixtures.json")
    }

    fn load_fixtures() -> serde_json::Map<String, Value> {
        let raw = std::fs::read_to_string(fixture_path())
            .expect("token-fixtures.json committed (see scripts/gen_laya_token_fixtures.py)");
        match serde_json::from_str::<Value>(&raw).expect("fixture JSON parses") {
            Value::Object(doc) => doc["fixtures"].as_object().expect("fixtures object").clone(),
            _ => panic!("fixture root must be an object"),
        }
    }

    /// Ordered-object serialization: insertion order preserved, `, `/`: `
    /// separators, Python escapes.
    #[test]
    fn dumps_matches_python_json_dumps() {
        let obj = OValue::Obj(vec![
            ("b".into(), OValue::Int(1)),
            ("a".into(), OValue::Str("x \"q\" \n\u{1}".into())),
            ("t".into(), OValue::Bool(true)),
            ("n".into(), OValue::Null),
            ("f".into(), OValue::Float(1.5)),
            ("nested".into(), OValue::Obj(vec![("z".into(), OValue::Int(0)), ("y".into(), OValue::Arr(vec![OValue::Int(1)]))])),
        ]);
        assert_eq!(
            obj.dumps(false),
            "{\"b\": 1, \"a\": \"x \\\"q\\\" \\n\\u0001\", \"t\": true, \"n\": null, \"f\": 1.5, \"nested\": {\"z\": 0, \"y\": [1]}}"
        );
        // floats: integral keep .0; exponent form matches repr's shape.
        assert_eq!(OValue::Float(3.0).dumps(false), "3.0");
        assert_eq!(OValue::Float(0.5).dumps(false), "0.5");
        assert_eq!(OValue::Float(1e-5).dumps(false), "1e-05");
        assert_eq!(OValue::Float(2.5e16).dumps(false), "2.5e+16");
        // ensure_ascii escapes non-ASCII (the _to_internal instructions path).
        assert_eq!(OValue::Str("é".into()).dumps(true), "\"\\u00e9\"");
        assert_eq!(OValue::Str("é".into()).dumps(false), "\"é\"");
    }

    /// serialize_state: strings pass through, objects serialize ordered.
    #[test]
    fn serialize_state_passthrough_and_order() {
        assert_eq!(serialize_state(&OValue::Str("raw".into())), "raw");
        let obj = OValue::Obj(vec![
            ("final".into(), OValue::Str("f".into())),
            ("user".into(), OValue::Str("u".into())),
            ("guide".into(), OValue::Str("g".into())),
        ]);
        assert_eq!(serialize_state(&obj), "{\"final\": \"f\", \"user\": \"u\", \"guide\": \"g\"}");
    }

    /// build_sequence's budget/truncation math on a hand-computed case: the
    /// head truncates to opt_budget, state right-truncates to room, markers
    /// land at their assembly positions.
    #[test]
    fn build_sequence_budgets_and_markers() {
        let head: Vec<u32> = (100..140).collect(); // 40 ids
        let opt_a: Vec<u32> = [9, 201, 202].to_vec(); // mask + 2
        let opt_b: Vec<u32> = [9, 301, 302].to_vec();
        let state: Vec<u32> = (500..600).collect(); // 100 ids
        let (ids, markers) = build_sequence(&head, &[opt_a.clone(), opt_b.clone()], &state, 64, 48, false, 1, 2);
        // opt_budget = 48 - 6 = 42 -> head keeps all 40; ids: cls + 40 head + sep
        assert_eq!(ids[0], 1);
        assert_eq!(ids[41], 2);
        assert_eq!(markers, vec![42, 45]);
        assert_eq!(&ids[42..45], &opt_a[..]);
        assert_eq!(&ids[45..48], &opt_b[..]);
        assert_eq!(ids[48], 2); // closing sep before the state
        // room = 64 - 50 - 1 = 13 state ids + final sep = 64 total
        assert_eq!(ids.len(), 64);
        assert_eq!(&ids[49..62], &(500..513).collect::<Vec<u32>>()[..]);
        assert_eq!(ids[63], 2);
        // too many options -> the even-shrink path (opt_budget < 16): 8
        // options of 6 ids blow the 48-token head budget to 0 < 16, so every
        // option shrinks to per = max(4, (48-16)/8) = 4 ids and the head
        // re-truncates to the recomputed budget of 16.
        let many: Vec<Vec<u32>> = (0..8).map(|i| vec![9, 400 + i, 401 + i, 402 + i, 403 + i, 404 + i]).collect();
        let (ids2, markers2) = build_sequence(&head, &many, &state, 512, 48, false, 1, 2);
        assert_eq!(markers2, vec![18, 22, 26, 30, 34, 38, 42, 46]);
        assert_eq!(&ids2[18..22], &many[0][..4]);
        // head kept exactly the recomputed 16-id budget; the 100-id state
        // then fits whole: 1 + 16 + 1 + 32 + 1 + 100 + 1
        assert_eq!(&ids2[1..17], &(100..116).collect::<Vec<u32>>()[..]);
        assert_eq!(ids2.len(), 152);
    }

    /// temperature lookup + confidence math against the SDK definitions.
    #[test]
    fn temperature_and_confidence_math() {
        let temps = Temperatures {
            per_qtype: [1.6369030475616455, 1.2514300346374512, 1.983399510383606],
            by_options: vec![("choice:3-5".into(), 1.7601518630981445), ("noul:2".into(), 1.983399510383606)],
        };
        assert_eq!(temp_bucket(0, 3), "choice:3-5");
        assert_eq!(temp_bucket(1, 2), "score:2");
        assert_eq!(temp_bucket(2, 2), "noul:2");
        assert_eq!(temps.lookup(0, 3), 1.7601518630981445); // by_options hit
        assert_eq!(temps.lookup(1, 3), 1.2514300346374512); // score:3-5 absent -> per-qtype
        assert_eq!(temps.lookup(0, 2), 1.6369030475616455); // choice:2 absent -> per-qtype
        // softmax_scaled: uniform logits stay uniform; argmax survives scaling
        let p = softmax_scaled(&[1.0, 2.0, 3.0], 1.0);
        assert!((p[2] - p[1] * (1.0f32.exp())).abs() < 1e-6);
        assert!((p.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        // one-hot -> confidence 1; uniform 2-way -> confidence 0
        assert!((confidence_from_probs(&[1.0, 0.0], 2) - 1.0).abs() < 1e-6);
        assert!((confidence_from_probs(&[0.5, 0.5], 2)).abs() < 1e-6);
        assert_eq!(confidence_from_probs(&[1.0], 1), 1.0);
    }

    /// The weightless fixture parity: for every committed fixture —
    /// state serialization byte-exact, sequence assembly token-exact,
    /// response payload shape+values matching the SDK's own response.
    #[test]
    fn fixture_parity_packing_assembly_and_answers() {
        let fixtures = load_fixtures();
        assert!(fixtures.len() >= 8, "all 8 golden fixtures committed, got {}", fixtures.len());
        for (name, fx) in &fixtures {
            // (a) state serialization: ordered pairs -> byte-exact Python text
            let state = OValue::from_pairs(&fx["state_pairs"]);
            assert_eq!(
                serialize_state(&state),
                fx["state_text"].as_str().unwrap(),
                "{name}: serialize_state drifted from json.dumps"
            );

            let temps = Temperatures {
                per_qtype: [
                    fx["temperature"][0].as_f64().unwrap(),
                    fx["temperature"][1].as_f64().unwrap(),
                    fx["temperature"][2].as_f64().unwrap(),
                ],
                by_options: fx["temperature_by_options"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.as_f64().unwrap()))
                    .collect(),
            };

            // (b) sequence assembly from the recorded pieces == SDK's ids+markers
            let mut qs: Vec<(String, InternalQ)> = Vec::new();
            let mut infs: Vec<Inference> = Vec::new();
            for (r, q) in fx["questions"].as_array().unwrap().iter().enumerate() {
                let crit = match q["type"].as_str().unwrap() {
                    "choice" => Crit::Choice(
                        q["crit_pairs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|kv| {
                                let kv = kv.as_array().unwrap();
                                (
                                    kv[0].as_str().unwrap().to_string(),
                                    kv[1].as_str().map(str::to_string),
                                )
                            })
                            .collect(),
                    ),
                    "score" => Crit::Score(
                        q["crit_pairs"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|s| s.as_str().unwrap().to_string())
                            .collect(),
                    ),
                    _ => {
                        // noul: criteria may be absent entirely (Python's
                        // `crit or {}` — both descriptions fall to defaults).
                        let get = |k: &str| {
                            q["crit_pairs"]
                                .as_array()
                                .unwrap_or(&Vec::new())
                                .iter()
                                .find(|kv| kv.as_array().unwrap()[0].as_str() == Some(k))
                                .and_then(|kv| kv.as_array().unwrap()[1].as_str().map(str::to_string))
                        };
                        Crit::Noul { false_: get("false"), true_: get("true") }
                    }
                };
                let iq = InternalQ {
                    t: q["type"].as_str().unwrap().to_string(),
                    ins: q["ins"].as_str().unwrap().to_string(),
                    crit,
                };
                // the rendered head/option texts must match the SDK's too
                let opts = iq.options();
                let recorded: Vec<String> = q["opt_texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_string())
                    .collect();
                assert_eq!(opts, recorded, "{name}: render_options drifted");

                let head_ids: Vec<u32> = q["head_ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
                let opt_ids: Vec<Vec<u32>> = q["opt_ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|o| o.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect())
                    .collect();
                let state_ids: Vec<u32> = q["state_ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
                let (ids, markers) = build_sequence(
                    &head_ids,
                    &opt_ids,
                    &state_ids,
                    fx["max_len"].as_u64().unwrap() as usize,
                    fx["head_max_len"].as_u64().unwrap() as usize,
                    false,
                    50281,
                    50282,
                );
                let want_ids: Vec<u32> = q["ids"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
                let want_markers: Vec<usize> = q["markers"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
                assert_eq!(ids, want_ids, "{name}: assembled ids drifted");
                assert_eq!(markers, want_markers, "{name}: markers drifted");

                qs.push((q["id"].as_str().unwrap().to_string(), iq));
                infs.push(Inference {
                    logits: fx["logits"][r]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_f64().unwrap() as f32)
                        .collect(),
                    act_probability: fx["act_probability"][r].as_f64().unwrap() as f32,
                });
            }

            // (c) response payload vs the SDK's own system_one response
            let got = answers_payload(&qs, &infs, fx["n_tokens"].as_u64().unwrap() as usize, &temps);
            let want = &fx["expected"];
            assert_value_close(&got, want, 1.5e-4, name);
        }
    }

    /// Structural equality with a float tolerance: object key SETS equal at
    /// every level (order is a serialization concern, not a shape one),
    /// arrays equal length + order, numbers within tol, everything else
    /// exactly equal.
    fn assert_value_close(got: &Value, want: &Value, tol: f64, name: &str) {
        match (got, want) {
            (Value::Object(g), Value::Object(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: object field count");
                for (k, wv) in w {
                    let gv = g.get(k).unwrap_or_else(|| panic!("{name}: missing field {k}"));
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (Value::Array(g), Value::Array(w)) => {
                assert_eq!(g.len(), w.len(), "{name}: array length");
                for (gv, wv) in g.iter().zip(w.iter()) {
                    assert_value_close(gv, wv, tol, name);
                }
            }
            (Value::Number(g), Value::Number(w)) => {
                let (g, w) = (g.as_f64().unwrap(), w.as_f64().unwrap());
                assert!(
                    (g - w).abs() <= tol,
                    "{name}: numeric drift {g} vs {w} (tol {tol})"
                );
            }
            _ => assert_eq!(got, want, "{name}: scalar/shape mismatch"),
        }
    }
}

//! T222: the kev judge-parity harness — per-primitive metrics over a corpus
//! dir (choice top-1, score MAE, noul AUROC, mean |p_diff|, option-order
//! flip rate), the fixture pins, the wire-shape pin, and the daemon-gated
//! live leg. Offline-first: everything below the live leg runs against the
//! committed fixtures only (tests/fixtures/kev/); no network, no weights.
//!
//! Why there is no weights-loaded inference here: the CONFIRMED contestant
//! `jaredpalmer/kev-0.8b` sits on Qwen/Qwen3.5-0.8B-Base — 18 Gated
//! DeltaNet + 6 full attention (see src/kev_config.rs) — which
//! candle-transformers 0.11 cannot load ("DeltaNet, candle-blocked"). The
//! harness pins everything AROUND that gap so T223's forward plugs into a
//! ready, measured surface: the metrics, the corpus discipline, and the
//! /judge wire shape (identical to the RLAgent path's, minus the rl_agent
//! extension — kev has no act head and nothing is fabricated).

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// The harness: corpus loading, validation, metrics
// ---------------------------------------------------------------------------

/// One evaluated corpus record: probabilities under the canonical option
/// ordering (`probs`, aligned to `options`), the labeled index, and — for
/// paired records — the SAME question with the options presented in the
/// flipped order (`flipped.probs` re-aligned to the canonical option
/// identity: slot i of both arrays is the same option).
#[derive(Debug, Clone)]
struct Record {
    id: String,
    qtype: String,
    options: Vec<String>,
    label: usize,
    probs: Vec<f64>,
    flipped: Option<(Vec<String>, Vec<f64>)>,
}

/// Per-primitive metrics over a corpus dir. Semantics (pinned by the tests
/// below, matching the kev card's evaluation panels):
/// * `choice_top1` — fraction of choice records whose argmax(probs) is the
///   labeled option.
/// * `score_mae` — mean |expected level − label| over score records, the
///   expected level being the answers_payload formula Σ i·p_i.
/// * `noul_auroc` — Mann-Whitney AUROC of P(yes)=probs[1] against the yes/no
///   label over noul records (ties score 0.5); a one-class corpus is a
///   corpus error, not a 0.5-by-convention.
/// * `mean_abs_p_diff` — mean over PAIRED records of the per-option
///   mean |p_canonical − p_flipped| (the card's "option order can flip
///   answers" sensitivity, averaged over slots then records).
/// * `order_flip_rate` — fraction of PAIRED records whose argmax flips
///   between the two orderings (any qtype).
#[derive(Debug, Clone, PartialEq)]
struct CorpusMetrics {
    choice_top1: f64,
    score_mae: f64,
    noul_auroc: f64,
    mean_abs_p_diff: f64,
    order_flip_rate: f64,
    n_choice: usize,
    n_score: usize,
    n_noul: usize,
    n_paired: usize,
}

/// Load every `*.json` shard in `dir`, sorted by file name (determinism:
/// the metrics never depend on directory order), each shaped
/// `{"records": [...]}`. The harness refuses a malformed corpus — the
/// fixture-state shape pin below is the committed example of the contract.
fn load_corpus(dir: &Path) -> Result<Vec<Record>, String> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("reading corpus dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("corpus") && n.ends_with(".json"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(format!(
            "corpus dir {} has no corpus*.json shards (other fixtures are not corpus records)",
            dir.display()
        ));
    }
    let mut records = Vec::new();
    for f in &files {
        let raw = std::fs::read_to_string(f).map_err(|e| format!("reading {}: {e}", f.display()))?;
        let v: Value = serde_json::from_str(&raw).map_err(|e| format!("parsing {}: {e}", f.display()))?;
        let shard = v
            .get("records")
            .and_then(|r| r.as_array())
            .ok_or_else(|| format!("{}: missing \"records\" array", f.display()))?;
        for r in shard {
            records.push(parse_record(r).map_err(|e| format!("{}: {e}", f.display()))?);
        }
    }
    Ok(records)
}

/// The corpus contract: required fields with the right types, probability
/// arrays summing to 1 within 1e-3 (the wire's round4 keeps them there),
/// the label inside the option range, noul exactly two slots, and paired
/// records carrying the same option SET in both orderings.
fn parse_record(r: &Value) -> Result<Record, String> {
    let id = r
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("id must be a string")?
        .to_string();
    let qtype = r
        .get("qtype")
        .and_then(|v| v.as_str())
        .ok_or("qtype must be a string")?
        .to_string();
    if !matches!(qtype.as_str(), "choice" | "score" | "noul") {
        return Err(format!("{id}: unknown qtype {qtype:?}"));
    }
    let options: Vec<String> = r
        .get("options")
        .and_then(|v| v.as_array())
        .ok_or("options must be an array")?
        .iter()
        .map(|v| v.as_str().map(str::to_string).ok_or("options entries must be strings"))
        .collect::<Result<_, _>>()?;
    if options.is_empty() {
        return Err(format!("{id}: empty options"));
    }
    let probs: Vec<f64> = r
        .get("probs")
        .and_then(|v| v.as_array())
        .ok_or("probs must be an array")?
        .iter()
        .map(|v| v.as_f64().ok_or("probs entries must be numbers"))
        .collect::<Result<_, _>>()?;
    if probs.len() != options.len() {
        return Err(format!("{id}: probs length {} != options length {}", probs.len(), options.len()));
    }
    if probs.iter().sum::<f64>() - 1.0 > 1e-3 || 1.0 - probs.iter().sum::<f64>() > 1e-3 {
        return Err(format!("{id}: probs sum {} outside 1 ± 1e-3", probs.iter().sum::<f64>()));
    }
    let label = r
        .get("label")
        .and_then(|v| v.as_u64())
        .ok_or("label must be a non-negative integer")? as usize;
    if label >= options.len() {
        return Err(format!("{id}: label {label} out of range"));
    }
    let flipped = match r.get("flipped") {
        None | Some(Value::Null) => None,
        Some(f) => {
            let fo: Vec<String> = f
                .get("options")
                .and_then(|v| v.as_array())
                .ok_or("flipped.options must be an array")?
                .iter()
                .map(|v| v.as_str().map(str::to_string).ok_or("flipped options entries must be strings"))
                .collect::<Result<_, _>>()?;
            let fp: Vec<f64> = f
                .get("probs")
                .and_then(|v| v.as_array())
                .ok_or("flipped.probs must be an array")?
                .iter()
                .map(|v| v.as_f64().ok_or("flipped probs entries must be numbers"))
                .collect::<Result<_, _>>()?;
            if fp.len() != fo.len() || fp.len() != probs.len() {
                return Err(format!("{id}: flipped arrays disagree in length with the canonical record"));
            }
            if fp.iter().sum::<f64>() - 1.0 > 1e-3 || 1.0 - fp.iter().sum::<f64>() > 1e-3 {
                return Err(format!("{id}: flipped probs sum outside 1 ± 1e-3"));
            }
            // Same option multiset in both orderings — a reordered question,
            // not a different one.
            let canon: BTreeMap<&String, usize> =
                options.iter().fold(BTreeMap::new(), |mut m, o| {
                    *m.entry(o).or_default() += 1;
                    m
                });
            let flipped_set: BTreeMap<&String, usize> =
                fo.iter().fold(BTreeMap::new(), |mut m, o| {
                    *m.entry(o).or_default() += 1;
                    m
                });
            if canon != flipped_set {
                return Err(format!("{id}: flipped options are not a permutation of the canonical options"));
            }
            Some((fo, fp))
        }
    };
    Ok(Record { id, qtype, options, label, probs, flipped })
}

/// argmax with a deterministic tie rule (first maximum — the same rule the
/// wire's answer assembly uses); the committed fixtures avoid ties.
fn argmax(p: &[f64]) -> usize {
    let mut best = 0usize;
    for (i, v) in p.iter().enumerate() {
        if *v > p[best] {
            best = i;
        }
    }
    best
}

/// Mann-Whitney AUROC via midranks (ties score 0.5). Degenerate inputs
/// (one class only) are a corpus error — a pinned metric must not silently
/// become 0.5 by convention.
fn auroc(scored: &[(f64, bool)]) -> Result<f64, String> {
    let n_pos = scored.iter().filter(|(_, y)| *y).count();
    let n_neg = scored.len() - n_pos;
    if n_pos == 0 || n_neg == 0 {
        return Err("auroc needs both classes in the corpus".to_string());
    }
    let mut order: Vec<usize> = (0..scored.len()).collect();
    order.sort_by(|a, b| scored[*a].0.partial_cmp(&scored[*b].0).unwrap_or(std::cmp::Ordering::Equal));
    let mut rank = vec![0.0f64; scored.len()];
    let mut i = 0;
    while i < order.len() {
        let mut j = i;
        while j + 1 < order.len() && scored[order[j + 1]].0 == scored[order[i]].0 {
            j += 1;
        }
        let mid = ((i + j + 2) as f64) / 2.0; // 1-based midrank
        for k in i..=j {
            rank[order[k]] = mid;
        }
        i = j + 1;
    }
    let rank_sum_pos: f64 = scored
        .iter()
        .enumerate()
        .filter(|(_, (_, y))| *y)
        .map(|(idx, _)| rank[idx])
        .sum();
    // U = R_pos - n_pos*(n_pos+1)/2; AUROC = U / (n_pos*n_neg).
    let u = rank_sum_pos - (n_pos * (n_pos + 1)) as f64 / 2.0;
    Ok(u / (n_pos * n_neg) as f64)
}

fn evaluate(records: &[Record]) -> Result<CorpusMetrics, String> {
    let mut n_choice = 0;
    let mut hits = 0;
    let mut n_score = 0;
    let mut score_abs_err = 0.0;
    let mut noul_scores: Vec<(f64, bool)> = Vec::new();
    let mut n_paired = 0;
    let mut flips = 0;
    let mut p_diff_sum = 0.0;
    for r in records {
        match r.qtype.as_str() {
            "choice" => {
                n_choice += 1;
                if argmax(&r.probs) == r.label {
                    hits += 1;
                }
            }
            "score" => {
                n_score += 1;
                let expected: f64 =
                    r.probs.iter().enumerate().map(|(i, v)| i as f64 * v).sum();
                score_abs_err += (expected - r.label as f64).abs();
            }
            "noul" => {
                if r.probs.len() != 2 {
                    return Err(format!("{}: noul records have exactly two slots", r.id));
                }
                noul_scores.push((r.probs[1], r.label == 1));
            }
            other => return Err(format!("{}: unknown qtype {other:?}", r.id)),
        }
        if let Some((_, fp)) = &r.flipped {
            n_paired += 1;
            if argmax(&r.probs) != argmax(fp) {
                flips += 1;
            }
            p_diff_sum +=
                r.probs.iter().zip(fp.iter()).map(|(a, b)| (a - b).abs()).sum::<f64>() / fp.len() as f64;
        }
    }
    if n_choice == 0 || n_score == 0 || noul_scores.is_empty() || n_paired == 0 {
        return Err(
            "corpus must contain at least one choice, score, noul and paired record each"
                .to_string(),
        );
    }
    Ok(CorpusMetrics {
        choice_top1: hits as f64 / n_choice as f64,
        score_mae: score_abs_err / n_score as f64,
        noul_auroc: auroc(&noul_scores)?,
        mean_abs_p_diff: p_diff_sum / n_paired as f64,
        order_flip_rate: flips as f64 / n_paired as f64,
        n_choice,
        n_score,
        n_noul: noul_scores.len(),
        n_paired,
    })
}

fn kev_fixture_dir() -> PathBuf {
    // cargo runs test binaries with cwd = package root (the T48 doctrine).
    PathBuf::from("tests/fixtures/kev")
}

// ---------------------------------------------------------------------------
// Spec test family 1: fixture-state shape pin (fields, types, probability
// sums within 1e-3)
// ---------------------------------------------------------------------------

#[test]
fn fixture_state_shape_pin() {
    // The corpus: every record parses under the strict contract (the pin IS
    // the parse — parse_record refuses on any field/type/sum violation).
    let records = load_corpus(&kev_fixture_dir()).expect("corpus loads");
    assert_eq!(records.len(), 13, "committed corpus record count");
    let paired = records.iter().filter(|r| r.flipped.is_some()).count();
    assert_eq!(paired, 7, "committed corpus paired-record count");

    // The wire fixture: request fields, types, and probability sums.
    let wf: Value = serde_json::from_str(
        &std::fs::read_to_string(kev_fixture_dir().join("wire-fixture.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(wf["model"], "kev-0.8b");
    let req = &wf["request"];
    assert!(req["state"].is_object(), "state is an object");
    assert!(req["state"]["from"].is_string() && req["state"]["body"].is_string());
    let qs = req["questions"].as_object().expect("questions object");
    assert_eq!(qs.len(), 3);
    assert_eq!(qs["department"]["type"], "choice");
    assert_eq!(qs["department"]["criteria"].as_array().unwrap().len(), 3, "choice criteria as an ordered label list");
    assert_eq!(qs["urgency"]["type"], "score");
    assert_eq!(qs["urgency"]["criteria"].as_array().unwrap().len(), 3);
    assert_eq!(qs["churn_risk"]["type"], "noul");
    // Logits: one f32 row per question, sized to its option count.
    for (qid, n) in [("department", 3), ("urgency", 3), ("churn_risk", 2)] {
        let row = wf["logits"][qid].as_array().unwrap_or_else(|| panic!("{qid} logits"));
        assert_eq!(row.len(), n, "{qid}: one logit per option");
        assert!(row.iter().all(|v| v.is_number()), "{qid}: numeric logits");
    }
    // Expected response: probability vectors (choice/score — noul is a
    // single value) sum to 1 within 1e-3 (the round4 wire contract).
    let answers = wf["expected"]["answers"].as_object().unwrap();
    for (qid, a) in answers {
        let t = a["type"].as_str().unwrap();
        if t == "noul" {
            assert!(a["noul"].is_number(), "{qid}: noul value present");
            continue;
        }
        let probs = a["probabilities"].as_object().unwrap_or_else(|| panic!("{qid} probabilities"));
        let sum: f64 = probs.values().map(|v| v.as_f64().unwrap()).sum();
        assert!((sum - 1.0).abs() <= 1e-3, "{qid}: probability sum {sum} outside 1 ± 1e-3");
        assert!(a["confidence"].is_number(), "{qid}: confidence present");
    }
}

// ---------------------------------------------------------------------------
// Spec test family 2: metrics determinism pins (pinned counts and values)
// ---------------------------------------------------------------------------

#[test]
fn metrics_determinism_pins() {
    let records = load_corpus(&kev_fixture_dir()).expect("corpus loads");
    let m1 = evaluate(&records).expect("metrics compute");
    let m2 = evaluate(&records).expect("metrics compute");
    assert_eq!(m1, m2, "same corpus, same metrics — bit-identical");

    // Pinned counts.
    assert_eq!(m1.n_choice, 5);
    assert_eq!(m1.n_score, 4);
    assert_eq!(m1.n_noul, 4);
    assert_eq!(m1.n_paired, 7);

    // Pinned values (hand-derived; 1e-9 tolerance for f64 accumulation).
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(close(m1.choice_top1, 0.8), "choice_top1 = {}", m1.choice_top1);
    assert!(close(m1.score_mae, 0.1625), "score_mae = {}", m1.score_mae);
    assert!(close(m1.noul_auroc, 0.75), "noul_auroc = {}", m1.noul_auroc);
    assert!(close(m1.mean_abs_p_diff, 1.64 / 7.0), "mean_abs_p_diff = {}", m1.mean_abs_p_diff);
    assert!(close(m1.order_flip_rate, 4.0 / 7.0), "order_flip_rate = {}", m1.order_flip_rate);

    // The corpus loader is order-independent: a second shard with the same
    // records in a different file yields identical metrics (determinism of
    // the harness over corpus dirs, not just one file).
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("corpus-b-second.json"), std::fs::read_to_string(kev_fixture_dir().join("corpus.json")).unwrap()).unwrap();
    let m_shard = evaluate(&load_corpus(tmp.path()).unwrap()).unwrap();
    assert_eq!(m1, m_shard, "shard layout must not change the metrics");
}

// ---------------------------------------------------------------------------
// Spec test family 3: option-order flip-rate fixture runs both orderings
// ---------------------------------------------------------------------------

#[test]
fn option_order_flip_rate_fixture() {
    let records = load_corpus(&kev_fixture_dir()).expect("corpus loads");
    // Both orderings really ran: every paired record carries a distinct
    // ordering of the SAME options (the permutation is verified at parse).
    let paired: Vec<&Record> = records.iter().filter(|r| r.flipped.is_some()).collect();
    assert_eq!(paired.len(), 7);
    for r in &paired {
        let (fo, fp) = r.flipped.as_ref().unwrap();
        assert_ne!(fo, &r.options, "{}: the paired ordering must differ", r.id);
        assert_eq!(fo.len(), fp.len());
        // Slot alignment: both arrays are indexed by the CANONICAL option
        // identity, so the flip metric compares the same option twice.
        let mut canon_sorted = r.options.clone();
        canon_sorted.sort();
        let mut flipped_sorted = fo.clone();
        flipped_sorted.sort();
        assert_eq!(canon_sorted, flipped_sorted);
    }
    // The pinned flip behavior: 4 of the 7 paired records flip their argmax
    // (c1, s1, n1, n3 — the card's "option order can flip answers" caveat,
    // measurable on the committed fixture).
    let m = evaluate(&records).unwrap();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(close(m.order_flip_rate, 4.0 / 7.0));
    assert!(close(m.mean_abs_p_diff, 1.64 / 7.0));
    // Each of the three qtypes contributes at least one paired record, so
    // the flip metric spans all three answer shapes.
    for t in ["choice", "score", "noul"] {
        assert!(paired.iter().any(|r| r.qtype == t), "paired {t} record missing");
    }
}

// ---------------------------------------------------------------------------
// Spec test family 4: wire-shape pin — the kev path answers the SAME /judge
// JSON shape as the RLAgent path (the seam T223's forward plugs into)
// ---------------------------------------------------------------------------

#[test]
fn wire_shape_pin_kev_answers_the_same_judge_shape() {
    let wf: Value = serde_json::from_str(
        &std::fs::read_to_string(kev_fixture_dir().join("wire-fixture.json")).unwrap(),
    )
    .unwrap();

    // (a) The contract math, recomputed INDEPENDENTLY from the fixture
    // logits (f32 softmax at the single fitted temperature, entropy
    // confidence, the score formula, round4) — the wire the kev path must
    // serve. Committed `expected` == the recomputation.
    let temp = wf["temperature"].as_f64().unwrap();
    let got = json_answers(&wf["request"], &wf["logits"], temp, wf["usage_input_tokens"].as_u64().unwrap() as usize, "kev-0.8b");
    assert_value_close(&got, &wf["expected"], 1e-9, "kev wire");

    // (b) Shape equality with the SDK-RECORDED RLAgent response (the
    // committed laya goldens): the ONLY deltas are the model label and the
    // rl_agent extension — kev has no act head, nothing fabricated.
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/laya/golden-vectors.json").unwrap(),
    )
    .unwrap();
    let rl = &golden["fixtures"][0]["expected"];
    for key in ["answers", "usage", "model"] {
        assert!(rl.get(key).is_some() && got.get(key).is_some(), "top-level {key} on both wires");
    }
    assert_eq!(rl["usage"].as_object().unwrap().len(), got["usage"].as_object().unwrap().len());
    assert_eq!(got["model"], "kev-0.8b");
    assert_eq!(rl["model"], "rl-agent");
    // Per-answer-type field sets, keyed by type across both wires.
    // Per-type field sets are consistent WITHIN each wire; the cross-wire
    // delta is exactly the rl_agent extension (checked per-answer below).
    let mut shapes: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for (tag, wire) in [("rlagent", rl), ("kev", &got)] {
        for (_qid, a) in wire["answers"].as_object().unwrap() {
            let t = a["type"].as_str().unwrap().to_string();
            let mut fields: Vec<String> = a.as_object().unwrap().keys().cloned().collect();
            fields.sort();
            match shapes.get(&(tag.to_string(), t.clone())) {
                None => {
                    shapes.insert((tag.to_string(), t), fields);
                }
                Some(prev) => assert_eq!(prev, &fields, "field-set drift for type {t} ({tag})"),
            }
        }
    }
    // The recorded RLAgent shapes carry rl_agent + the core fields; the kev
    // wire carries exactly the core fields (minus rl_agent, the one ext).
    let rl_choice = &rl["answers"]["department"];
    let kev_choice = &got["answers"]["department"];
    let strip = |a: &Value| -> Vec<String> {
        a.as_object().unwrap().keys().filter(|k| k != &"rl_agent").cloned().collect()
    };
    assert_eq!(strip(rl_choice), strip(kev_choice), "core choice field sets identical");
    assert!(rl_choice.get("rl_agent").is_some());
    assert!(kev_choice.get("rl_agent").is_none());
    for qid in ["urgency", "churn_risk"] {
        assert_eq!(strip(&rl["answers"][qid]), strip(&got["answers"][qid]));
        assert!(got["answers"][qid].get("rl_agent").is_none());
    }
    // Values agree where both wires record them (same logits, same math).
    assert_eq!(got["answers"]["department"]["choice"], "billing");
    assert_eq!(got["answers"]["churn_risk"]["noul"], 0.625);
}

/// The kev answer assembly, recomputed independently of the bin crate
/// (integration tests cannot import the binary): f32 softmax at the single
/// fitted temperature, entropy confidence, the Σ i·p score, Python round4.
/// This IS the contract T223's forward plugs into; the serializer that
/// produces it on the daemon path is pinned in src (judge_pack's
/// kev_wire_shape_no_act_head_no_fabricated_ext).
fn json_answers(request: &Value, logits: &Value, temp: f64, n_tokens: usize, model: &str) -> Value {
    let f32 = |x: f64| x as f32;
    let softmax = |zl: &[f64]| -> Vec<f64> {
        let t = f32(temp);
        let z: Vec<f32> = zl.iter().map(|v| f32(*v) / t).collect();
        let max = z.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mut p: Vec<f32> = z.iter().map(|v| (v - max).exp()).collect();
        let sum: f32 = p.iter().sum();
        for v in p.iter_mut() {
            *v /= sum;
        }
        p.iter().map(|v| *v as f64).collect()
    };
    let round4 = |v: f64| -> f64 {
        format!("{v:.4}").parse().unwrap_or(v)
    };
    let confidence = |p: &[f64]| -> f64 {
        if p.len() < 2 {
            return 1.0;
        }
        let mut ent = 0.0f32;
        for &v in p {
            let v = v.clamp(1e-12, 1.0) as f32;
            ent -= v * v.ln();
        }
        1.0 - (ent as f64) / (p.len() as f64).ln()
    };
    let mut answers = serde_json::Map::new();
    for (qid, q) in request["questions"].as_object().unwrap() {
        let zl: Vec<f64> = logits[qid].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let p = softmax(&zl);
        let answer = match q["type"].as_str().unwrap() {
            "choice" => {
                // Ordered label list (the wire fixture uses the list form so
                // the recomputation is order-identical to the daemon path).
                let keys: Vec<String> = q["criteria"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
                let mut best = 0;
                for (i, v) in p.iter().enumerate() {
                    if *v > p[best] {
                        best = i;
                    }
                }
                let mut probabilities = serde_json::Map::new();
                for (kk, v) in keys.iter().zip(p.iter()) {
                    probabilities.insert(kk.clone(), serde_json::json!(round4(*v)));
                }
                serde_json::json!({
                    "type": "choice",
                    "choice": keys[best],
                    "probabilities": Value::Object(probabilities),
                    "confidence": round4(confidence(&p)),
                })
            }
            "score" => {
                let levels: Vec<&str> = q["criteria"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
                let score: f64 = p.iter().enumerate().map(|(i, v)| i as f64 * v).sum();
                let mut probabilities = serde_json::Map::new();
                for (i, v) in p.iter().enumerate() {
                    probabilities.insert(i.to_string(), serde_json::json!(round4(*v)));
                }
                let mut legend = serde_json::Map::new();
                for (i, c) in levels.iter().enumerate() {
                    legend.insert(i.to_string(), serde_json::json!(c));
                }
                serde_json::json!({
                    "type": "score",
                    "score": round4(score),
                    "legend": Value::Object(legend),
                    "probabilities": Value::Object(probabilities),
                    "confidence": round4(confidence(&p)),
                })
            }
            "noul" => serde_json::json!({"type": "noul", "noul": round4(p[1])}),
            other => panic!("unknown question type {other:?}"),
        };
        answers.insert(qid.clone(), answer);
    }
    serde_json::json!({
        "model": model,
        "answers": Value::Object(answers),
        "usage": {"input_tokens": n_tokens, "output_tokens": 0},
    })
}

/// Structural equality with a float tolerance (numbers within tol,
/// everything else exact) — object key ORDER is not a shape concern (the
/// same rule as the laya golden pins).
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
            assert!((g - w).abs() <= tol, "{name}: numeric drift {g} vs {w} (tol {tol})");
        }
        _ => assert_eq!(got, want, "{name}: scalar/shape mismatch"),
    }
}

// ---------------------------------------------------------------------------
// The pins T205-style: BOTH revisions pinned in src (adapter + base), and
// the no-fabrication shape rule, pinned from the integration gate.
// ---------------------------------------------------------------------------

#[test]
fn kev_pins_live_in_src() {
    let src = std::fs::read_to_string("src/kev_config.rs").unwrap();
    for pin in [
        "jaredpalmer/kev-0.8b",
        "bf75a6a8848ea6960ff2ed108d9ed44c2941174f",
        "Qwen/Qwen3.5-0.8B-Base",
        "dc7cdfe2ee4154fa7e30f5b51ca41bfa40174e68",
        "2.35",
    ] {
        assert!(src.contains(pin), "src/kev_config.rs must pin {pin}");
    }
}

// ---------------------------------------------------------------------------
// The live leg: daemon-feature-gated, skip (never fail) when the fetch is
// unavailable; when it IS available, verify the artifact against BOTH pins
// (provenance base repo/revision AND the measured file sha256s) and assert
// the loader's classified refusal is the honest candle-blocked one.
// ---------------------------------------------------------------------------

#[cfg(feature = "daemon")]
mod live {
    use super::*;
    use sha2::Digest;

    const LIVE_PARITY_ENV: &str = "CHUG_LAYA_LIVE_PARITY";
    const KEV_REPO: &str = "jaredpalmer/kev-0.8b";
    const KEV_REVISION: &str = "bf75a6a8848ea6960ff2ed108d9ed44c2941174f";
    const KEV_BASE_REPO: &str = "Qwen/Qwen3.5-0.8B-Base";
    const KEV_BASE_REVISION: &str = "dc7cdfe2ee4154fa7e30f5b51ca41bfa40174e68";

    fn live() -> bool {
        std::env::var(LIVE_PARITY_ENV).ok().as_deref() == Some("1")
    }

    /// The pinned checkpoint, verified end to end when the network allows
    /// it: BOTH revisions (provenance base repo/revision), the PEFT config
    /// values, the measured file sha256s — and the classified refusal: the
    /// confirmed contestant's target_modules carry the Gated DeltaNet
    /// signature, so no loader may serve it by substituting an
    /// architecture (T223 inherits the port).
    #[test]
    fn live_kev_checkpoint_pins_and_refusal() {
        if !live() {
            eprintln!(
                "skipping: set {LIVE_PARITY_ENV}=1 (and build with --features daemon) to verify the pinned kev checkpoint"
            );
            return;
        }
        // Cache-first fetch of the SMALL gate files. ANY failure here is a
        // SKIP (the spec's offline-first rule): the live leg measures when
        // it can and stays silent when it cannot.
        let api = match hf_hub::api::sync::Api::new() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("skipping: hub client unavailable: {e}");
                return;
            }
        };
        let repo = hf_hub::Repo::with_revision(
            KEV_REPO.to_string(),
            hf_hub::RepoType::Model,
            KEV_REVISION.to_string(),
        );
        let get = |rel: &str| -> Option<std::path::PathBuf> {
            match api.repo(repo.clone()).get(rel) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("skipping: {rel} unavailable: {e}");
                    None
                }
            }
        };
        let Some(adapter_path) = get("adapter_config.json") else { return };
        let Some(prov_path) = get("provenance.json") else { return };
        let (Ok(adapter_raw), Ok(prov_raw)) = (
            std::fs::read_to_string(&adapter_path),
            std::fs::read_to_string(&prov_path),
        ) else {
            eprintln!("skipping: fetched files unreadable");
            return;
        };

        // From here the artifact EXISTS — pin mismatches are hard failures
        // (a moved base or changed config is a policy-affecting artifact
        // change the operator must re-pin deliberately).
        let adapter: Value = serde_json::from_str(&adapter_raw).expect("adapter_config.json parses");
        let prov: Value = serde_json::from_str(&prov_raw).expect("provenance.json parses");
        assert_eq!(adapter["peft_type"], "LORA");
        assert_eq!(adapter["r"], 16, "pinned rank");
        assert_eq!(adapter["lora_alpha"], 32, "pinned alpha");
        assert_eq!(adapter["base_model_name_or_path"], KEV_BASE_REPO);
        assert_eq!(adapter["use_rslora"], false, "scale = alpha/r, not alpha/sqrt(r)");
        assert_eq!(adapter["use_dora"], false);
        let base = &prov["config"]["base"];
        assert_eq!(base, KEV_BASE_REPO, "provenance base repo pin");
        assert_eq!(prov["config"]["base_revision"], KEV_BASE_REVISION, "provenance base revision pin");

        // The measured file hashes re-verified against the downloaded bytes
        // (sha256 — the T205 discipline, artifact-level).
        if let Some(measured) = prov.get("measured_checkpoint") {
            for (file, key) in [
                ("adapter_model.safetensors", "adapter_sha256"),
                ("head.pt", "head_sha256"),
            ] {
                let Some(want) = measured.get(key).and_then(|v| v.as_str()) else { continue };
                let Some(path) = get(file) else { return };
                let bytes = std::fs::read(&path).expect("read fetched file");
                let digest = sha2::Sha256::digest(&bytes);
                let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
                assert_eq!(hex, want.to_lowercase(), "{file}: sha256 drift vs provenance");
            }
        }

        // The honesty pin: the target_modules carry the Gated DeltaNet
        // signature (in_proj_* — no plain qwen3 attention has them), so the
        // base classifies candle-blocked. Nothing here substitutes an
        // architecture; T223 inherits the port.
        let targets: Vec<&str> = adapter["target_modules"]
            .as_array()
            .expect("target_modules array")
            .iter()
            .map(|v| v.as_str().expect("target module strings"))
            .collect();
        assert!(
            targets.iter().any(|t| t.starts_with("in_proj_")),
            "the confirmed 0.8B base must still be the Gated DeltaNet hybrid: {targets:?}"
        );
        assert!(targets.contains(&"out_proj"), "DeltaNet out_proj among the targets");
    }
}

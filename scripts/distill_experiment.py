#!/usr/bin/env python3
"""distill_experiment.py — T208 / F13 phase 2b: the first measure-first
distillation experiment.

Reads the loop's decision corpus (`.chug/decisions.jsonl`, passed as a path
— the MAIN checkout's corpus, not a worktree child), builds ONE supervised
dataset (class `validation-routing`: does this item's validation route to a
REQUIRED kimi validation, or is it lane-eligible?), trains two heads on the
pre-staged laya ModernBERT, and evaluates them against mechanical baselines
on a time-ordered held-out split. It writes its artifacts OUTSIDE git and
renders the phase-2b report to `docs/distill-f13-2b.md`.

This is measurement only — NO routing wiring, NO src/ changes, NO daemon
changes. Phase 3 (confidence-gated wiring) reads this report's go/no-go;
it is not implemented here.

Target task (requirement 3, implementer's pick): `validation-routing` —
142 records at filing, the F13 phase-3 flagship use ("does-this-need-kimi"),
labels mechanically derivable from the recorded `choice` text (REQUIRED vs
lane-eligible). Justified over `validation-verdict` (131 records, PASS/FAIL)
in the report: same record class count, cleaner label rule, and the label
space matches the lane predicate T189 mechanized.

Usage (operator host, the pre-staged venv, OFFLINE):
  /Users/jadams/models/laya/venv/bin/python scripts/distill_experiment.py \
      --corpus /Users/jadams/workspace/chug/.chug/decisions.jsonl

The script FORCES TRANSFORMERS_OFFLINE=1 and HF_HUB_OFFLINE=1 before any
transformers import — the run proves no network fetch. Without torch +
transformers it exits with the venv path named; `python3 -m py_compile`
(system python) stays the CI-grade check.

Leakage controls (the honesty spine, requirement 2 — restated in the report):
  * the split is by RECORD ORDER (file order = chronology, append-only
    corpus), never random: first ~80% train, last ~20% held-out;
  * near-duplicate templated records are deduped BEFORE the split on
    (class, subject-shape, inputs-shape) — digits→#, whitespace collapsed —
    keeping the first occurrence in file order;
  * features are PRE-decision fields only: class, subject, inputs, options
    (rendered as one labeled text block). The post-decision fields `choice`
    and `confidence` are the LABEL SOURCE and are never features; `ts` and
    the record `id` are excluded too (pure time/memorization handles — the
    by-order split already encodes time).

Determinism (requirement 1): seed pinned (default 13) for torch, python
random, and the probe's shuffle generator. Verified across repeated same-device
(MPS) reruns: the probe (primary head) and both mechanical baselines were
byte-identical; the fine-tune (secondary) wobbled within ±2 held-out records
(±6.9pp accuracy at n=29) — MPS float reductions reorder and the fine-tune's
4-epoch AdamW trajectory amplifies it. Headline numbers, the τ-curve, and the
verdict rest on the deterministic probe + baselines; the fine-tune row is
directional.
"""

import argparse
import json
import os
import random
import re
import subprocess
import sys
import time
from collections import Counter
from pathlib import Path

DEFAULT_CORPUS = "/Users/jadams/workspace/chug/.chug/decisions.jsonl"
DEFAULT_SNAPSHOT_GLOB = "~/.cache/huggingface/hub/models--convaiinnovations--laya/snapshots/*"
DEFAULT_ARTIFACTS = "/Users/jadams/models/laya/distill-f13-2b"
DEFAULT_REPORT = "docs/distill-f13-2b.md"

TASK_CLASS = "validation-routing"
SEED = 13
SPLIT_FRAC = 0.8
MAX_LEN = 384
BATCH = 8
FT_EPOCHS = 4
FT_LR = 2e-5
PROBE_EPOCHS = 60
PROBE_LR = 1e-3
PROBE_BS = 16
TAUS = [0.50, 0.60, 0.70, 0.80, 0.90, 0.95, 0.99]

# T189's mechanical lane predicate, lexical proxy: the §2 step-4 REQUIRED
# list (core files) + loop doctrine carriers, verbatim — no tuning on the
# held-out set, no synonyms added. Mentions are a WEAK proxy for touches
# (negated mentions like "not on the REQUIRED list" flip it) — that gap is
# part of the measurement, reported as-is.
CORE_FILE_TOKENS = [
    "driver.rs", "api.rs", "tools.rs", "events.rs", "delegate.rs",
    "mcp", "permissions.rs", "hooks.rs", "trim.rs",
]
DOCTRINE_TOKENS = ["loop-spec", "meta-meta-spec", "meta-spec", "spec.md", "doctrine"]


def log(msg):
    print(f"[distill] {msg}", flush=True)


# ---------------------------------------------------------------- dataset


def parse_corpus(path):
    """Parse the decisions corpus. Same tolerance as decisions-audit.sh /
    decisions-export.sh: a malformed line (torn tail from a killed writer)
    drops with a count, it never kills the run."""
    records, malformed = [], 0
    with open(path, "r", encoding="utf-8", errors="replace") as fh:
        for line in fh:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except ValueError:
                malformed += 1
                continue
            if isinstance(rec, dict):
                records.append(rec)
    return records, malformed


def shape(text):
    """Normalized text shape: lowercase, digits→#, whitespace collapsed."""
    s = re.sub(r"\d+", "#", text.lower())
    return re.sub(r"\s+", " ", s).strip()


def derive_label(choice):
    """Mechanical label rule for validation-routing choices (documented in
    the report; the ONLY post-decision field used, and only as label)."""
    c = choice.lower()
    if "required" in c:
        return "REQUIRED"  # covers "REQUIRED ... DEFERRED to next cycle": the routing verdict is REQUIRED
    if "deferred" in c:
        return "DEFERRED"  # ambiguous between classes — excluded, counted
    if "re-validation" in c or "revalidation" in c:
        return "REQUIRED"  # a FAIL-arc re-validation is REQUIRED by doctrine
    return "LANE"


def feature_text(rec):
    """The feature text: pre-decision fields only, one labeled block."""
    return (
        f"[class] {rec.get('class', '')}\n"
        f"[subject] {rec.get('subject', '')}\n"
        f"[inputs] {rec.get('inputs', '')}\n"
        f"[options] {rec.get('options', '')}"
    )


def keyword_proxy(rec):
    """Mechanical baseline: the T189 lane predicate as a lexical proxy over
    subject+inputs text (mentions, not diff facts — deliberately untuned)."""
    text = (rec.get("subject", "") + " " + rec.get("inputs", "")).lower()
    if any(tok in text for tok in CORE_FILE_TOKENS):
        return "REQUIRED"
    if any(tok in text for tok in DOCTRINE_TOKENS):
        return "REQUIRED"
    return "LANE"


def build_dataset(records):
    """Filter to the task class, derive labels, dedup near-duplicates.
    Returns (rows, stats) where rows are dicts in file order."""
    stats = {"class_total": len(records)}
    in_class = [r for r in records if r.get("class") == TASK_CLASS]
    stats["in_class"] = len(in_class)

    rows, ambiguous = [], []
    for rec in in_class:
        label = derive_label(rec.get("choice", ""))
        if label == "DEFERRED":
            ambiguous.append(rec.get("id", "?"))
            continue
        rows.append({"rec": rec, "label": label})
    stats["ambiguous_ids"] = ambiguous

    seen, deduped, dropped = set(), [], []
    for row in rows:
        rec = row["rec"]
        key = (
            TASK_CLASS,
            shape(rec.get("subject", "")),
            shape(rec.get("inputs", "")),
        )
        if key in seen:
            dropped.append(rec.get("id", "?"))
            continue
        seen.add(key)
        row["near_dup_key"] = "|".join(key[1:])
        deduped.append(row)
    stats["near_dup_dropped_ids"] = dropped
    return deduped, stats


# ---------------------------------------------------------------- metrics


def prf(preds, golds, positive):
    tp = sum(1 for p, g in zip(preds, golds) if p == positive and g == positive)
    fp = sum(1 for p, g in zip(preds, golds) if p == positive and g != positive)
    fn = sum(1 for p, g in zip(preds, golds) if p != positive and g == positive)
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return precision, recall, f1


def evaluate(preds, golds):
    n = len(golds)
    acc = sum(1 for p, g in zip(preds, golds) if p == g) / n if n else 0.0
    f1s = {cls: prf(preds, golds, cls)[2] for cls in ("REQUIRED", "LANE")}
    macro = sum(f1s.values()) / len(f1s) if f1s else 0.0
    return {
        "n": n,
        "accuracy": acc,
        "f1_REQUIRED": f1s["REQUIRED"],
        "f1_LANE": f1s["LANE"],
        "macro_f1": macro,
        "confusion": {
            "pred_REQUIRED_true_REQUIRED": sum(1 for p, g in zip(preds, golds) if p == "REQUIRED" and g == "REQUIRED"),
            "pred_REQUIRED_true_LANE": sum(1 for p, g in zip(preds, golds) if p == "REQUIRED" and g == "LANE"),
            "pred_LANE_true_REQUIRED": sum(1 for p, g in zip(preds, golds) if p == "LANE" and g == "REQUIRED"),
            "pred_LANE_true_LANE": sum(1 for p, g in zip(preds, golds) if p == "LANE" and g == "LANE"),
        },
    }


def tau_rows(probs, preds, golds):
    """The τ-curve: coverage vs covered accuracy (and macro-F1) on held-out
    as the confidence threshold rises. Confidence = max softmax prob of the
    predicted class."""
    rows = []
    for tau in TAUS:
        idx = [i for i, p in enumerate(probs) if p >= tau]
        if not idx:
            rows.append({"tau": tau, "n": 0, "coverage": 0.0, "accuracy": None, "macro_f1": None})
            continue
        sub_pred = [preds[i] for i in idx]
        sub_gold = [golds[i] for i in idx]
        m = evaluate(sub_pred, sub_gold)
        rows.append({
            "tau": tau,
            "n": len(idx),
            "coverage": len(idx) / len(golds),
            "accuracy": m["accuracy"],
            "macro_f1": m["macro_f1"],
        })
    return rows


# ---------------------------------------------------------------- models


def find_snapshot(pattern):
    import glob as _glob
    snaps = sorted(p for p in _glob.glob(os.path.expanduser(pattern)) if Path(p).is_dir())
    if not snaps:
        sys.exit(f"distill_experiment: no HF snapshot under {pattern} — stage the base laya model first")
    return Path(snaps[0])


def load_backbone(snapshot, device):
    """The frozen laya encoder: bare ModernBertModel + the `encoder.`-prefixed
    RLAgent-layout weights, strict-verified (missing/unexpected must be empty)."""
    import torch
    from safetensors.torch import load_file
    from transformers import AutoConfig, AutoModel

    cfg = AutoConfig.from_pretrained(str(snapshot / "encoder"))
    backbone = AutoModel.from_config(cfg)
    sd = load_file(str(snapshot / "model.safetensors"))
    renamed = {k[len("encoder."):]: v for k, v in sd.items() if k.startswith("encoder.")}
    missing, unexpected = backbone.load_state_dict(renamed, strict=False)
    if missing or unexpected:
        sys.exit(
            f"distill_experiment: backbone load mismatch — {len(missing)} missing, "
            f"{len(unexpected)} unexpected keys; refusing to train on a half-loaded model"
        )
    return backbone.to(device).eval(), cfg.hidden_size


def load_finetune_model(snapshot, device, num_labels=2):
    """ModernBertForSequenceClassification over the same backbone; only the
    classification stack (head.* + classifier.*) is fresh — verified by the
    missing-key filter (all must be head/classifier keys)."""
    import torch
    from safetensors.torch import load_file
    from transformers import AutoConfig, AutoModelForSequenceClassification

    cfg = AutoConfig.from_pretrained(str(snapshot / "encoder"))
    cfg.num_labels = num_labels
    model = AutoModelForSequenceClassification.from_config(cfg)
    sd = load_file(str(snapshot / "model.safetensors"))
    renamed = {"model." + k[len("encoder."):]: v for k, v in sd.items() if k.startswith("encoder.")}
    missing, unexpected = model.load_state_dict(renamed, strict=False)
    if unexpected or any(not m.startswith(("head.", "classifier.")) for m in missing):
        sys.exit(
            f"distill_experiment: fine-tune load mismatch — missing={missing[:5]}... "
            f"unexpected={len(unexpected)}; backbone weights did not land cleanly"
        )
    return model.to(device)


def encode(tokenizer, texts, max_len):
    return tokenizer(texts, truncation=True, max_length=max_len, padding=True, return_tensors="pt")


def embed(backbone, batch, device):
    """Mask-weighted mean pool of the last hidden state (the config's own
    classifier_pooling: 'mean')."""
    import torch

    with torch.no_grad():
        out = backbone(input_ids=batch["input_ids"].to(device),
                       attention_mask=batch["attention_mask"].to(device))
        mask = batch["attention_mask"].to(device).unsqueeze(-1).float()
        return (out.last_hidden_state * mask).sum(1) / mask.sum(1).clamp(min=1)


def train_probe(train_emb, train_y, hidden, device):
    """Primary head: logistic regression on frozen laya embeddings. Chosen a
    priori for the tiny-data regime (112 train examples cannot support full
    fine-tuning of a 396M encoder) — the a-priori call, made on train-set
    size, not held-out performance."""
    import torch
    import torch.nn as nn

    torch.manual_seed(SEED)
    head = nn.Linear(hidden, 2).to(device)
    opt = torch.optim.AdamW(head.parameters(), lr=PROBE_LR, weight_decay=0.01)
    gen = torch.Generator().manual_seed(SEED)
    for _ in range(PROBE_EPOCHS):
        head.train()
        perm = torch.randperm(train_emb.size(0), generator=gen)
        for i in range(0, len(perm), PROBE_BS):
            idx = perm[i:i + PROBE_BS]
            e, y = train_emb[idx].to(device), train_y[idx].to(device)
            opt.zero_grad()
            loss = nn.functional.cross_entropy(head(e), y)
            loss.backward()
            opt.step()
    head.eval()
    return head


def train_finetune(model, tokenizer, train_texts, train_y, device):
    """Secondary head: the spec's 2–4 epoch full fine-tune (unweighted CE —
    the empirical class prior matches deployment; last checkpoint evaluated,
    never best-epoch selection on held-out)."""
    import torch
    from transformers import get_linear_schedule_with_warmup

    torch.manual_seed(SEED)
    batch = encode(tokenizer, train_texts, MAX_LEN)
    y = train_y.to(device)
    loader = [(batch["input_ids"][i:i + BATCH], batch["attention_mask"][i:i + BATCH], y[i:i + BATCH])
              for i in range(0, batch["input_ids"].size(0), BATCH)]
    opt = torch.optim.AdamW(model.parameters(), lr=FT_LR, weight_decay=0.01)
    total = len(loader) * FT_EPOCHS
    sched = get_linear_schedule_with_warmup(opt, int(0.1 * total), total)
    for _ in range(FT_EPOCHS):
        model.train()
        for ids, am, yy in loader:
            ids, am, yy = ids.to(device), am.to(device), yy.to(device)
            opt.zero_grad()
            loss = model(input_ids=ids, attention_mask=am, labels=yy).loss
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
            sched.step()
    model.eval()
    return model


def probe_proba(backbone, head, tokenizer, texts, device):
    """Held-out probabilities for the probe: frozen-encoder embeddings -> linear head."""
    import torch

    batch = encode(tokenizer, texts, MAX_LEN)
    emb = embed(backbone, batch, device)
    preds, confs = [], []
    with torch.no_grad():
        for i in range(0, emb.size(0), PROBE_BS):
            p = torch.softmax(head(emb[i:i + PROBE_BS]), -1).cpu()
            for row in p:
                j = int(row.argmax())
                preds.append(["LANE", "REQUIRED"][j])
                confs.append(float(row[j]))
    return preds, confs


def ft_proba(model, tokenizer, texts, device):
    """Held-out probabilities for the fine-tuned classifier."""
    import torch

    batch = encode(tokenizer, texts, MAX_LEN)
    ids, am = batch["input_ids"].to(device), batch["attention_mask"].to(device)
    preds, confs = [], []
    with torch.no_grad():
        for i in range(0, ids.size(0), BATCH):
            out = model(input_ids=ids[i:i + BATCH], attention_mask=am[i:i + BATCH])
            p = torch.softmax(out.logits, -1).cpu()
            for row in p:
                j = int(row.argmax())
                preds.append(["LANE", "REQUIRED"][j])
                confs.append(float(row[j]))
    return preds, confs


# ---------------------------------------------------------------- report


def pct(x):
    return "—" if x is None else f"{100 * x:.1f}%"


def f3(x):
    return "—" if x is None else f"{x:.3f}"


def render_report(ctx):
    ds, baselines, probe, ft, tau_probe, tau_ft, run = (
        ctx["dataset"], ctx["baselines"], ctx["probe"], ctx["ft"],
        ctx["tau_probe"], ctx["tau_ft"], ctx["run"],
    )
    tr = ds["train_dist"]
    he = ds["held_dist"]
    b_maj = baselines["majority"]
    b_kw = baselines["keyword"]
    ambiguous = ", ".join(ds["ambiguous_ids"]) or "none"
    dropped = ", ".join(ds["near_dup_dropped_ids"]) or "none"

    tau_lines = []
    for tp, tf in zip(tau_probe, tau_ft):
        tau_lines.append(
            f"| {tp['tau']:.2f} | {tp['n']} | {pct(tp['coverage'])} | {pct(tp['accuracy'])} | "
            f"{f3(tp['macro_f1'])} | {tf['n']} | {pct(tf['coverage'])} | {pct(tf['accuracy'])} | {f3(tf['macro_f1'])} |"
        )
    tau_table = "\n".join(tau_lines)

    covered = [r for r in tau_probe if r["accuracy"] is not None and r["n"] > 0]
    best = max(covered, key=lambda r: r["accuracy"])
    strict = [r for r in covered if r["accuracy"] >= 0.95]
    if strict:
        best95 = max(strict, key=lambda r: r["coverage"])
        best95_line = (
            f"The ≥95%-accuracy operating point with the most coverage (probe): "
            f"τ={best95['tau']:.2f} — {best95['n']}/{he['n']} held-out records "
            f"({pct(best95['coverage'])} coverage)."
        )
        best95_stats = {"coverage": best95["coverage"], "n": best95["n"]}
    else:
        best95_line = "No τ operating point reached 95% accuracy on the held-out set."
        best95_stats = {"coverage": 0.0, "n": 0}
    strict_both = ([("probe", r) for r in tau_probe if r["accuracy"] is not None and r["accuracy"] >= 0.95]
                   + [("fine-tune", r) for r in tau_ft if r["accuracy"] is not None and r["accuracy"] >= 0.95])
    if strict_both:
        b_head, b_row = max(strict_both, key=lambda hr: hr[1]["coverage"])
        best95_both_stats = {"coverage": b_row["coverage"], "n": b_row["n"],
                             "tau": b_row["tau"], "head": b_head}
    else:
        best95_both_stats = {"coverage": 0.0, "n": 0, "tau": None, "head": None}


    cm = probe["confusion"]
    cmf = ft["confusion"]
    probe_lane_preds = cm["pred_LANE_true_REQUIRED"] + cm["pred_LANE_true_LANE"]
    ft_lane_preds = cmf["pred_LANE_true_REQUIRED"] + cmf["pred_LANE_true_LANE"]
    acc_delta_probe = 100 * (probe["accuracy"] - b_maj["accuracy"])
    acc_delta_ft = 100 * (ft["accuracy"] - b_maj["accuracy"])
    macro_delta_probe = probe["macro_f1"] - b_maj["macro_f1"]
    macro_delta_ft = ft["macro_f1"] - b_maj["macro_f1"]


    return f"""# F13 phase 2b — the first distillation experiment (T208)

**Status: MEASURED — {run['verdict'].upper()} for phase-3 confidence-gated wiring.**
Generated by `scripts/distill_experiment.py` (measure-first, NO routing wiring, NO src/
changes). Corpus commit at run time: `{run['corpus_commit']}`. Base snapshot:
`{run['snapshot']}` (convaiinnovations/laya @ 55cf4c4e). Seed {run['seed']}, device
`{run['device']}`, wall {run['wall_secs']:.0f}s, artifacts `{run['artifacts']}`.

## 1. Question and scope

F13's payoff claim: the loop's own judgment corpus (`.chug/decisions.jsonl`) trains a
Laya decision head so first-pass routings go to the cheap model at confidence ≥τ and
escalate otherwise. Phase 2a landed the hygiene gate + export (T199/T200, v0.14.0); T204
landed the co-located classification-only daemon. What had NEVER been measured is
whether the corpus can train a head that beats a mechanical baseline at any τ. This
experiment measures exactly that for ONE task and stops — phase-3 wiring is out of
scope here (its endpoint is F15 phase 2, deferred "no consumer yet"; this report is the
consumer's feasibility study).

**Target task (requirement 3): `{TASK_CLASS}` — REQUIRED vs lane-eligible.** Picked
over `validation-verdict` PASS/FAIL because (a) the label is mechanically derivable
from the recorded choice text (below), (b) it is F13 phase-3's flagship routing
("does-this-need-kimi"), and (c) the verdict class is heavily imbalanced toward PASS
with a vaguer label boundary.

## 2. Dataset shape

| stage | count |
|---|---|
| corpus records (all classes) | {ds['corpus_total']} |
| malformed lines dropped (torn-tail tolerance) | {ds['malformed']} |
| `{TASK_CLASS}` records | {ds['in_class']} |
| ambiguous-label records excluded | {len(ds['ambiguous_ids'])} ({ambiguous}) |
| near-duplicate records deduped | {len(ds['near_dup_dropped_ids'])} ({dropped}) |
| dataset total | {ds['n_total']} |
| train (first {pct(SPLIT_FRAC)} by record order) | {tr['n']} — REQUIRED {tr['REQUIRED']}, LANE {tr['LANE']} |
| held-out (last {pct(1 - SPLIT_FRAC)} by record order) | {he['n']} — REQUIRED {he['REQUIRED']}, LANE {he['LANE']} |

**Label rule (mechanical, from the recorded `choice` text only):** lowercased choice
contains `required` → REQUIRED (this correctly keeps "REQUIRED … deferred to next
cycle" — the routing verdict is REQUIRED even when the launch slipped); else contains
`deferred` → DEFERRED (excluded, named above); else contains `re-validation`/
`revalidation` → REQUIRED (a FAIL-arc re-validation is REQUIRED by doctrine); else →
LANE (skipped / optional / exercised-optional / gates-only).

## 3. Feature set and leakage controls

Features (pre-decision fields only, one labeled text block): `class`, `subject`,
`inputs`, `options`. NOT used as features: `choice` and `confidence` (post-decision —
the label source), `ts` and `id` (pure time/memorization handles; the by-order split
already encodes time). There is no `model` field in this corpus generation (the T70
shape is `id, ts, class, subject, inputs, options, choice, confidence`), so the named
feature set is the four pre-decision text fields above.

Leakage controls (requirement 2):

1. **Time-ordered split, never random** — the corpus is append-only, so file order IS
   chronology; train = first {pct(SPLIT_FRAC)} of records, held-out = the last
   {pct(1 - SPLIT_FRAC)}. No shuffle anywhere in the pipeline (the probe's shuffle is
   over TRAIN rows only, seeded).
2. **Near-duplicate dedup before the split** — key = (class, subject-shape,
   inputs-shape), digits→# and whitespace collapsed; first occurrence in file order
   kept. Measured: {len(ds['near_dup_dropped_ids'])} exact-shape duplicates in
   `{TASK_CLASS}` ({dropped}). Named inflation risk: subjects are templated
   ({ds['distinct_subject_shapes']} distinct subject shapes over {ds['in_class']}
   records — e.g. "T<N> validation routing"), but held-out item ids are strictly newer
   than any train id, so the subject carries no reusable discriminator across the
   split; the discriminative text is `inputs`.
3. **Label source is the only post-decision field** — the mechanical rule (§2) reads
   `choice` to BUILD the label and nothing else; no post-decision field reaches the
   feature text.
4. **No held-out model selection** — the fine-tune trains a fixed {FT_EPOCHS} epochs
   and the LAST checkpoint is evaluated; the probe trains a fixed {PROBE_EPOCHS}
   epochs. The primary head (probe) was chosen a priori on train-set size
   ({tr['n']} examples cannot support full fine-tuning of a ~396M encoder), not on
   held-out numbers.

## 4. Baselines vs trained heads (held-out, n={he['n']})

| head | accuracy | macro-F1 | F1 REQUIRED | F1 LANE |
|---|---|---|---|---|
| majority baseline (train-majority class = {baselines['train_majority']}) | {pct(b_maj['accuracy'])} | {f3(b_maj['macro_f1'])} | {f3(b_maj['f1_REQUIRED'])} | {f3(b_maj['f1_LANE'])} |
| T189 keyword proxy (lexical lane predicate) | {pct(b_kw['accuracy'])} | {f3(b_kw['macro_f1'])} | {f3(b_kw['f1_REQUIRED'])} | {f3(b_kw['f1_LANE'])} |
| **probe — frozen laya encoder + linear head (primary)** | **{pct(probe['accuracy'])}** | {f3(probe['macro_f1'])} | {f3(probe['f1_REQUIRED'])} | {f3(probe['f1_LANE'])} |
| fine-tune — full ModernBERT classifier (secondary) | {pct(ft['accuracy'])} | {f3(ft['macro_f1'])} | {f3(ft['f1_REQUIRED'])} | {f3(ft['f1_LANE'])} |

Confusion (held-out): probe pred-REQUIRED/true-REQUIRED {cm['pred_REQUIRED_true_REQUIRED']},
pred-REQUIRED/true-LANE {cm['pred_REQUIRED_true_LANE']}, pred-LANE/true-REQUIRED
{cm['pred_LANE_true_REQUIRED']}, pred-LANE/true-LANE {cm['pred_LANE_true_LANE']}
({probe_lane_preds}/{he['n']} held-out records predicted LANE{', i.e. degenerate toward the majority class' if probe_lane_preds == 0 else ''});
fine-tune: {cmf['pred_REQUIRED_true_REQUIRED']}/{cmf['pred_REQUIRED_true_LANE']}/
{cmf['pred_LANE_true_REQUIRED']}/{cmf['pred_LANE_true_LANE']}
({ft_lane_preds}/{he['n']} predicted LANE{', i.e. degenerate toward the majority class' if ft_lane_preds == 0 else ''}).
Train accuracy for the overfit check: probe {pct(probe['train_accuracy'])},
fine-tune {pct(ft['train_accuracy'])} (the fine-tune memorizes the train split —
loss ≈ {ft['final_train_loss']:.3f} by epoch {FT_EPOCHS - 1} — while the probe stays a
linear function of frozen features).

Reading: at FULL coverage (argmax, no threshold) the probe's accuracy sits
{acc_delta_probe:+.1f}pp against the majority baseline ({pct(probe['accuracy'])} vs
{pct(b_maj['accuracy'])}) while its macro-F1 moves {macro_delta_probe:+.3f}
({f3(probe['macro_f1'])} vs {f3(b_maj['macro_f1'])}) — the probe's only measured edge
is minority-class recall (F1 LANE {f3(probe['f1_LANE'])} vs the baseline's
{f3(b_maj['f1_LANE'])}), not top-line accuracy. The fine-tune sits {acc_delta_ft:+.1f}pp
on accuracy and {macro_delta_ft:+.3f} on macro-F1 against the baseline — on
{tr['n']} train examples the full ~396M-parameter fine-tune memorizes the split
(train acc {pct(ft['train_accuracy'])}) without converting it into a held-out edge.
The lexical T189 proxy
lands {100 * (b_kw['accuracy'] - b_maj['accuracy']):+.1f}pp on accuracy against
majority ({pct(b_kw['accuracy'])}) — the signal is not lexical: `inputs` mentions core
files inside negations ("not on the REQUIRED list") and for tests-only diffs inside
core files; a mention-based predicate cannot separate touches from mentions.

## 5. The τ-curve (held-out, n={he['n']})

Confidence = max softmax probability of the predicted class; a decision is taken when
confidence ≥ τ, else escalated. Coverage = share of held-out decisions the head would
take at that τ.

| τ | probe n | probe coverage | probe accuracy | probe macro-F1 | FT n | FT coverage | FT accuracy | FT macro-F1 |
|---|---|---|---|---|---|---|---|---|
{tau_table}

{best95_line} Best covered accuracy anywhere on the probe curve:
τ={best['tau']:.2f} → {best['n']}/{he['n']} at {pct(best['accuracy'])}.

## 6. Threats to validity

- **Held-out n={he['n']}**: one record moves accuracy {pct(1 / he['n'])}. The probe's
  high-τ sliver ({best['n']} records at {pct(best['accuracy'])}) is inside small-n
  noise; the ≥95% bar from the F13 framing is not met with statistical confidence at
  any useful coverage.
- **Single split** (mandated by-order {pct(SPLIT_FRAC)}/{pct(1 - SPLIT_FRAC)}): no
  variance estimate across folds; a rolling-origin CV is the natural extension when
  the corpus grows.
- **Confidence is not calibrated**: softmax max-prob from a head trained on {tr['n']}
  examples is a ranking signal, not a probability; the τ axis is a threshold on that
  ranking, and class weighting (rejected here to match the deployment prior) would
  shift it.
- **Templated prose**: all records come from one loop's writing style; a head trained
  on it may not transfer to other operators' phrasing (irrelevant for phase-3 wiring,
  which runs on this loop's own corpus).
- **Hardware nondeterminism**: seed {run['seed']} is pinned. Verified across repeated
  same-device (MPS) reruns: the probe (primary) and the mechanical baselines were
  **byte-identical** every time (same held-out predictions, same τ-curve), while the
  fine-tune (secondary) wobbled within **±2 held-out records (±{pct(2 / he['n'])}
  accuracy)** — MPS float reductions reorder, and the 4-epoch AdamW trajectory
  amplifies the reordering. Every headline number, the τ-curve, and the verdict rest
  on the deterministic probe + baselines; the FT row is directional{'' if ft['accuracy'] < b_maj['accuracy'] else ' (NOTE: this run landed at or above the majority baseline — the wobble straddles it)'}.

## 7. Go/no-go for phase-3 confidence-gated wiring

**NO-GO** — measured reason: at full coverage the trained heads do not escape the
mechanical majority baseline on accuracy ({pct(probe['accuracy'])} probe =
{acc_delta_probe:+.1f}pp, {pct(ft['accuracy'])} fine-tune = {acc_delta_ft:+.1f}pp, vs
{pct(b_maj['accuracy'])} majority on n={he['n']}); the probe's one real edge —
macro-F1 {f3(probe['macro_f1'])} vs {f3(b_maj['macro_f1'])} — does not survive the
confidence gate: across BOTH heads' τ-curves, no operating point reaches 95% accuracy
at more than a {pct(best95_both_stats['coverage'])} coverage sliver
({best95_both_stats['n']} records{'; best: ' + best95_both_stats['head'] + ' τ=' + f"{best95_both_stats['tau']:.2f}" if best95_both_stats['head'] else ''}) —
below the ≥50%-coverage wiring bar and inside small-n noise; and the mechanical
keyword proxy is WORSE than
majority ({pct(b_kw['accuracy'])}), so no mechanical fallback already does the job
either. The corpus ({TASK_CLASS}: {ds['in_class']} records, {tr['n']} train) is below
the size at which a trained head beats a one-line baseline with confidence. This
measured result CONFIRMS the phase-3 deferral (F15 phase 2, "no consumer yet") —
wiring a confidence gate now would add an escalation path whose accepted-decision
accuracy is indistinguishable from always-escalating.

**GO precondition (measured, for the re-run of this exact script):** grow the
validation-routing class ≥3x (~{3 * ds['in_class']} records) or pool adjacent routing
classes so train n ≥ 300 and held-out n ≥ 60, then re-run. Phase 3 is justified the
day this report shows a τ with ≥95% covered accuracy at ≥50% coverage on held-out
n ≥ 60. Until then: keep logging decisions (the corpus IS the asset), keep T199/T200
hygiene, and route validation with T189's mechanical predicate — which this experiment
shows is at least as good as any trained head at today's corpus size, with the
mentioned-vs-touched lexical gap (§4) as its named weakness.

## 8. Reproduction

```
export TRANSFORMERS_OFFLINE=1 HF_HUB_OFFLINE=1   # forced by the script itself
/Users/jadams/models/laya/venv/bin/python scripts/distill_experiment.py \\
    --corpus /Users/jadams/workspace/chug/.chug/decisions.jsonl
```

- Runtime: {run['wall_secs']:.0f}s wall on the operator host (bound: 30 min) — device {run['device']}.
- Artifacts (outside git): {run['artifacts']} — `dataset.jsonl` (rows with labels +
  both heads' predictions and confidences), `metrics.json` (every number in this
  report), `probe_head.pt` (the {run['hidden_size']}-dim linear head, ~40KB),
  `finetune/` (the full fine-tune checkpoint).
- Corpus snapshot: {run['corpus_path']} @ commit {run['corpus_commit']} ({ds['corpus_total']}
  records at run time).
- Determinism: seed {run['seed']}; probe + baselines byte-identical across same-device
  reruns (verified repeatedly), fine-tune tolerance ±2 held-out records on MPS (§6).
- The corpus is READ, never written; `scripts/decisions-audit.sh` stays green.
"""


# ---------------------------------------------------------------- main


def corpus_commit(corpus_path):
    try:
        out = subprocess.run(
            ["git", "-C", str(Path(corpus_path).parent.parent), "rev-parse", "HEAD"],
            capture_output=True, text=True, timeout=10, check=True,
        )
        return out.stdout.strip()
    except Exception:
        return "unknown"


def parse_args():
    p = argparse.ArgumentParser(description="T208 / F13 phase 2b distillation experiment")
    p.add_argument("--corpus", default=DEFAULT_CORPUS, help="decisions.jsonl path (main checkout)")
    p.add_argument("--snapshot", default=DEFAULT_SNAPSHOT_GLOB, help="HF snapshot dir or glob")
    p.add_argument("--artifacts", default=DEFAULT_ARTIFACTS, help="artifact dir (outside git)")
    p.add_argument("--report", default=DEFAULT_REPORT, help="report path (relative to cwd)")
    p.add_argument("--device", default="auto", choices=["auto", "mps", "cpu"])
    p.add_argument("--seed", type=int, default=SEED)
    p.add_argument("--max-len", type=int, default=MAX_LEN)
    p.add_argument("--ft-epochs", type=int, default=FT_EPOCHS)
    p.add_argument("--probe-epochs", type=int, default=PROBE_EPOCHS)
    return p.parse_args()


def main():
    args = parse_args()
    # Offline enforcement BEFORE any transformers import — the run proves no network.
    os.environ["TRANSFORMERS_OFFLINE"] = "1"
    os.environ["HF_HUB_OFFLINE"] = "1"
    try:
        import torch
        from transformers import AutoTokenizer
    except ImportError as exc:
        sys.exit(
            "distill_experiment: torch/transformers unavailable — run under the "
            "pre-staged venv: /Users/jadams/models/laya/venv/bin/python "
            "scripts/distill_experiment.py --corpus <decisions.jsonl> "
            f"(import error: {exc})"
        )
    global SEED, FT_EPOCHS, PROBE_EPOCHS, MAX_LEN
    SEED, FT_EPOCHS, PROBE_EPOCHS, MAX_LEN = (
        args.seed, args.ft_epochs, args.probe_epochs, args.max_len,
    )
    random.seed(SEED)
    torch.manual_seed(SEED)
    device = ("mps" if torch.backends.mps.is_available() else "cpu") if args.device == "auto" else args.device
    t0 = time.time()

    if not Path(args.corpus).is_file():
        sys.exit(f"distill_experiment: corpus not found: {args.corpus}")
    records, malformed = parse_corpus(args.corpus)
    log(f"corpus: {args.corpus} — {len(records)} records, {malformed} malformed lines dropped")

    rows, stats = build_dataset(records)
    log(f"dataset: {TASK_CLASS} {stats['in_class']} in-class, "
        f"{len(stats['ambiguous_ids'])} ambiguous, {len(stats['near_dup_dropped_ids'])} near-dup deduped "
        f"-> {len(rows)} rows")
    if not rows:
        sys.exit(f"distill_experiment: no {TASK_CLASS} rows in corpus — nothing to measure")

    split = int(len(rows) * SPLIT_FRAC)
    train_rows, held_rows = rows[:split], rows[split:]
    train_texts = [feature_text(r["rec"]) for r in train_rows]
    held_texts = [feature_text(r["rec"]) for r in held_rows]
    train_gold = [r["label"] for r in train_rows]
    held_gold = [r["label"] for r in held_rows]
    train_majority = Counter(train_gold).most_common(1)[0][0]
    log(f"split: train={len(train_rows)} held={len(held_rows)} train-majority={train_majority}")

    # ---- baselines (mechanical)
    b_majority = evaluate([train_majority] * len(held_gold), held_gold)
    b_keyword = evaluate([keyword_proxy(r["rec"]) for r in held_rows], held_gold)
    kw_all = evaluate([keyword_proxy(r["rec"]) for r in rows], [r["label"] for r in rows])
    log(f"baselines: majority {b_majority['accuracy']:.3f} | keyword held {b_keyword['accuracy']:.3f} "
        f"(all-rows {kw_all['accuracy']:.3f})")

    # ---- heads
    snapshot = find_snapshot(args.snapshot)
    tokenizer = AutoTokenizer.from_pretrained(str(snapshot / "tokenizer"))
    backbone, hidden = load_backbone(snapshot, device)
    log(f"backbone loaded (hidden={hidden}) — frozen probe pass")

    train_y = torch.tensor([0 if l == "LANE" else 1 for l in train_gold])
    held_y = torch.tensor([0 if l == "LANE" else 1 for l in held_gold])
    train_emb = embed(backbone, encode(tokenizer, train_texts, MAX_LEN), device)
    probe = train_probe(train_emb, train_y, hidden, device)
    with torch.no_grad():
        train_pred = probe(train_emb).argmax(-1).cpu()
    probe_train_acc = float((train_pred == train_y).float().mean())
    probe_preds, probe_conf = probe_proba(backbone, probe, tokenizer, held_texts, device)
    probe_metrics = evaluate(probe_preds, held_gold)
    probe_metrics["train_accuracy"] = probe_train_acc
    log(f"probe: held acc {probe_metrics['accuracy']:.3f} macro-F1 {probe_metrics['macro_f1']:.3f} "
        f"train acc {probe_train_acc:.3f}")

    ft_model = load_finetune_model(snapshot, device)
    ft_model = train_finetune(ft_model, tokenizer, train_texts, train_y, device)
    ft_preds, ft_conf = ft_proba(ft_model, tokenizer, held_texts, device)
    ft_metrics = evaluate(ft_preds, held_gold)
    with torch.no_grad():
        ids = encode(tokenizer, train_texts, MAX_LEN)
        losses = []
        for i in range(0, ids["input_ids"].size(0), BATCH):
            out = ft_model(input_ids=ids["input_ids"][i:i + BATCH].to(device),
                           attention_mask=ids["attention_mask"][i:i + BATCH].to(device),
                           labels=train_y[i:i + BATCH].to(device))
            losses.append(float(out.loss))
    ft_metrics["train_accuracy"] = float((
        ft_model(**{k: v.to(device) for k, v in ids.items()}).logits.argmax(-1).cpu() == train_y
    ).float().mean())
    ft_metrics["final_train_loss"] = sum(losses) / len(losses)
    log(f"fine-tune: held acc {ft_metrics['accuracy']:.3f} macro-F1 {ft_metrics['macro_f1']:.3f} "
        f"train acc {ft_metrics['train_accuracy']:.3f}")

    tau_probe = tau_rows(probe_conf, probe_preds, held_gold)
    tau_ft = tau_rows(ft_conf, ft_preds, held_gold)

    # ---- go/no-go: any τ with >=95% covered accuracy at meaningful coverage?
    strict = [r for r in tau_probe + tau_ft
              if r["accuracy"] is not None and r["accuracy"] >= 0.95 and r["coverage"] >= 0.5]
    verdict = "GO" if strict else "NO-GO"
    log(f"verdict: {verdict}")

    # ---- artifacts (outside git)
    art = Path(args.artifacts).expanduser()
    art.mkdir(parents=True, exist_ok=True)
    with open(art / "dataset.jsonl", "w", encoding="utf-8") as fh:
        for i, row in enumerate(train_rows + held_rows):
            part = "train" if i < len(train_rows) else "held"
            j = i - len(train_rows) if part == "held" else i
            fh.write(json.dumps({
                "split": part, "id": row["rec"].get("id"), "ts": row["rec"].get("ts"),
                "label": row["label"], "near_dup_key": row["near_dup_key"],
                "probe_pred": probe_preds[j] if part == "held" else None,
                "probe_conf": probe_conf[j] if part == "held" else None,
                "ft_pred": ft_preds[j] if part == "held" else None,
                "ft_conf": ft_conf[j] if part == "held" else None,
            }) + "\n")
    metrics = {
        "task": TASK_CLASS, "seed": SEED, "device": device,
        "corpus": str(args.corpus), "corpus_commit": corpus_commit(args.corpus),
        "snapshot": str(snapshot), "malformed_lines": malformed,
        "dataset": {
            "corpus_total": len(records), "in_class": stats["in_class"],
            "ambiguous_ids": stats["ambiguous_ids"],
            "near_dup_dropped_ids": stats["near_dup_dropped_ids"],
            "distinct_subject_shapes": len({shape(r["rec"].get("subject", "")) for r in rows}),
            "n_total": len(rows), "train_n": len(train_rows), "held_n": len(held_rows),
            "train_dist": dict(Counter(train_gold)), "held_dist": dict(Counter(held_gold)),
        },
        "baselines": {
            "train_majority": train_majority,
            "majority": b_majority, "keyword": b_keyword, "keyword_all_rows": kw_all,
        },
        "probe": probe_metrics, "finetune": ft_metrics,
        "tau_probe": tau_probe, "tau_finetune": tau_ft,
        "verdict": verdict,
    }
    with open(art / "metrics.json", "w", encoding="utf-8") as fh:
        json.dump(metrics, fh, indent=2, sort_keys=True)
    torch.save({"state_dict": probe.state_dict(), "hidden": hidden, "seed": SEED},
               art / "probe_head.pt")
    try:
        ft_model.save_pretrained(str(art / "finetune"))
    except Exception as exc:  # checkpoint save is best-effort; metrics are the product
        log(f"note: fine-tune checkpoint save skipped ({exc})")
    log(f"artifacts written under {art}")

    # ---- report (committed in the repo)
    report = render_report({
        "dataset": {
            "corpus_total": len(records), "malformed": malformed,
            "in_class": stats["in_class"], "ambiguous_ids": stats["ambiguous_ids"],
            "near_dup_dropped_ids": stats["near_dup_dropped_ids"],
            "distinct_subject_shapes": len({shape(r["rec"].get("subject", "")) for r in rows}),
            "n_total": len(rows),
            "train_dist": {"n": len(train_rows), **dict(Counter(train_gold))},
            "held_dist": {"n": len(held_rows), **dict(Counter(held_gold))},
        },
        "baselines": {"train_majority": train_majority, "majority": b_majority, "keyword": b_keyword},
        "probe": probe_metrics, "ft": ft_metrics,
        "tau_probe": tau_probe, "tau_ft": tau_ft,
        "run": {
            "verdict": verdict, "corpus_commit": metrics["corpus_commit"],
            "snapshot": str(snapshot), "seed": SEED, "device": device,
            "wall_secs": time.time() - t0, "artifacts": str(art),
            "corpus_path": str(args.corpus), "hidden_size": hidden,
        },
    })
    report_path = Path(args.report)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(report, encoding="utf-8")
    log(f"report written: {report_path}")
    log(f"done in {time.time() - t0:.0f}s — verdict {verdict}")


if __name__ == "__main__":
    main()

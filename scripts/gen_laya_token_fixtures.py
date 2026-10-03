"""Generate token-level parity fixtures for T204 from the Python laya SDK (cpu/fp32).

Complements /Users/jadams/models/laya/golden-vectors.json (the orchestrator's
probabilities goldens): this script replays the SAME 8 fixtures through
rl_agent_api.system_one's internals — serialize_state, build_sequence,
the collate/forward — and records, per fixture:

  state_text    json.dumps(state, ensure_ascii=False)  (what build_sequence tokenizes)
  state_pairs   the state as ordered [key, value] pairs (JSON objects lose
                insertion order in serde_json, so the Rust test rebuilds the
                ordered state from pairs and asserts the byte-exact text)
  state_ids     FULL tokenization of the (mask-masked) state text, pre-truncation
  questions     per-question internals: type/ins text, criteria as ordered
                pairs, head text, head_ids (full), option texts, opt_ids
                ([MASK] + ids[:48]), and the final ids + markers from
                build_sequence (the assembled, budget-truncated sequence)
  logits        the RAW (pre-temperature) scorer logits per option, f32
  act_probability  softmax(act)[0], f32 — the rl_agent ext field
  n_tokens      the batched attention_mask sum system_one reports as usage

The Rust side (src/judge_pack.rs + tests/laya_parity.rs) asserts, with NO
model weights in the loop:
  - serialize_state(from_pairs(state_pairs)) == state_text        (byte-exact)
  - assembly(head_ids, opt_ids, state_ids)      == (ids, markers) (token-exact)
  - answers_payload(logits, act, n_tokens)      == expected          (shape+values)

so packing/sequence/answer-shape parity is pinned weightless, and the
probability goldens stay pinned by the live (weights-gated) test.

Usage: /Users/jadams/models/laya/venv/bin/python scripts/gen_laya_token_fixtures.py
(writes tests/fixtures/laya/token-fixtures.json; needs the HF cache pre-staged
or network for snapshot_download).
"""
import importlib.util
import json
import sys

import numpy as np
import torch
from huggingface_hub import snapshot_download

GOLDEN = "/Users/jadams/models/laya/golden-vectors.json"
OUT = "tests/fixtures/laya/token-fixtures.json"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[name] = mod
    spec.loader.exec_module(mod)
    return mod


def to_pairs(v):
    """JSON-dump-stable ordered view: dicts -> [key, value] pair lists."""
    if isinstance(v, dict):
        return [[k, to_pairs(x)] for k, x in v.items()]
    if isinstance(v, (list, tuple)):
        return [to_pairs(x) for x in v]
    return v


def agent_for(repo_id):
    snap = snapshot_download(repo_id)
    sys.path.insert(0, snap)
    for m in ("rl_agent_api", "rl_common"):
        sys.modules.pop(m, None)
    api = load_module("rl_agent_api", f"{snap}/rl_agent_api.py")
    common = importlib.import_module("rl_common")
    agent = api.RLAgent(snap, device="cpu")
    return agent, common, snap


def replay(agent, common, state, questions):
    """Exactly system_one's internals, recording every intermediate."""
    tok = agent.tok
    cfg = agent.cfg
    qids = list(questions.keys())
    items, recs = [], []
    for qid in qids:
        q = agent._to_internal(questions[qid])
        seq, markers = common.build_sequence(tok, state, q, cfg["max_len"], cfg["head_max_len"])
        if len(markers) != len(common.render_options(q)):
            raise ValueError(f"question {qid!r}: options do not fit")
        items.append({"ids": seq, "markers": markers, "qtype": common.QTYPES[q["t"]],
                      "target": [0.0] * len(markers), "label": -1,
                      "episode": 0, "ep_step": 0, "ep_len": 1, "src": "api"})
        mask_tok = tok.mask_token
        opts = common.render_options(q)
        head_text = "%s question: %s" % (q["t"], str(q["ins"]).replace(mask_tok, " "))
        state_text = common.serialize_state(state).replace(mask_tok, " ")
        recs.append({
            "id": qid,
            "type": q["t"],
            "ins": q["ins"],
            "crit_pairs": to_pairs(q["crit"]),
            "head_text": head_text,
            "head_ids": tok(head_text, add_special_tokens=False)["input_ids"],
            "opt_texts": opts,
            "opt_ids": [[tok.mask_token_id] + tok(" " + o.replace(mask_tok, " "), add_special_tokens=False)["input_ids"][:48] for o in opts],
            "state_ids": tok(state_text, add_special_tokens=False)["input_ids"],
            "ids": seq,
            "markers": markers,
        })
    # the system_one collate + forward, verbatim
    b = common.collate_items([items], tok.pad_token_id)
    with torch.no_grad():
        logits, act = agent.model(b["input_ids"], b["attention_mask"], b["marker_pos"],
                                  b["marker_mask"], b["qtype"])
    logits = logits.float().numpy()
    act = torch.softmax(act.float(), -1).numpy()
    n_tokens = int(b["attention_mask"].sum())
    return qids, recs, logits, act, n_tokens


def main():
    gold = json.load(open(GOLDEN))
    out = {
        "generated_by": "scripts/gen_laya_token_fixtures.py (Python SDK internals, cpu/fp32; "
                        "replays golden-vectors.json's 8 fixtures)",
        "fixtures": {},
    }
    agents = {}
    for fx in gold["fixtures"]:
        repo = fx["checkpoint"]
        if repo not in agents:
            agents[repo] = agent_for(repo)
        agent, common, _ = agents[repo]
        qids, recs, logits, act, n_tokens = replay(agent, common, fx["state"], fx["questions"])
        out["fixtures"][fx["name"] + ":" + fx["command"] if fx["name"] == "riskgate_base" else fx["name"]] = {
            "checkpoint": repo,
            "state_pairs": to_pairs(fx["state"]),
            "state_text": common.serialize_state(fx["state"]),
            "max_len": agent.cfg["max_len"],
            "head_max_len": agent.cfg["head_max_len"],
            "temperature": agent.cfg["temperature"],
            "temperature_by_options": agent.cfg.get("temperature_by_options", {}),
            "questions": recs,
            "logits": [[float(x) for x in logits[r, :len(recs[r]["markers"])]] for r in range(len(recs))],
            "act_probability": [float(act[r, 0]) for r in range(len(recs))],
            "n_tokens": n_tokens,
            "expected": fx["expected"],
        }
        print(f"recorded {fx['name']} ({repo}): {len(recs)} question(s), {n_tokens} tokens", file=sys.stderr)

    with open(OUT, "w") as f:
        json.dump(out, f, indent=1)
    print(f"wrote {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()

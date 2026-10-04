#!/usr/bin/env bash
# judge-parity.sh — T223: the judge-parity bake-off RUN (T220's measurement
# half, sequenced after T222's loader half).
#
# Runs the judges over the two real corpora and emits the T222 metric set
# per class:
#   (a) T204's stop-judge golden vectors (tests/fixtures/laya/golden-vectors.json)
#       — the PARITY panel: the served checkpoint must reproduce the
#       SDK-recorded probabilities (within 1e-3, the live-parity tolerance),
#       plus the option-order flip panel over the same fixtures;
#   (b) the labeled decision-log holdout — the ACCURACY panel: rows from
#       scripts/decisions-export.sh (decisions joined with outcome labels),
#       closed-set labels only, the time-ordered last-20% tail (the F13
#       split doctrine), judged on outcome prediction: choice top-1 over
#       {landed-clean, fixed-up, reverted} + noul AUROC over clean-vs-not.
#
# Contestants:
#   * Laya — the two committed checkpoints, each pinned by revision sha
#     (T205): convaiinnovations/laya (the shipped default) serves the
#     holdout + its golden fixtures; tampajohn/laya-stop-completion-judge
#     serves the stop-judge golden fixtures. Both run through the REAL
#     daemon /judge path (release build) — the exact served wire, not a
#     test double.
#   * kev-0.8b — T222's loader, pinned sha. The daemon is pointed at it
#     verbatim; whatever happens is the record (today: the classified
#     candle-blocked refusal — the base is a Gated DeltaNet hybrid
#     candle-transformers 0.11 cannot load).
#   * Heman10x-NGU/openJev-verdict-2.0 — id-resolution probe only (one GET,
#     recorded verbatim; nothing else fetched mid-run).
#
# Fail-closed measurement (T217 doctrine): the runner exits nonzero if ANY
# judge fails to load, if a judge request fails, or if the golden parity
# drifts past 1e-3 — a partial measurement is emitted and REPORTED, never
# passed off as green. Idempotent: same corpora + pinned revisions + binary
# rebuild the same artifacts byte-for-byte (no wall-clock fields anywhere).
#
# Artifacts (outside git, the distill-f13-2b convention):
#   $JP_ARTIFACTS/  holdout-export.jsonl + export-summary.txt + sha256 pins,
#                   requests.jsonl (the manifest), responses/, holdout-meta.json,
#                   kev-refusal.txt, verdict2-probe.{json,txt}, extern-anchor.txt,
#                   served-*.txt, binary-version.txt, commit.txt, metrics.json
#
# Usage:
#   scripts/judge-parity.sh [CORPUS]
#     CORPUS is a decisions.jsonl path, OR a directory holding
#     .chug/decisions.jsonl; empty = this checkout's .chug/decisions.jsonl.
#     (The decision corpus is host runtime state — point the runner at the
#     main checkout when running from a worktree.)
#   Env: JP_ARTIFACTS (default /Users/jadams/models/laya/judge-parity-t223),
#        CHUG_BIN (default $CARGO_TARGET_DIR/release/chug, built if absent),
#        LAYA_DEFAULT_REV / LAYA_STOP_REV / KEV_REF (pinned shas; a re-pin is
#        a deliberate operator act).
set -uo pipefail
export LC_ALL=C

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ART="${JP_ARTIFACTS:-/Users/jadams/models/laya/judge-parity-t223}"
CORPUS="${1:-}"
if [ -z "$CORPUS" ]; then
  CORPUS="$REPO_ROOT/.chug/decisions.jsonl"
elif [ -d "$CORPUS" ]; then
  CORPUS="$CORPUS/.chug/decisions.jsonl"
fi

# T205 provenance: every contestant pinned by revision sha. The laya shas are
# the cache-resolved revisions the committed goldens were generated from
# (refs/main at filing, 2026-10-04); the kev shas are T222's.
LAYA_DEFAULT_SPEC="${LAYA_DEFAULT_REPO:-convaiinnovations/laya}@${LAYA_DEFAULT_REV:-55cf4c4ebb4ebe31b2550e8bdf3bd21b9975385}"
LAYA_STOP_SPEC="${LAYA_STOP_REPO:-tampajohn/laya-stop-completion-judge}@${LAYA_STOP_REV:-c1931d4ffbb90c3584ef936b5d8eae7bbe8c82d}"
KEV_REF="${KEV_REF:-jaredpalmer/kev-0.8b@bf75a6a8848ea6960ff2ed108d9ed44c2941174f}"
VERDICT2_ID="Heman10x-NGU/openJev-verdict-2.0"

GOLDENS="$REPO_ROOT/tests/fixtures/laya/golden-vectors.json"
EXPORT_SH="$REPO_ROOT/scripts/decisions-export.sh"

fail() { printf 'judge-parity: FAIL: %s\n' "$*" >&2; }
note() { printf 'judge-parity: %s\n' "$*" >&2; }  # stderr: stdout stays reserved for command substitution — never mix logs into it

# ---------------------------------------------------------------------------
# 0. Binary + inputs (fail-closed before anything runs)
# ---------------------------------------------------------------------------
mkdir -p "$ART/responses"
CHUG_BIN="${CHUG_BIN:-${CARGO_TARGET_DIR:-$REPO_ROOT/target}/release/chug}"
if [ ! -x "$CHUG_BIN" ]; then
  note "building the daemon binary (release + daemon feature) -> $CHUG_BIN"
  ( cd "$REPO_ROOT" && cargo build --release --features daemon ) || { fail "daemon build failed"; exit 1; }
fi
[ -x "$CHUG_BIN" ] || { fail "no daemon binary at $CHUG_BIN"; exit 1; }
[ -f "$GOLDENS" ] || { fail "missing $GOLDENS"; exit 1; }
[ -f "$EXPORT_SH" ] || { fail "missing $EXPORT_SH"; exit 1; }
"$CHUG_BIN" --version > "$ART/binary-version.txt" 2>&1
( cd "$REPO_ROOT" && git rev-parse HEAD ) > "$ART/commit.txt" 2>/dev/null || printf 'unknown\n' > "$ART/commit.txt"

PID_LIST=""
cleanup() { for p in $PID_LIST; do kill "$p" >/dev/null 2>&1 || true; done; wait 2>/dev/null || true; PID_LIST=""; }
trap cleanup EXIT
trap 'cleanup; exit 130' INT   # Ctrl-C: reap every daemon this script started, then stop
trap 'cleanup; exit 143' TERM

# ---------------------------------------------------------------------------
# 1. Corpus (b): the labeled decision-log holdout via T200's export (a pure
#    filter). The summary line (N rows, M labeled, K violations) is the
#    corpus-size record; the sha256s pin the exact bytes measured.
# ---------------------------------------------------------------------------
if [ -f "$CORPUS" ] && [ -r "$CORPUS" ]; then
  bash "$EXPORT_SH" "$CORPUS" > "$ART/holdout-export.jsonl" 2> "$ART/export-summary.txt" \
    || { fail "decisions-export.sh failed"; exit 1; }
else
  fail "no readable decision corpus at $CORPUS — the holdout cannot be measured (fail-closed)"
  exit 1
fi
SUMMARY="$(cat "$ART/export-summary.txt")"
note "export: $SUMMARY"

# ---------------------------------------------------------------------------
# 2. Build the request manifest: corpus (a) fixtures (routed per fixture
#    checkpoint, canonical + option-flipped orderings) + corpus (b) holdout
#    records (outcome choice + noul, canonical + flipped). Python stdlib
#    only — no judge client, no inference, no network.
# ---------------------------------------------------------------------------
python3 - "$GOLDENS" "$ART" "$CORPUS" "$LAYA_DEFAULT_SPEC" "$LAYA_STOP_SPEC" <<'PYEOF' || { fail "manifest build failed"; exit 1; }
import hashlib, json, sys

goldens_path, art, corpus_path, laya_default_spec, laya_stop_spec = sys.argv[1:6]
goldens = json.load(open(goldens_path))
out = open(f"{art}/requests.jsonl", "w")

OUTCOME_LABELS = ["landed-clean", "fixed-up", "reverted"]
OUTCOME_DESC = {
    "landed-clean": "the work merged or landed with no fix-up and no revert",
    "fixed-up": "a follow-up commit was needed before it landed",
    "reverted": "the work was undone or reverted",
}

def outcome_choice_q(order):
    keys = OUTCOME_LABELS if order == "canon" else list(reversed(OUTCOME_LABELS))
    return {"type": "choice",
            "instructions": "A coding-agent loop's decision record is shown in the state "
                            "(class, subject, inputs, and the options that were considered). "
                            "Predict which outcome the decision ultimately led to.",
            "criteria": {k: OUTCOME_DESC[k] for k in keys}}

def outcome_noul_q():
    # noul's option order is FIXED by the SDK contract ([false, true]) — the
    # criteria keys select descriptions, never slots — so noul has no second
    # ordering and contributes no flip pair.
    return {"type": "noul",
            "instructions": "Did the recorded decision land cleanly — no fix-up commit, no revert?",
            "criteria": {"false": "no — the decision was fixed up or reverted",
                         "true": "yes — the decision landed cleanly"}}

def flip_question(q):
    """One golden question's second ordering: choice criteria (object or list)
    and score levels reverse; noul is order-fixed -> None (no flip pair)."""
    if q["type"] == "choice" and isinstance(q.get("criteria"), dict):
        g = dict(q); g["criteria"] = {k: q["criteria"][k] for k in reversed(list(q["criteria"]))}
        return g
    if q["type"] == "choice" and isinstance(q.get("criteria"), list):
        g = dict(q); g["criteria"] = list(reversed(q["criteria"])); return g
    if q["type"] == "score":
        g = dict(q); g["criteria"] = list(reversed(q["criteria"])); return g
    return None

def emit(leg, ckpt, name, order, request, idx=None):
    key = f"{idx:02d}-{name}" if idx is not None else name
    rec = {"leg": leg, "checkpoint": ckpt, "fixture": name, "key": key, "order": order,
           "path": f"responses/{leg}-{key}-{order}.json", "request": request}
    out.write(json.dumps(rec) + "\n")

# --- corpus (a): the golden fixtures, routed by their own checkpoint field ---
# Keyed by INDEX, not name: several fixtures share a name (4x riskgate_base)
# with different states — a name key would collide their response files.
for i, fx in enumerate(goldens["fixtures"]):
    name, ckpt = fx["name"], fx["checkpoint"]
    # Fail-closed routing (T217): a fixture naming an unknown checkpoint must
    # stop the run, never fall open onto the stop spec (a silently misrouted
    # fixture would measure the wrong judge and read as parity).
    known = {"convaiinnovations/laya": laya_default_spec,
             "tampajohn/laya-stop-completion-judge": laya_stop_spec}
    if ckpt not in known:
        raise SystemExit(f"fixture {name!r}: unknown checkpoint {ckpt!r} — refusing to route (fail-closed)")
    spec = known[ckpt]
    for order in ("canon", "flip"):
        questions = {}
        for qid, q in fx["questions"].items():
            questions[qid] = (order == "flip" and flip_question(q)) or q
        emit("goldens", spec, name, order, {"state": fx["state"], "questions": questions}, idx=i)
print(f"manifest: {len(goldens['fixtures'])} golden fixtures x 2 orderings", file=sys.stderr)

# --- corpus (b): the labeled closed-set holdout, time-ordered last-20% tail ---
rows = [json.loads(l) for l in open(f"{art}/holdout-export.jsonl") if l.strip()]
labeled = [r for r in rows if r.get("outcome")]
closed = [r for r in labeled if r["outcome"]["choice"] in OUTCOME_LABELS]
n_hold = max(1, round(0.2 * len(closed)))
holdout = closed[-n_hold:]
meta = {"rows": len(rows), "labeled": len(labeled),
        "closed_set_labeled": len(closed), "prose_labels_excluded": len(labeled) - len(closed),
        "holdout_rule": "last 20% of the closed-set labeled rows in file order (the F13 time-ordered tail)",
        "holdout_n": len(holdout)}
json.dump(meta, open(f"{art}/holdout-meta.json", "w"), indent=1)

for r in holdout:
    state = {"from": "decision-log-holdout", "class": r["class"], "subject": r["subject"],
             "inputs": r["inputs"], "options_recorded": r["options"]}
    for order in ("canon", "flip"):
        qs = {"outcome_class": outcome_choice_q(order), "outcome_clean": outcome_noul_q()}
        emit("holdout", laya_default_spec, r["id"], order, {"state": state, "questions": qs})
out.close()
print(f"manifest: holdout {len(holdout)} records x 2 orderings", file=sys.stderr)

# --- provenance pins: the exact bytes measured (T205/T208 discipline) -------
pins = {
    "corpus_path": corpus_path,
    "corpus_sha256": hashlib.sha256(open(corpus_path, "rb").read()).hexdigest(),
    "holdout_export_sha256": hashlib.sha256(open(f"{art}/holdout-export.jsonl", "rb").read()).hexdigest(),
    "goldens_sha256": hashlib.sha256(open(goldens_path, "rb").read()).hexdigest(),
    "export_summary": open(f"{art}/export-summary.txt").read().strip(),
    "laya_default": laya_default_spec, "laya_stop": laya_stop_spec,
}
json.dump(pins, open(f"{art}/pins.json", "w"), indent=1)
PYEOF

# ---------------------------------------------------------------------------
# 3. Serve + measure. One daemon per laya checkpoint (the model loads before
#    the socket binds, so healthz == warm); every response stored verbatim.
# ---------------------------------------------------------------------------
RUN_OK=1

wait_healthy() { # $1 sock, $2 attempts, [$3 pid — bail out early when the daemon already died]
  local i=0
  while [ "$i" -lt "$2" ]; do
    if [ -n "${3:-}" ] && ! kill -0 "$3" 2>/dev/null; then return 1; fi  # exited: refusal or crash
    curl -s --max-time 5 --unix-socket "$1" http://chug/healthz 2>/dev/null | grep -q '"status":"ok"' && return 0
    sleep 1; i=$((i + 1))
  done
  return 1
}

serve_leg() { # $1 checkpoint spec, $2 log suffix; sets CUR_SOCK to the socket path on success.
  # MUST be called from the parent shell — NEVER inside "$( ...)": the daemon
  # pid is booked into PID_LIST here, and command substitution runs this
  # function in a subshell whose PID_LIST copy dies with it, orphaning the
  # daemon (the leak class the T223 fix-up round closed — the validator found
  # every serve-leg daemon orphaned per run). The kev leg below is this same
  # pattern written inline, and is why it never leaked.
  local spec="$1" suffix="$2" home sock pid
  CUR_SOCK=""
  home="$(mktemp -d "${TMPDIR:-/tmp}/jp-home.XXXXXX")" || return 1
  sock="$home/judge.sock"
  CHUG_HOME="$home" CHUG_DAEMON_SOCK="$sock" CHUG_LAYA_CHECKPOINT="$spec" \
    "$CHUG_BIN" daemon > "$ART/daemon-$suffix.log" 2>&1 &
  pid=$!
  PID_LIST="$pid $PID_LIST"
  if ! wait_healthy "$sock" 240 "$pid"; then
    fail "the $suffix daemon never became healthy (checkpoint $spec) — see $ART/daemon-$suffix.log"
    return 1
  fi
  printf '%s\n' "$spec" > "$ART/served-$suffix.txt"
  note "$suffix daemon warm: $spec"
  CUR_SOCK="$sock"
}

post_manifest() { # $1 jq select over the request manifest
  local sel="$1" line req path status
  while IFS= read -r line; do
    req="$(printf '%s' "$line" | jq -c '.request')"
    path="$(printf '%s' "$line" | jq -r '.path')"
    status="$(curl -s --max-time 60 --unix-socket "$CUR_SOCK" -o "$ART/$path" -w '%{http_code}' \
      -X POST -H 'Content-Type: application/json' --data-binary "$req" http://chug/judge 2>/dev/null)" || status="curl-error"
    if [ "$status" != "200" ]; then
      fail "judge request $path returned $status"
      RUN_OK=0
    fi
  done < <(jq -c "$sel" "$ART/requests.jsonl")
}

stop_daemons() { for p in $PID_LIST; do kill "$p" >/dev/null 2>&1 || true; done; wait 2>/dev/null || true; PID_LIST=""; }

# --- leg A: the default checkpoint (the holdout + its golden fixtures) ---
serve_leg "$LAYA_DEFAULT_SPEC" "laya-default" || RUN_OK=0
if [ -n "$CUR_SOCK" ] && [ -S "$CUR_SOCK" ]; then
  post_manifest "select(.leg==\"holdout\" or (.leg==\"goldens\" and .checkpoint==\"$LAYA_DEFAULT_SPEC\"))"
fi
stop_daemons

# --- leg B: the stop-completion checkpoint (its golden fixtures) ---
serve_leg "$LAYA_STOP_SPEC" "laya-stop" || RUN_OK=0
if [ -n "$CUR_SOCK" ] && [ -S "$CUR_SOCK" ]; then
  post_manifest "select(.leg==\"goldens\" and .checkpoint==\"$LAYA_STOP_SPEC\")"
fi
stop_daemons

# --- leg C: kev-0.8b, pinned sha — whatever the daemon does is the record ---
home="$(mktemp -d "${TMPDIR:-/tmp}/jp-home.XXXXXX")"
sock="$home/kev.sock"
CHUG_HOME="$home" CHUG_DAEMON_SOCK="$sock" CHUG_LAYA_CHECKPOINT="$KEV_REF" \
  "$CHUG_BIN" daemon > "$ART/kev.log" 2>&1 &
kev_pid=$!
PID_LIST="$kev_pid $PID_LIST"
KEV_LOADED=0
i=0
while [ "$i" -lt 60 ]; do
  kill -0 "$kev_pid" 2>/dev/null || break            # exited: refusal or crash
  if wait_healthy "$sock" 1; then KEV_LOADED=1; break; fi
  i=$((i + 1))
done
if [ "$KEV_LOADED" -eq 1 ]; then
  fail "the kev checkpoint LOADED — this runner has no kev measurement leg yet; extend the runner before trusting any kev number (fail-closed: an unmeasured contestant)"
  RUN_OK=0
  printf 'LOADED (no kev measurement leg in this runner)\n' > "$ART/kev-refusal.txt"
else
  if grep -q "cannot serve yet" "$ART/kev.log" 2>/dev/null; then
    note "kev leg: the daemon refused the pinned checkpoint (classified) — recorded"
  else
    note "kev leg: the daemon failed to load (unclassified failure) — recorded"
  fi
  RUN_OK=0  # fail-closed: a judge failed to load, whatever the reason
  cp "$ART/kev.log" "$ART/kev-refusal.txt"
fi
stop_daemons

# --- the verdict-2.0 id-resolution probe (one GET, recorded verbatim) ---
probe="$(curl -s --max-time 15 -o "$ART/verdict2-probe.json" -w '%{http_code}' \
  "https://huggingface.co/api/models/$VERDICT2_ID" 2>/dev/null)" || probe="unreachable"
printf '%s\n' "$probe" > "$ART/verdict2-probe.txt"
note "verdict-2.0 id probe: HTTP $probe (200 = resolves; 401/404 = absent/private as written)"

# --- the LocalLLaMA typed-decisions external anchor: only if already local ---
if ls "$REPO_ROOT"/../*typed-decisions* >/dev/null 2>&1 || ls "${HOME:-/nonexistent}"/models/*typed-decisions* >/dev/null 2>&1; then
  note "external anchor: a local typed-decisions suite exists — run it alongside (not this runner's job)"
  printf 'local\n' > "$ART/extern-anchor.txt"
else
  note "external anchor: no local typed-decisions suite — skipped (req 4: never fetched mid-run)"
  printf 'not-local-skipped\n' > "$ART/extern-anchor.txt"
fi

# ---------------------------------------------------------------------------
# 4. Metrics: the T222 set per class over both corpora, from the stored
#    responses. Golden parity tolerance 1e-3 (the live-parity tolerance).
# ---------------------------------------------------------------------------
python3 - "$ART" "$GOLDENS" <<'PYEOF' || { fail "metric computation failed"; exit 1; }
import json, os, sys

art, goldens_path = sys.argv[1], sys.argv[2]
TOL = 1e-3
OUTCOME_LABELS = ["landed-clean", "fixed-up", "reverted"]
manifest = [json.loads(l) for l in open(f"{art}/requests.jsonl") if l.strip()]

def argmax(ps):  # first-max on exact ties. NOT the wire's rule — the daemon's
    # max_by returns the LAST max — but the divergence is harmless: the runner
    # derives every top-1 here from the stored probability vectors (served and
    # recorded alike) with this one rule and never reads the daemon's choice.
    best = 0
    for i, v in enumerate(ps):
        if v > ps[best]:
            best = i
    return best

def auroc(scored):  # Mann-Whitney midranks (ties 0.5); one-class is an error
    n_pos = sum(1 for _, y in scored if y)
    n_neg = len(scored) - n_pos
    if n_pos == 0 or n_neg == 0:
        raise ValueError("auroc needs both classes in the corpus")
    order = sorted(range(len(scored)), key=lambda i: scored[i][0])
    ranks = [0.0] * len(scored)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and scored[order[j + 1]][0] == scored[order[i]][0]:
            j += 1
        mid = (i + j + 2) / 2  # 1-based midrank
        for k in range(i, j + 1):
            ranks[order[k]] = mid
        i = j + 1
    r_pos = sum(ranks[idx] for idx, (_, y) in enumerate(scored) if y)
    u = r_pos - n_pos * (n_pos + 1) / 2
    return u / (n_pos * n_neg)

# ---- corpus (a): golden parity + the flip panel -------------------------
goldens = json.load(open(goldens_path))
fx_by_key = {f"{i:02d}-{fx['name']}": fx for i, fx in enumerate(goldens["fixtures"])}
parity = []        # (fixture, qid, option-id, served, recorded)
a_pairs = {}       # key -> {order: answers}
a_flips, a_pdiffs = [], []
a_top_hits = a_top_n = 0
a_score_abs, a_score_n = 0.0, 0

for rec in manifest:
    if rec["leg"] != "goldens":
        continue
    fx = fx_by_key[rec["key"]]
    resp = json.load(open(f"{art}/{rec['path']}"))
    a_pairs.setdefault(rec["key"], {})[rec["order"]] = resp["answers"]
    if rec["order"] != "canon":
        continue
    exp = fx["expected"]["answers"]
    for qid, a in resp["answers"].items():
        e = exp[qid]
        if e["type"] == "choice":
            keys = list(e["probabilities"].keys())
            served = [a["probabilities"][k] for k in keys]
            recorded = [e["probabilities"][k] for k in keys]
            parity.extend((rec["fixture"], qid, k, s, r) for k, s, r in zip(keys, served, recorded))
            a_top_n += 1
            a_top_hits += int(argmax(served) == argmax(recorded))
        elif e["type"] == "score":
            k = len(e["probabilities"])
            served = [a["probabilities"][str(i)] for i in range(k)]
            recorded = [e["probabilities"][str(i)] for i in range(k)]
            parity.extend((rec["fixture"], qid, str(i), s, r) for i, (s, r) in enumerate(zip(served, recorded)))
            a_score_abs += abs(sum(i * p for i, p in enumerate(served)) - e["score"])
            a_score_n += 1
        elif e["type"] == "noul":
            parity.append((rec["fixture"], qid, "true", a["noul"], e["noul"]))

max_parity = max((abs(s - r) for _, _, _, s, r in parity), default=None)
parity_bad = [t for t in parity if abs(t[3] - t[4]) > TOL]

for key, orders in a_pairs.items():
    fx = fx_by_key[key]
    if "flip" not in orders:
        continue
    for qid, canon in orders["canon"].items():
        e = fx["expected"]["answers"][qid]
        flip = orders["flip"].get(qid)
        if flip is None:
            continue
        if e["type"] == "choice":
            keys = list(e["probabilities"].keys())
            c = [canon["probabilities"][k] for k in keys]
            f = [flip["probabilities"][k] for k in keys]  # keyed by label: identity-safe
            a_flips.append(argmax(c) != argmax(f))
            a_pdiffs.append(sum(abs(x - y) for x, y in zip(c, f)) / len(f))
        elif e["type"] == "score":
            k = len(e["probabilities"])
            c = [canon["probabilities"][str(i)] for i in range(k)]
            # the flip reversed the levels: flipped slot j holds level k-1-j
            f = [flip["probabilities"][str(k - 1 - i)] for i in range(k)]
            a_flips.append(argmax(c) != argmax(f))
            a_pdiffs.append(sum(abs(x - y) for x, y in zip(c, f)) / len(f))
        # noul: order fixed by the SDK contract — no pair, by design

# ---- corpus (b): holdout accuracy + the flip panel ----------------------
hold_meta = json.load(open(f"{art}/holdout-meta.json"))
holdout_ids = {rec["fixture"] for rec in manifest if rec["leg"] == "holdout"}
labels = {}
for l in open(f"{art}/holdout-export.jsonl"):
    if not l.strip():
        continue
    r = json.loads(l)
    if r["id"] in holdout_ids:
        labels[r["id"]] = (r["outcome"]["choice"], 1 if r["outcome"]["choice"] == "landed-clean" else 0)

b_choice, b_noul, b_pairs = [], [], {}
for rec in manifest:
    if rec["leg"] != "holdout":
        continue
    resp = json.load(open(f"{art}/{rec['path']}"))
    oc, clean = labels[rec["fixture"]]
    if rec["order"] == "canon":
        a = resp["answers"]
        keys = list(a["outcome_class"]["probabilities"].keys())
        probs = [a["outcome_class"]["probabilities"][k] for k in keys]
        b_choice.append((keys[argmax(probs)] == oc, keys, probs))
        b_noul.append((a["outcome_clean"]["noul"], clean == 1))
        b_pairs[rec["fixture"]] = {"canon": a}
    else:
        b_pairs.setdefault(rec["fixture"], {})["flip"] = resp["answers"]

b_top_hits = sum(1 for ok, _, _ in b_choice if ok)
b_flips, b_pdiffs = [], []
for rid, orders in b_pairs.items():
    if "flip" not in orders:
        continue
    keys = list(orders["canon"]["outcome_class"]["probabilities"].keys())
    c = [orders["canon"]["outcome_class"]["probabilities"][k] for k in keys]
    f = [orders["flip"]["outcome_class"]["probabilities"][k] for k in keys]
    b_flips.append(argmax(c) != argmax(f))
    b_pdiffs.append(sum(abs(x - y) for x, y in zip(c, f)) / len(f))

rate = lambda hits, n: round(hits / n, 4) if n else None
metrics = {
    "runner": "scripts/judge-parity.sh (T223)",
    "binary_version": open(f"{art}/binary-version.txt").read().strip(),
    "commit": open(f"{art}/commit.txt").read().strip(),
    "served": {f: open(f"{art}/{f}").read().strip()
               for f in ["served-laya-default.txt", "served-laya-stop.txt"] if os.path.exists(f"{art}/{f}")},
    "pins": json.load(open(f"{art}/pins.json")),
    "corpus_a_goldens": {
        "fixtures": len(fx_by_key),
        "parity_tolerance": TOL,
        "parity_max_abs_p_diff": max_parity,
        "parity_violations": len(parity_bad),
        "choice_top1_agreement": {"hits": a_top_hits, "n": a_top_n, "rate": rate(a_top_hits, a_top_n)},
        "score_mae_vs_recorded": round(a_score_abs / a_score_n, 6) if a_score_n else None,
        "order_flip_rate": {"flips": sum(a_flips), "n": len(a_flips), "rate": rate(sum(a_flips), len(a_flips))},
        "mean_abs_p_diff_flipped": round(sum(a_pdiffs) / len(a_pdiffs), 6) if a_pdiffs else None,
        "noul_note": "noul is order-fixed by the SDK contract ([false, true]) — no flip pair by design",
    },
    "corpus_b_holdout": {
        **hold_meta,
        "task": "outcome prediction over decision-log records (features: class, subject, inputs, "
                "options — pre-decision fields only, the F13 leakage rule; choice/confidence excluded)",
        "choice_top1": {"hits": b_top_hits, "n": len(b_choice), "rate": rate(b_top_hits, len(b_choice))},
        "noul_auroc_clean_vs_not": round(auroc(b_noul), 4),
        "order_flip_rate": {"flips": sum(b_flips), "n": len(b_flips), "rate": rate(sum(b_flips), len(b_flips))},
        "mean_abs_p_diff_flipped": round(sum(b_pdiffs) / len(b_pdiffs), 6) if b_pdiffs else None,
        "score_note": "no score question in the holdout — n/a",
    },
    "parity_failed": len(parity_bad),
}
json.dump(metrics, open(f"{art}/metrics.json", "w"), indent=1)
open(f"{art}/parity-violations.json", "w").write(json.dumps(parity_bad, indent=1))

ca, cb = metrics["corpus_a_goldens"], metrics["corpus_b_holdout"]
print(f"corpus (a) goldens: fixtures={ca['fixtures']} parity_max|dp|={ca['parity_max_abs_p_diff']:.2e} "
      f"violations={ca['parity_violations']} | choice top-1 agreement {a_top_hits}/{a_top_n} "
      f"score MAE {ca['score_mae_vs_recorded']} flip {ca['order_flip_rate']['flips']}/{ca['order_flip_rate']['n']} "
      f"mean|dp| {ca['mean_abs_p_diff_flipped']}")
print(f"corpus (b) holdout: n={cb['holdout_n']} choice top-1 {b_top_hits}/{len(b_choice)} ({cb['choice_top1']['rate']}) "
      f"noul AUROC {cb['noul_auroc_clean_vs_not']} flip {cb['order_flip_rate']['flips']}/{cb['order_flip_rate']['n']} "
      f"mean|dp| {cb['mean_abs_p_diff_flipped']}")
if parity_bad:
    print(f"PARITY FAILURES: {len(parity_bad)} option(s) beyond {TOL}", file=sys.stderr)
    sys.exit(2)
PYEOF
METRICS_OK=$?

# ---------------------------------------------------------------------------
# 5. Fail-closed verdict: nonzero if any leg failed (daemon load, judge
#    request, parity drift) — today the kev refusal alone forces nonzero.
# ---------------------------------------------------------------------------
[ "$METRICS_OK" -eq 0 ] || RUN_OK=0
if [ "$RUN_OK" -eq 0 ]; then
  fail "measurement incomplete or a judge failed to load — the artifacts under $ART carry the partial verdict (fail-closed, T217 doctrine)"
  exit 1
fi
note "measurement complete: $ART/metrics.json"
exit 0

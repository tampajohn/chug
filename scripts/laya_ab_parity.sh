#!/bin/sh
# T204 (F15) dev-time A/B parity: external layad (TCP :8420) vs the baked-in
# chug judge daemon (unix socket) on the SAME fixture request. Not CI — needs
# a running layad and a daemon build with weights. Usage:
#
#   scripts/laya_ab_parity.sh [n_fixtures]
#
# Defaults: the committed SDK goldens (tests/fixtures/laya/golden-vectors.json,
# whose state+questions ARE the /judge request body), ALL of them; pass a
# number for a quick first-N run. The chug daemon must be serving a real
# checkpoint:
#
#   CHUG_LAYA_CHECKPOINT=/path/to/checkpoint ./target/release/chug daemon &
#
# Then this script talks the wire directly on both transports.
#
# Comparison semantics (non-vacuous by construction — a fixture that cannot
# actually be compared FAILS, it never passes by both sides missing the same
# field):
#   * risk-bearing fixtures (both sides answer `answers.risk`): the choice is
#     compared exactly and the probabilities at 1e-3 (4-decimal truncation);
#   * every other fixture: the FULL answers object is compared at 1e-3, so
#     the billing / stop-judge fixtures check every field that DOES exist;
#   * a fixture where one side has `answers.risk` and the other does not, a
#     non-JSON response, or a missing answers object is a FAIL;
#   * the script exits non-zero if NOTHING was compared non-vacuously — a
#     zero-comparison run proves nothing and must never report parity.
#
# Under `set -eu` no jq chain below may hard-error on an absent field or a
# broken response: every extraction routes through `normalize`/`pick`, which
# always produce comparable JSON, so a bad side surfaces as a per-fixture
# FAIL instead of an aborted run.
set -eu

COUNT="${1:-}"
SOCK="${CHUG_DAEMON_SOCK:-$HOME/.chug/daemon.sock}"
LAYA="${LAYA_URL:-http://127.0.0.1:8420}"
FIXTURES="tests/fixtures/laya/golden-vectors.json"

command -v jq >/dev/null || { echo "jq required" >&2; exit 1; }
[ -f "$FIXTURES" ] || { echo "missing fixtures: $FIXTURES" >&2; exit 1; }
[ -S "$SOCK" ] || { echo "no chug daemon socket at $SOCK — start one (chug daemon)" >&2; exit 1; }

# Normalize one side's raw response into canonical comparable JSON:
#   - a non-JSON or empty body becomes {"parity_error": …} (never a jq error);
#   - every number is truncated to 4 decimals (the 1e-3 tolerance);
#   - object keys are sorted (field order is not part of the wire contract).
normalize() {
    _body="$1"
    if [ -z "$_body" ] || ! printf '%s' "$_body" | jq -e 'type' >/dev/null 2>&1; then
        printf '%s' '{"parity_error":"response is not valid JSON"}'
        return 0
    fi
    printf '%s' "$_body" | jq -cS '
        def _trunc: if type == "number" then (. * 10000 | floor / 10000) else . end;
        def _norm: if type == "object" then with_entries(.value |= _norm)
                   elif type == "array" then map(_norm)
                   else _trunc end;
        _norm
    ' 2>/dev/null || printf '%s' '{"parity_error":"normalization failed"}'
}

# Extract one jq expression from an ALREADY-NORMALIZED doc, tolerating any
# shape (indexing a non-object errors in plain jq — `try` turns that into
# null, so an absent/garbage field can never abort the script).
pick() {
    _doc="$1"; _expr="$2"
    printf '%s' "$_doc" | jq -c "try ($_expr) catch null" 2>/dev/null || printf 'null'
}

# One line, bounded (failure dumps stay readable).
snippet() {
    printf '%s' "$1" | cut -c1-240
}

total=0
fail=0
compared=0   # fixtures compared NON-vacuously (real fields on both sides)
risk_n=0
full_n=0
i=0
LEN=$(jq '.fixtures | length' "$FIXTURES")
while [ "$i" -lt "$LEN" ] && { [ -z "$COUNT" ] || [ "$total" -lt "$COUNT" ]; }; do
    request=$(jq -c --argjson i "$i" '{state: .fixtures[$i].state, questions: .fixtures[$i].questions}' "$FIXTURES")
    name=$(jq -r --argjson i "$i" '.fixtures[$i].name // ("fixture-" + ($i | tostring))' "$FIXTURES")

    # A dead side is a per-fixture FAIL (empty body -> parity_error), never
    # an aborted run.
    layad_body=$(curl -sS --max-time 10 -X POST "$LAYA/judge" \
        -H 'Content-Type: application/json' -d "$request" 2>/dev/null || true)
    daemon_body=$(curl -sS --max-time 30 --unix-socket "$SOCK" \
        -X POST http://localhost/judge -H 'Content-Type: application/json' -d "$request" 2>/dev/null || true)

    layad_doc=$(normalize "$layad_body")
    daemon_doc=$(normalize "$daemon_body")

    layad_risk=$(pick "$layad_doc" '.answers.risk')
    daemon_risk=$(pick "$daemon_doc" '.answers.risk')
    layad_answers=$(pick "$layad_doc" '.answers')
    daemon_answers=$(pick "$daemon_doc" '.answers')

    status=ok
    detail=""
    if [ "$layad_risk" != "null" ] && [ "$daemon_risk" != "null" ]; then
        # Risk-bearing fixture: choice exact + probabilities at 1e-3.
        compared=$((compared + 1)); risk_n=$((risk_n + 1))
        layad_choice=$(pick "$layad_risk" '.choice')
        daemon_choice=$(pick "$daemon_risk" '.choice')
        layad_probs=$(pick "$layad_risk" '.probabilities')
        daemon_probs=$(pick "$daemon_risk" '.probabilities')
        if [ "$layad_choice" != "$daemon_choice" ]; then
            status=fail
            detail="choice: layad=$layad_choice daemon=$daemon_choice"
        elif [ "$layad_probs" != "$daemon_probs" ]; then
            status=fail
            detail="probabilities (1e-3): layad=$layad_probs daemon=$daemon_probs"
        elif [ "$layad_risk" != "$daemon_risk" ]; then
            status=fail
            detail="answers.risk differs beyond choice+probabilities"
        fi
        summary="risk choice=$layad_choice probs=$layad_probs"
    elif [ "$layad_risk" = "null" ] && [ "$daemon_risk" = "null" ]; then
        # Non-risk fixture: compare the FULL answers object — every field
        # that exists, at 1e-3. An ABSENT==ABSENT pass is impossible.
        if [ "$layad_answers" != "null" ] && [ "$daemon_answers" != "null" ]; then
            compared=$((compared + 1)); full_n=$((full_n + 1))
            if [ "$layad_answers" != "$daemon_answers" ]; then
                status=fail
                detail="full answers object (1e-3) differs"
            fi
            summary="answers=$(snippet "$layad_answers")"
        else
            status=fail
            detail="no answers object to compare: layad=$layad_answers daemon=$daemon_answers"
            summary="answers=<none>"
        fi
    else
        # Exactly one side answered the risk question: a shape mismatch.
        status=fail
        detail="answers.risk on one side only"
        summary="risk=<one-sided>"
    fi

    total=$((total + 1))
    if [ "$status" = ok ]; then
        echo "ok   $name: $summary"
    else
        fail=$((fail + 1))
        echo "FAIL $name: $detail"
        echo "  layad:  $(snippet "$layad_doc")"
        echo "  daemon: $(snippet "$daemon_doc")"
    fi
    i=$((i + 1))
done

[ "$total" -gt 0 ] || { echo "no fixtures in $FIXTURES" >&2; exit 1; }
# The non-vacuity guard: a run that compared NOTHING real proved nothing.
if [ "$compared" -eq 0 ]; then
    echo "parity: 0/$total fixtures compared non-vacuously — the A/B proved nothing" >&2
    exit 1
fi
echo "parity: $((total - fail))/$total fixtures pass ($risk_n risk-style choice+probabilities, $full_n full-answers), 1e-3"
[ "$fail" -eq 0 ]

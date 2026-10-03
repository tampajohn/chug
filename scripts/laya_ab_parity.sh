#!/bin/sh
# T204 (F15) dev-time A/B parity: external layad (TCP :8420) vs the baked-in
# chug judge daemon (unix socket) on the SAME fixture request. Not CI — needs
# a running layad and a daemon build with weights. Usage:
#
#   scripts/laya_ab_parity.sh [n_fixtures]
#
# Defaults: the committed SDK goldens (tests/fixtures/laya/golden-vectors.json,
# whose state+questions ARE the /judge request body), first 3. The chug daemon
# must be serving a real checkpoint:
#
#   CHUG_LAYA_CHECKPOINT=/path/to/checkpoint ./target/release/chug daemon &
#
# Then this script talks the wire directly on both transports.
set -eu

COUNT="${1:-3}"
SOCK="${CHUG_DAEMON_SOCK:-$HOME/.chug/daemon.sock}"
LAYA="${LAYA_URL:-http://127.0.0.1:8420}"
FIXTURES="tests/fixtures/laya/golden-vectors.json"

command -v jq >/dev/null || { echo "jq required" >&2; exit 1; }
[ -S "$SOCK" ] || { echo "no chug daemon socket at $SOCK — start one (chug daemon)" >&2; exit 1; }

total=0
fail=0
i=0
LEN=$(jq '.fixtures | length' "$FIXTURES")
while [ "$i" -lt "$LEN" ] && [ "$total" -lt "$COUNT" ]; do
    request=$(jq -c --argjson i "$i" '{state: .fixtures[$i].state, questions: .fixtures[$i].questions}' "$FIXTURES")
    name=$(jq -r --argjson i "$i" '.fixtures[$i].name // "fixture \(.)"' "$FIXTURES")

    layad_body=$(curl -sS --max-time 10 -X POST "$LAYA/judge" \
        -H 'Content-Type: application/json' -d "$request")
    daemon_body=$(curl -sS --max-time 30 --unix-socket "$SOCK" \
        -X POST http://localhost/judge -H 'Content-Type: application/json' -d "$request")

    # Compare the verdict fields layad's contract defines: answers.risk.choice
    # and answers.risk.probabilities (1e-3 tolerance via 4-decimal truncation).
    layad_choice=$(printf '%s' "$layad_body" | jq -r '.answers.risk.choice // "ABSENT"')
    daemon_choice=$(printf '%s' "$daemon_body" | jq -r '.answers.risk.choice // "ABSENT"')
    layad_probs=$(printf '%s' "$layad_body" | jq -S '.answers.risk.probabilities // "ABSENT"' \
        | jq 'with_entries(.value |= (. * 10000 | floor / 10000))')
    daemon_probs=$(printf '%s' "$daemon_body" | jq -S '.answers.risk.probabilities // "ABSENT"' \
        | jq 'with_entries(.value |= (. * 10000 | floor / 10000))')

    total=$((total + 1))
    if [ "$layad_choice" = "$daemon_choice" ] && [ "$layad_probs" = "$daemon_probs" ]; then
        echo "ok   $name: choice=$layad_choice probs=$layad_probs"
    else
        fail=$((fail + 1))
        echo "FAIL $name"
        echo "  layad:  choice=$layad_choice probs=$layad_probs"
        echo "  daemon: choice=$daemon_choice probs=$daemon_probs"
    fi
    i=$((i + 1))
done

[ "$total" -gt 0 ] || { echo "no fixtures in $FIXTURES" >&2; exit 1; }
echo "parity: $((total - fail))/$total within 1e-3"
[ "$fail" -eq 0 ]

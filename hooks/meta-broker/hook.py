#!/usr/bin/env python3
"""meta-broker PreToolUse hook handler — T341, the permission-broker pack.

stdin carries Claude's PreToolUse hook payload. When the meta-harness broker
listens on <claude-dir>/meta-broker.sock, one HTTP POST over that unix socket
asks it to adjudicate:

    POST /permission HTTP/1.1          {"tool_name": "...", "input_summary":
                                        "...", "session_id": "...", "cwd":
                                        "...", "contract_version": "1"}

and the reply {"decision": "allow"|"deny"|"ask", "reason": "...", "ttl":
"..."} is honored: allow/deny print Claude's permissionDecision document to
stdout; ask prints nothing (the stock prompt path, unchanged). Fail-open by
construction — socket absent, connect refused, >800ms, a non-200, a
malformed reply, an unknown decision, or a failed audit append all print
nothing and exit 0. A broker crash can never wedge a session.

Every brokered decision appends one JSONL line to <claude-dir>/
meta-broker-audit.jsonl (the harness's audit trail); stock-path outcomes
append nothing. The decision only lands if its audit line is written — an
unauditable decision is not a decision.

Run by hook.sh (which owns the socket-absent fast path); stdin must be the
hook payload (a tty stdin would block — pipe something, e.g. `echo '{}' |
hook.py`). META_BROKER_DEBUG=1 keeps stderr visible for diagnosis.
"""

import json
import os
import socket
import sys
import time
from datetime import datetime, timezone

# The spec's broker budget: the whole interaction (connect through reply)
# gets 800ms of wall clock; anything slower is the stock path.
BUDGET_SECONDS = 0.8

# input_summary is a bounded rendering of the tool input — the socket
# payload stays small no matter how large the actual tool input is.
INPUT_SUMMARY_MAX_CHARS = 2000

# The wire constants (docs/meta-harness.md, the T341 section).
REQUEST_PATH = "/permission"
CONTRACT_VERSION = "1"
DECISIONS = ("allow", "deny", "ask")


def fail_open(why=None):
    """The stock path, byte-identical: no output, exit 0. META_BROKER_DEBUG=1
    names the reason on stderr — diagnosis only, never the session's way."""
    if why and os.environ.get("META_BROKER_DEBUG"):
        sys.stderr.write("meta-broker: stock path (%s)\n" % why)
    sys.exit(0)


def rfc3339_now():
    """RFC3339 UTC with milliseconds — the daemon wire's timestamp shape."""
    now = datetime.now(timezone.utc)
    return "%s.%03dZ" % (now.strftime("%Y-%m-%dT%H:%M:%S"), now.microsecond // 1000)


def input_summary(tool_input):
    """Bounded summary of the tool input: compact JSON, truncated hard."""
    if tool_input is None:
        return None
    try:
        text = json.dumps(tool_input, separators=(",", ":"), default=str)
    except (TypeError, ValueError):
        return None
    if len(text) > INPUT_SUMMARY_MAX_CHARS:
        text = text[:INPUT_SUMMARY_MAX_CHARS] + "..."
    return text


def build_request(payload):
    """The broker request. Absent values are omitted keys — never
    fabricated zeros, never empty strings (the contract's event rule)."""
    tool_name = payload.get("tool_name")
    if not isinstance(tool_name, str) or not tool_name:
        return None
    req = {"tool_name": tool_name, "contract_version": CONTRACT_VERSION}
    summary = input_summary(payload.get("tool_input"))
    if summary is not None:
        req["input_summary"] = summary
    for key in ("session_id", "cwd"):
        value = payload.get(key)
        if isinstance(value, str) and value:
            req[key] = value
    return req


def post_request(sock_path, body):
    """One HTTP POST over the unix socket, whole-interaction 800ms budget.
    Returns the raw reply text; every failure raises (the caller fails
    open)."""
    deadline = time.monotonic() + BUDGET_SECONDS

    def remaining():
        left = deadline - time.monotonic()
        if left <= 0:
            raise TimeoutError("broker budget exhausted")
        return left

    started = time.monotonic()
    conn = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        conn.settimeout(remaining())
        conn.connect(sock_path)
        head = (
            "POST %s HTTP/1.1\r\n"
            "Host: meta-broker\r\n"
            "Content-Type: application/json\r\n"
            "Content-Length: %d\r\n"
            "Connection: close\r\n\r\n" % (REQUEST_PATH, len(body))
        )
        conn.settimeout(remaining())
        conn.sendall(head.encode() + body)
        chunks = []
        while True:
            conn.settimeout(remaining())
            chunk = conn.recv(65536)
            if not chunk:
                break
            chunks.append(chunk)
    finally:
        conn.close()
    latency_ms = int(round((time.monotonic() - started) * 1000))
    return b"".join(chunks).decode("utf-8", "replace"), latency_ms


def parse_reply(raw):
    """HTTP status + body -> the decision document, or None (fail open).
    Only the decision gates brokered-vs-stock: a 200 whose body carries
    decision allow|deny|ask is brokered; everything else is the stock path.
    reason/ttl are tolerated in any shape (non-strings read as absent)."""
    try:
        head, _, body_text = raw.partition("\r\n\r\n")
        status = int(head.split(" ")[1])
    except (IndexError, ValueError):
        return None
    if status != 200:
        return None
    try:
        reply = json.loads(body_text)
    except ValueError:
        return None
    if not isinstance(reply, dict):
        return None
    decision = reply.get("decision")
    if decision not in DECISIONS:
        return None
    doc = {"decision": decision}
    reason = reply.get("reason")
    if isinstance(reason, str) and reason:
        doc["reason"] = reason
    ttl = reply.get("ttl")
    if isinstance(ttl, str) and ttl:
        doc["ttl"] = ttl
    return doc


def audit_line(claude_dir, request, decision_doc, latency_ms):
    """One JSONL audit line for a brokered decision. Returns True when the
    line landed — the decision only takes effect when its audit line does."""
    line = {
        "ts": rfc3339_now(),
        "tool_name": request["tool_name"],
        "decision": decision_doc["decision"],
        "broker_latency_ms": latency_ms,
        "contract_version": CONTRACT_VERSION,
    }
    if "reason" in decision_doc:
        line["reason"] = decision_doc["reason"]
    if "ttl" in decision_doc:
        line["ttl"] = decision_doc["ttl"]
    for key in ("input_summary", "session_id", "cwd"):
        if key in request:
            line[key] = request[key]
    path = os.path.join(claude_dir, "meta-broker-audit.jsonl")
    try:
        # One append, one line: the JSONL contract. The directory exists by
        # the time the socket does (the harness created both), but an
        # operator may have removed it — a failed append fails open.
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(json.dumps(line, separators=(",", ":")) + "\n")
    except OSError:
        return False
    return True


def decision_stdout(decision_doc):
    """Claude's PreToolUse permissionDecision document (allow/deny)."""
    return json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": decision_doc["decision"],
                "permissionDecisionReason": decision_doc.get("reason", ""),
            }
        }
    )


def main():
    claude_dir = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.join(
        os.path.expanduser("~"), ".claude"
    )
    sock_path = os.path.join(claude_dir, "meta-broker.sock")
    try:
        payload = json.loads(sys.stdin.read())
    except ValueError:
        fail_open("payload is not JSON")
    if not isinstance(payload, dict):
        fail_open("payload is not an object")
    request = build_request(payload)
    if request is None:
        fail_open("payload carries no tool_name")
    try:
        body = json.dumps(request, separators=(",", ":")).encode()
        raw, latency_ms = post_request(sock_path, body)
    except (OSError, TimeoutError, ValueError) as exc:
        fail_open("broker post failed: %r" % (exc,))
    decision_doc = parse_reply(raw)
    if decision_doc is None:
        fail_open("reply is not a brokered decision")
    if not audit_line(claude_dir, request, decision_doc, latency_ms):
        # The audit leg IS the contract: a decision that cannot be recorded
        # does not happen. Stock path, silently.
        fail_open()
    if decision_doc["decision"] == "ask":
        # Falls through to the stock prompt path, unchanged — but the
        # decision was brokered, so the audit line above is its record.
        fail_open()
    sys.stdout.write(decision_stdout(decision_doc) + "\n")
    sys.exit(0)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception:  # noqa: BLE001 — fail-open swallows everything
        fail_open()

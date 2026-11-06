#!/usr/bin/env python3
"""ideate.py — T326: the dormant loop's ideation cycle (the machine proposes,
the operator disposes).

The operator expectation that started this row: "I thought it would come up
with new features if left with an empty backlog." It did ideate — but its
only idea source was its own tooling, so ideation collapsed inward (the
T317-T324 meta class). The fix is a DIFFERENT idea source and a ratification
boundary: this script builds a deterministic ideas packet (FEATURES.md parked
items, deferred/blocked threads in LEDGER + docs, the eval-triage class
census from .chug/decisions.jsonl, site/mod staleness facts), turns it into
at most 3 outward-facing proposal stubs, gates each through the Laya noul
meta-class gate (daemon, fail CLOSED), files the survivors into
specs/proposed/ + `## Proposed` rows with status `proposed`, and — per the
2026-11-06 operator blanket delegation (the amendment) — auto-ratifies
eligible `proposed` rows older than 7 days by flipping them to `open`.

Invariants this script exists to hold:

  - The loop NEVER self-flips a proposal the operator vetoed, and NEVER
    auto-flips an internal-artifact row: rows carrying the never-dispatch
    note (`do not dispatch` / `operator-owned`, the P005-P007 marker) are
    skipped by the ratification sweep, whatever their age.
  - The noul gate runs BEFORE filing, always. Daemon unavailable, timeout,
    unparsable answer -> FAIL CLOSED to rejection (better silent than meta).
  - Deterministic, no network: the only I/O beyond the repo is the LOCAL
    daemon socket (the T204 judge transport), and the packet is derived
    from repo files alone.
  - Idempotent by slug: a re-run over unchanged inputs (the same week) files
    nothing — the dedupe key is the slug, checked against both the filed
    spec files and the TODO.md P-rows.
  - TODO.md edits follow the T279 checked-flip discipline: rows are rebuilt
    FROM CELLS, every assert fires PRE-WRITE, and the byte-carry assert is
    IMPORTED from scripts/wrap_assert.py (never re-authored).

Usage:
    python3 scripts/ideate.py [ROOT]            # full cycle (ratify + propose)
    python3 scripts/ideate.py --ratify-only     # only the auto-ratification sweep
    python3 scripts/ideate.py --gate-only FILE  # noul-gate one stub file (exit 2 = reject)
    python3 scripts/ideate.py --check [ROOT]    # verify lane consistency, exit 0/1

Env seams (tests + operator overrides):
    CHUG_NOUL_SOCK      the daemon socket (default: $CHUG_DAEMON_SOCK, then
                        $CHUG_HOME/daemon.sock, then ~/.chug/daemon.sock)
    CHUG_NOUL_TIMEOUT   the gate's socket timeout in seconds (default 2)
"""

import argparse
import datetime
import json
import os
import re
import subprocess
import sys
from http.client import HTTPConnection

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from wrap_assert import assert_byte_carry  # noqa: E402

# The amendment's ratification age (7d) and the per-cycle filing cap.
RATIFY_AFTER_DAYS = 7
MAX_PROPOSALS = 3

# The never-flip markers: a row whose notes carry either is an
# internal-artifact proposal the operator (or their out-of-band session)
# executes — the ratification sweep NEVER flips it, and the filing path
# never emits one (machine rows are outward-facing or they are rejected).
NEVER_FLIP_MARKERS = ("do not dispatch", "operator-owned")

# The TODO.md data-row shape (the T8 guard's own count — `expected exactly
# 6 cells, got N`) and its legal statuses (the T8 vocabulary).
EXPECTED_CELLS = 6
LEGAL_STATUSES = ("todo", "in-progress", "blocked", "done", "open", "proposed", "vetoed")

DATE_RE = re.compile(r"\d{4}-\d{2}-\d{2}")
P_ID_RE = re.compile(r"P(\d+)")


# --- TODO.md row plumbing (the T279 shapes, mirrored from todo_flip.py) -------

def _row_cells(line):
    """Parse one physical line into table cells, mirroring the T8 guard's
    `table_cells` so the two parsers cannot drift apart. Non-row lines
    (no leading `|` after trim) return None; header (first cell `id`) and
    all-dash separator rows return None."""
    stripped = line.strip()
    if not stripped.startswith("|"):
        return None
    cells = [cell.strip() for cell in stripped.strip("|").split("|")]
    if cells and cells[0] == "id":
        return None
    if cells and all(cell != "" and all(ch == "-" for ch in cell) for cell in cells):
        return None
    return cells


def read_todo_lines(root):
    """TODO.md as a line list (split('\\n') — NOT splitlines(): every
    non-edited line is carried byte-exact, and a trailing newline survives
    the join as the carried empty last element)."""
    path = os.path.join(root, "TODO.md")
    if not os.path.isfile(path):
        raise SystemExit(f"ideate: no TODO.md at {path} — nothing to read")
    with open(path, "r", encoding="utf-8") as f:
        return path, f.read().split("\n")


def p_rows(lines):
    """[(idx, cells)] for the Proposed-lane rows — first cell matching
    P<digits>. T-rows are never matched, so the sweep can never flip the
    queue; that is the "loop never self-flips" boundary in its purest form."""
    out = []
    for idx, line in enumerate(lines):
        cells = _row_cells(line)
        if cells is not None and P_ID_RE.fullmatch(cells[0]):
            out.append((idx, cells))
    return out


def require_lane(lines):
    """The `## Proposed` heading must exist — the loop never creates the
    lane (T326 seeded it at ship time); a repo without it gets no rows."""
    for line in lines:
        if line.strip() == "## Proposed":
            return
    raise SystemExit(
        "ideate: TODO.md carries no '## Proposed' lane — the loop never "
        "creates the lane (T326 seeds it); not filing"
    )


def _assert_row_shape(rebuilt, pid, what):
    """The T279 pre-write structural asserts, with the probed actuals."""
    physical = rebuilt.splitlines()
    if len(physical) != 1:
        raise AssertionError(
            f"ideate {what}: the rebuilt row for {pid} is not ONE physical "
            f"line (probed {len(physical)}) — file untouched"
        )
    cells = _row_cells(rebuilt)
    if cells is None or len(cells) != EXPECTED_CELLS:
        got = len(cells) if cells is not None else 0
        raise AssertionError(
            f"ideate {what}: the rebuilt row for {pid} parses into {got} "
            f"cells (need exactly {EXPECTED_CELLS}) — the T8 guard would "
            f"fail it post-edit; file untouched"
        )


def _splice(lines, idx, rebuilt):
    return lines[:idx] + [rebuilt] + lines[idx + 1:]


# --- the ideas packet (deterministic, repo-local facts only) ------------------

def parked_features(root):
    """FEATURES.md `## Parked` items as (Fid, title) — the packet's parked
    half. Only the item's own line is parsed (the bold header is one
    physical line in the shipped file); prose below is ignored."""
    path = os.path.join(root, "FEATURES.md")
    if not os.path.isfile(path):
        return []
    with open(path, "r", encoding="utf-8") as f:
        lines = f.read().split("\n")
    start = None
    for i, line in enumerate(lines):
        if line.strip() == "## Parked":
            start = i + 1
            break
    if start is None:
        return []
    out = []
    for line in lines[start:]:
        if line.startswith("## "):
            break
        m = re.match(r"\s*-\s*\*\*(F\d+)\s*—\s*(.+?)\*\*", line)
        if m:
            out.append((m.group(1), m.group(2).strip()))
    return out


def blocked_threads(root):
    """Deferred/blocked/parked thread fragments from LEDGER.md and docs/ —
    the packet's blocked half. Deterministic scan (sorted file order, first
    match wins), each fragment capped at 60 chars."""
    paths = [os.path.join(root, "LEDGER.md")]
    docs = os.path.join(root, "docs")
    if os.path.isdir(docs):
        paths += [os.path.join(docs, n) for n in sorted(os.listdir(docs))]
    out = []
    seen = set()
    for path in paths:
        if not os.path.isfile(path):
            continue
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            for line in f:
                if not re.search(r"\b(deferred|blocked on|parked since)\b", line, re.I):
                    continue
                text = line.strip().lstrip("-* ").strip()
                text = re.sub(r"\s+", " ", text)[:60].strip()
                if text and text not in seen:
                    seen.add(text)
                    out.append(text)
                if len(out) >= 3:
                    return out
    return out


def triage_census(root):
    """The eval-triage class census: decisions.jsonl record counts by class
    (outcomes excluded — they are backfills, not decisions). Malformed
    lines drop (the corpus-parse tolerance); ranked by count desc then
    class asc — deterministic at any corpus shape."""
    path = os.path.join(root, ".chug", "decisions.jsonl")
    if not os.path.isfile(path):
        return []
    counts = {}
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
            except Exception:
                continue
            cls = rec.get("class") if isinstance(rec, dict) else None
            if not cls or cls == "outcome":
                continue
            counts[cls] = counts.get(cls, 0) + 1
    return sorted(counts.items(), key=lambda kv: (-kv[1], kv[0]))[:3]


def release_lag(root):
    """The site staleness fact: (newest stable v* tag, days old) from the
    LOCAL tags (the site-sync facts shape — `git tag --sort=-creatordate`,
    pre-release channels out of scope). No tags, no git, or an unanchorable
    epoch returns None — the fact reads unknown, the candidate never fires
    on an unknown."""
    try:
        out = subprocess.run(
            ["git", "tag", "--sort=-creatordate", "-l", "v*"],
            cwd=root, capture_output=True, text=True, timeout=10, check=True,
        )
    except Exception:
        return None
    tags = [t.strip() for t in out.stdout.split("\n") if t.strip()]
    stable = [t for t in tags if "-" not in t]
    if not stable:
        return None
    tag = stable[0]
    try:
        out = subprocess.run(
            ["git", "log", "-1", "--format=%ct", tag],
            cwd=root, capture_output=True, text=True, timeout=10, check=True,
        )
        epoch = int(out.stdout.strip())
    except Exception:
        return None
    now = datetime.datetime.now(datetime.timezone.utc).timestamp()
    return (tag, max(0, int((now - epoch) // 86400)))


def chugwatch_version():
    """The mod staleness fact: the local chug-watch checkout's version, or
    'unknown' — a HOST-local read (never network), and a missing checkout
    is a legal unknown."""
    path = os.path.join(os.path.expanduser("~"), "workspace", "chug-watch", "Cargo.toml")
    if not os.path.isfile(path):
        return "unknown (no local checkout)"
    try:
        with open(path, "r", encoding="utf-8") as f:
            for line in f:
                m = re.match(r'\s*version\s*=\s*"([^"]+)"', line)
                if m:
                    return m.group(1)
    except Exception:
        return "unknown"
    return "unknown"


# --- the noul gate (the Laya meta-class classification, fail CLOSED) ----------

NOUL_CONTEXT = (
    "A coding-loop supervisor's ideation gate (T326). Before a machine-filed "
    "feature proposal enters the public TODO.md's Proposed lane, the Laya "
    "noul gate classifies ONE thing: is the proposal primarily about the "
    "loop's OWN tooling, assertions, and bookkeeping (wrap_assert legs, "
    "fixtures, eval machinery, digests, TODO bookkeeping) — or is it "
    "outward-facing (a capability for chug's users, site, or ecosystem)? "
    "The loop's ideation collapsed inward once already (the T317-T324 meta "
    "class); a meta-class proposal is rejected — better silent than meta."
)

NOUL_INSTRUCTIONS = (
    "Classification only, never a quality verdict: is this proposal "
    "primarily about the loop's own tooling/assertions/bookkeeping rather "
    "than an outward-facing capability for chug's users or ecosystem?"
)


class _UnixHTTPConnection(HTTPConnection):
    """http.client over the T204 unix-socket transport (the 0600 daemon
    socket). Timeout applies to connect + read."""

    def __init__(self, sock_path, timeout):
        super().__init__("localhost", timeout=timeout)
        self._sock_path = sock_path

    def connect(self):
        import socket

        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(self.timeout)
        s.connect(self._sock_path)
        self.sock = s


def noul_socket():
    return (
        os.environ.get("CHUG_NOUL_SOCK")
        or os.environ.get("CHUG_DAEMON_SOCK")
        or os.path.join(
            os.environ.get("CHUG_HOME") or os.path.expanduser("~/.chug"), "daemon.sock"
        )
    )


def noul_gate(title, stub_text):
    """True = outward-facing (file it); False = reject.

    FAILS CLOSED (the spec's rule — better silent than meta): daemon
    unavailable, connect/read timeout, non-200, unparsable body, or an
    answer whose choice is anything but the outward 'no' — all reject. Only
    an explicit confident 'no' (not-meta) passes.
    """
    try:
        timeout = float(os.environ.get("CHUG_NOUL_TIMEOUT") or 2)
    except ValueError:
        timeout = 2.0
    try:
        body = json.dumps(
            {
                "state": {
                    "context": NOUL_CONTEXT,
                    "proposal_title": title,
                    "proposal_stub": stub_text,
                },
                "questions": {
                    "meta_class": {
                        "type": "choice",
                        "instructions": NOUL_INSTRUCTIONS,
                        "criteria": {
                            "no": "outward-facing - a real capability for chug's users, site, or ecosystem; file it",
                            "yes": "meta-class - primarily the loop's own tooling/assertions/bookkeeping; reject it",
                        },
                    }
                },
            }
        )
        conn = _UnixHTTPConnection(noul_socket(), timeout)
        try:
            conn.request(
                "POST",
                "/judge",
                body=body,
                headers={"Content-Type": "application/json"},
            )
            resp = conn.getresponse()
            raw = resp.read()
            if resp.status != 200:
                return False
        finally:
            conn.close()
        answer = json.loads(raw.decode("utf-8"))["answers"]["meta_class"]
        return answer.get("choice") == "no"
    except Exception:
        return False


# --- candidates (deterministic rules over the packet) -------------------------

def slugify(text):
    s = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return s[:40].strip("-") or "proposal"


def estimate_pri(estimate):
    return {"XS": 1, "S": 2, "M": 3, "L": 4}.get(estimate, 3)


def build_candidates(packet):
    """At most MAX_PROPOSALS candidate stubs, one rule per packet source,
    in fixed order: the census's top friction class, the parked/unblock
    decision, the release train. Every rule is a pure function of the
    packet, so the same week's inputs yield the same slugs (the idempotence
    the dedupe needs)."""
    cands = []
    census = packet["census"]
    if census and census[0][1] >= 2:
        cls, n = census[0]
        cands.append(
            {
                "title": f"Decision-corpus insights on chug.sh: the {cls} class, browsable",
                "slug": f"decision-corpus-{slugify(cls)}-insights",
                "value": "The F13 decision corpus's recurring friction classes and their outcome accuracy become a public chug.sh page — the loop's measured learning is visible outside the repo.",
                "slice": "Static page generation from scripts/decisions-export.sh output into the site build (the data is already exported; no new backend).",
                "estimate": "S",
                "source": f"decisions.jsonl census: class '{cls}' recurs {n}x — the top recurring friction class (dogfood pain), T326 ideas packet.",
            }
        )
    if packet["parked"]:
        fid, ftitle = packet["parked"][0]
        cands.append(
            {
                "title": f"Decide {fid}: unpark or kill",
                "slug": f"{fid.lower()}-unpark-decision",
                "value": "FEATURES.md stops carrying a parked feature indefinitely — either it ships or it is formally killed, and the public roadmap is honest either way.",
                "slice": "Re-read the feature's spec and current harness state; write a keep/kill memo with the deciding facts; the operator picks the branch.",
                "estimate": "XS",
                "source": f"FEATURES.md ## Parked: {fid} ({ftitle}) sits parked with no re-examination (T326 ideas packet).",
            }
        )
    elif packet["blocked"]:
        thread = packet["blocked"][0]
        cands.append(
            {
                "title": f"Unblock: {thread}",
                "slug": f"unblock-{slugify(thread)}",
                "value": "A thread the repo has carried as deferred/blocked gets a concrete unblocking slice instead of aging silently in the ledger.",
                "slice": "Write the smallest operator-approved slice that moves the thread; file it with the blocking fact attached.",
                "estimate": "S",
                "source": f"LEDGER/docs name this thread deferred/blocked (T326 ideas packet).",
            }
        )
    lag = packet["release_lag"]
    if lag and lag[1] >= 14:
        cands.append(
            {
                "title": "Cut the next chug release train",
                "slug": "next-release-train",
                "value": "The site's release card stops lagging main — users see the shipped state, not a stale tag.",
                "slice": "Run the standing release train: notes, tag, GitHub release, site sync.",
                "estimate": "S",
                "source": f"newest stable tag {lag[0]} is {lag[1]} days old; main has moved past it (T326 ideas packet).",
            }
        )
    return cands[:MAX_PROPOSALS]


# --- the packet print ---------------------------------------------------------

def packet_line(packet):
    parts = []
    parts.append("parked=" + (",".join(fid for fid, _ in packet["parked"]) or "none"))
    parts.append(f"blocked={len(packet['blocked'])}")
    census = packet["census"]
    parts.append(
        "census="
        + (",".join(f"{cls}({n})" for cls, n in census) or "empty")
    )
    lag = packet["release_lag"]
    parts.append("release=" + (f"{lag[0]} {lag[1]}d old" if lag else "unknown"))
    parts.append(f"chug-watch={packet['chugwatch']}")
    return "ideation packet (T326): " + "; ".join(parts)


# --- filing -------------------------------------------------------------------

def proposed_dir(root):
    return os.path.join(root, "specs", "proposed")


def filed_slugs(lines, root):
    """The dedupe keys already taken: slugs of the filed spec files AND of
    the TODO.md P-rows' spec cells (a row whose file was removed still
    holds the slug — the row is the ledger)."""
    slugs = set()
    pdir = proposed_dir(root)
    if os.path.isdir(pdir):
        for name in os.listdir(pdir):
            m = re.fullmatch(r"p\d+-([a-z0-9-]+)\.md", name)
            if m:
                slugs.add(m.group(1))
    for _, cells in p_rows(lines):
        base = os.path.basename(cells[2]) if len(cells) > 2 else ""
        m = re.fullmatch(r"p\d+-([a-z0-9-]+)\.md", base)
        if m:
            slugs.add(m.group(1))
    return slugs


def next_p_number(lines, root):
    nums = []
    for _, cells in p_rows(lines):
        m = P_ID_RE.fullmatch(cells[0])
        if m:
            nums.append(int(m.group(1)))
    pdir = proposed_dir(root)
    if os.path.isdir(pdir):
        for name in os.listdir(pdir):
            m = re.fullmatch(r"p(\d+)-[a-z0-9-]+\.md", name)
            if m:
                nums.append(int(m.group(1)))
    return (max(nums) + 1) if nums else 1


def stub_text(pid, cand):
    """The 5-line stub shape (the seeded lane's exact shape: a title line
    and four bullets)."""
    return (
        f"# {pid} — {cand['title']}\n"
        "\n"
        f"- **User-visible value**: {cand['value']}\n"
        f"- **First slice**: {cand['slice']}\n"
        f"- **Estimate class**: {cand['estimate']}.\n"
        f"- **Source fact**: {cand['source']}\n"
    )


def proposal_row(pid, cand, fname, today, week):
    notes = (
        f"machine-filed {today.isoformat()} (T326 cycle {week}) — "
        "awaiting operator ratification"
    )
    return [
        pid,
        cand["title"],
        f"specs/proposed/{fname}",
        str(estimate_pri(cand["estimate"])),
        "proposed",
        notes,
    ]


def file_proposal(root, lines, cand, pnum, today, week):
    """One filing: write the stub file, then append the row after the last
    P-row (pre-write asserts, then the write — the T279 order). Returns
    (new_lines, printed_name) or raises with the file untouched."""
    pid = f"P{pnum:03d}"
    fname = f"p{pnum:03d}-{cand['slug']}.md"
    text = stub_text(pid, cand)
    cells = proposal_row(pid, cand, fname, today, week)
    # The hazard check runs on the CELLS, never the joined row — the join
    # itself carries the cell separators (" | "), so a row-level check would
    # fire on every well-formed row (the smoke run's first fire).
    for cell in cells:
        for hazard in ("|", "\n"):
            if hazard in cell:
                raise AssertionError(
                    f"ideate filing: cell {cell!r} for {pid} contains "
                    f"{hazard!r} — the T8 split hazard; nothing written"
                )
    row = "| " + " | ".join(cells) + " |"
    pdir = proposed_dir(root)
    os.makedirs(pdir, exist_ok=True)
    with open(os.path.join(pdir, fname), "w", encoding="utf-8") as f:
        f.write(text)
    # Append the row after the LAST P-row (the lane's rows are contiguous
    # at the section's end; with no P-rows, after the separator row that
    # follows the lane's table header).
    pidx = [i for i, _ in p_rows(lines)]
    if pidx:
        at = pidx[-1] + 1
    else:
        at = None
        for i, line in enumerate(lines):
            if line.strip() == "## Proposed":
                at = i
                break
        if at is None:
            raise AssertionError("ideate filing: the lane vanished mid-cycle")
        at += 1
        while at < len(lines) and not _row_cells(lines[at]):
            at += 1
        at = min(at, len(lines))
    others_before = lines[:at] + lines[at + 1:]
    # INSERT semantics — the row lands BETWEEN lines, nothing is replaced
    # (a splice-replace at `at` would destroy the line there: the smoke run
    # lost the trailing newline element exactly this way). Every original
    # line must survive byte-identical, in order, as the new list's
    # subsequence (the T279 carry discipline, position-free).
    new_lines = lines[:at] + [row] + lines[at:]
    assert_byte_carry(lines, new_lines)
    assert_byte_carry(others_before, new_lines[:at] + new_lines[at + 1:])
    _assert_row_shape(row, pid, "filing")
    with open(os.path.join(root, "TODO.md"), "w", encoding="utf-8") as f:
        f.write("\n".join(new_lines))
    return new_lines, fname


# --- the amendment: auto-ratification -----------------------------------------

def ratify(root, today):
    """The 2026-11-06 amendment sweep: any `proposed` P-row older than 7
    days flips to `open` — unless vetoed (status vetoed / row deleted) or
    carrying the never-dispatch note. A row whose notes carry no parseable
    filed date is NEVER flipped (the loop never flips what it cannot prove
    the age of). Every flip is printed as the cycle disposition line."""
    path, lines = read_todo_lines(root)
    flipped = []
    for idx, cells in p_rows(lines):
        if len(cells) != EXPECTED_CELLS or cells[4] != "proposed":
            continue
        notes = cells[5]
        low = notes.lower()
        if any(marker in low for marker in NEVER_FLIP_MARKERS):
            continue
        m = DATE_RE.search(notes)
        if not m:
            continue
        try:
            filed = datetime.date.fromisoformat(m.group(0))
        except ValueError:
            continue
        if (today - filed).days < RATIFY_AFTER_DAYS:
            continue
        new_cells = list(cells)
        new_cells[4] = "open"
        new_notes = notes.rstrip()
        if f"auto-ratified {today.isoformat()}" not in new_notes:
            new_notes = f"{new_notes} — auto-ratified {today.isoformat()} (T326)"
        new_cells[5] = new_notes
        rebuilt = "| " + " | ".join(new_cells) + " |"
        _assert_row_shape(rebuilt, cells[0], "ratify")
        others_before = lines[:idx] + lines[idx + 1:]
        new_lines = _splice(lines, idx, rebuilt)
        assert_byte_carry(others_before, new_lines)
        lines = new_lines
        flipped.append(cells[0])
    if flipped:
        with open(path, "w", encoding="utf-8") as f:
            f.write("\n".join(lines))
        print(f"ideation auto-ratified (T326): {', '.join(flipped)}")
    return flipped


# --- --check ------------------------------------------------------------------

def check(root):
    """The lane consistency probe (the ideation spec's check: line): every
    P-row has 6 cells, a legal status, an existing spec file; every
    auto-ratified row reads `open`. Exit 0 clean, 1 with the probed
    actuals."""
    _, lines = read_todo_lines(root)
    require_lane(lines)
    errors = []
    for _, cells in p_rows(lines):
        pid = cells[0] if cells else "?"
        if len(cells) != EXPECTED_CELLS:
            errors.append(f"{pid}: parses into {len(cells)} cells (need exactly {EXPECTED_CELLS})")
            continue
        if cells[4] not in LEGAL_STATUSES:
            errors.append(f"{pid}: status {cells[4]!r} is not one of the T8 legal statuses")
        spec = os.path.join(root, cells[2])
        if not os.path.isfile(spec):
            errors.append(f"{pid}: spec cell {cells[2]!r} names no existing file")
        if "auto-ratified" in cells[5] and cells[4] != "open":
            errors.append(f"{pid}: carries an auto-ratified note but status is {cells[4]!r}")
        if cells[4] == "proposed" and any(
            marker in cells[5].lower() for marker in NEVER_FLIP_MARKERS
        ):
            pass  # internal artifacts are LEGAL in the lane — just never flipped
    if errors:
        for e in errors:
            print(f"ideation check (T326): FAIL {e}")
        return 1
    print("ideation check (T326): lane consistent")
    return 0


# --- gate-only ----------------------------------------------------------------

def gate_only(stub_path):
    """Noul-gate ONE stub file (the test surface for the meta-reject /
    outward-pass pair). Exit 0 = outward (file it), 2 = rejected (meta or
    fail-closed), 1 = the stub file itself is unreadable."""
    try:
        with open(stub_path, "r", encoding="utf-8") as f:
            text = f.read()
    except OSError as e:
        print(f"ideate --gate-only: cannot read {stub_path}: {e}")
        return 1
    first = text.split("\n", 1)[0].strip()
    m = re.match(r"#\s*P\d+\s*—\s*(.+)$", first)
    title = m.group(1).strip() if m else first.lstrip("# ").strip()
    if noul_gate(title, text):
        print(f"ideation gate (T326): PASS {title}")
        return 0
    print(f"ideation reject (T326): meta-class {title}")
    return 2


# --- the cycle ----------------------------------------------------------------

def propose(root, today):
    packet = {
        "parked": parked_features(root),
        "blocked": blocked_threads(root),
        "census": triage_census(root),
        "release_lag": release_lag(root),
        "chugwatch": chugwatch_version(),
    }
    print(packet_line(packet))
    path, lines = read_todo_lines(root)
    require_lane(lines)
    taken = filed_slugs(lines, root)
    pnum = next_p_number(lines, root)
    week = "-W".join(
        [str(today.isocalendar()[0]), f"{today.isocalendar()[1]:02d}"]
    )
    filed = 0
    for cand in build_candidates(packet):
        if filed >= MAX_PROPOSALS:
            break
        if cand["slug"] in taken:
            continue  # idempotence: the same week's packet yields the same slugs
        pid = f"P{pnum:03d}"
        if not noul_gate(cand["title"], stub_text(pid, cand)):
            print(f"ideation reject (T326): meta-class {cand['title']}")
            continue
        lines, fname = file_proposal(root, lines, cand, pnum, today, week)
        print(f"ideation filed (T326): {fname}")
        taken.add(cand["slug"])
        pnum += 1
        filed += 1
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="T326 ideation cycle (deterministic, no network)")
    ap.add_argument("--ratify-only", action="store_true", help="only the auto-ratification sweep")
    ap.add_argument("--gate-only", metavar="STUB_FILE", help="noul-gate one stub file (exit 2 = reject)")
    ap.add_argument("--check", action="store_true", help="verify lane consistency")
    ap.add_argument("root", nargs="?", default=".", help="repo root (default: cwd)")
    args = ap.parse_args(argv)
    if args.check:
        return check(args.root)
    if args.gate_only:
        return gate_only(args.gate_only)
    today = datetime.datetime.now(datetime.timezone.utc).date()
    ratify(args.root, today)
    if args.ratify_only:
        return 0
    return propose(args.root, today)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except AssertionError as e:
        print(f"ideate: {e}", file=sys.stderr)
        sys.exit(1)

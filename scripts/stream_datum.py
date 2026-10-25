#!/usr/bin/env python3
"""stream_datum.py — T305: the wrap corridor's mechanical at-stamp
stream-datum generator.

The wrap corridor's stream-datum figures — the eval body's own-stream datum
at eval-write, the wrap-notes' T257 color, the state write's datum, and the
fill's FINAL datum — were hand-composed per wrap, and the composed figures
kept being carried from probes run at EARLIER stamps. The cycle-1986 eval
(trip 460) found THREE committed corridor texts falsified at their own named
stamps within one window:

  the cycle-1910 wrap notes — `32 iters` vs TRUE 55 `type=="iteration"`
  events <= its own named stamp (2026-10-24T14:43:16.922Z) — the eval-write
  figure carried forward into the notes;

  the cycle-1910 wrap fill — `55 iters` vs TRUE 78 <= its own named stamp
  (2026-10-24T14:49:34.186Z) — the notes-stamp figure carried forward into
  the fill (the TRUE reconciliation: 78 + 3 post-fill legs == whole-final 81);

  the cycle-1982 wrap fill (53af72d) — `ok:false 4 == 4` vs TRUE 5 AND
  `trim 0 == 0` vs TRUE 1 at its own named stamp (04:42:12.713Z) — the
  fill's own datum-probe leg (04:41:56.033Z) was the 5th ok:false leg and
  the stream's trim event (04:41:19.331Z) predated the named stamp.

Every fire was caught one-or-more chains later by the eval-digest backstop
(zero casualty — the T272 designed net), but the T272 false-at-write census
moved 54 -> 58 in one window and each fire costs correction prose at the
next eval/wrap. The T272 doctrine (enumeration by flag, never recall)
already mandates the mechanical read at write time; the fires persisted
because that read was a multi-jq compose re-derived per datum (four datums
per corridor) under wrap time pressure. The T273/T295 precedent
(scripts/wrap_assert.py) shows the loop adopts a helper when the mechanical
read is ONE command. This module is that command — a PROBE (a datum
generator, not an assertion; the assert-shaped sibling stays
wrap_assert.py), stdlib-only python3 (no jq dependency), reading the same
events.jsonl schema eval-digest.sh reads with jq.

Usage:
  python3 scripts/stream_datum.py <events-file> [--at <ISO-8601 ts>]

Emits the corridor datum as `key: value` lines, one per field:

  iters: N                      count of type=="iteration" events, ts <= stamp
  first-anchor: <ts>            the first such anchor (stream order)
  last-anchor: <ts>             the last such anchor (== the stamp by default)
  wall-secs: N.NNN              the span between the two anchors (none if
                                either anchor's ts is unparsable)
  ok-false: N                   count of type=="tool_result" with .ok == false
  ok-false-legs: <ts> [, ...]   the legs' ts values, stream order
  ctx-edit: accepted A / rejected R
                                .accepted bool — true counts A, everything
                                else counts R (A + R == the scoped fires)
  trim: N                       count of type=="trim" events
  trim-legs: <ts> [, ...]       the trims' ts values, stream order
  goal: accepted A / rejected R | none
                                reads .outcome ("accepted"/"rejected") — the
                                verdict field is .outcome, NOT .verdict; a
                                goal event with neither outcome counts on
                                neither side
  goal-ts: <ts> | none          the last goal event's ts (any goal event)
  bytes: N                      the file's own size (never stamp-scoped)
  mtime: <ISO ts>               the file's own mtime, UTC Z form (never
                                stamp-scoped)

The stamp defaults to the stream's last type=="iteration" anchor (a stream
with no iteration anchor falls back to its last parsable event ts, so a
tool_result-only stream still reads scoped rather than empty); --at
overrides it (ISO-8601 Z form, e.g. 2026-10-24T14:43:16.922Z). Scoping is
STRING-COMPARE per the corpus convention: an event is in scope iff its ts
<= the stamp, so every count above excludes events with ts > the stamp —
the 1982-fill shape is the pin: a trim BEFORE the stamp is counted, one
AFTER is excluded; an ok:false leg after the stamp is excluded from both
the count and the leg list. Malformed lines drop silently (the digest's
fromjson? convention); an event with no ts field scopes as "" <= any stamp
(included — an untimestamped event cannot be proven post-stamp).

Exit codes: 0 emits the datum; 2 on a usage error (malformed --at,
missing or unreadable events file).
"""

import argparse
import datetime
import json
import os
import re
import sys

# ISO-8601 Z form: the corpus convention's stamp shape (fractional seconds
# optional). Also the --at validator and the wall-arithmetic parser's input.
TS_RE = re.compile(
    r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d+))?Z$"
)


def ts_epoch(ts):
    """ISO-8601 Z-form timestamp -> epoch seconds (float), or None.

    Hand-rolled (not datetime.fromisoformat) so any fractional precision
    parses on any python3 — pre-3.11 fromisoformat rejects the `Z` suffix
    and demands exactly 3 or 6 fractional digits.
    """
    m = TS_RE.match(ts)
    if m is None:
        return None
    micro = int(((m.group(7) or "") + "000000")[:6])
    try:
        dt = datetime.datetime(
            int(m.group(1)), int(m.group(2)), int(m.group(3)),
            int(m.group(4)), int(m.group(5)), int(m.group(6)),
            micro, tzinfo=datetime.timezone.utc,
        )
    except ValueError:
        return None
    return dt.timestamp()


def load_events(path):
    """Parse the stream; malformed lines drop silently (fromjson? parity).

    A line that is valid JSON but not an object (a bare array or scalar)
    drops too — it carries no event shape to scope.
    """
    events = []
    with open(path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except ValueError:
                continue
            if isinstance(ev, dict):
                events.append(ev)
    return events


def in_scope(ev, stamp):
    """String-compare scoping: the event's ts <= the stamp (the corpus
    convention). None stamp = unscoped (an empty stream's trivial case). An
    event with no ts field scopes as "" <= any stamp — included."""
    if stamp is None:
        return True
    ts = ev.get("ts")
    if not isinstance(ts, str):
        ts = ""
    return ts <= stamp


def of_type(events, stamp, kind):
    return [ev for ev in events if ev.get("type") == kind and in_scope(ev, stamp)]


def resolve_stamp(events, at):
    """--at wins; else the stream's LAST iteration anchor (stream order);
    else the stream's last parsable event ts (the no-iteration fallback)."""
    if at is not None:
        return at
    for ev in reversed(events):
        if ev.get("type") == "iteration" and isinstance(ev.get("ts"), str):
            return ev["ts"]
    for ev in reversed(events):
        if isinstance(ev.get("ts"), str):
            return ev["ts"]
    return None


def legs_field(evs):
    """The legs' ts values joined ", " in stream order; `none` when empty."""
    ts_list = [ev["ts"] for ev in evs if isinstance(ev.get("ts"), str)]
    return ", ".join(ts_list) if ts_list else "none"


def datum(events, stamp):
    """The corridor datum fields, in the pinned output order (bytes/mtime
    are the caller's append — they are the file's own facts, not the
    stream's)."""
    it = [
        ev for ev in of_type(events, stamp, "iteration")
        if isinstance(ev.get("ts"), str)
    ]
    okf = [
        ev for ev in of_type(events, stamp, "tool_result")
        if ev.get("ok") is False
    ]
    ce = of_type(events, stamp, "ctx_edit")
    ce_acc = sum(1 for ev in ce if ev.get("accepted") is True)
    tm = of_type(events, stamp, "trim")
    gl = of_type(events, stamp, "goal")
    gl_acc = sum(1 for ev in gl if ev.get("outcome") == "accepted")
    gl_rej = sum(1 for ev in gl if ev.get("outcome") == "rejected")
    first = it[0]["ts"] if it else "none"
    last = it[-1]["ts"] if it else "none"
    wall = "none"
    if it:
        e0, e1 = ts_epoch(first), ts_epoch(last)
        if e0 is not None and e1 is not None:
            wall = f"{e1 - e0:.3f}"
    if gl:
        goal = f"goal: accepted {gl_acc} / rejected {gl_rej}"
        last_ts = gl[-1].get("ts")
        goal_ts = last_ts if isinstance(last_ts, str) else "none"
    else:
        goal = "goal: none"
        goal_ts = "none"
    return [
        f"iters: {len(it)}",
        f"first-anchor: {first}",
        f"last-anchor: {last}",
        f"wall-secs: {wall}",
        f"ok-false: {len(okf)}",
        f"ok-false-legs: {legs_field(okf)}",
        f"ctx-edit: accepted {ce_acc} / rejected {len(ce) - ce_acc}",
        f"trim: {len(tm)}",
        f"trim-legs: {legs_field(tm)}",
        goal,
        f"goal-ts: {goal_ts}",
    ]


def main(argv=None):
    p = argparse.ArgumentParser(
        prog="stream_datum.py",
        description=(
            "T305: emit the wrap corridor's stream datum as `key: value` "
            "lines, scoped at the named stamp (default: the stream's last "
            "type==\"iteration\" anchor)."
        ),
    )
    p.add_argument("events", help="the .chug/events*.jsonl stream to read")
    p.add_argument(
        "--at", metavar="TS", default=None,
        help="the named stamp, ISO-8601 Z form (e.g. 2026-10-24T14:43:16.922Z)",
    )
    args = p.parse_args(argv)
    if args.at is not None and TS_RE.match(args.at) is None:
        p.error(
            f"--at must be ISO-8601 Z form (e.g. 2026-10-24T14:43:16.922Z); "
            f"probed {args.at!r}"
        )
    try:
        events = load_events(args.events)
        size = os.path.getsize(args.events)
        mtime = os.stat(args.events).st_mtime
    except OSError as e:
        print(f"stream_datum: unreadable events file: {e}", file=sys.stderr)
        return 2
    stamp = resolve_stamp(events, args.at)
    mtime_iso = datetime.datetime.fromtimestamp(
        mtime, datetime.timezone.utc
    ).strftime("%Y-%m-%dT%H:%M:%SZ")
    lines = datum(events, stamp)
    lines.append(f"bytes: {size}")
    lines.append(f"mtime: {mtime_iso}")
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())

# T326 — Ideation mode: empty backlog triggers outward-facing feature proposals, operator ratifies

estimate: ~140 changed lines (loopd ideation leg + scripts/ideate.py + Laya noul gate + TODO.md Proposed section + fixtures)

## Why

Operator expectation, stated 2026-11-06: "I thought it would come up with new
features if left with an empty backlog." It did ideate — but its only idea
source was its own tooling, so ideation collapsed inward (the T317–T324 meta
class). The fix is not less ideation, it is a DIFFERENT idea source and a
ratification boundary: proposals land in a `## Proposed` lane, the operator
flips them to `open`, the loop never self-flips. This is the L1/L2 rung of
the self-improvement ratification ladder: the machine proposes, the human
disposes.

Depends on T325 (dormant mode) — ideation must launch into a quiet loop, not
compete with manufactured meta-work.

## What

1. **Ideation cycle** (in `loopd.sh`): while dormant, when
   `now - last_ideation_ts >= 7d` (state in `.chug/loopd/dormant-state.json`,
   same file as T325), launch ONE glm-5-3-flash call (not kimi — this is a
   ~2k-token structured task) running `scripts/ideate.py`.
2. **Ideas packet** (`scripts/ideate.py` builds, deterministic, no network):
   - FEATURES.md statuses (parked items: F16),
   - deferred/blocked threads named in LEDGER + docs (sandbox-deploy go-live, SCR
     spike),
   - eval-triage class census from `decisions.jsonl` (top recurring friction
     classes = dogfood pain),
   - site/mod staleness facts (chug.sh release lag, chug-watch version).
3. **Proposal shape**: ≤3 proposals per cycle, each a 5-line stub
   (title / user-visible value / first slice / estimate class / source fact).
   Written to `specs/proposed/t<next>-<slug>.md` and appended as rows in a
   `## Proposed` section of TODO.md with status `proposed`. The loop MUST NOT
   implement or flip `proposed` rows; only the operator flips to `open`.
4. **Meta-class gate**: before filing, each proposal passes the Laya noul
   gate (daemon, `~/.chug/daemon.sock`): "is this proposal primarily about
   the loop's own tooling/assertions/bookkeeping?" YES → rejected, logged as
   `ideation reject (T326): meta-class <title>`. Daemon unavailable → fail
   CLOSED to rejection for this cycle (better silent than meta).
5. **Seed the lane at ship time**: write the `## Proposed` section with three
   hand-filed stubs (sandbox-deploy go-live; F16 unpark decision; SCR spike
   pending spark GPU) so the lane exists before the first machine cycle.

## Tests

- `ideate.py` fixture run over a canned decisions.jsonl produces ≤3 stubs,
  correct file + row shapes, idempotent on re-run (same week = no
  duplicates; dedupe key = slug).
- Gate fixtures: a meta-class stub ("add wrap_assert leg") is rejected; an
  outward stub ("chug.sh cost dashboard") passes (mocked noul).
- loopd fixture: dormant + timer due → ideation leg launches on glm; not
  dormant → no ideation.
- Proposed-lane safety: an eval cycle over a TODO.md whose only rows are
  `proposed` files nothing and treats open count as 0 (dormancy holds).

## Out of scope

- Auto-implementing proposals (ratification stays with the operator).
- Web/ecosystem scanning as an idea source (no network in the loop; the
  operator seeds ecosystem items by hand into `## Proposed`).
- The first three seed proposals' implementations (operator flips decide).

## Amendment (2026-11-06, operator blanket delegation)

The operator approved all current proposals and delegated ratification
going forward: **a proposal with no operator response is approved.** The
lane therefore auto-ratifies: at each ideation cycle, any `proposed` row
older than 7 days flips to `open` automatically unless the operator has
vetoed it (veto = delete the row or set status `vetoed`). Every auto-flip
is logged in the cycle disposition (`ideation auto-ratified (T326): P###,
P###`) so the action stays visible in the git log. The meta-class noul
gate still runs BEFORE filing — auto-ratification never bypasses the
outward-facing requirement. Internal-artifact proposals (company content
that must not enter the public repo) are filed with an `operator-owned`
note and are NEVER auto-flipped or dispatched; they wait for the operator
or their Claude session to execute them out-of-band.

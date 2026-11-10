# T328 — chug-watch daemon-socket provider (finish T219's unfinished half)

estimate: ~130 changed lines (chug-watch hooks/register.js provider + hooks/parse.js render + parse.test.ts battery)

## Why

T219 landed the daemon's `/sessions` endpoint; the chug-watch mod still
probes over SSH to K7 for everything. The daemon (chugd, `~/.chug/daemon.sock`,
0600, HTTP-over-unix, live since 2026-10-13) is the designed provider: local
where it runs, sub-millisecond, no SSH round-trip per probe. The mod's 1s
liveness tick + 60s probe currently pays SSH latency on every refresh and
shows nothing about the daemon itself (model loaded? Laya verdicts served?
triage route decisions?). Finish the half T219 deferred.

## What

1. **Provider chain** (hooks/register.js): probe order — (a) local
   `~/.chug/daemon.sock` if present (HTTP GET `/status` and `/sessions` over
   the unix socket, ~5 lines with `net` + raw HTTP, no new deps), (b) fall
   back to the current SSH probe unchanged. A box without a daemon behaves
   exactly as today.
2. **Render** (hooks/parse.js): the cockpit gains a daemon cell in the band —
   model name, uptime, verdicts-served counter from `/status`; the pane's
   sessions section reads `/sessions` when local. Degraded states render
   explicitly: `daemon: off` (no socket), `daemon: stale` (socket present,
   no response in 2s → fall back to SSH and mark the cell dim).
3. **No new failure modes**: socket errors must never throw past the
   provider boundary — the probe wraps in try/catch and degrades to SSH
   (today's behavior is the floor).

## Tests

- parse.test.ts battery additions: daemon-cell render for ok/off/stale
  states; provider selection order (local socket wins, SSH fallback on
  error); a malformed `/status` body degrades to SSH, never throws.
- A recorded real `/status` payload as the fixture (capture from K7's live
  daemon at build time; do not hand-invent the shape).

## Out of scope

- Extending the daemon's API (consume `/status` + `/sessions` as they are;
  new endpoints are a daemon item, not this one).
- Cross-box daemon federation (K7's daemon stays host-scoped; the mod on
  F94 keeps using SSH for K7 state — that is correct, not a gap).
- The mod's SSH probe removal (SSH remains the fallback forever).

## Check (composed at dispatch — the decision-B sanctioned landing gate; landed with the item)

The chug-watch npm test battery stays RED by design on this host (no claude-code/testing module anywhere resolvable — the T344 probe chain; the shim fails closed loudly, which IS the sanctioned state). The landing gate verifies the sanctioned shape instead — both must exit 0:

check: node scripts/t328-failclosed-check.mjs && node scripts/t328-provider-smoke.mjs

Legs: (1) shim without CLAUDE_CODE_TESTING_PATH exits nonzero with a diagnostic naming the probe chain; (2) with the env var set it re-exports the pointed module verbatim (identity, not a copy); (3) the npm test script routes the TS battery through the shim — and fails with the diagnostic while the real module is absent. Provider smoke: temp unix socket serving the recorded /healthz + /sessions + /status/loop fixtures — local wins, SSH fallback on absent/unresponsive (2s stale budget), malformed body degrades without throwing, parse render covers ok/off/stale.

LANDED 2026-11-10: chug-watch sibling merge 79031bc (impl c09ade8 + review nit 09be3cf), kimi PASS d1794334192-7.

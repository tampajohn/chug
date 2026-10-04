# T219 — daemon /sessions registry: chug runs register TTL heartbeats on the 0600 socket

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test daemon_sessions_registry

estimate: ~450 lines (registry store + routes + TTL eviction + self-registration + emitter helper + pins)

## Concern

Operator 2026-10-03: "have sessions registered in the daemon." Today
there is no host-local answer to "what chug runs are alive on this box":
the Claude Code chug-watch mod (~/workspace/chug-watch) SSH-probes K7 for
loopd status, cycle logs, and daemon health — four-plus SSH round trips
for what one local socket query could answer. The T204 judge daemon
already owns the right transport: a host-scoped 0600 unix socket with an
HTTP surface (`/judge`, `/healthz`). It should grow a `/sessions`
registry: every chug process (loopd cycles, delegate children, dashd, the
daemon itself) POSTs a heartbeat/registration, and `GET /sessions`
returns the live registry. Mods and dashboards then read one 0600 socket
instead of SSHing.

## Repo context

- src/daemon.rs (T204): HTTP/1.1 over `$CHUG_HOME/daemon.sock` (default
  `~/.chug/daemon.sock`, `CHUG_DAEMON_SOCK` overrides), mode 0600,
  host-scoped by design (NOT the per-repo `.chug`); single-instance
  flock on `$CHUG_HOME/daemon.lock`; `curl --unix-socket` is the
  debugging tool; every failure mode fails OPEN (the judge degrades per
  command, never blocks a run).
- The daemon's request surface today: `POST /judge` (inference) and
  `GET /healthz` (liveness). The feature-off StubBackend (T204/T215)
  refuses `/judge` outright — a stub must never fabricate a verdict, and
  must never fabricate liveness either.
- T215: loopd resolves the daemon-capable binary once per run and spawns
  the ensure; the daemon is HOST-SCOPED — one per box, serving any run.
- Emitters exist today with stable identities: loopd cycles
  (.chug/loopd/cycle-<ts>.log), delegate children (src/delegate.rs —
  pid + goal_sha + events path at launch), dashd, and the daemon itself.

## Requirements

1. `POST /sessions` registers or refreshes one run: JSON body
   `{id, role, started, last_event_ts, status}`; `role` one of
   `loopd-cycle`, `delegate-child`, `dashd`, `daemon`. Upsert by `id`
   (a heartbeat is the same shape as a first registration). The daemon
   self-registers at startup and heartbeats on each registry serve.
2. TTL expiry: an entry with no heartbeat within the TTL (default
   10 minutes, one named const) is evicted — lazily on read is fine.
   `GET /sessions` returns only live entries:
   `{sessions: [{id, role, started, last_event_ts, status, age_sec}], now}`.
3. Additive: `/judge` and `/healthz` behavior byte-unchanged; unknown
   paths keep today's refusal shape. No change to the judge wire shapes.
4. Fail-open everywhere: a registry store error never fails `/judge`;
   emitters treat a failed POST as a no-op (best-effort, never blocks a
   run, never trips `set -e` in loopd.sh — the `|| true` house style).
5. Host-scoped like the rest of the daemon: same socket, same 0600, same
   `$CHUG_HOME`/`CHUG_DAEMON_SOCK` resolution. No TCP listener.
6. The feature-off StubBackend refuses `/sessions` exactly as it refuses
   `/judge` — a stub must never fabricate a registry.
7. Emitters (minimal wiring, one shared best-effort helper): the daemon
   self-registers; loopd.sh heartbeats at cycle start (one curl line);
   delegate children register at launch (the launch site already knows
   pid/goal/events path); dashd registers at startup. Heartbeat cadence
   per emitter: on start and at most once per minute while active.

## Tests

- New tests/daemon_sessions_registry.rs pins: (a) register → GET
  roundtrip returns the entry with the posted fields; (b) re-POST upserts
  (one entry per id, refreshed last_event_ts); (c) an entry older than
  the TTL is evicted from GET; (d) all four roles round-trip; (e) the
  feature-off stub refuses /sessions; (f) /judge and /healthz are
  untouched (existing daemon_feature_off/daemon_lifecycle pins stay
  green).

## Out of scope

- Cross-box aggregation via remote-bridge (the actual fleet): a separate
  future item — this endpoint is host-scoped like the rest of the daemon.
- The chug-watch mod's provider switch to the daemon-socket transport
  (mod-side change, not this repo).
- Rich dashboard rendering, historical session archives, auth beyond the
  0600 host scoping, any TCP listener.

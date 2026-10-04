# T221 — close the T219 validator's findings: parse_rfc3339 day-0 fail-open + age_sec value pin + TTL boundary pin

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test daemon_sessions_registry

estimate: ~80 lines (one guard clause + one wire pin + one boundary pin + one flake fix)

## Concern

The T219 kimi validation (d1791098231-4, VERDICT PASS) left three
actionable findings the merge deliberately filed forward:

1. **MINOR correctness bug (verified black-box, both profiles):**
   `parse_rfc3339`'s day check `day > days_in_month(...)` misses
   `day == 0`. For March, `days_from_civil` computes
   `(153*0+2)/5 + 0 - 1` → u64 underflow: debug build the conn thread
   PANICS ("attempt to subtract with overflow", src/daemon.rs ~960 —
   the POST gets an empty reply, curl rc 52; the daemon survives via
   thread isolation); release build it wraps to the previous day
   (2024-03-00 → 2024-02-29) so the POST returns ok but the entry is
   instantly TTL-evicted — never visible in GET /sessions. Both violate
   the fail-open-to-server-now contract for this invalid-input class.
   Off every spec-pinned path (the shipped emitters never emit day-00),
   but the fix is one clause: reject `day < 1` (month already bounded).
2. **WEAK TEST:** `age_sec` is pinned only as `is_number` — mutant m5
   (age since last heartbeat instead of since `started`, the spec req-2
   semantic INVERTED) survived all 23 tests. A value pin closes it.
3. **WEAK TEST:** the 10-minute TTL default is unpinned in a wide band —
   mutant m6 (SESSION_TTL_SECS 600→300) passed everything (stale leg
   700s and fresh leg 5s classify identically for any TTL in (5, 700)).
   A boundary pin closes it.

Finding 4 (the `now`-straddle flake in
register_then_get_roundtrips_posted_fields — a second-boundary straddle
between the server's stamp and the client's `rfc3339(now_secs())`
fails the pin in a ~ms window) rides along: relax to a ±2s band.

## Requirements

1. `parse_rfc3339` rejects `day < 1` exactly like it rejects day 32:
   a day-0 timestamp fails the parse → the registration fails open to
   server-now (fresh entry, `started_epoch: None`, age 0) — the
   fail-open contract restored. One clause; no other grammar change.
2. Wire pin (tests/daemon_sessions_registry.rs): a day-0 `started`
   POST still returns `{"ok":true}` AND the entry is VISIBLE in
   `GET /sessions` (not instantly evicted) — the release-profile
   symptom pinned end to end.
3. Value pin for `age_sec`: register with `started` = server-now minus
   ~120s (compute from a POSTed RFC3339 string), GET returns
   `age_sec >= 120` — inverting the derivation (m5) fails the pin.
4. TTL boundary pin (daemon::tests unit table): a 599s-old entry LIVES
   and a 601s-old entry is EVICTED — flipping the const to 300 (m6)
   fails the 599-live leg; the boundary pins 600 exactly.
5. The roundtrip pin's `now` comparison becomes a ±2s band (finding 4
   flake fix); everything else byte-unchanged.
6. Additive only: no wire shape change, no behavior change beyond the
   day-0 guard; /judge, /healthz, and the existing T219 pins stay
   byte-green.

## Tests

- daemon::tests unit: day-0 (and day-32 control) parse legs; the
  599/601 TTL boundary table.
- tests/daemon_sessions_registry.rs wire: day-0 POST → ok + visible in
  GET; age_sec >= 120 value leg; the band-relaxed roundtrip stays
  green across a straddled boundary.

## Out of scope

- Findings 5-6 (dashd wiring — external supervisor, role reserved and
  documented; self-heartbeat on POST) — adjudicated no-action.
- Any RFC3339 grammar widening; any emitter change.

# T365 — capabilities_contract daemon-probe tests read the host's real socket: the loop's own daemon reds every gate run

estimate: ~25 changed lines (in-test CHUG_HOME isolation for the probe tests)

## Why

Observed live at the cycle-3208 post-merge gates (2026-11-11): the
capabilities_contract tests that assert "no daemon here" (`a_wired_hook_
flips_permission_broker_without_any_socket`, `a_wired_status_hook_flips_
status_mod_without_any_socket`) read the HOST's real daemon socket
(`/Users/jadams/.chug/daemon.sock`, alive since the loop's own supervisor
daemon started). Any gate run on a host with a live daemon goes red on a
green tree — the loop reds its own gates every dormant cycle. The T363
validator (kimi) hit the same 2 failures, proved them environmental
(clean `CHUG_HOME` → both pass), and the cycle-3208 post-merge needed a
partition proof (full suite green under `CHUG_HOME=/tmp/...`, install_sh
green without it) to land.

## What

The daemon-probe tests isolate their socket namespace themselves: set
`CHUG_HOME` to a per-test temp dir before the probe (the same isolation
the tests already assume conceptually — "no daemon here" must mean "no
daemon in THIS namespace", not "the operator's box happens to be quiet").
No production-code change: the fixtures already run the binary as a child,
so the env rides the existing `.env()` call.

## Tests

- The two probe tests pass on a host WITH a live daemon socket (the
  2026-11-11 host state is the acceptance case).
- No new fixture surface: the isolation is two `.env("CHUG_HOME", …)`
  calls in the existing tests.

## Out of scope

- install_sh's fixture-HOME behavior (it sets its own HOME correctly; the
  cycle-3208 failure there was the orchestrator's own CHUG_HOME override,
  not a repo gap).
- Any change to the daemon itself.

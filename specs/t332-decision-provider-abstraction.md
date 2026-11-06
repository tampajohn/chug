# T332 — decision-model provider abstraction: proxy Jev alongside local Laya (pilot boxes need no daemon)

estimate: ~180 changed lines (src decision provider module + chugd/loopd call-site rewires + profile keys + fixtures)

## Why

Operator, 2026-11-06: "the databricks proxy now exposes a model called jev
for us, so we could use that instead of laya locally." Three call sites
consume a decision model today or at ship time: the T259 loopd triage
cascade, the T326 ideation meta-class noul gate, and the F13/F15 risk-gate
lineage. All assume the local chugd (candle ModernBERT, unix socket). For
company adoption that assumption is the wall: every pilot box would need a
Rust daemon + a 421M model before the loop gets its cheapest superpowers.
A hosted Jev behind the company proxy makes the decision layer
zero-install — and gives non-company users the same shape through any
compatible endpoint. Local Laya stays the free/offline provider; this item
adds the second provider and the routing, it does NOT retire the daemon.

## What

1. **Discovery first** (do this before any rewiring, commit the findings
   as `docs/decision-providers.md`): probe the proxy for the jev serving
   shape — the tools-proxy `/v1/systemone` surface is the expected home
   (verify request/response contract for noul/choice/score, auth header,
   model name). If the shape differs, the doc records the actual contract
   and the provider implements THAT, not the guess.
2. **Provider abstraction** (`src/decision/`): `DecisionProvider` with two
   impls — `LocalDaemon` (existing socket client, extracted unchanged) and
   `ProxySystemOne` (HTTP, noul/choice/score, 10s timeout, fail-closed per
   call site). Selection: `CHUG_DECISION_PROVIDER=auto|local|proxy`
   (profile key, rides the T330 profile contract); `auto` = local socket
   when present and healthy, else proxy when configured, else each call
   site's existing no-provider behavior (T259: route kimi; T326: reject
   meta-class filings).
3. **Call-site rewires** (mechanical, behavior-preserving): T259 triage,
   T326 gate, daemon-status surfaces. Each keeps its current degraded
   semantics — the provider change must be invisible in every fixture that
   does not configure a proxy.
4. **Cost legibility**: proxy-Jev calls log to Langfuse with the
   `decision` tag (tiny $ per call, but the T327 dashboard should show
   them; no free invisible spend).

## Tests

- Provider fixtures: recorded noul/choice/score round-trips (from the
   discovery probe) drive `ProxySystemOne`; socket fixtures drive
   `LocalDaemon`; auto-selection matrix (socket present/absent x proxy
   configured/absent).
- T259/T326 call-site fixtures rerun green under each provider, including
   provider-down degrade paths.
- The `CHUG_DECISION_PROVIDER` key installs via T330 profile fixtures.

## Out of scope

- Retiring or replacing the local Laya model/daemon (offline + $0 stays).
- Kev (premise denied in the T222/T223 bake-off) and other system-one
  contenders (the abstraction admits them later; this item wires two).
- Pilot-kit distribution of proxy credentials (P005's lane).

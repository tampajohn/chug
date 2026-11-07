# Decision providers — the verified `/v1/systemone` contract

T332's discovery record (probed 2026-11-07, committed with the arc). The
company LLM proxy exposes a hosted System One decision endpoint whose
request/response shape is byte-compatible with the local laya daemon's
`/judge` surface. That makes the proxy a drop-in second provider for every
decision-model call site: same body, same answer envelope, plus per-call
cost in `usage`.

## The wire contract (verified live)

**Request** — `POST <base>/v1/systemone`, `Content-Type: application/json`,
`Authorization: Bearer <token>` (or `x-api-key: <token>`; missing auth →
HTTP 401). The body is the `/judge` body as-is — `state` and `questions` at
the top level — plus a REQUIRED `model` field naming the hosted decision
model (a short name like `jev`; the canonical id also works). `model` is
not optional: an omitted/empty model is rejected with HTTP 400
`model "" is not an allowed decision endpoint or systemone model`.

**Question types** (all three verified):

- `choice` — criteria is an OBJECT of `{key: description}`. Answer:
  `{type:"choice", choice:"<key>", probabilities:{<key>:p,...}, confidence:N}`.
- `noul` — criteria object as for choice. Answer:
  `{type:"noul", noul:<p>}` — the probability the model assigns to the
  noul (uncertain) verdict; no choice/confidence fields.
- `score` — criteria must be an ARRAY (exactly two entries: `[low, high]`,
  strings or `{label, description}` objects). Answer:
  `{type:"score", score:<0..1>, legend:{0:...,1:...}, probabilities:{...}, confidence:N}`.
  A `score` question with no `criteria` is rejected (HTTP 400, zod
  `expected array`).

**Response envelope** (all types): `{"model":"typesafe/…","answers":{<question>:…},"usage":{"input_tokens":N,"output_tokens":N,"cost":<dollars>,"is_byok":false,"cost_details":{…}},"id":"gen-dec-…","provider":"TypeSafe"}`.
The `answers` sub-shape is what call sites parse — identical to the
daemon's; `usage.cost` is the Langfuse `decision`-tag legibility figure.

Malformed bodies fail with HTTP 400 + a `{"error":{"message":…}}` envelope
listing the offending JSON path — worth reading in a probe, never worth
retrying.

## Config

- `CHUG_DECISION_PROVIDER=auto|local|proxy` — the selection key. `auto`
  (default): local daemon socket when present and healthy, else proxy when
  configured, else the call site's existing no-provider behavior. `local`:
  daemon only. `proxy`: proxy only (a proxy call without configured
  endpoint/token fails the call site's existing degrade path). Unset,
  blank, or unknown-case/whitespace variants resolve per the
  trim+lowercase rule; any OTHER value is a hard error naming the valid
  modes (Rust: `decision_mode`; the shell/ideate mirrors refuse the same
  way and take the call site's degrade path).
- `CHUG_DECISION_PROXY_URL` — the proxy base URL (`<base>/v1/systemone` is
  posted; a trailing slash is tolerated). `CHUG_DECISION_PROXY_TOKEN` —
  the credential, sent as `Authorization: Bearer …` (a tokenless call is
  legal at the wire level and answers 401 = the degrade path).
- All three ride the T330 profile contract as OPTIONAL pass-through keys:
  `install.sh --profile` accepts them (a profile owns the required LLM
  keys plus these), and both loopd loaders export them verbatim with
  explicit env winning — `~/.chug/profile.env` via
  `scripts/loopd_profile_loader.sh`, `~/.chug/loopd.env` via
  `scripts/loopd_env_loader.sh`. `profiles/decision-proxy.env` is the
  placeholder-values example. The real endpoint/token never land in the
  repo (the 2026-10 scrub + the T249 lint are the guard).

## Local daemon (the first provider, unchanged)

`POST http://localhost/judge` over the `chugd` unix socket
(`$CHUG_DAEMON_SOCK`, then `$CHUG_HOME`, then `~/.chug/daemon.sock`),
0600, same `{state, questions}` body, same `{answers:{…}}` envelope. The
daemon serves `choice` (the risk-gate and the T259 triage) and the T326
`noul` gate; `score` is proxy-shaped — a `score` question routed to the
local daemon is a caller error, and the provider abstraction treats it as
such.

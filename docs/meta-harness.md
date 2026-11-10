# The chug meta-harness capability contract

A meta-harness (herdr, orca, bridge) supervises chug instead of driving it
by hand. Until T339 every such harness version-sniffed — "is this chug new
enough for X?" — which breaks both ways: a new feature reads as absent on
an old binary it actually has, and a version guess breaks the day a
backport lands. This contract replaces sniffing with **feature detection**:
run `chug capabilities`, read one JSON document, branch on the keys.

The document is stable. Anything that would break a conforming reader
bumps `contract_version` (see [Versioning](#versioning)); everything else
lands additively, so each later meta item (T340-T343) can ship without a
single consumer edit.

## Reading the document

`chug capabilities` prints one pretty-JSON document and exits 0:

```json
{
  "contract_version": "1",
  "compat": "consumers must treat unknown features as false and ignore unknown keys",
  "features": {
    "daemon_status": false,
    "lifecycle_events": false,
    "permission_broker": false,
    "usage_events": false,
    "status_mod": false
  },
  "versions": {
    "chug": "0.19.1",
    "daemon": null
  }
}
```

- `contract_version` — which revision of THIS document the producer
  speaks. The one key every document must carry.
- `features` — the feature registry (below). `false` = not available on
  this build; a version string = available, shipped at that version.
- `versions.chug` — the version of the chug binary that printed it.
- `versions.daemon` — the live daemon's self-reported version; `null`
  when no daemon is live or the daemon wire does not carry a version yet
  (today's `/healthz` does not — the report lands with T340's row).
- `compat` — the stability rule, in-band, so a harness sees it every time
  it reads capabilities rather than only in these docs.

The command is cheap by construction — no daemon contact, no file I/O —
so it is safe to call at harness startup, per cycle, or in a probe loop.
It completes in well under 50ms with or without a live daemon; a missing
daemon is reported `false`/`null`, never an error and never a hang.

To validate or canonicalize a document (yours or a fixture's), run it
through chug's reference reader:

```console
$ chug capabilities --check /path/to/document.json   # canonical form on stdout
$ echo $?                                            # non-zero on a malformed document
```

## The stability rule

A conforming consumer MUST do two things, and may rely on nothing else:

1. **Reading an unknown feature MUST behave exactly as reading `false`.**
   A feature the consumer has never heard of is not shipped, as far as it
   is concerned. This is why chug's reader ([`Features::shipped`] in
   `src/capabilities.rs`) returns false for any name outside its registry.
2. **Unknown keys MUST be ignored.** Producers add keys (that is how
   features and fields land); consumers that fail on them break by
   construction.

Both rules are pinned in `tests/capabilities_contract.rs`: an output with
injected unknown keys/values parses identically through the reference
reader, and the extra `compat` key in the output is itself an exercise of
rule 2.

## Feature keys

All rows before T352 were `false` until landed; each flips on when its row
lands. The "shipped value" column is the value it carries once available.

| key | today | shipped value | landed by | what it reports when shipped |
| --- | --- | --- | --- | --- |
| `daemon_status` | `false` | boolean | T340 | a live chugd answers the typed status endpoints; `false` = absent or unreachable, never a hang |
| `lifecycle_events` | `false` | version string | T340 | the lifecycle/status event surface and its wire version (`"1"` when shipped) |
| `permission_broker` | `false` | boolean | T341 | the opt-in permission-broker hook pack is installed (detected by hook presence, not socket presence) |
| `usage_events` | `false` | boolean | T343 | the per-turn usage-emitter hook pack is installed |
| `status_mod` | `false` | boolean | T342 | the session-status mod hook pack is installed |
| `auto_update` | `true` | boolean | T352 | `chug update` exists (self-update with version-floor protection + the stale-release source-build fallback) — compiled into every build, no probe |

## The typed status enum

Surfaces that report what a session or loop is doing use ONE vocabulary —
harnesses render it once and read it everywhere:

| status | meaning |
| --- | --- |
| `working` | executing normally — a turn, an iteration, a cycle |
| `blocked` | stopped, pending something; `blocked_reason` says what |
| `needs-input` | stopped, pending an operator answer (a question) |
| `done` | the work item finished |
| `parked` | deliberately set aside by the operator (unpark to resume) |

`blocked` and `needs-input` are the "the loop needs you" family; a
harness treats both as attention-required, with the reason distinguishing
them. A surface whose reality is narrower than the five (a turn-level
view has no `parked`) maps onto the nearest member rather than inventing
a value; extensions are additive ([Versioning](#versioning)).

## Blocked-reason vocabulary

A `blocked` status carries one of these reasons:

| reason | meaning |
| --- | --- |
| `permission` | a permission ask is pending |
| `question` | a question for the operator is pending |
| `error` | the last step failed (retry/exhaustion state in the detail) |
| `budget` | an iteration / wall-clock / token budget tripped |

T340's loop endpoints add `external` — a blocker outside this repo (a
missing remote, an absent path) — when that row lands; additive within
the contract version.

## Event shapes, as they land

None of these surfaces exists yet. The shapes below are the contract each
landing row implements against; fields are additive-only within a
contract version, and **absent values are omitted keys — never fabricated
zeros, never empty strings**.

### T340 — chugd typed status endpoints (`daemon_status`, `lifecycle_events`)

`GET /status/loop` on the daemon socket:

```json
{
  "status": "working",
  "cycle_id": "…",
  "iteration": 3,
  "item_id": "T341",
  "blocked_reason": null,
  "blocked_detail": null,
  "children": [{ "kind": "impl", "id": "…", "state": "…" }],
  "as_of": "…"
}
```

with `status` drawn from the loop's own working/blocked/idle/dormant
shape and every blocked case surfacing — the missing-repo case reports
`blocked` with reason `external` and the detail, it never vanishes.
`GET /status/items` returns one row per open TODO item:
`{ id, title, pri, status, blocked_reason? }` — the queue view without
parsing TODO.md. Every response carries `contract_version`. Socket
absent or endpoints unreachable: `chug capabilities` reports
`daemon_status: false` (bounded probe, never a hang).

### T341 — permission broker (`permission_broker`)

An opt-in PreToolUse hook pack (`hooks/meta-broker/`, shipped in-repo and
installed by `install.sh --with-meta-pack`). When
`<claude-dir>/meta-broker.sock` exists and answers, the hook POSTs one HTTP
request over that unix socket:

```
POST /permission HTTP/1.1
Host: meta-broker
Content-Type: application/json
Content-Length: n
Connection: close

{ "tool_name": "…", "input_summary": "…", "session_id": "…", "cwd": "…", "contract_version": "1" }
```

- `input_summary` — a bounded rendering of the tool input (compact JSON,
  truncated hard at 2000 chars, `...` appended when truncated); the socket
  payload stays small no matter how large the actual tool input is.
- Absent values are omitted keys — never fabricated zeros, never empty
  strings (the contract's event rule, applied to the request).
- The whole interaction (connect through reply) gets **800ms** of wall
  clock; anything slower is a fail-open.

and honors the reply:

```json
{ "decision": "allow|deny|ask", "reason": "…", "ttl": "…" }
```

| decision | what the hook does | what Claude sees |
| --- | --- | --- |
| `allow` | prints the `permissionDecision: allow` document, exit 0 | the tool runs, no prompt |
| `deny` | prints the `permissionDecision: deny` document, exit 0 | the tool is refused with the reason |
| `ask` | prints nothing, exit 0 | the stock prompt path, unchanged |

`ttl` is advisory for the v1 hook: it is recorded in the audit line and not
acted on (a caching hook revision would honor it; the semantics stay the
harness's). `reason` and `ttl` are optional; a non-string reads as absent.

**Fail-open by construction** — every condition below lands on the same
path: exit 0, no output, no audit line. Stock permission evaluation is
byte-identical; a broker crash can never wedge a session.

| condition | behavior |
| --- | --- |
| socket absent (`-S` fails) | stock path, <5ms added (shell builtins only, no fork) |
| connect refused (a stale socket) | stock path |
| >800ms (connect, send, or read) | stock path |
| non-200 HTTP status | stock path |
| malformed body / missing `decision` / unknown `decision` | stock path |
| python3 missing (the handler's runtime) | stock path |
| audit append fails | stock path (below) |
| any other handler error | stock path |

The audit leg is the contract's record: every **brokered** decision —
`allow`, `deny`, and `ask` — appends exactly one JSONL line to
`<claude-dir>/meta-broker-audit.jsonl`; stock-path outcomes append nothing
(no noise). The decision only lands if its audit line is written — an
unauditable decision is not a decision (fail-open, the gap is observable by
the harness as a missing line). The line schema:

```json
{ "ts": "…", "tool_name": "…", "decision": "allow", "reason": "…", "ttl": "…",
  "broker_latency_ms": 4, "input_summary": "…", "session_id": "…", "cwd": "…",
  "contract_version": "1" }
```

`ts` is RFC3339 UTC with milliseconds; `broker_latency_ms` is the POST
round trip (connect through reply); absent values are omitted keys.

**Install / uninstall / detection.** `install.sh --with-meta-pack` installs
the pack to `<claude-dir>/hooks/meta-broker/` and wires its PreToolUse hook
into `<claude-dir>/settings.json` additively (existing hooks untouched,
re-runs a no-op, the write atomic) — the pack comes from the SAME verified
release tarball the binary comes from, or from the working tree when the
script runs from a repo checkout that carries it.
`install.sh --uninstall-meta-pack` is the exact reverse, offline: it
removes exactly the pack's own block and the installed pack dir.
`chug capabilities` reports `permission_broker: true` when the pack is
**wired** — detected by the hook's presence in the user-level settings
(the pack marker in a `hooks.PreToolUse` command), never by socket
presence: a listening socket without the wired hook brokers nothing, and a
wired hook with no socket is a <5ms no-op. `<claude-dir>` is
`$CLAUDE_CONFIG_DIR` when set, else `~/.claude` — the same resolution the
hook uses at run time.

### T342 — session status mod (`status_mod`)

An opt-in hook pack (`hooks/meta-status/`, same `--with-meta-pack` /
`--uninstall-meta-pack` install legs as T341, same OFF-by-default rule)
whose handlers ride the hook events that SEE the truth — turn ends,
permission asks, errors, operator questions — and transcribe each into
one typed status file per session,
`<claude-dir>/meta-status/<session-id>.json`:

```json
{ "status": "working", "blocked_reason": null, "detail": "Bash", "updated_at": "…", "contract_version": "1" }
```

`status` is the typed enum above, exactly; `blocked_reason` the
vocabulary above; `detail` (the tool name or the ask's message) rides
only when known — absent values are omitted keys, never fabricated. The
event table:

| event | condition | written |
| --- | --- | --- |
| `UserPromptSubmit` | — | `working` |
| `PreToolUse` | `AskUserQuestion` | `blocked` / `question` |
| `PreToolUse` | any other tool | `working` (+ tool detail) |
| `PostToolUse` | response carries an error marker | `blocked` / `error` |
| `PostToolUse` | otherwise | `working` |
| `Notification` | message mentions `permission` | `blocked` / `permission` |
| `Notification` | otherwise | nothing (not a status fact) |
| `Stop` | file says `blocked`/`question` | nothing (the question stays pending) |
| `Stop` | otherwise | `done` |
| `SessionEnd` | — | `done` |

The write is a single equal-length overwrite of the same inode: the body
is space-padded to a constant length (trailing spaces are legal JSON
whitespace) and `date`'s format string IS the document, written through a
`1<>` open — readers see the previous or the new COMPLETE document, never
a partial one, with no rename and no truncate window. Session end leaves
the file at `done` (no reaper needed).

Reader contract: harnesses poll the file (inotify/fswatch optional);
absent file = not-a-meta-session; stale (>10min by the file mtime or
`updated_at`) = treat as unknown. The mod is pure transcription — <2ms
per hook event (builtins plus one `exec date`), no network, no LLM calls
— it never writes anything else and never touches the session's own
state. Silent when unwatched: the pack's total footprint is one small
file per session; nothing else changes, and a missing status dir means
skip. Non-meta sessions are unaffected whether or not the pack is
installed.

### T343 — usage events (`usage_events`)

One JSONL line per turn-complete (the pack wires Claude's `Stop` event),
appended to `~/.claude/meta-usage.jsonl`:

```json
{ "ts": "…", "session_id": "…", "model_requested": "…", "model_served": "…", "input_tokens": 1234, "output_tokens": 567, "cache_read_tokens": 7, "cache_creation_tokens": 2, "contract_version": "1" }
```

**Presence contract — the row's rule.** A field is in the line only when
the event carries it. An absent value is an OMITTED KEY — never a
fabricated zero, never the requested name copied into the served slot
(that conflation is the bug this row fixes). Consumers must treat every
key except `ts`, `session_id` and `contract_version` as optional, and a
carried zero as data (a cache accounting that reads zero is a fact, not a
gap).

| Field | Extracted from | Present when |
| :- | :- | :- |
| `ts` | the write itself (RFC3339 UTC) | always |
| `session_id` | the payload's `session_id` | always — a payload without a safe one emits NO line (an unattributable turn is no row, not a blank one) |
| `model_requested` | the payload's `model_requested`, else its `model` (the session's configured name — what was ASKED for) | the payload carries either |
| `model_served` | the payload's `model_served` ONLY | the payload carries it — never derived from `model` |
| `input_tokens` / `output_tokens` | the payload's first `"usage":{…}` object (Messages-API shape; a `message.usage` embedding matches the same rule) | the usage object carries them |
| `cache_read_tokens` / `cache_creation_tokens` | the same usage object's `cache_read_input_tokens` / `cache_creation_input_tokens` (renamed to the line's schema) | the usage object carries them |
| `contract_version` | the pack | always (`"1"`) |

Today's Claude Code `Stop` payloads carry none of the model or usage
fields (the only turn-final fact they pin is `last_assistant_message` —
text), so today's lines are the honest marker
`{"ts":…,"session_id":…,"contract_version":"1"}`. The transcript is
deliberately NOT read: it is written asynchronously and may not yet
include the current turn, so mining it would attribute the PREVIOUS
turn's usage to this one — exactly the wrongness this pack exists to end.
A payload that does carry the fields — a harness re-emitting enriched
events on the T327 pattern, or a future Claude build — is transcribed in
full. The emitter is pure transcription: one `exec date` per turn (the
line IS the format string), <2ms added, no network — Langfuse shipping
stays with the OTel path, never in this hook. Install:
`install.sh --with-meta-pack`; uninstall removes the emitter and leaves
the accumulated log in place (a historical record, not state).
Detection: `chug capabilities` flips `usage_events: true` when the hook
is wired (hook presence, never file presence).

**Chug-side model-identity truth (the same row, chug's own streams).**
The loop stamps the AS-SERVED identity — `Response::served_model()`, the
response body's top-level `model` field, which the Messages API (and the
streaming `message_start` frame) sets to the model that actually
answered — into its own telemetry, populated only when the provider
reports it, for proxy and daemon paths alike:

- `.chug/events.jsonl` — every `iteration` line carries `model_served`
  (the as-served name of the response that produced that line's tokens);
- the `abort` line carries BOTH names distinctly — `model` (the requested
  name, the T12 resume hint) and `model_served`;
- `.chug/decisions.jsonl` — records gain the optional tail key
  `model_served` (the deciding turn's as-served identity), so the F13
  corpus names the model that actually decided.

On all three, a provider that reports none leaves the key out — the
requested name is never copied into the served slot. Additive optional
keys: existing consumers parse unchanged.

### T354 — loopd's native OSC 7501 status (the terminal channel)

`loopd.sh` reports its own state to the terminal it runs on with **OSC
7501** — Mitchell Hashimoto's Program Status Protocol (an
`ESC ] 7501 ; …` sequence well-behaved terminals ignore and terminal
harnesses parse). This is the third observation channel of this contract,
and the terminal-native one: a harness that owns the pty (herdr, cmux,
Agent Deck — or plain `ssh`) sees the loop's state with zero install and
zero sockets. Where the fleet previously shipped a per-harness shim that
parsed `loopd.log` and re-emitted 7501 on the loop's behalf, the supervisor
now emits natively and that shim retires for every terminal harness at
once.

Emission points (one record per transition, the supervisor's own legs):

| loopd leg | state | msg channel |
| --- | --- | --- |
| cycle start | `working` | `cycle start: <mode> cycle on <model>` |
| ideation (item) start | `working` | `ideation cycle: proposing outward-facing features` |
| cycle OK (the accepted goal) | `idle` | `cycle ok` |
| cheap-exit skip (T258/T259) | `idle` | `cycle skipped: no evaluation needed now` |
| dormant cheap-exit (T325) | `idle` | `dormant: queue empty` |
| cycle death | `blocked` | `<reason>: cycle ended without goal complete (rc=N)` |
| build gate red | `blocked` | `error: build failed (rc=N) — …` |
| disposition commit failing | `blocked` | `error: disposition commit failed — …` |
| the 3-strike HALTs | `blocked` | `error: HALTED after 3 consecutive …` |

A `blocked` msg is one line headed by its T340 blocked-reason word —
`budget` (the child died on the budget/abort exit) or `error` (everything
else) — so the terminal surface and the daemon surface (T340's
`blocked_reason`) read the same vocabulary from the same writer. The wire
is the **id-less root record**:
`ESC ] 7501 ; state=<s>[:msg=<base64>] ESC \` — `msg` is one base64 line,
the whole sequence ≤4096 bytes. Records carry no `id`, `app`, or
`progress` keys: the loop's state is one root record, and optional keys
staying absent is exactly the additivity this contract promises.

**The gating contract (compat-critical).** The emission happens ONLY when
(a) stdout is a TTY *and* (b) `CHUG_OSC7501` is unset or `=1`. Every
redirected context — `nohup`, launchd, a pipe, a log file — receives ZERO
escape bytes: log parsers must never see one. `CHUG_OSC7501=0` (or any
non-1 value) disables even on a TTY. Non-harness operation is
byte-identical to pre-T354.

Probe: `./loopd.sh osc <state> [one-line reason]` emits one record through
the real gated emitter and exits — the operator's and the tests'
behavioral surface (the `routing`/`sleep-ok`/`predicate` probe pattern).

Implementation note: the wire follows the published OSC 7501 specification
(the T354 semantics table plus the protocol's field rules). The fleet
shim's exact bytes (herdr-fleet-mods `252a6f8`) were not available on the
build host, so byte-compat is claimed at the spec level — id-less root,
`state` + optional base64 `msg`, `ESC \` terminator — not at the
shim-diff level. `tests/loopd_osc7501.rs` parses every emitted byte with a
spec-derived reference parser and pins the gating contract end to end.

#### The PTY's verbosity dial: `CHUG_LOOP_VERBOSE` levels (T356, T359)

The 7501 dot answers "is it alive"; the compact heartbeat (T356) answers
"what transition just happened"; the operator's follow-up (2026-11-10)
asks for "what is it actually doing" on the same PTY. One knob dials it —
`CHUG_LOOP_VERBOSE=<n>` (the spawned pilot supervisor inherits it through
the environment; `chug loop run` passes nothing extra):

| level | surface | the terminal shows |
| --- | --- | --- |
| `0` | silent | nothing — no heartbeat lines, no turn detail, no tee (the 7501 dot is the dot's own knob, `CHUG_OSC7501`) |
| `1` — the default on a TTY | compact heartbeat (T356) | the cycle start, iteration milestones (one per `LOOPD_HB_EVERY`=25), merge flips, the dormant line, `BLOCKED:` — ≤15 lines per 200-iteration cycle |
| `2` | verbose | level 1 PLUS one line per tool call (`[loop] T042 iter 37 — tool bash: <summary, 80>` — a `...` truncation marker past 80; errors pass in full, up to the driver's own preview cap) and each model text block capped at ~3 lines (the same marker on truncation) |
| `3` | trace | level 1 PLUS the cycle's full output: every new `cycle-*.log` line teed to stdout as it lands (≤2s), unfiltered and unprefixed — the stream IS the cycle log |

The gating contract is the standing one, extended: **stdout must be a
TTY** — a redirected context (nohup, launchd, a pipe, a log file) is level
0 whatever the knob says, so log parsers never see a heartbeat line, a tee
line, or an escape byte. On a TTY, unset/empty is level 1 (the T356
default, byte-identical to pre-T359); any value outside `0..3` degrades to
0 (default-safe — an unreadable knob must never turn detail on). The
mechanism is a line-count-latched tail of the files the loop already
writes (the cycle's `.chug/events.jsonl` for tool results, the cycle log
for model text and the trace) — nothing new is logged and nothing is
double-written: the child's stdout is captured, its stderr goes to the
cycle log, so the tee is the only thing these lines ever reach the TTY
through. Level 2's summaries follow the T356 source-string rule: one
source (the event's own preview, the cycle log's own model line) feeds the
summary — never a re-derived copy. The T354 7501 emission is unaffected:
the dot and the line stay in sync at every level. Fixtures:
`tests/loopd_verbose.rs` (per level) and the gating extensions at the
bottom of `tests/loopd_heartbeat.rs`.

#### Choosing among the three observation channels

| channel | audience | read how | carries |
| --- | --- | --- | --- |
| T340 daemon endpoints (`daemon_status`) | harnesses integrated with the host daemon; dashboards | one bounded socket probe | the richest typed view: cycle id, iteration, children, `blocked_reason` |
| T342 status files (`status_mod`) | harnesses that poll the filesystem; Claude-session supervision; no terminal required | poll `<claude-dir>/meta-status/<session-id>.json` | per-session typed status, atomic single-file overwrite |
| T354 OSC 7501 (this row) | harnesses that own the pty; ssh/container/tmux-bridged terminals; zero install | watch the terminal stream | loop-level state + a one-line msg; nothing to install, works anywhere a pty does |

The channels are siblings, not competitors: T340 needs the daemon on the
host, T342 needs the hook pack installed, 7501 needs only a terminal. A
harness picks by what it already has; all three speak the same typed status
enum and the same blocked-reason vocabulary.

## Spawning a loop: the goal rides `--goal`, not the prompt

A harness that spawns a loop (`chug loop init && chug loop run` in a
target repo) passes the spawn goal as a FLAG: `chug loop init --goal
"<goal text>"` (or `--goal-file <path>` for a long goal), or `chug loop
run --goal` when the target's queue is still empty (seed-then-run).
`--goal` seeds the queue — TODO.md gains one row citing a generated spec
under `specs/` that carries the goal verbatim and a `check:` line — so
the loop works the goal instead of waking on an empty queue and going
dormant (T355; the 2026-11-09 herdr report: "the queue stayed at 0 for
the goal i spawned it with" — the goal had only ridden the spawn prompt).

The session prompt is dead text to the loop: a cycle reads its queue
(TODO.md), its contract (LOOP-SPEC.md), and nothing else. A goal that
only rode the prompt never becomes work. Compat: bare `chug loop init`
is unchanged (empty queue + Proposed lane) — only `--goal` seeds; and
`chug loop run --goal` on a NON-empty queue refuses (exit 2) rather than
silently append to a queue nobody read, so seeding a live queue is
`chug loop init --goal`'s explicit, reported act.

## Versioning

Within one `contract_version`, every change is **additive**:

- new feature keys (consumers read them as `false`),
- a feature value upgrading `false` → `true` or → a version string (the
  shape the registry already allows),
- new optional fields on event shapes, new keys in `versions`,
- new members of open vocabularies (a new status value, a new blocked
  reason).

**Breaking — bump `contract_version`:** removing or renaming any key,
changing a value's type outside the boolean→version-string upgrade,
changing the meaning of an existing value, or any change a conforming
consumer (the two MUSTs above) could not ride through untouched. A
consumer that reads `contract_version` it does not know should refuse the
document loudly rather than guess — that is the bump doing its job.

## Anthropic-side protocol surface

The broker (T341) and the status mod (T342) ride Claude's hook protocol —
PreToolUse permission decisions, turn/stop events — documented at
<https://docs.claude.com/en/docs/claude-code/hooks>. Those protocol
semantics are Anthropic-side and out of this contract's control; this
document pins only what chug ships around them. Protocol asks that need
Anthropic changes are tracked out-of-band, never here.

## Where it lives

- Producer + reference reader: `src/capabilities.rs`
  (`chug capabilities`, `chug capabilities --check <file>`).
- Contract pins: `tests/capabilities_contract.rs`, fixture
  `tests/fixtures/capabilities/contract-v1.json`.
- The landing rows: T340 (daemon status + lifecycle events), T341
  (permission broker), T342 (session status mod), T343 (usage events).
- T341's shipping leg: `hooks/meta-broker/` (the hook pack),
  `install.sh --with-meta-pack` / `--uninstall-meta-pack` (the opt-in
  wiring), `tests/meta_broker_hook.rs` (the hook + audit battery).
- T342's shipping leg: `hooks/meta-status/` (the status mod pack, the
  same install legs), `tests/meta_status_hook.rs` (the typed-status
  battery: event table, atomicity under a hammering reader, the
  one-file footprint, session-end cleanup, the <2ms budget).
- T343's shipping leg: `hooks/meta-usage/` (the usage emitter pack, the
  same install legs — the accumulated log survives uninstall),
  `tests/meta_usage_hook.rs` (the usage battery: full/partial event
  transcription, the presence contract, the never-conflate rule, the
  <2ms no-network budget), plus the chug-side stamping pins
  (`src/eventlog.rs`, `src/driver/tests/events.rs`, `src/decisions.rs`).

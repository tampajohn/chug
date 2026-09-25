# T22 — Bash tool description: macOS `timeout` mirage note

check: cd /Users/jadams/workspace/chug && cargo test

## Why (evidence)

Cycle-7 evaluation finding **M2**: macOS has no GNU `timeout` command,
and models reach for it anyway — **two independent exit-127 occurrences
in one day, across two model families**:

1. The T20 glm impl child, at iteration ~39 of 40, invented
   `timeout 5 …` for a banner dogfood (its spec never mentioned
   `timeout` — zero occurrences in `specs/t20-banner-worktree-head.md`),
   got `sh: timeout: command not found`, and burned two of its last
   three iterations recovering — a direct contributor to its 40/40
   wrap death (`events-t20-impl-20260925-182346.jsonl`).
2. The cycle-7 orchestrator (kimi), during evaluation dogfooding, ran
   the identical `timeout 5 …` and got the identical exit 127.

META-SPEC:126 already names the platform-correct idiom
(`perl -e 'alarm 600; exec @ARGV' …` on macOS), but implementation
children never read META-SPEC — they receive only their t-spec and goal.
The ONE surface every session of every role sees is the tool
description list itself. macOS ships `perl` at `/usr/bin/perl` by
default, so the idiom is always available.

## Repo context

- `src/tools.rs:78-79` — the bash tool's schema entry; its description
  is a single string literal:
  `"Run a shell command via `sh -c` in the working directory. Captures stdout+stderr and the exit code. 120s timeout; long output is truncated (head+tail kept)."`
- The description is currently **unpinned** (verified by grep at eval
  time: no test asserts it). `tool_schemas()` (`:38`) is a pure function
  returning `Vec<Value>` — directly assertable in `tools.rs`'s existing
  test module (`:731`).
- Note the description already says "120s timeout" — that phrase refers
  to the DRIVER's own kill cap (`BASH_TIMEOUT_SECS`), which the new
  sentence must not confuse. The new note is about the *command the
  model writes*, not the driver's cap.
- Behavioral surface: **zero** — tool descriptions ship to the model in
  the API request; nothing in the driver parses them. The only risk is
  token bloat (one sentence) and drift (hence the pin).

## Requirements

1. Append ONE sentence to the bash tool's description string naming the
   platform fact and the idiom. Suggested wording (implementer may
   adjust minimally for grammar, but must keep both load-bearing
   tokens — `timeout` and `perl -e 'alarm` — verbatim):
   `"On macOS there is no `timeout` command; bound long commands with `perl -e 'alarm N; exec @ARGV' <cmd>` instead."`
2. No other description changes; no behavior changes anywhere; the
   driver's own 120s cap wording stays intact.
3. README gate: no README change — tool descriptions are model-facing,
   and README's bounded-gates doctrine already lives in META-SPEC.

## Tests

- New pin in `src/tools.rs`'s test module: `tool_schemas()` contains a
  `bash` entry whose `description` contains both `timeout` (in the
  mirage-warning context, i.e. assert the full substring
  `no \`timeout\` command`) and `perl -e 'alarm`. Assert on the schema
  JSON, not on a duplicated literal, so the test fails if the
  description is reverted or the idiom is corrupted.
- Non-vacuousness: reverting the description to the pre-T22 string must
  fail the new pin (the implementer demonstrates this once during the
  round).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo
  test` all green.
- Adversarial validation (REQUIRED — touches `src/tools.rs`, LOOP-SPEC
  §2.4): confirm the description gained exactly one sentence, the pin
  asserts against the live schema (not a copy), the revert-mutant dies,
  and no other tool description or behavior changed.

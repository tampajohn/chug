# T12 — Budget-abort output names the model + fallback hint

check: cargo test

## Concern

muse-glimmer-30b died three times on 40-iteration budgets during SPEC-9
round 2 (`.chug/transcript.jsonl:386,394`; `.chug/LEDGER-spec9-archive.md`)
before a human/protocol-level kimi-k3 fallback, and the validation child budget
had to be raised 25→40 after repeated exhaustion (`7108ec4`). When a run dies
on budget, the abort output gives the reason but not the *model* that died or
the concrete resume-with-different-model command — the operator (often another
chug) has to reconstruct both (EVALUATION.md I9). This is a hint, not
auto-fallback: model routing policy is a human decision.

## Repo context

- `src/driver.rs`: `abort_exit` emits `Event::Aborted { reason }` after pushing
  the freshest ledger; SPEC.md's contract is that aborts print the ledger and a
  `chug run --resume` hint (console sink renders this).
- `src/events.rs`: ConsoleSink formats abort output.
- The model id lives in `RunConfig.model` / the client; `LoopCtx` does not
  currently carry it — threading it through is the bulk of the change.
- Chat mode: per-turn budget aborts return to idle; keep their output as-is or
  add the same model mention — either is acceptable, autonomous is the target.

## Requirements

1. On autonomous budget aborts (iteration or time), the abort output includes:
   the model used, the budget exhausted (iters/minutes), and an explicit hint
   line: `resume: chug run --resume [--model <other>]` (with the current model
   named).
2. Stuck-tripwire and operator aborts may share the model line but the
   fallback hint is only required for budget deaths.
3. No automatic retry, no model switching, no new flags.

## Tests

- Drive a budget-exhausted autonomous run with the existing
  `ScriptedLlm`/`RecordingSink` harness → abort output (event or rendered
  sink text) contains the model id and `--resume --model`.
- Non-budget aborts still print the ledger exactly as before (no regression
  in existing abort tests).

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all
  green.
- A `--max-iters 1` run against a scripted endpoint prints the model + resume
  hint on abort.

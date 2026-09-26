# T58 — `delegate` launch gains `resume: true`; `status` summarizes the latest run segment

check: cargo test && cargo clippy --all-targets -- -D warnings

## Repo context

`delegate` (T23, `1012dca`) launches bounded child `chug run`s detached
and polls them via `status`. Two gaps, both observed live in cycle 18's
T38 arc and verified in the code at the cycle-26 eval:

1. **`launch` cannot express `--resume`.** When the T38 kimi validator
   died 50/50 post-mutations pre-verdict, recovery was a hand-rolled
   bash `chug run --resume` under nohup — the exact launch pattern T24
   replaced — because the tool has no resume leg. The abort output's own
   resume line (`chug run --spec <spec> --goal "<goal>" --cwd <cwd>
   --resume [--model <other>]`) shows spec/goal/model are still passed
   on a resume; `--resume` is purely additive to the argv.
2. **`status` misreports a resumed child.** `summarize_events`
   (`src/tools.rs:975`) scans the WHOLE `.chug/events.jsonl` and latches
   `goal_seen` / `abort_seen` / `abort_reason` / `budget_low_seen` on ANY
   matching line (`src/tools.rs:1003-1013`). A resumed child appends a
   NEW `run_start` + fresh iterations to the same stream, but the
   pre-resume segment's abort line keeps `abort_seen` latched, so
   `state()` reports `aborted` while the resumed child runs healthy
   (cycle-18 orchestrator fell back to `ps`).

Mid-arc child deaths are the loop's most common child failure (cycle-26
eval I3: t37/t38/t47/t55), so resume-from-crash through the delegation
surface is an observed, not hypothetical, demand.

## Requirements

1. **Schema** — `delegate`'s `launch` action gains an optional boolean
   `resume` (default false / absent). The schema description names what
   it does: append `--resume` to the child argv (continue the child's
   prior run from its `.chug/transcript.jsonl` instead of starting
   fresh).
2. **Argv** — `delegate_child_argv` (the pure seam, T39) appends
   `--resume` when and only when `resume == true`; spec/goal/model and
   any budget flags are passed exactly as today; absent/false `resume`
   produces a byte-identical argv to pre-T58 (pinned).
3. **Return text** — the launch response names the resume leg when used
   (e.g. a `resume: true` field in the returned summary), mirroring the
   T39 `max_tokens` echo pattern (only when set).
4. **Latest-segment summary** — `summarize_events` resets the verdict
   latches (`goal_seen`, `abort_seen`, `abort_reason`, `budget_low_seen`)
   each time a new `run_start` line is encountered, so the summary
   describes the LATEST run segment; `max_iters` and `last_iteration`
   already take the last-seen values and keep doing so. A stream whose
   first segment aborted and whose second segment is mid-run summarizes
   as `running`; one whose second segment goal-completed summarizes as
   `done`.
5. **README.md** — the `delegate` paragraph gains one short clause:
   `launch` accepts `resume: true` to continue a child's aborted run,
   and `status` summarizes the child's latest run segment. Integrated,
   no new bullet.

## Tests

- `delegate_child_argv`: with `resume: true` the argv ends with
  `--resume` after all other flags; with `resume` absent/false the argv
  is byte-identical to the pre-T58 list (whole-list pin, T39 style).
- `summarize_events`: (a) two-segment stream (abort in segment 1,
  `run_start` + iterations in segment 2) → `state() == "running"`,
  `abort_seen == false`; (b) goal in segment 2 after abort in segment 1
  → `state() == "done"`; (c) `budget_low` only in segment 1 →
  `budget_low_seen == false`; (d) single-segment streams behave exactly
  as before (regression pin).
- Schema pin (T22/T41 style): the live `tool_schemas()` output for
  `delegate` names the `resume` property for `launch`.

## Acceptance

- `check:` passes in the impl worktree (full suite + clippy).
- End-to-end proof via the existing `CHUG_DELEGATE_BIN` stub harness or
  a scripted fixture: a launch with `resume: true` produces the
  `--resume`-suffixed invocation; a status call over a synthetic
  two-segment events file reports the second segment's state.
- No `|` in the TODO row's notes cell (T40).

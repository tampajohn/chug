# T191 — runbooks/: per-use-case operator runbooks

check: cargo test --test todo_consistency

estimate: ~250 lines (runbooks dir + 5 docs + README section)

## Concern

Operator 2026-10-01: "some simple runbooks defined for various chug uses
in a runbooks folder could be helpful." The doctrine files (SPEC.md,
LOOP-SPEC.md, META-META-SPEC.md) are the loop's own operating manual —
an operator who wants to USE chug for a task has no short, copy-pasteable
entry point. Each recurring use case should be a one-page runbook:
when to use it, the exact command/goal template, and the expected arc.

## Repo context

- README cold-read audit (META-META §6) already guards "usable by a
  newcomer" — runbooks are the task-level companion, linked from README.
- T188 (auto-spec) + T189 (low-stakes lane) change the quick-task arc;
  the runbook for it references those flags but must be accurate TODAY
  (write against current flags, annotate the T188/T189 follow-up).
- Existing worked examples to distill: vampflake feasibility eval
  (VAMPFLAKE-CHUG-FEASIBILITY.md arc), codex adversarial review arc,
  loopd operations (loopd.sh status / kickstart / HALT recovery),
  dashd dogfood loop on K7.

## Requirements

1. New `runbooks/` directory, one file per use case, each ≤ ~60 lines:
   - `runbooks/quick-task.md` — single chore/small task: goal template,
     budget flags, what the transcript/ledger look like, resume on
     budget-death (T63 doctrine).
   - `runbooks/feature.md` — spec'd feature work: write the spec
     (check: + estimate: per T150), run, adversarial validation,
     verdict handling.
   - `runbooks/repo-eval.md` — feasibility evaluation of an external
     repo (the vampflake pattern): eval goal, deliverable file, verdict
     format.
   - `runbooks/loop-ops.md` — loopd operations: start/status/stop,
     HALT marker recovery, worktree cleanup, where cycle logs live.
   - `runbooks/adversarial-review.md` — pointing chug (or chug+codex)
     at a repo/diff for review, intake of findings as TODO rows.
2. `runbooks/README.md` index: one line per runbook — the use case in
   the operator's words, not the file name.
3. Every command in every runbook is copy-pasteable as-is (real flags,
   no placeholders beyond <goal>); each runbook states its expected
   wall-clock arc (minutes vs hours) so operators calibrate.
4. README.md gains a "Runbooks" section linking the index (one or two
   sentences + link — no inline duplication, per the append-only
   README anti-pattern META-META §6 guards).

## Tests

- todo_consistency passes (this row's spec + estimate line).
- Acceptance: a cold reader following runbooks/quick-task.md verbatim
  reaches a completed `chug run` without reading any other doc
  (self-report in the cycle Outcomes entry).

## Out of scope

- Video/interactive tutorials; per-domain runbooks beyond the five
  above (add later by operator request); rewriting doctrine files.

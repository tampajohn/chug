# Runbook: adversarial review of a repo or diff

**Use when** you want a second pair of eyes: correctness bugs in a
snapshot or on a diff/commit — read-only work whose deliverable is a
findings report. **Arc: ~20–60 minutes.** The reviewer is chug itself, or
an external reviewer (the codex-review arc); intake is the same either way.

## 1. Run the review

```bash
chug run --spec review-spec.md \
  --goal "Source-level adversarial review per the spec: find correctness bugs; write findings-report.md; change nothing but that file" \
  --model anthropic-system.ai.kimi-k3 --max-iters 40 --max-minutes 60
```

Run it in the repo or worktree under review. The review spec pins shape,
not scope creep: requirement 1 = graded findings with file:line evidence;
a non-negotiation = no edits outside the report; the `check:` gates the
report's existence and structure:

```markdown
check: test -f findings-report.md && grep -q '^## 1\.' findings-report.md

estimate: ~1 changed line (the report)
```

Scope goes in the goal, concretely: "review HEAD", "review the last 3
commits", "review `git diff main..HEAD`". Start from a clean tree (or a
dedicated worktree) so the nothing-else-changed constraint is auditable
with `git status` afterwards.

## 2. What good findings look like

The codex-review shape (`reviews/CODEX-REVIEW-20260928.md` is the worked
example):

- A summary paragraph up top naming what the green suite misses — "the
  tests pass, and here is why that isn't enough".
- Severity-graded sections: `## 1. CORRECTNESS BUGS` →
  `### HIGH — <title>`.
- Each finding carries **Evidence** (`src/foo.rs:817`-style references),
  a concrete **Trigger** (or interleaving), and a suggested test when the
  bug is testable.

## 3. Intake: findings → TODO rows

Triage before filing: drop duplicates and out-of-scope nits, keep
correctness bugs and integrity gaps.

- One TODO row per finding, named for the reviewer — `<finding>
  (<reviewer> review)` — exactly how T140–T143 landed from the codex
  review (e.g. "goal_complete denial bypass (codex review)").
- One spec per row: `specs/t<N>-<reviewer>-<slug>.md`, with a `check:`
  line that pins the fix's test and an `estimate:` line.
- Claims validation couldn't kill become named follow-ups on the row —
  never silently dropped.

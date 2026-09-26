# T44 — Pipeline overlap: item N+1 impl runs while item N validates

check: cargo test

## Concern

LOOP-SPEC's "ONE child at a time" hard rule forces a fully serial pipeline:
impl N (5–35 min) → validate N (10–30 min) → merge → impl N+1. Measured
cycle-14 (fresh eval + 4 items): 56 min serial. But impl children work in
DISJOINT git worktrees and validators are read-only on main — the collision
surface is only the merge step, which the orchestrator already serializes.
Operator directive (2026-09-26): "feels awfully slow — speed it up."

## Repo context

- `LOOP-SPEC.md` §2 hard rules: "ONE child at a time, foreground,
  bounded gates" (amended to backgrounded+polled in 988741d).
- Worktree-per-item isolation means impl N+1 cannot touch item N's files;
  the merge + row-flip + gates sequence in main is inherently serial and
  stays so.
- File-overlap risk: many rows touch the same file (src/tools.rs in 4 of
  the last 10 items). Overlap must be gated on DISJOINT target files.

## Requirements

1. Amend LOOP-SPEC §2: after impl child N completes and its validator
   launches, the orchestrator MAY launch impl child N+1 **iff** the two
   items' spec-named target files are disjoint. Doctrine items (any spec
   touching LOOP/META/META-META/SELF-SPEC or TODO.md format) NEVER overlap.
2. At most 2 children in flight (1 validator + 1 impl). Never 2 impls.
3. Merges stay strictly serial in queue order: N merges before N+1 even if
   N+1 finishes first. A FAIL on N stops N+1's merge until the fix-up arc
   resolves (N+1's branch may need rebase — orchestrator handles).
4. Both children still harvested per T19 before their worktrees are removed.
5. Update the "Hard rules" section to state the new invariant: one WRITER
   per file set, serial merges.

## Tests

- Doctrine item: validation REQUIRED (kimi) — review checks the disjointness
  gate wording, the doctrine-exclusion list, and the serial-merge invariant.
- Acceptance: a subsequent multi-item cycle's events.jsonl shows impl N+1's
  run_start before validator N's goal verdict (overlap observable).

## Out of scope

- Parallel impls of 3+ items; parallel validators; auto-conflict-resolution.

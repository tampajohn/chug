# T43 — EVALUATION.md Outcomes pruning rule + first compaction

check: grep -Fq 'Outcomes keeps the last' LOOP-SPEC.md && test "$(awk '/^## Outcomes/,0' EVALUATION.md | wc -l)" -lt 400 && grep -Fq '### Cycle 5 ' EVALUATION.md && grep -Fq '### Cycle 13 ' EVALUATION.md && grep -Fq '### Cycle 14 ' EVALUATION.md

## Concern

One concern: EVALUATION.md's Outcomes section grows monotonically and every
fresh evaluation must carry the whole file forward — a self-inflicted
context tax on the loop's own handoff document.

## Repo context

- Measured at the cycle-18 evaluation: EVALUATION.md is 65,802 bytes / 625
  lines, of which the `## Outcomes` section is **50,957 bytes / 371 lines
  (77%)**, spanning cycles 5–17. `TODO.md` adds another 30KB of done-row
  narrative. Both grow every cycle by design (T34's per-item Outcomes
  doctrine).
- The cost: a fresh evaluation rewrites EVALUATION.md wholesale (the
  §1–§6 + Handoff body is replaced; Outcomes must be preserved verbatim
  through the rewrite) — ≈13k tokens of output per fresh eval just to carry
  old narratives forward, growing monotonically.
- The narratives are **redundant with git**: every row-flip commit message
  carries the same saga in fuller detail (e.g. `7cb20cc`, `1e82ffb`), and
  every done TODO row names its merge ref. Compacted Outcomes lose no
  information that isn't one `git log` away.
- LOOP-SPEC Phase 3 owns the Outcomes bullet ("EVALUATION.md's **Outcomes**
  section is complete and truthful …"); it says nothing about bounds.

## Requirements

1. **Doctrine (LOOP-SPEC.md, Phase 3):** the Outcomes bullet gains a
   pruning rule containing the exact tokens `Outcomes keeps the last`:
   Outcomes keeps the last 6 cycles in full; older entries are compacted to
   one line each (`### Cycle N (date) — items landed + refs, one-line
   verdict`) at wrap time, with the rule noting the full narrative lives in
   git (row-flip commits + TODO done rows). Every other LOOP-SPEC line
   byte-identical.
2. **First compaction (EVALUATION.md, this row performs it):** cycle entries
   5 through 13 (9 entries, ~250 lines) each become a one-line entry of the
   form `### Cycle N (date) — <one line naming items landed + key refs>`.
   Cycle 14 and newer stay in full. The `## Outcomes` heading, the file's
   §1–§6 + Handoff body, and all non-Outcomes content remain byte-identical.
3. Post-compaction the `## Outcomes` section is under 400 lines (the
   check's bound). The spec's original 200-line bound was estimated at
   cycle-18 authorship ("cycle 14–17 in full plus 9 one-liners lands
   ≈150") — cycles 18–20 then landed ≈256 more full-entry lines before
   this row was worked, so the cycle-21 orchestrator raised the bound
   pre-dispatch: cycles 14–20 in full ≈315 lines + 9 one-liners ≈335,
   and the 400 bound leaves room for this cycle's own wrap entry. The
   doctrine rule (req 1) is what bounds growth long-term: each wrap
   compacts the 7th-newest cycle to a one-liner.
4. Truthfulness: each compacted line must name the cycle's landed items and
   at least one commit ref — verifiable against `git log`.

## Tests

- The spec `check:` line above (worktree-relative; greps + line bound).
- Non-vacuousness: the check fails on the pre-compaction file (371-line
  Outcomes) and on LOOP-SPEC.md without the rule.
- Reviewer verification: `git log --oneline` confirms each compacted line's
  refs exist.

## Acceptance

- Check green in the worktree; `git diff --stat` shows LOOP-SPEC.md +
  EVALUATION.md only.
- Commit message cites the 50,957/65,802-byte measurement.

## Out of scope

- Pruning TODO.md done rows (30KB — a separate, guard-coupled decision:
  the T8 guard reads TODO.md; archive mechanics need their own row if a
  future evaluation files one).
- Changing T34's per-item Outcomes doctrine (entries are still written at
  landing; only their long-term retention changes).

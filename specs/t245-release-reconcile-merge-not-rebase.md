# T245 — release reconcile-after-tag must MERGE (v0.17.4 orphaned by rebase)

check: cargo test --test todo_consistency

estimate: ~60 lines (doctrine sentence + pin legs + row)

## Concern

Cycle-116 wrap evidence: the cycle-115 wrap cut `chore: release v0.17.4`
(47ed9a5) and pushed commit+tag; the operator's b0f602a landed on origin
after; the cycle-116 launch reconcile REBASED the release commit onto the
operator's work (→ dda73ab) and pushed main — leaving the PUBLISHED tag
v0.17.4 pointing at the orphaned pre-rebase commit. Consequences:

- `git describe --tags` from HEAD resolves to v0.17.3 (undercounts every
  describe-based consumer) until the next tag re-anchors.
- `git log v0.17.4..HEAD` (release-notes range) includes the rebased
  release commit itself and mis-scopes the "since last tag" trigger count.
- site-sync's RELEASE region (T240, creatordate-based) is unaffected; the
  version ON main is correct (0.17.4) — this is an anchoring defect, not a
  content defect.

Tags are immutable (published, never moved) — the orphan is permanent
history. The fix is doctrine: prevent the next orphan.

## Requirements

1. LOOP-SPEC's wrap/release doctrine (the Tag-at-wrap clause or Hard
   rules) gains the reconcile rule: when the remote moves after a release
   tag is published, integrate by MERGE, never rebase — a rebase orphans
   the published tag (the cycle-116 v0.17.4 evidence named). If a merge
   is impossible mid-wrap, the release commit stays local until the
   integrate lands (tag pushes with the integrate, still atomic).
2. Name the failure mode in the release trigger's counting sentence:
   the "≥3 items since the newest v* tag" count is computed against a tag
   that must be an ANCESTOR of HEAD — `git merge-base --is-ancestor
   <tag> HEAD` is the sanity check before cutting the next tag; a
   non-ancestor newest tag means the count anchors on the previous
   ancestor tag and the orphan is named in the notes.
3. Pin legs in the appropriate loop_spec pin file (or a new small one):
   (a) the MERGE-never-rebase sentence present in LOOP-SPEC.md;
   (b) the ancestor-check sentence present; (c) each leg RED-provable by
   token removal.
4. The row's own notes name the standing orphan: v0.17.4 → 47ed9a5
   (unreachable from main); v0.17.5's cut re-anchors `git describe`.

## Tests

- The two pin legs RED-proven by token removal and restored.
- todo_consistency stays green (row format).

## Out of scope

- Moving/re-tagging v0.17.4 (immutable published history — forbidden).
- Changing scripts/release-notes.sh anchoring (the creatordate site path
  is unaffected; the script's range note is a docs sentence here, not a
  script edit — a script-side anchor fix is a separate row if wanted).

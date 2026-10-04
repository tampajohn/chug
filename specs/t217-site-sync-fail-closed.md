# T217 — site-sync must fail CLOSED on unreadable repo inputs (never publish fallback zeros)

check: cargo test --test site_sync

estimate: ~150 lines (input guard + pins + doctrine note)

## Concern

Operator 2026-10-03: "stats are broke." chug.sh's STATS region was
gutted to fallback zeros (0/0 items, n/a tests, 0 cycles, n/a eval,
"No landed-item commits yet") by site commit 58c3a0b — a sync that ran
OUTSIDE loopd (no loopd.log invocation, no cycle-log mention) with all
repo inputs unreadable at once: TODO.md, .chug/loopd logs, git log.
The all-inputs-missing signature says the runner's repo path pointed at
an empty or harvested directory (orphan-child vs worktree-removal
race — the T186 class). The sync then committed AND pushed, replacing
159 lines of real stats with fallbacks on the live page. The wrap sync
minutes earlier (2416c44) had written correct stats with the same
code — the difference was purely input readability.

## Repo context

- scripts/site-sync.sh: STATS/TIMELINE/FEATURES regions each have
  fallback text for missing inputs (by design, for a first-ever run on
  an empty repo). The bug: fallbacks are published with the same
  authority as real data.
- loopd runs the sync at wrap (logged "site-sync: rewrote/committed/
  pushed") and cycle start (T98 stats sync) — both with the real repo.
  The rogue writer had commit+push capability to chug-site but no repo
  read access: an orphan/delegate process whose worktree was removed
  (T186 harvest-liveness doctrine; T214 fences queued).
- The fallback zeros look IDENTICAL to a legitimately-empty repo's
  first run — so the guard can't key on the values; it must key on
  input readability.

## Requirements

1. Input guard before any region write: TODO.md unreadable OR
   .chug/loopd absent OR git log empty => the sync writes NOTHING,
   commits NOTHING, prints one named-error line ("refusing to publish:
   repo inputs unreadable at <path>"), exits nonzero. First-ever runs
   on a genuinely empty repo are impossible in loop context (TODO.md
   always exists); if a bootstrap path is needed it takes an explicit
   --bootstrap flag.
2. Per-region independence is NOT required — one global guard (all
   regions read from the same repo).
3. The guard's error path is covered by the existing site_sync fixture
   harness: a fixture with SITE present but CHUG missing TODO.md must
   leave index.html byte-identical and exit nonzero.
4. Doctrine: T98/T99's sync notes gain the fail-closed sentence;
   DEPENDENCIES.md (T203) notes chug-site publish is guarded.
5. Adjacent but in-scope: the commit message records the input stats
   (items/tests/cycles counts) so a gutting commit is distinguishable
   from a real one at a glance.

## Tests

- Fixture: missing TODO.md -> no write, nonzero exit, named error.
- Fixture: missing .chug/loopd -> same.
- Fixture: all inputs present -> regions written, counts in the commit
  message.
- Acceptance: the next rogue-context sync leaves the live page
  untouched (recorded in cycle Outcomes).

## Out of scope

- Hunting the specific orphan that wrote 58c3a0b (T186/T214 cover the
  class); regenerating the current stats (operator may run one manual
  sync — not loop work); retry/backoff (fail-closed means the NEXT
  legitimate sync repairs).

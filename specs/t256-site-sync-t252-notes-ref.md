# T256 — T252 done-row notes lack a resolvable commit ref (site-sync timeline skip)

## Repo context

`scripts/site-sync.sh`'s `done_rows()` (line 1143) scans each done
row's notes cell (TODO.md field $7) for commit candidates of ≥7 hex
chars and `git cat-file -e`-verifies each (line ~1295); a done row
whose notes carry no resolvable ref gets a warn (`T252: done-row
notes cite no commit found in the chug repo (git cat-file) — no
timeline entry`) and NO machine timeline entry on the generated
site. T252's notes cell ("done (orchestrator-direct, cycle 130
wrap-close) — bracketed the live fence call …") carries no hex ref
at all: the row landed orchestrator-direct at the cycle-130
wrap-close and the flip commit (7aa1287, which contains both the fix
and the row flip) was never cited in-band. The warn has fired on
every site-sync run since cycle 130 — 62 fires censused in
loopd.log at the cycle-190 eval, ALL naming T252 (no other row
affected). LOOP-SPEC §3's wrap-truthfulness rule ("every `done` row
has a commit ref") is gapped in-band for this row: the ref exists
one hop away (the cycle-130 Outcomes line names 7aa1287) but the
row itself does not.

Verified at filing (the verify-the-indictment bar):
`git cat-file -e 7aa1287^{commit}` resolves; `git show --stat
7aa1287` shows the T252 fix + row flip in one commit; site-sync's
merge-by-ref rule (a curated entry already citing the ref keeps its
prose, the raw row renders nowhere) suppresses any duplicate, so
adding the ref is safe whether or not a curated entry cites it.

**estimate: ~2 changed lines** (one TODO.md notes-cell edit;
orchestrator-direct — children are forbidden TODO.md edits by hard
rule, so this row can never be dispatched).

## Requirements

- Edit TODO.md's T252 row notes cell to cite the landing commit
  `7aa1287` (e.g. "done (orchestrator-direct, cycle 130 wrap-close,
  commit 7aa1287) — …"), preserving the rest of the notes verbatim.
- The notes cell MUST contain no `|` (the T8 guard splits every row
  on it).
- No other row, file, or cell is touched.

## Tests

- `cargo test --release --test todo_consistency` green after the
  edit (the T8 table guard).
- `grep -q "7aa1287" TODO.md` — the ref present in-band.
- Extraction simulation: the row's notes now yield a ≥7-hex
  candidate that `git cat-file -e` verifies (the warn's precondition
  disappears on the next site-sync run).

## Acceptance

- T252's notes cite 7aa1287; todo_consistency green; zero other
  diff lines.

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --release --test todo_consistency && grep -q "7aa1287" TODO.md

# T293 — repo-root recursive-walk remedy: prune-first/scoped-probe doctrine + pin

## Repo context

Doctrine row (META-META-SPEC.md + one new pin test), pri 4, SOLO (doctrine
items never overlap), kimi validation REQUIRED (the loop/spec doctrine
itself is on LOOP-SPEC step 4's REQUIRED list).

The eval-state open-threads carries the **repo-root recursive-walk 300s
kill** census, whose bullet letter fires at the fifth occurrence: *"a 5th
fire files the row."* The fifth fire landed at the cycle-673 wrap
(counted d1791821960-7): a compose-probe batch ran
`find . -name '*.plist' -not -path './target*'` from the repo root — a
full recursive walk (the `-not -path` clause filters output lines but
never prunes the traversal, so the walk still descends every directory
including `target/`) — and the bash cap killed the process group at 300s
mid-walk. Zero casualty (the batch's earlier legs' output was preserved;
no figure was ever quoted from the killed leg; the plist locate was
re-probed SCOPED and found `com.tampajohn.chug-loopd.plist` at the repo
root in <1s), but the burn is the class the census paces: four prior
fires of the same shape (an unbounded `find .`/`grep -r` over the repo
root dying at the cap).

The census bullet's discipline — "probes scoped to .chug/, tests/*.rs,
named files" — exists as hygiene but has no load-bearing doctrine
sentence and no pin: nothing mechanically greets a cold orchestrator
composing a locating probe. This row executes the bullet's armed letter.

estimate: ~40 changed lines (one doctrine paragraph in META-META-SPEC.md
+ one new pin file tests/loop_spec_scoped_walk.rs + the bullet's remedy
note in the doctrine paragraph itself)

## Requirements

1. **The ban sentence.** META-META-SPEC.md's probe-discipline area (the
   T282 probe-at-write / T284 inspect-first paragraphs — the section
   that already governs how orchestrators compose probes) gains ONE
   paragraph, the **scoped-walk rule**, carrying the literal token
   `prune-first`:

   - Orchestrator probes NEVER run a recursive walk from the repo root
     (`find .`, `grep -r .`, `du .` and kin) inline: the walk descends
     `target/` and the shared caches and dies at the bash cap — the
     repo-root recursive-walk 300s-kill class (5 fires, the cycle-673
     wrap's `find . -name '*.plist'` the fifth).
   - Locating probes take the SCOPED shape first: named directories
     (`ls .chug/`, `ls tests/`, a direct `[ -f path ]` ladder over the
     candidate paths), or `git ls-files` (index-resident, no walk), or a
     bounded `find <named-dir> -maxdepth N`.
   - When a repo-root walk is genuinely unavoidable, it runs
     **prune-first**: `find . -path ./target -prune -o ... -print` (the
     `-prune` stops the descent; a `-not -path` filter does NOT — the
     cycle-673 fire's exact mechanism), still under a bounded window
     when the pruned tree can exceed the bash cap.
   - The paragraph names the census (repo-root recursive-walk 300s kill)
     and the fifth fire's record id d1791821960-7 as its evidence, and
     closes with the class letter: the counted census + this doctrine
     sentence IS the guard; a mechanical walk-detector over probe text
     is the brittle-awk class and stays rejected.

2. **The pin.** New test file `tests/loop_spec_scoped_walk.rs`
   (doctrine-pin shape per the existing `tests/loop_spec_*.rs` pins —
   read the doctrine file, assert the load-bearing sentences):
   - asserts META-META-SPEC.md contains the `prune-first` token;
   - asserts the paragraph names the census (`recursive-walk 300s kill`)
     and the record id `d1791821960-7`;
   - asserts the paragraph carries the ban's mechanism sentence (a
     `-not -path` filter does not prune the traversal).
   RED-proof discipline: the orchestrator RED-proves the pin pre-merge
   by mutating the doctrine sentence in a worktree and watching the pin
   fail.

3. **No helper script.** A `scripts/scoped-find.sh` was weighed and
   DECLINED: the failure mode is composing the walk at all, not the
   walk's internals; the doctrine sentence + the census is the
   proportionate guard at 5 fires. A 6th fire re-weighs the helper.

## Tests

- `cargo test --release --test loop_spec_scoped_walk` green (the new pin).
- `cargo test --release --test loop_spec_doctrine_prune --test todo_consistency`
  green (no doctrine-format drift; the TODO row stays well-formed).
- Full suite green at merge (the row touches no src).

## Acceptance

- META-META-SPEC.md carries the scoped-walk paragraph with the
  `prune-first` token, the census name, the record id, and the
  mechanism sentence.
- `tests/loop_spec_scoped_walk.rs` exists, is RED-proven, and passes in
  the full suite.
- The eval-state open-threads bullet's next refresh names the row's
  landing as the letter's execution (the wrap that merges it updates
  the bullet to the post-letter re-arm: a 6th fire re-weighs the helper
  script).

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a && touch src/*.rs tests/*.rs; set -o pipefail; grep -c 'prune-first' META-META-SPEC.md && cargo test --release --test loop_spec_scoped_walk --test loop_spec_doctrine_prune --test todo_consistency 2>&1 | tail -20

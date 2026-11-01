# T314 — `scripts/scoped-find.sh`: the canned scoped-find helper (the T293 6th-fire re-weigh: LAND)

check: cargo test && [ "$(scripts/scoped-find.sh . -- -name 'com.tampajohn.chug-loopd.plist')" = "./scripts/com.tampajohn.chug-loopd.plist" ]

## Repo context

The T293 scoped-walk rule (META-META-SPEC's probe-discipline area, pinned
by `tests/loop_spec_scoped_walk.rs`) carries the counted census + the
doctrine sentence as the guard, with the mechanical walk-detector rejected
as the brittle-awk class — and an explicit re-arm: "a 6th fire re-weighs
the declined `scripts/scoped-find.sh` helper". The sixth repo-root
recursive-walk 300s kill fired at the cycle-2729 eval's boot: the plist
probe
`find . -path ./target -prune -o -path './target-shared*' -prune -o -name '*.plist' -print`
— prune-first over target/target-shared* but `.git` unpruned and NO
bounded window — descended `.git/`/`.chug/` and the process group was
killed at the 300s bash cap AFTER printing the answer (count 1, the
designed launcher plist). Zero casualty: the answer printed before the
kill, and the scoped re-probe (`ls scripts/*.plist`) confirmed in under a
second. The census moves 5 → 6 and the trigger fires. The failure mode is
composing the walk at all: the six fires span distinct shapes (the
cycle-673 `-not -path` filter, this unbounded prune-first) across
different orchestrators, so the sentence alone does not fully arm a cold
composer. The re-weigh's verdict is LAND: a canned scoped-find makes the
scoped shape the default reach. This is a positive tool, NOT a
detector/gate — the brittle-awk class stays rejected.

estimate: ~110 changed lines (the script ~45 + the pin leg ~35 + the
doctrine amendment ~8 + the pin header comment ~6 + the spec).

## Requirements

1. **`scripts/scoped-find.sh`** (new, executable): the reach-first scoped
   find.
   - Usage: `scripts/scoped-find.sh <root> [--maxdepth N] -- <find-args...>`.
   - It composes
     `find <root> -maxdepth <N> -path <root>/target -prune -o -path '<root>/target-shared*' -prune -o -path <root>/.git -prune -o <find-args> -print`
     with `N` defaulting to 4. (`-maxdepth` is a global option placed
     right after `<root>` — accepted by both GNU and macOS BSD find; the
     T299 compose-time rule binds: no GNU-only flags.)
   - It echoes the exact composed command to stderr (the probe stays
     self-describing) before running it — the bounded window is baked,
     never remembered.
   - Exit status is find's own.
2. **The T293 doctrine amendment** (META-META-SPEC.md, one paragraph
   edit): the census parenthetical moves "five fires counted" → "six
   fires counted" with the sixth fire's evidence named (the cycle-2729
   eval boot's pruned-but-unbounded plist walk, killed at the cap after
   printing count-1, zero casualty), and the closing re-arm clause
   becomes the LANDED verdict — the 6th fire re-weighed and LANDED the
   helper (T314), reach for it first. **The T285 pin-surface
   declaration** (probed at filing: the pin's leg (c) asserts
   `` `scripts/scoped-find.sh` `` EXACTLY ONCE in the probe-discipline
   window; leg (b) asserts lowercase `prune-first` EXACTLY ONCE and the
   SCOPED shapes each once; the negative leg asserts `-not -path`
   EXACTLY TWICE; leg (a) asserts the lead/ban/kin/census-name/record
   `d1791821960-7`/quoted fifth-fire command/zero-casualty note each
   once; leg (c) asserts the mechanism sentence, the GUARD letter, and
   the WALK_DETECTOR_REJECT letter each once): the amendment MUST keep
   the backtick-quoted helper-path token occurring exactly once, MUST
   NOT add a lowercase `prune-first` or uppercase `PRUNE-FIRST`
   occurrence, MUST NOT add a third `-not -path`, and MUST keep every
   other needle's count and the paragraph's placement (after the T284
   probe-hygiene lead, before the `## Write \`EVALUATION.md\`` heading).
3. **The pin** (`tests/loop_spec_scoped_walk.rs`): add leg (e) asserting
   the helper exists at `scripts/scoped-find.sh` (resolved from the
   runtime checkout per the file's T48 idiom), is executable, and its
   text carries the prune coverage (the `/target`, `/target-shared*`,
   and `/.git` `-prune` clauses) and the bounded window (`-maxdepth`
   with a default). The battery's loop_spec_scoped_walk count moves
   4 → 5 (the wrap battery enumeration moves with it — quoted per-wrap,
   not pinned). The file's header comment keeps the fifth-fire narrative
   (historical evidence) and gains one sentence naming the sixth fire
   and the T314 landing (comment accuracy, not asserted).

## Tests

- The new leg (e) green + the existing 4 legs green (5/5).
- RED-proof (the orchestrator, pre-merge): delete the script → leg (e)
  red; strip the `.git` prune clause → red; strip the `-maxdepth`
  default → red.
- The doctrine amendment keeps the whole pin green (the count legs
  verified by running the target after the edit).

## Acceptance

- `scripts/scoped-find.sh` exists, is executable, is BSD-clean, echoes
  the composed command to stderr, and its smoke
  `scripts/scoped-find.sh . -- -name '*.plist'` returns the launcher
  plist in seconds (never the 300s walk).
- Pin 5/5 green; the META-META-SPEC census reads six fires with the
  sixth named; the LANDED rewording keeps every prior needle's count.

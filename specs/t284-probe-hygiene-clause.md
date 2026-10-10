# T284 — Probe hygiene: exit codes and accessor shapes (META-META-SPEC clause)
estimate: ~30 changed lines (one ~12-line META-META-SPEC.md paragraph; doctrine, md-only)

check: cd "$(git rev-parse --show-toplevel)" && export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && touch src/*.rs tests/*.rs && grep -q 'Probe hygiene (T284' META-META-SPEC.md && grep -q -- '|| true' META-META-SPEC.md && grep -q 'keys:' META-META-SPEC.md && grep -q 'Corpus counts (T265)' META-META-SPEC.md && grep -q 'Corpus counts at wrap (T281)' META-META-SPEC.md && grep -q 'Outcomes-entry figures (T282)' META-META-SPEC.md && cargo test --release --test todo_consistency --test eval_outcomes_carry --test eval_state_delta --test loop_spec_cheap_exit --test loop_spec_check_wall --test loop_spec_doctrine_prune --test loop_spec_eval_read_path_pointer --test loop_spec_recovery --test loop_spec_validation_lane --test internal_info_lint

## Repo context

- Two counted census classes share one root — a probe composed from
  habit instead of inspection — and neither has a doctrine home:
  - The **grep-exit-1 census** (53 instances at trip 91, the 52-row
    trigger crossed): `grep -c` exits 1 when the count is ZERO, and a
    bare `grep` exits 1 on no-match — when that leg is a chain's last
    command (or `&&`-chained), the call's exit code is 1 and the events
    flags record `ok:false` for a leg whose data WAS the intended
    answer (0 todo rows IS the drained queue; 0 open FEATURES items IS
    the empty roadmap). Trip 91 alone fired three: the TODO `| todo |`
    count probe, the FEATURES open-scan interior `grep -viE` no-match,
    and the FEATURES unchecked-box `grep -c` zero count.
  - The **jq-probe-slip family** (8 sightings at trip 91, accessor axis
    at 4): a jq projection composed from schema MEMORY — `.content`
    where the field is `.preview`, `.iteration`/`.verdict` where the
    event carries neither — exits 0 printing `null` or `?` for every
    leg (valid jq, wrong data), so the slip never trips the ok:false
    flag and surfaces only when the output is read as nonsense. The
    fix is always the same: inspect the shape first
    (`jq 'keys:'`-style, or the event-type filter the T272 clause
    already pins), THEN project.
- The T272 clause (`**Enumeration by flag, never recall (T272,
  enumeration-by-flag).**`, immediately above the corpus list in
  META-META-SPEC.md) pins WHERE whole-stream enumerations come from but
  not the per-probe composition discipline; the T265/T281/T282 clauses
  pin figure provenance. The gap both censuses live in is the probe
  itself: its exit-code masking and its accessor provenance.
- The jq family's 10th-sighting trigger for a jq-idioms row stays
  unmet (8 < 10); this row lands anyway because the grep census's
  52-row trigger DID cross, and the two classes share the one root —
  the paragraph disciplines both in one writing (the parked jq
  accessor-shape instance lands with it, per the trip-90 note that the
  first parked instance rides the grep census row).
- **Pin-carrier discipline (T80):** META-META-SPEC.md is a pinned
  doctrine carrier — NINE test binaries read it (enumerated
  mechanically from `grep -l META-META-SPEC tests/*.rs` at filing):
  `tests/todo_consistency.rs`, `tests/eval_outcomes_carry.rs`,
  `tests/eval_state_delta.rs`, `tests/loop_spec_cheap_exit.rs`,
  `tests/loop_spec_check_wall.rs`, `tests/loop_spec_doctrine_prune.rs`,
  `tests/loop_spec_eval_read_path_pointer.rs`,
  `tests/loop_spec_recovery.rs`, `tests/loop_spec_validation_lane.rs`,
  plus `tests/internal_info_lint.rs` walking every tracked file. The
  new paragraph is append-only, must NOT quote any existing pinned
  needle verbatim, and the T265/T281/T282 paragraphs' bold-lead needles
  must survive intact (the check's survival greps — T282's paragraph
  lands before this row in queue order, so its lead is pinned here).

## Requirements

1. Add ONE paragraph to `META-META-SPEC.md`, placed immediately after
   the `**Enumeration by flag, never recall (T272,
   enumeration-by-flag).**` paragraph, bold-led
   `**Probe hygiene (T284, exit codes and accessor shapes).**`,
   carrying both roots:
   - **Exit-code root:** an intentional emptiness or no-match probe —
     a `grep -c` that may count zero, a bare `grep`/`grep -viE` that
     may match nothing — is NEVER a chain's last command unmasked: it
     carries `|| true` (the `;`-masked or `|| true` shape), because
     grep's exit 1 on zero-count/no-match turns a correct empty answer
     into an `ok:false` events flag and, at a chain's tail, a failing
     call. The 53-instance grep-exit-1 census is the evidence class.
   - **Accessor-shape root:** a jq projection is composed from the
     event's INSPECTED shape — a `keys:` (or equivalent shape) probe
     first — never from schema memory: a guessed accessor exits 0 on
     wrong data (every projection `null`), so the slip is invisible to
     the flags and only reading the output catches it. The 8-sighting
     jq-probe-slip family is the evidence class.
   - One sentence naming the shared root: both classes are a probe
     composed from habit instead of inspection — mask the exit code
     you expect, inspect the shape before you project.
2. No other repo file changes (this spec and the TODO row are the
   orchestrator's bookkeeping, not the child's).
3. The paragraph must not duplicate any existing META-META-SPEC
   sentence verbatim (the exact-count pins), must not alter the
   T265/T272/T281/T282 paragraphs, and must name no
   operator-identifying paths (the internal-info lint).

## Tests

- The `check:` line's six greps (the new bold lead, the `|| true` and
  `keys:` root needles, and the T265/T281/T282 survival greps).
- Every pin binary that reads META-META-SPEC.md green (the `check:`
  line's nine `--test` legs + the lint) — the T80 pinned-carrier
  discipline for an md-only edit.

## Acceptance

- META-META-SPEC.md carries the probe-hygiene paragraph; `check:`
  green.
- Review/merge gates per the docs-only classification (the diff touches
  ONLY `*.md`): the T80 guard floor
  (`cargo test --release --test todo_consistency`) PLUS the pin
  binaries named above; the full suite skipped per the T80 letter.
- Validation: kimi REQUIRED (doctrine edit — META-META-SPEC.md is on
  the step-4 core list by construction).

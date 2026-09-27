# T96 — META-META-SPEC spec quality bar: the `check:` filter must run every test the change adds

check: grep -q 'broad enough to run every test the change adds' META-META-SPEC.md && grep -q 't90' META-META-SPEC.md

## Repo context

Cycle-53 eval §2 I5 (t90 validator finding 2, cycle-52 Outcomes): T90's
spec check was `cargo test --bin chug permissions` — the plural filter
missed the 5 strongest driver-integration legs, and a mutant SURVIVED
under the spec's own check while dying under the broader `permission`
stem. This is the third lesson in the spec-quality-bar class:
worktree-relative checks (T30), no `cargo test --lib` (T67-era, nine child
streams bit). Lessons in this class are per-spec-author, not per-spec —
they belong in the quality bar in META-META-SPEC.md's "Extend TODO.md"
section, next to the two existing sentences (the worktree-relative
sentence with its T21/T26 bites, and the `--lib` sentence with its
t22…t64 bites). T30/T33 precedent: META-META-SPEC.md is editable work
doctrine (not a human-spec file); one hunk, everything else byte-identical.

## Requirements

1. ONE sentence (two at most) added to the spec quality bar in
   META-META-SPEC.md's "Extend TODO.md" section, immediately after the
   `--lib` sentence, carrying BOTH needles the check greps:
   the principle — a cargo-test `check:` filter MUST be broad enough to
   run every test the change adds (prefer the module stem over a narrower
   substring; verify by running the filter and confirming the new tests
   are in the run set) — and the `t90` bite citation (the `permissions`
   filter missed the driver-integration legs the `permission` stem
   caught; a mutant survived).
2. Every other line of META-META-SPEC.md byte-identical, including the
   T30 and `--lib` sentences and their citations.
3. No other file touched (no TODO-format change, no LOOP-SPEC change).

## Tests

- Content check per the `check:` line (both needles).
- `cargo test --test todo_consistency` green (the doctrine-token guard
  must not trip on the new sentence — if it pins quality-bar text, update
  the pin minimally and prove it RED against the pre-edit file).
- Full `cargo test` green as the sweep for any other pin on the
  quality-bar paragraph.

## Acceptance

- `check:` green verbatim in the worktree; full suite green.
- Diff is META-META-SPEC.md (+ at most a swept pin file) ONLY.

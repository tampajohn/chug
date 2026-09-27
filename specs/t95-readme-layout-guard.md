# T95 — README Development layout gains `permissions` + module set-equality guard

check: grep -q 'permissions' README.md && cargo test --test readme_layout

## Repo context

Cycle-53 eval §6(c): README.md's Development layout line lists 27 modules
in its `src/{…}` braces; `ls src/*.rs` minus `main.rs` has 28 —
**`permissions` is missing** (T90 added `src/permissions.rs` and its
README ## Permissions section but not the layout line). This is the T86
class (T86 added tgrep/plan/hooks by hand, verified 27/27 by two humans) —
the SECOND sighting, so the class has earned automation: a mechanical
set-equality guard so the next module addition cannot silently drift.

## Requirements

1. README Development layout braces list gains `permissions`
   (alphabetical position, matching the existing sort order: …
   `observ,permissions,plan,…`). Every other byte of the line unchanged.
2. New `tests/readme_layout.rs`: parse the README Development section's
   `src/{...}` braces list (it wraps across a newline — parse
   newline-tolerantly), split on commas, trim, and assert SET EQUALITY
   with the filesystem set: every `src/*.rs` file stem except `main`.
   The test resolves the repo root at RUNTIME (the T48 doctrine —
   `env!(CARGO_MANIFEST_DIR)` is FORBIDDEN, `tests/no_compile_time_manifest_dir.rs`
   pins that; follow the existing runtime-root idiom used by the other
   guard tests).
3. Failure message names BOTH directions: modules missing from README and
   README entries with no file (so the next drift diagnoses itself).
4. The guard is one concern, one file, std-only (no new dependencies).

## Tests

- The guard itself, green post-fix.
- RED proof recorded in the commit message: revert the README edit (drop
  `permissions`) → the guard fails naming `permissions` as missing from
  README; add a phantom README entry → fails naming it as file-less.
- `tests/no_compile_time_manifest_dir.rs` stays green (runtime-root
  idiom).

## Acceptance

- `check:` green in the worktree; full suite + clippy green.
- Diff: README.md (one word) + tests/readme_layout.rs (new) ONLY.
- NOT md-only (a `.rs` lands) → full gates at review and post-merge per
  LOOP-SPEC §2 step 3's classification.

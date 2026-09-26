# T41 — Sandbox-candor pass: filesystem tool descriptions name the refusal; README names web_fetch's exception

check: cargo test

## Concern

One concern: the cwd-confinement of the filesystem tools is under-documented
on the surfaces models actually read (tool descriptions) and the README's
"one documented exception" line went stale when web_fetch landed — both are
candor gaps on the sandbox story.

## Repo context

- The filesystem tools refuse paths outside the run cwd
  (`src/tools.rs:1151-1180`, error `path escapes cwd: <path>`). But the
  schema descriptions undersell it: `read_file`'s path is "File path
  relative to cwd" (`src/tools.rs:65`), `edit_file`'s is "File path relative
  to cwd" (`:78`), `glob`'s base is "Optional base directory relative to cwd
  (must stay inside cwd)" (`:128`); only `write_file` warns "(must stay
  inside cwd)" (`:90`). None names the refusal or the escape hatch.
- **The bites (cycle-16, `.chug/events-20260926-043119.jsonl`):** the
  orchestrator hit `path escapes cwd` twice in one run — `read_file
  /tmp/chug-loop-t37/src/webfetch.rs` (reviewing a child worktree) and
  `write_file /tmp/eval-head.md` (scratch space) — each costing an iteration
  plus a bash-heredoc workaround. The cycle-16 eval logged the class as a
  watch item ("fired live on THIS eval"); it has now fired in two
  consecutive evaluations. T22 precedent: the tool description is the only
  universal surface (impl children never read META-SPEC).
- README Tools intro: "All paths sandboxed to `--cwd` (`delegate` is the one
  documented exception …)". Since T37, `web_fetch` also reaches outside the
  sandbox by design — its own paragraph says so ("Like `delegate`, it
  reaches outside the cwd sandbox by design — it is network, not
  filesystem"). A cold reader sees a contradiction.
- T22 (`6068654`) is the template: a description change pinned by a test
  asserting the LIVE `tool_schemas()` output (not a copied literal), so
  future drift fails the suite.

## Requirements

1. `read_file`, `edit_file`, `write_file`, `glob` (+ `list_dir` if its
   wording admits outside paths) gain candid wording in their schema
   descriptions: paths outside the cwd are **refused** (`path escapes cwd`),
   and cross-tree reads/writes (e.g. a child's `/tmp` worktree) go through
   `bash`. Uniform short clause; the existing wording's load-bearing tokens
   (caps, defaults, `offset`/`limit` notes) stay intact.
2. The error string `path escapes cwd` itself is unchanged (tests pin it at
   `src/tools.rs:1657-1661`).
3. README Tools intro: the parenthetical is corrected to name both
   exceptions — `delegate` (absolute child-worktree paths) and `web_fetch`
   (network, not filesystem) — one edit, the rest of the sentence
   byte-identical.
4. No behavioral change to any tool; no other file touched.

## Tests

- T22-style pins on the LIVE `tool_schemas()` output: each touched
  description contains the refusal naming `path escapes cwd` (or equally
  explicit "refused" wording) and the `bash` escape hatch; sentence-count /
  position assertions per T22's pattern where practical.
- A pin that the README sentence names both `delegate` and `web_fetch`
  (repo-file test, mirroring existing README-pin style if one exists;
  otherwise a plain `read_to_string` assertion in the tools test module).
- Existing description pins (T22's bash note, web_fetch's description, the
  delegate schema) keep passing unmodified.

## Acceptance

- `cargo build`, `cargo clippy --all-targets -- -D warnings`, `cargo test`
  all green.
- Mutation-ready: reverting any one description to its pre-T41 text fails at
  least one test.
- Commit message cites the two cycle-16 `path escapes cwd` sightings.

## Out of scope

- Relaxing the confinement itself (a security-posture decision for the
  operator; this row is candor only).
- Scratch-space doctrine (use `.chug/` for scratch — the orchestrator's
  option today; no code change needed).

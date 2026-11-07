# T333 — `chug loop` subcommand: init + run the loop in ANY repo, no clone

estimate: ~200 changed lines (src/cli loop subcommand + embedded scaffold templates + loopd exec wiring + docs/adopt quickstart rewrite + fixtures)

## Why

Operator, 2026-11-07: "the tutorial says to clone chug for loop, we should
just build that in as a sub command in chug. Also cloning would have it
start chugging on chug, not a loop md or anything else."

Two adoption-blocking flaws in the T329 quickstart:
1. It tells pilots to `git clone` the chug repo to get `loopd.sh` — the
   install.sh binary should be sufficient.
2. Worse, the clone path starts the loop on CHUG'S OWN TODO.md — a pilot's
   first experience is the loop consuming chug's backlog instead of the
   pilot's repo. Exactly backwards for adoption: the loop must scaffold
   and run against the CURRENT repo's own queue.

## What

1. **`chug loop init`** (new subcommand, run in the target repo):
   - Scaffolds: `LOOP-SPEC.md` (a GENERIC minimal template — the queue
     rules, the check:-line contract, the gate expectations; NOT chug's
     self-referential doctrine/pins, which are chug-repo-specific),
     `TODO.md` (header + empty queue + the `## Proposed` lane header from
     T326), `.chug/` runtime dir, and appends `.chug/` to `.gitignore` if
     absent.
   - Idempotent: existing files are never clobbered; each scaffolded file
     is reported as `created` / `kept`. `--force` overwrites with a backup
     (`.bak`) beside each replaced file.
2. **`chug loop run`** (same repo):
   - Verifies the scaffold (missing → prints `run chug loop init first`
     and exits 2, no half-starts).
   - Execs the supervisor against the CURRENT directory: the loop's queue
     is this repo's TODO.md, worktrees live under this repo, commits land
     on this repo's branches. Implementation choice (embed loopd.sh as a
     binary asset vs exec from an install-time share dir) is the
     implementer's, but the contract is: after `install.sh`, NO clone of
     tampajohn/chug is ever required.
   - Environment resolution identical to today's loopd (profile.env via
     T330; CHUG_DECISION_PROVIDER via T332).
3. **docs/adopt/ quickstart rewrite** (rides this item, same PR): the
   10-minute start becomes `install.sh --profile <name>` →
   `cd <your-repo> && chug loop init && chug loop run`. Delete the clone
   step everywhere (adopt page, README quickstart).

## Tests

- init fixture in a temp dir: creates the four artifacts; second run
  reports `kept` for all; `--force` writes backups.
- Scaffold validity: the generated TODO.md parses under the
  todo_consistency rules; the generated LOOP-SPEC.md contains the
  check:-contract sentence.
- run fixture: missing scaffold exits 2 with the init hint; with
  scaffold, the supervisor is invoked with cwd = the temp repo (assert
  via a stubbed exec).
- Guard test: running `chug loop run` inside the chug repo itself still
  uses chug's own files (no behavior change for the existing setup).

## Out of scope

- Porting loopd.sh to Rust (the subcommand may exec the script; a Rust
  rewrite is a separate item if ever wanted).
- Multi-repo/fleet operation of one loop (single-repo per run, as today).
- The imem pilot kit text update (operator-side, follows the release).

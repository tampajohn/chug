# T346 — release pipeline: every run since v0.17.28 cancelled; GitHub releases stop at v0.17.27

estimate: ~60 changed lines (release.yml + possibly loopd release-cadence logic + fixtures/notes)

## Why

Discovered 2026-11-09 while updating K7: GitHub releases stop at v0.17.27
(2026-11-04) although the loop has tagged v0.17.28, v0.18.0, v0.19.0,
v0.19.1, v0.20.0, v0.21.0 since. Every release.yml run for those tags is
`cancelled` (3–16 min in, actor tampajohn). install.sh follows
`releases/latest/download`, so **pilots installing today get v0.17.27 —
missing install profiles, the loop subcommand, the adopt page, the meta
pack** — the entire adoption path this quarter depends on.

## What

1. **Root-cause first** (commit the finding in the PR body): every run
   cancelled minutes after tag push while the loop pushes eval commits on
   a minutes cadence. Candidate mechanisms to check in order:
   - the loop's own release-cadence logic (T245 recounts, "NO TAG rides"
     dispositions) deleting/re-pushing tags — a deleted tag cancels its
     in-flight run;
   - an API actor (the loop's gh token) cancelling runs outright;
   - workflow-level settings (no concurrency block exists today — rule
     this IN as the fix, not the cause).
2. **Fix per the finding**, and additionally:
   - add `concurrency: { group: release, cancel-in-progress: false }` to
     release.yml so no future mechanism starves a release build;
   - if the loop deletes/re-pushes tags, that behavior changes: a pushed
     tag is final (a wrong tag gets a NEW patch version, never a moved
     one) — encode in LOOP-SPEC.
3. **Backfill**: once fixed, re-trigger release builds for v0.21.0 (and
   v0.18.0/v0.19.x/v0.20.0 if their assets matter) so `releases/latest`
   is current — and verify `install.sh` on a clean box installs a binary
   whose `chug capabilities` shows the meta features.

## Tests

- The fix's PR includes the cancelled-run IDs as evidence and the
  post-fix run id as proof (green run, assets present).
- loopd fixture if the cadence logic changes: a tag once pushed is never
  deleted/moved by the release leg.

## Out of scope

- CI minutes/cost policy (a green release per tag is the requirement;
  cadence of TAGS is the loop's existing business).
- Publishing the meta pack to any marketplace (P004's lane).

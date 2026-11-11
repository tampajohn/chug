# T364 — release workflow: apt leg must survive azure mirror DNS failures (pin fallback + retry/backoff)

estimate: ~40 changed lines (.github/workflows/release.yml only)

## Why

Fleet Primary diagnosed 2026-11-11 (run 43045899125): the
aarch64-unknown-linux-gnu release leg failed TWICE on the same
infrastructure failure — `Temporary failure resolving
'azure.archive.ubuntu.com'` for ~19 minutes across jammy apt loops. The
mirror file (/etc/apt/apt-mirrors.txt) prioritizes the azure mirror on
GitHub-hosted runners, so one DNS wobble kills an entire release while
the code is verified good. A release pipeline that can be blocked by a
mirror's DNS is a pipeline with a single point of failure it doesn't
need.

## What

1. **Pin the fallback**: before any apt invocation in the workflow,
   rewrite the apt mirror list to `archive.ubuntu.com` (or set
   `Acquire::http::Proxy` / sources directly) so the azure mirror is
   never the single path. The azure mirror is a speed optimization;
   correctness must not depend on it.
2. **Retry with backoff** on apt update/install steps: 3 attempts,
   10s/30s/90s, failing only after all three (a 2-minute DNS wobble
   must not fail the build).
3. **Comment the incident**: one line in the workflow citing run
   43045899125 so the next reader knows why the pin exists.

## Tests

- The workflow's apt steps render with the pin + retry (yamllint/shell
  check in CI itself).
- A simulated DNS failure fixture (hosts override in a test job or a
  documented manual verification) shows the retry path engaged.
- Acceptance: the v0.26.0 rerun (or next release) completes the
  aarch64 leg — cite the run id in the done notes.

## Out of scope

- Re-cutting v0.26.0 (the tag is fixed; its release completes via the
  DNS-recovered rerun — this item hardens FUTURE releases).
- Other runner architectures (apply the same pin wherever apt runs in
  the file).

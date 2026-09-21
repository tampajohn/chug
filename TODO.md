# TODO

| id | title | spec | pri | status | notes |
|----|-------|------|-----|--------|-------|
| T1 | API retry survives endpoint restarts | specs/t1-api-retry-endpoint-restart.md | 1 | todo | 2026-09-20: two meta sessions died to transient muse resets (conn reset ~30s) — chug retries only ~15s (4 attempts, 1-8s backoff). Connection-level errors (reset/refused/timeout) should retry for minutes (e.g. 8 attempts over ~5 min); 4xx stays fail-fast |
| T2 | Per-request activity timeout on LLM calls | specs/t2-llm-activity-timeout.md | 2 | todo | 2026-09-20: a round-2 child hung 5+ min at 0% CPU on a stale HTTP connection; only the 600s read timeout saved it (and the wedge-kill got there first). Add ~180s no-bytes timeout distinct from total read timeout |
| T3 | Fresh run detects spec/ledger mismatch | specs/t3-stale-ledger-detection.md | 2 | todo | 2026-09-21: a fresh `chug run --spec SPEC-9` in the repo inherited the OLD SPEC-5 meta's LEDGER.md ("goal met") because ensure_seeded skips when a ledger exists. New runs should compare the goal/spec against the ledger's project and archive+reseed on mismatch (or always seed a run-scoped ledger) |

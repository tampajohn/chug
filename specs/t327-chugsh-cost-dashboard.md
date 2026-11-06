# T327 — chug.sh cost dashboard: daily Langfuse spend in a COST region

estimate: ~110 changed lines (scripts/site-sync.sh COST region + docs.html/index.html render + fixtures)

## Why

The loop's cost story is its strongest public proof (T258/T259 took empty
days to near-$0; verified 2026-10-13 at $38 for a half-day) — and it is
invisible. Langfuse on K7 has the authoritative numbers; chug.sh shows none
of them. The operator reads spend by hand-querying ClickHouse. A public,
self-updating cost region dogfoods the observability story and makes the
cost program's effect legible to anyone evaluating the harness.

## What

1. **Collector** (in `scripts/site-sync.sh`, runs on K7 where Langfuse is
   local): query ClickHouse `observations` for the chug project, last 14
   days, per-day `sum(total_cost)` + call count + per-model split
   (kimi/glm/laya-local). Exact recipe (verified by hand 2026-11-06):
   `docker exec langfuse-clickhouse-1 clickhouse-client -q "SELECT
   toDate(start_time), provided_model_name, count(), sum(total_cost) FROM
   observations WHERE project_id='chug' AND start_time >= now() - INTERVAL
   14 DAY GROUP BY 1,2"`. Fail-closed (T217 letter): docker or CH down →
   emit the T216 placeholder, never a fabricated zero.
2. **COST region** (new region marker pair, same mechanics as STATS):
   - index.html: headline tile — yesterday's total, 7-day average, 14-day
     sparkline (the existing sparkline idiom from the mod, pre-rendered
     characters, no JS), and the current TRUE streak from the latest
     cheap-exit disposition line.
   - docs.html: per-day table (date, calls, kimi $, glm $, total $) for the
     14 days, generated like the RELEASE region.
3. **Freshness**: region regenerates on every site-sync run (hourly cron);
   the tile shows `as of <ts>` like the STATS region.

## Tests

- site-sync fixture battery: canned CH output → region renders correct
  totals/sparkline/table; empty CH output → placeholder; CH unreachable →
  placeholder + sync otherwise succeeds (the region must never break the
  deploy).
- The placeholder path leaves any previously published COST region content
  untouched (stale beats blank).

## Out of scope

- Langfuse API keys / public Langfuse access (site stays static; numbers are
  baked at sync time).
- Fleet/other-project costs on the public site (chug project only — the
  fleet project's numbers are internal).
- Alerting (a cost tripwire is a fleet item, not a chug.sh page).

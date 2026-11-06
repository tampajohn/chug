# P006 — Langfuse adoption dashboard (internal)

- **User-visible value**: per-team visibility into pilot usage — loops running, cycles, flips, spend per team — the evidence feed for the Q4 review and the early-warning system for a pilot burning money.
- **First slice**: Langfuse project-per-team convention + one dashboard (Langfuse boards or Grafana over ClickHouse) with team/day spend and cycle counts.
- **Estimate class**: M (mostly dashboard assembly; the T327 collector query is the recipe).
- **Source fact**: Q4 goal deliverable #6; extends the T327 cost work from self-telemetry to multi-tenant telemetry.

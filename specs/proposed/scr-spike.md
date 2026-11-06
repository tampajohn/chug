# P003 — SCR spike (SGLang suffix-cache-reuse)

- **User-visible value**: 65% of baseline prefix-reuse FLOPs on mid-context-edit workloads — directly relevant to chug's LIVE_CTX editing path (T192 lineage) if it ports.
- **First slice**: operator-approved muse pause window on a spark box, then the SGLang 0.5.16 monkeypatch smoke test against Qwen3.6-27B.
- **Estimate class**: M (a spike window plus writeup; porting beyond the spike is separate).
- **Source fact**: blocked on spark GPU headroom since 2026-10-01; co-tenant headroom measured 7-9GB/box, insufficient without a muse pause.

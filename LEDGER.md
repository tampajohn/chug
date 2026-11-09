# Ledger

## Done
- Boot + disposition: staleness-forced NON-TRIP real eval on kimi (740th); routing d1794183647-1; single-driver; cycle-3151 closed fully
- **FORENSICS COMPLETE — the canonical window re-derived (supersedes the staging-mirror reads)**:
  - `.chug/digest/` was a FOREIGN/STALE staging mirror (divergent 11-07/11-08 per-file timeline — e.g. events-20261108-053400 "kimi 200" exists ONLY there; the canonical 053400 rides 11-07), vanished ~00:20:41Z (external; grep-proven ZERO references in src/scripts/tests/loopd.sh — the loop never touches it). No row; the eval body names it for the operator
  - The AUTHORITATIVE corpus = the canonical `.chug/events*.jsonl` + `.chug/eval-digest.md` (00:04:17Z, generated from the canonical archive, pre-flatten): every window figure RE-DERIVED from it; closes EXACT
- The TRUE window (740th): 77 new stream files / 5,314 iters + 17 child harvests / 1,216; digest CONCURS 110,652+5,314+1,216==117,182 EXACT; files 1,711 (digest 1,710 + my 00:06:29 rotation, ls-re-verified post-flatten)
  - 713th = events-20261107-053400 (the 739th's OWN kimi stream, 200/18,252, goal-none iter-budget; the 3073 wrap zombie-repaired per T255) — 102-bound FIRES
  - 714th = events-20261107-060214 (the zombie-repair segment, glm 64/1,327 accepted)
  - 715th-789th = 75 corridors (cycles 3074-3151, all glm): 72 accepted + 3 goal-none deaths [716th 104631 (200/12,190), 717th 142551 (200/10,822), 723rd 213748 (200/9,472) — all iteration-budget, zombie-repaired, chain intact]; the 718th (164504, 145/7,719) carried the T331-arc mid-gate rejection (accepted 1/rejected 1)
  - 102-bound census 58 -> 67 (NINE fires: 713th + 715th/124 + 716th/200 + 717th/200 + 718th/145 + 722nd/107 + 723rd/200 + 724th/115 + 725th/142); non-fire streak reads 64 (726th-789th)
  - Quiet top MOVES 139 -> 142 (the 725th, events-20261107-234310, 142/2,612 [22:52:56->23:36:28], glm accepted QUIET routine corridor)
  - ok:false aggregate 340 across the 77 (713th 10 + 714th 9 + 74-rotated 313 + 3151's 8), all corridors self-enumerated designed 0-REAL
  - ctx aggregate 58 (54 rotated + 3151's 2 + 713th/714th 1+1); trim aggregate 28; strict post-reset trim streak reads 9 at window end (last reset events-20261108-212218; 8 files + the 3151 trim-0); MY stream fired trim 00:20:05.726Z -> streak RESETS at the 790th
  - THREE cheap-exit dispositions interleaved (cycles 3074-3151 = 78 numbers, 75 streams)
- Backstop: 706th closes EXACT (3054 stream 100/2,316, events-20261106-121352, run_start 2026-11-06T11:03:05.904Z count-1, datum CONCURS; fill prefix 96/2,245.604 <= 100/2,315.846; ok:false 4==4 designed; ctx 1/0; trim 0; accepted; NON-fire; correct-accounting 519->520; INEXACT HOLDS 8); 707th hands off (3058 stream 98/2,263, events-20261106-132328, run_start 2026-11-06T12:13:52.264Z count-1)
- Re-arm 638th DISCHARGED EXACT (3012 slot 96/2,776, events-20261105-152533, datum 96/2776.298 CONCURS, count-1; pairing [638+102==740]; (b) NEGATIVE, watch HOLDS 69 the THIRD reading, the 315th discharge); 1 rides [639th = 3016 stream 79/4,733, events-20261105-171528 verified 79/4733.155 count-1; (b) NEGATIVE projected, watch HOLDS 69 the FOURTH reading]
- **Phantom-routing-id finding (SECOND sighting — my first-sighting claim corrected per T70)**: the 739th eval body mis-recorded its routing id (phantom d1793993436-1; TRUE d1794010784-1 at corpus line 6567); the phantom outcome d1794028828-9 + the T70 re-target supplement d1794030001-1 (subject = the TRUE id, landed-clean) were BOTH landed at the 3073 wrap — the substance was ALREADY adjudicated; my triage record's "first sighting / correct forward at THIS wrap's batch" framing corrected forward at the eval body; DECLINED stands (no row; watch: a SECOND phantom files the batch-probe row) — eval-triage d1794184625-3
- README W46 cold-read audit COMPLETE: layout 47/47 EXACT, links 5/5+3 aux exist, quickstart truthful; ONE finding — Tools headline enumeration omits mcp_resource -> T338 FILED (specs/t338-readme-tools-mcp-resource.md, pri 4, ~15 lines); eval-triage d1794184621-2
- T328 re-verified BLOCKED (~/workspace/chug-watch absent)
- Boot probe `60 0` machine==TRUE 0 (the TRUE 1,756th); roadmap SKIP the 608th; FEATURES unchecked count-0

## Next
1. Write EVALUATION.md body (740th, the canonical figures above) + splice Outcomes verbatim
2. Pins battery (T242 bg, canonical 12-target) + commit eval (routing + 2 triage + T338 row + spec) + push
3. Phase 2: T338 dispatchable (docs-only, ~15 lines — glm impl child) OR zero-dispatch note per budget; T328 stays blocked
4. Wrap corridor: wrap-state note → gates T242 bg → Outcomes 3152 + one-line 3146 → audit+batch (routing backfill + TRUE-739th phantom correction backfill + 2 triage) → state rewrite (640th->639th heal) → token-free notes + T257 color → probe → fill (asserts) → push → flips → goal_complete

## Blockers
- none

# T331 — risk-gate story page (docs + site section): why it's safe to let the loop write code

estimate: ~90 changed lines (docs/risk-gate.md + site-sync render + adopt.html cross-link)

## Why

Q4 goal deliverable #3: "risk-gate story." Company adoption dies at the
first engineering-leadership review without a crisp answer to "what stops
it from doing something bad?" The harness already HAS the answer — it is
just scattered across LOOP-SPEC, FEATURES (F13/F15 Laya risk-gate
precedent), the validator design, and the mutation-testing batteries. One
page, written for a skeptical staff engineer, is the deliverable.

## What

1. **docs/risk-gate.md** (source of truth) + rendered section on the site
   (same docs-page pipeline), linked from adopt.html's safety FAQ entry.
   Sections:
   - **The gates**: clippy -D warnings, nextest full suite, python battery,
     todo_consistency pins — what runs, when, and what "green" means
     (unpiped exits, the T217 fail-closed letter).
   - **The validators**: kimi validation passes, mutation testing
     (RED-proven mutants, survivor adjudication), family-independence rule
     (author model never validates its own work).
   - **The decision gate**: Laya/Jev risk-gate precedent (F13/F15) — typed
     verdicts before consequential actions; the decision-log corpus as
     evidence.
   - **The cost guardrails**: cheap-exit (T258), dormancy (T325), the T247
     valve — the loop cannot spend unboundedly while idle.
   - **The history**: the 2026-10 public-repo scrub and the T249
     internal-info lint as the standing redaction guard.
2. Tone contract: every claim cites the mechanism (file, feature id, or
   gate name) — no adjectives without a citation. The audience is the
   reviewer looking for the hole.

## Tests

- site-sync fixture: risk-gate.md renders; missing source → placeholder,
  deploy succeeds.
- docs link-check: every mechanism citation resolves to a real path or
  feature row (scripted grep over the repo).
- T249 lint passes over the new doc.

## Out of scope

- Changing any gate/validator behavior (this is documentation of what
  exists).
- Company-internal compliance mappings (SOC2 etc.) — internal kit material
  (P005), not public docs.

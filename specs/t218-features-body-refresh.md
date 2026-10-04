# T218 — features_generate preserves card BODIES forever (stale fragments never heal)

check: cargo test --test site_sync

estimate: ~120 lines (body-refresh rule + pins)

## Concern

Operator 2026-10-03 ("site still seems wrong"): chug.sh's F15 card
showed what-text "http\" for HOURS after both the escaped-pipe parse
fix (T216) and the F15 source tidy landed. Root cause:
features_generate (scripts/site-sync.sh) preserves every existing
card's markup and only ensures the BADGE — the body (<p>) is written
once at card creation and never refreshed. A body written by any
historically-buggy generator is locked in forever. Operator removed
the broken card so the generator re-synthesized it (commit 9a1a6e0) —
a workaround the generator should make unnecessary.

## Repo context

- features_generate comment: "every existing card keeps its position
  and markup (CSS classes/order preserved, req 2); cards whose name
  matches an F-item get their badge ensured". Preserve = position +
  CSS classes; the body text is a FEATURES.md FACT (like the badge),
  not curation.
- T193 fixed badge classification inputs; T216 fixed cell parsing;
  this is the third layer: body freshness.
- TIMELINE's curated-merge (T122) is the contrast: curated entries are
  operator-owned, machine entries regenerate. FEATURES cards have NO
  curated content — name, badge, and body all derive from the row.

## Requirements

1. Card bodies are machine-owned: for any card whose name matches an
   F-item, the <p> body is regenerated from FEATURES.md's what-text
   (same prep() pipeline: cap, emphasis strip, HTML escape, backtick
   code spans). Card position and CSS classes stay preserved (req 2
   unchanged); cards NOT matching an F-item keep everything
   (forward-compat for hand-authored cards).
2. Drift heals: a card whose body differs from the current row's
   what-text gets the new body on the next sync (badge and body in one
   pass).
3. tests/site_sync.rs pins: (a) a stale-fragment body ("http\"-class)
   is replaced by the row's real what-text; (b) position + class
   preservation still holds; (c) a non-F-item card's body is
   untouched.
4. Acceptance: no future generator bug can fossilize a card body —
   the next sync after a bad write repairs it (recorded in Outcomes).

## Tests

- The three pins above; existing T193/T216 pins stay green.

## Out of scope

- Curated feature cards (none exist; if ever wanted, a marker
  convention is a separate item); TIMELINE region rules.

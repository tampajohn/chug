# T216 — site-sync: escaped pipes in FEATURES.md cells break the awk field split

check: cargo test --test site_sync

estimate: ~100 lines (gsub-before-split + pins)

## Concern

Operator 2026-10-03: "the last feature on the website is incorrect."
chug.sh's F15 card (Baked-in Laya judge daemon) renders status QUEUED
with what-text "http\" — phase 1 LANDED (T204 merged; the daemon is
running on K7). Cause: the F15 name cell contains
`CHUG_JUDGE=daemon\|http\|off` with markdown-escaped pipes, and the
features_rows awk (scripts/site-sync.sh, -F'|') splits on the escaped
pipes anyway — $3 truncates at "daemon\" (the "LANDED" beyond the
split is never seen, so the landed test at namecell ~ /LANDED/ misses)
and $4 becomes the fragment "http\" as the what-text.

## Repo context

- scripts/site-sync.sh features_rows awk (~237-262): splits raw lines
  on `|`; escaped `\|` is not special to awk. T193 fixed the status
  classification inputs (open-rows-only in-flight); THIS is a parse
  bug one layer down — any row with `\|` in name or what cells
  renders truncated.
- F15's row is otherwise correct: name cell carries "phase 1 LANDED"
  and "CHILD A ... LANDED" — once parsed whole, the landed test fires.
- Only F15 currently uses `\|` (audit the file when fixing — pin any
  others found).

## Requirements

1. In features_rows: neutralize escaped pipes BEFORE the field split
   (e.g. line-level `gsub(/\\\|/, " ")` on a copy, then split) so
   `\|` never creates phantom fields; genuine cell delimiters still
   split. Name and what cells come through whole.
2. Do NOT "fix" the F15 source text as the remedy — the generator must
   be robust to `\|` (rows are free-form markdown); a source tidy is
   allowed IN ADDITION if it reads better.
3. tests/site_sync.rs pins: a fixture row with `\|` in the name cell
   (a) classifies by the FULL cell (LANDED beyond a `\|` counts),
   (b) emits the full what-text (no "http\" fragment), (c) a row with
   `\|` and no LANDED still yields queued/in-flight correctly.
4. Acceptance: next wrap's site-sync shows F15 landed with the full
   what-text on chug.sh (record in cycle Outcomes; do not hand-edit
   the site repo).

## Tests

- The three pins above; existing T193 classifier pins must stay green.

## Out of scope

- Reclassifying other features (none affected — audit confirms);
  markdown-table rendering of the site itself.

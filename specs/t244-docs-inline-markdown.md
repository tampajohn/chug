# T244 — docs.html markdown renderer: tables, lists, inline code (T241 gap)

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --test site_sync

estimate: ~180 lines (inline-markdown pass + pins)

## Concern

Operator 2026-10-05: "docs is rendering poorly (markdown table not
rendering etc)". chug.sh/docs.html (T241) renders code fences but NOT
inline markdown: pipe tables ship as literal `| a | b |` text (the T205
hosting table at lines ~348-352), list items keep their `- ` markers
(line ~316), inline backticks stay backticks. The corpus is full of all
three — the page reads like raw markdown in most sections.

## Repo context

- T241 (9d85cbb): scripts/site-sync.sh docs render (fence-aware,
  byte-sorted sections). The gap is inline-level, inside paragraphs and
  table blocks.
- The corpus is runbooks/*.md + README Quickstart — contains GFM pipe
  tables, `- ` and `1.` lists, `inline code`, **bold**, *italic*, and
  links. Render must stay deterministic and HTML-escape first
  (structure over the escaped text, not raw HTML passthrough).
- T241's 3 validator findings already fixed staging/pointer issues;
  this is the render-completeness follow-on, not a regression.

## Requirements

1. Pipe-table blocks (a `|`-led line run with a `|---|` separator)
   render to <table> with header/body; escaped pipes `\|` inside cells
   stay literal (the T216 lesson — this renderer must NOT split on
   them).
2. `- ` and `1.` runs render to <ul>/<ol><li>; continuation indents
   join the item; blank line ends the list.
3. Inline: `code` -> <code>, **bold** -> <strong>, *italic* -> <em>,
   [text](https://url) -> <a href> (https only, per the canvas rules);
   entities stay escaped (no double-escaping of &lt;).
4. Deterministic byte-stable output; docs.html unchanged when corpus
   unchanged (idempotence pin stands).
5. tests/site_sync.rs pins with REAL corpus fixtures: the T205 hosting
   table renders as <table> with 3 rows; a `- ` list becomes <ul>;
   `chug run` becomes <code>; no literal `|---|` or backtick survives
   outside <pre> blocks.

## Tests

- The four pins above; existing region pins stay green.
- Acceptance: chug.sh/docs.html shows the hosting table as a table
  and the quick-task runbook's list as a list (verified in Outcomes).

## Out of scope

- Full GFM (nested blocks, footnotes, task lists, images); restyling
  the page CSS for tables (use the page's existing classes; a
  borderless minimal table style is fine).

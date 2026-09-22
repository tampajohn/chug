# T5 — edit_file non-unique match returns match line numbers

check: cargo test

## Status

DONE-RECORD. Shipped in `9840aba` (closed by TODO row T5). Backfilled
under T8 (EVALUATION.md I10).

## Concern

2026-09-21 transcript mining: `edit_file: 'old' found 2 times` triggered
full-file `git checkout --` reverts + rewrites (3x in one round) instead of
a second anchored attempt. On a non-unique match the tool returned no
location information, so the model could not disambiguate.

## What shipped

- `src/tools.rs:269-301` — on a non-unique `old` match the error now lists
  the line numbers of every match plus a few lines of context each, capped
  at 20 matches, so the model can anchor a second attempt instead of
  reverting the file.

## Regression tests

`src/tools.rs` tests `:851-884`: two-match error lists both line numbers;
25 matches cap at 20. Run `cargo test tools::tests`.

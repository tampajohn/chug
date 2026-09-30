# T178 — loopd exports CHUG_BASH_TIMEOUT=300 for the whole fleet

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared && cargo test --test loopd_model_routing --test loopd_stale_binary --test loopd_spoof_guard --test loopd_orphan_reaper

## Repo context

`BASH_TIMEOUT_SECS = 120` (src/tools.rs:15) caps every bash tool call.
Every stream in the cycles-76–78 delta shows `timed out after Ns
(process group killed)` bash deaths — orchestrators ×3–4 per cycle
(cold/warm nextest gates exceed 120s and die; warm retry ~25s) and impl
children ×2–3 (cold cargo builds) — and the gate templates'
`perl -e 'alarm 600; …'` inner bound is cargo-culted under a 120s cap
(the alarm can never reach 600; EVALUATION.md §2.4). The precedence
chain already exists: `--bash-timeout` flag > `$CHUG_BASH_TIMEOUT` env >
120 default (src/main.rs:370-398). loopd.sh launches the orchestrator at
:334-336 with no flag; delegate children inherit the orchestrator's
process env (delegate has no env param — the T47 lesson — and T144's
scrub removes only target-dir vars), so ONE env line in loopd.sh lifts
orchestrator + every delegate child. loopd.sh pins live in
tests/loopd_model_routing.rs and the loopd_* harness files (the check
above runs them all).

estimate: ~60 changed lines (loopd.sh ~3 + comment, LOOP-SPEC step-3
gate-template note ~1 sentence + pin updates if a pin counts the alarm
idiom, README Development/loopd bullet ~10, pin-file adjustments ~30).

## Requirements

1. loopd.sh gains `export CHUG_BASH_TIMEOUT=300` beside the launch
   block (before the `chug run` at :334) with a comment naming the
   evidence (cycles-76–78 bash-timeout deaths; the retry tax feeds the
   minutes-death census) and the knob's precedence (flag > env > 120;
   src/main.rs:370).
2. LOOP-SPEC.md §2 step 3 (Review gates) gains ONE honest sentence: the
   `perl -e 'alarm N'` inner bound stays as the per-command wedge guard,
   and with CHUG_BASH_TIMEOUT=300 the alarm value for gates should be
   ≤ the bash cap so the inner bound fires first (the template's
   `alarm 600` becomes `alarm 280`). Update the step-3 gate template
   and step-5's post-merge re-run reference consistently (every
   `alarm 600` in LOOP-SPEC.md → `alarm 280`; grep to confirm none
   remain).
3. No code changes: src/tools.rs's default stays 120 (interactive/chat
   users keep the tight cap); src/main.rs untouched.
4. README: the Development or Continuous-mode section gains one bullet —
   loopd sets CHUG_BASH_TIMEOUT=300 for the loop fleet; `--bash-timeout`
   overrides; default 120 elsewhere.
5. Every existing loopd pin stays green (the check above) — if a pin
   counts the alarm-600 idiom in LOOP-SPEC.md, update the pin in the
   same commit and name it.

## Tests

- The check's four pin files green.
- RED-proof: if any pin file asserts the LOOP-SPEC alarm idiom or the
  loopd env surface, reverting the corresponding change fails that pin —
  state which pins cover which surface in the commit message.
- `bash -n loopd.sh` clean.

## Acceptance

- Spec check green; full `cargo test` green; clippy `-D` clean.
- `grep -n "CHUG_BASH_TIMEOUT" loopd.sh README.md` shows both surfaces;
  `grep -n "alarm 600" LOOP-SPEC.md` returns nothing.

## Out of scope

- Changing the 120s default for non-loop users, a per-command tool
  timeout parameter, delegate env plumbing (env inheritance already
  covers children), the T173 minutes budgets (sibling row, separate).

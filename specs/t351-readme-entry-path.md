# T351 — README entry-path cleanup: install-first quickstart, `chug loop` leads continuous mode, generic model example

estimate: ~80 changed lines (README.md only + a docs link-check fixture)

## Why

Docs audit 2026-11-09 (same lens as the site staleness audit, T349/T350):
the README is ~95% current — dormancy (T325), ideation (T326), decision
providers (T332), daemon status (T340), and profiles (T330) are all
documented, and docs/ is entirely this week's crop. Three gaps remain,
all in the ENTRY PATH a new reader takes:

1. **Quickstart requires a Rust toolchain.** The `## Quickstart` section
   opens with `cargo build && cargo install --path .` — the from-source
   path. The one-command install (install.sh → binary) exists and is
   what pilots/users should do first; the toolchain path belongs under
   Development. A quickstart that starts with "install Rust" contradicts
   the T330 one-command story.
2. **Continuous mode leads with `nohup ./loopd.sh`.** The section
   (README.md ~1058) is clone-era: it assumes you're inside a chug
   checkout. Post-T333 the lead should be `cd <repo> && chug loop init
   && chug loop run`, with `./loopd.sh status|stop|routing|sleep-ok`
   reframed as the supervisor-internals reference (the subcommands
   themselves are accurate — keep them, reframe their audience).
3. **Internal serving name in the public example.** Line ~78's
   `--model anthropic-system.ai.kimi-k3` only resolves inside one
   company's proxy — a public reader copy-pasting gets a provider error
   as their first impression. The example should use a generic,
   always-resolvable model name (or the profile indirection) with the
   internal name demoted to a comment.

## What

1. Quickstart rewrite: install.sh (binary, one command) → `chug loop
   init && chug loop run` in your repo → optional `--profile` note →
   from-source variant moved under Development.
2. Continuous mode: lead with the subcommand flow; reframe the loopd.sh
   subcommands as "supervisor internals (running your own loopd)".
3. Line ~78 example: generic model + a comment pointing at
   docs/install-profiles.md for provider-specific names.
4. docs/adopt/quickstart.md and README must not contradict (link-check
   fixture asserts the same two commands appear in both).

## Tests

- Docs link-check fixture: README's quickstart commands exist verbatim
  in docs/adopt/quickstart.md (no drift between the two entry paths).
- Hygiene grep: no `anthropic-system.ai` strings in the README
  quickstart section (they may remain in internals sections where the
  audience is operators).

## Out of scope

- Rewriting the (accurate) internals content — routing, valve, dormancy,
  ideation sections stay as-is.
- docs/ changes (all current per the audit).

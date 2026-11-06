# Ideation cycle (T326) — the dormant loop's weekly proposal run

The supervisor (`loopd.sh`) launches THIS spec on the glm ideation model when
the loop is dormant and the ideation timer is due. It is one bounded,
mechanical run: auto-ratify the `## Proposed` lane, file at most 3 gated
outward-facing proposals, commit. Nothing else. This spec is the child's
whole world — the loop NEVER self-flips and NEVER implements a proposal.

## Contract

1. Run `python3 scripts/ideate.py` from the repo root. It is deterministic
   (no network): it prints the ideas packet, auto-ratifies eligible
   `proposed` rows (the 2026-11-06 amendment's 7d sweep — never the rows
   carrying the never-dispatch marker), gates each new proposal through the
   Laya noul meta-class gate (daemon socket, fail CLOSED — a daemon-less
   host files nothing), and files the survivors as 5-line stubs in
   `specs/proposed/` + `## Proposed` rows with status `proposed`.
2. Verify: `python3 scripts/ideate.py --check` must exit 0.
3. Commit ONLY what the script changed (`TODO.md` and/or `specs/proposed/`)
   with subject: `eval: ideation cycle (T326): <the script's printed
   disposition lines, joined by ;>` — e.g. `eval: ideation cycle (T326):
   ideation auto-ratified (T326): P012; ideation filed (T326):
   p013-next-release-train.md`. Push. When the script filed and flipped
   NOTHING, there is nothing to commit — say so and stop (a no-op cycle is
   a success, not a failure).
4. NEVER hand-edit a proposal (the script is the only author), NEVER flip a
   `proposed` row yourself (the loop never self-flips; the only flips are
   the script's amendment sweep), NEVER implement a proposal. A proposal
   the gate rejected stays rejected — do not retry it, do not reword it.

check: python3 scripts/ideate.py --check

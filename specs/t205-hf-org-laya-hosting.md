# T205 — HF org hosting for laya checkpoints: VA-private fine-tunes + token path

check: cargo test

estimate: ~250 lines (daemon token/revision support + docs + loopd env wiring + operator runbook)

## Concern

Operator 2026-10-02: "push the model to the videoamp huggingface — gate
it to videoamp users." The gating mechanics: a PRIVATE repo in the
videoamp HF org (org-membership-gated; machines via read-scoped
HF_TOKEN) — NOT HF's "gated" feature (public + approval workflow, wrong
tool). Three artifact classes need different treatment: base laya
(convaiinnovations/laya, Apache 2.0, already public — no gating;
optional mirror for revision pinning), the stop-completion judge
(tampajohn/, trained on operator's personal Claude Code transcripts —
VA-org migration is the operator's IP call, flagged), and F13
loop-decision fine-tunes (trained on chug decisions.jsonl — PUBLIC-repo
work today, but once chug loops against vampflake/central the logs
encode internal structure/doctrine: VA-private from day one).

## Repo context

- T204 daemon: CHUG_LAYA_CHECKPOINT already overrides the model id;
  hf-hub reads HF_TOKEN natively (no auth code needed — env passthrough
  + error messaging only). Public chug's DEFAULT checkpoint must stay
  public (base laya) or every public user hits an auth wall; VA hosts
  override checkpoint + token.
- Secret-spill history (operator memory): tokens were once leaked via
  an env dump — the fine-tune publish path needs a hard hygiene gate.
- Fine-tune pipeline today: ~/models/laya/finetune/ on F94 (not in this
  repo) — training stays there; THIS item is the chug-side consumption
  path + the publish contract.
- K7 loopd env: CHUG_LAYA_CHECKPOINT + HF_TOKEN must reach daemon
  processes WITHOUT committing secrets (loopd env file outside the
  repo / operator Vault — never .chug/*.json in git).

## Requirements

1. Daemon (T204 surface): CHUG_LAYA_CHECKPOINT accepts `org/model@REV`
   (pinned revision sha/tag — reproducibility for a policy-affecting
   artifact); HF_TOKEN env passthrough honored; a private-repo
   auth failure produces ONE clear stderr line naming the fix
   (HF_TOKEN missing/insufficient) — never a silent retry storm.
2. Publish contract (documented in the repo — DEPENDENCIES.md section
   + runbooks/ entry): any laya fine-tune destined for the videoamp HF
   org is (a) PRIVATE at creation, (b) secret-scanned over its training
   corpus (gitleaks-class) with the scan result recorded in the model
   card, (c) revision-pinned by consumers. Base-laya mirroring to
   videoamp/laya-base is OPTIONAL (availability pinning only).
3. loopd wiring: K7's supervisor passes CHUG_LAYA_CHECKPOINT +
   HF_TOKEN from an out-of-repo env file; the env file path is
   documented, its CONTENTS never logged/committed (T190 notify
   fail-open pattern applies: daemon absent/unauthed -> degrade
   logged, loop continues).
4. Operator steps (listed in the spec's handoff section, not loop
   work): verify the videoamp HF org exists (else create with IT);
   create the private judge repo(s); set org membership; mint a
   read-scoped token into Vault; decide the stop-judge migration
   (personal-transcript-derived IP -> VA asset call).
5. DEPENDENCIES.md (T203) gains the HF row: huggingface.co /
   hf-hub downloads / optional (public default) or HF_TOKEN (private) /
   fail-open degrade.

## Tests

- Revision parsing: org/model, org/model@sha, local path all resolve
  (unit pins).
- Auth failure honesty: 401 from a private repo -> the named-fix
  stderr line, one events.jsonl note, run continues.
- No-secret regression: the repo grep for HF_TOKEN finds only the env
  VAR NAME (never a value) — pin as a test or pre-commit check.

## Out of scope

- Training/publishing the F13 fine-tunes themselves (F13 phases);
  migrating the stop-judge repo (operator step); HF Enterprise/SSO
  features; re-hosting base laya unless the mirror proves necessary.

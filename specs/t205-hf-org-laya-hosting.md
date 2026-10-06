# T205 — HF org hosting for laya checkpoints: org-private fine-tunes + token path

check: export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test

estimate: ~250 lines (daemon token/revision support + docs + loopd env wiring + operator runbook)

## Concern

Operator 2026-10-02: "push the model to the org huggingface — gate
it to org users." The gating mechanics: a PRIVATE repo in the
org HF org (org-membership-gated; machines via read-scoped
HF_TOKEN) — NOT HF's "gated" feature (public + approval workflow, wrong
tool). Three artifact classes need different treatment: base laya
(convaiinnovations/laya, Apache 2.0, already public — no gating;
optional mirror for revision pinning), the stop-completion judge
(tampajohn/, trained on operator's personal Claude Code transcripts —
org-HF-org migration is the operator's IP call, flagged), and F13
loop-decision fine-tunes (trained on chug decisions.jsonl — PUBLIC-repo
work today, but once chug loops against internal monorepos the logs
encode internal structure/doctrine: org-private from day one).

## Repo context

- T204 daemon: CHUG_LAYA_CHECKPOINT already overrides the model id;
  hf-hub reads HF_TOKEN natively (no auth code needed — env passthrough
  + error messaging only). Public chug's DEFAULT checkpoint must stay
  public (base laya) or every public user hits an auth wall; internal hosts
  override checkpoint + token.
- Secret-spill history (operator memory): tokens were once leaked via
  an env dump — the fine-tune publish path needs a hard hygiene gate.
- Fine-tune pipeline today: ~/models/laya/finetune/ on F94 (not in this
  repo) — training stays there; THIS item is the chug-side consumption
  path + the publish contract.
- K7 loopd env: CHUG_LAYA_CHECKPOINT + HF_TOKEN must reach daemon
  processes WITHOUT committing secrets (loopd env file outside the
  repo / operator Vault — never .chug/*.json in git).

## Interim posture (operator 2026-10-02, supersedes nothing below)

The Artifactory/Databricks/HF-org decision is being discussed by the
operator on Monday 2026-10-05 — NO org-side work (repo creation,
membership, Vault tokens) happens before that call. Interim hosting
for fine-tunes: LOCAL DIRECTORY (CHUG_LAYA_CHECKPOINT=<path> — T204
req 2 already covers local-dir loading) or PRIVATE tampajohn/* HF
repos (HF_TOKEN-gated, operator's personal org). New fine-tune repos
are created PRIVATE at inception per the publish contract below. The
loop-side work in this spec (endpoint-agnostic fetch, revision
pinning, token passthrough, hygiene gate) is required for the interim
too — only the org steps wait.

## Hosting options (ranked, operator 2026-10-02 Databricks question)

1. **JFrog Artifactory HF repository** (PREFERRED if available): the org
   already runs JFrog (wheels ship there); JFrog's Hugging Face repo
   type (local + remote/proxy, 7.9x+) hosts first-party models behind
   EXISTING SSO/IAM — no new membership silo. HF clients pull via
   HF_ENDPOINT pointed at the Artifactory repo URL + Artifactory token.
   VERIFY (operator): our Artifactory version + HF repo type enabled.
2. **the org HF org private repo** (the original plan): works today,
   but adds a second membership/token silo to manage.
3. **Databricks Model Registry** (REJECTED 2026-10-02, verified against
   Databricks lifecycle docs): the registry speaks ONLY the MLflow API
   (models:/ URIs, mlflow.<flavor>.load_model, artifact downloads via
   MLflow client) — NO HF Hub-compatible endpoint exists. Consuming
   from it would mean a bespoke MLflow fetch backend (REST + signed-URL
   downloads, Databricks-specific auth) instead of hf-hub's native
   path. Model Serving is the inference layer — wrong abstraction for
   shipping weights to a daemon.

## Requirements

1. Daemon (T204 surface): CHUG_LAYA_CHECKPOINT accepts `org/model@REV`
   (pinned revision sha/tag — reproducibility for a policy-affecting
   artifact); HF_TOKEN env passthrough honored; a private-repo
   auth failure produces ONE clear stderr line naming the fix
   (HF_TOKEN missing/insufficient) — never a silent retry storm.
   Endpoint-agnostic fetch: CHUG_HF_ENDPOINT (or HF_ENDPOINT if the
   hf-hub crate honors it — verify at impl; else ApiBuilder override)
   points downloads at Artifactory or any HF-compatible host; default
   stays public huggingface.co.
2. Publish contract (documented in the repo — DEPENDENCIES.md section
   + runbooks/ entry): any laya fine-tune destined for the org HF
   org is (a) PRIVATE at creation, (b) secret-scanned over its training
   corpus (gitleaks-class) with the scan result recorded in the model
   card, (c) revision-pinned by consumers. Base-laya mirroring to
   org/laya-base is OPTIONAL (availability pinning only).
3. loopd wiring: K7's supervisor passes CHUG_LAYA_CHECKPOINT +
   HF_TOKEN from an out-of-repo env file; the env file path is
   documented, its CONTENTS never logged/committed (T190 notify
   fail-open pattern applies: daemon absent/unauthed -> degrade
   logged, loop continues).
4. Operator steps (PENDING the Monday 2026-10-05 hosting decision —
   do NOT execute early): FIRST verify the Artifactory HF-repo option
   (version + repo type; if present it displaces the HF org); else
   verify the org HF org exists (else create with IT); create the
   private judge repo(s); set org membership / Artifactory perms; mint
   a read-scoped token into Vault; decide the stop-judge migration
   (personal-transcript-derived IP -> org asset call).
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

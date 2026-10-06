# laya checkpoint hosting: org-private fine-tunes + the token path (T205)

When to reach for this: you are hosting or consuming a laya judge
fine-tune for an internal machine fleet — the daemon's risk-gate
model. One-page contract here; the code pins are `src/hf_hosting.rs`,
`src/judge_model.rs`, and `tests/no_secret_spill.rs`.

## The gating decision (already made)

A PRIVATE repo in the org's HF org: org-membership-gated, machines
pull via a read-scoped `HF_TOKEN`. NOT HF's "gated" feature (public +
approval workflow — wrong tool). Three artifact classes, three postures:

| class | where | gating |
|---|---|---|
| base laya (`convaiinnovations/laya`, Apache 2.0) | public, upstream | none — the chug default stays public so no public user hits an auth wall; mirroring to `org/laya-base` is OPTIONAL (availability pinning only) |
| stop-completion judge (`tampajohn/…`, trained on the operator's personal Claude Code transcripts) | personal org today | migrating it to the org's HF org is the operator's IP call — flagged, not decided here |
| F13 loop-decision fine-tunes (trained on chug `decisions.jsonl`) | org-private from day one | public-repo work is fine only while chug loops against public repos; against internal monorepos the logs encode internal structure/doctrine |

## Hosting decision — PENDING the operator's Monday 2026-10-05 call

NO org-side work (repo creation, membership, Vault tokens) happens
before this. Ranked from the 2026-10-02 analysis:

1. **JFrog Artifactory HF repository** — PREFERRED if available (the org runs
   JFrog; the HF repo type hosts models behind existing SSO/IAM; clients
   point `HF_ENDPOINT` at the Artifactory repo URL). VERIFY first:
   Artifactory version + HF repo type enabled. If present, it displaces
   the HF org.
2. **the org HF org private repo** — works today, adds a second
   membership/token silo.
3. ~~Databricks Model Registry~~ — REJECTED 2026-10-02: speaks ONLY the
   MLflow API; no HF-Hub-compatible endpoint exists; would mean a
   bespoke fetch backend.

**Interim hosting (works today, no org steps):** a LOCAL DIRECTORY
(`CHUG_LAYA_CHECKPOINT=<path>`) or a PRIVATE `tampajohn/*` HF repo
behind `HF_TOKEN`. New fine-tune repos are created PRIVATE at inception
(contract below). The loop-side machinery in this runbook is required
for the interim too — only the org steps wait.

## Consumer config (the daemon's fetch path)

The judge daemon (T204) resolves its checkpoint from `CHUG_LAYA_CHECKPOINT`:

```sh
# local dir (interim; T204 req 2)
export CHUG_LAYA_CHECKPOINT="$HOME/models/laya/finetune/out"
# hub repo, pinned revision — REQUIRED SHAPE for policy-affecting artifacts
export CHUG_LAYA_CHECKPOINT="org/laya-stop-judge@9c6af39cdce45b570f0b7f8fad2b311c96019804"
export HF_TOKEN="<read-scoped-token-with-access-to-the-repo>"   # name only in this repo
# non-huggingface.co hosts (Artifactory or any HF-compatible endpoint):
export CHUG_HF_ENDPOINT="https://artifactory.example.com/hf"    # wins
export HF_ENDPOINT="https://artifactory.example.com/hf"         # fallback
```

Verified against hf-hub 0.4.3 (do not trust the crate to do this): it
reads NEITHER `HF_TOKEN` from the env (only `~/.cache/huggingface/token`,
`huggingface-cli login`) NOR `HF_ENDPOINT` via `Api::new()`. chug applies
both itself (`hf_hosting::build_hub_api`): `CHUG_HF_ENDPOINT` >
`HF_ENDPOINT` > `https://huggingface.co`; a non-empty `HF_TOKEN` overrides
the cached token file. Default stays public — public chug never requires
a token.

**Auth failure is honest:** a 401/403 from a private repo produces ONE
stderr line naming the fix — `set HF_TOKEN to a read-scoped token with
access to <repo>` (missing) or `HF_TOKEN is set but was rejected: check
scope/expiry/membership` (insufficient) — plus one
`judge_checkpoint_auth_error` note in `.chug/events.jsonl`, then the
daemon dies, the client latches, and the loop continues fail-open
(T190 shape). No retry storm: one fetch attempt, `max_retries=0`.

## loopd wiring (K7): the out-of-repo env file

The supervisor loads ONE env file at startup and every cycle (orchestrator,
delegate children, spawned judge daemon) inherits the exports:

```
$HOME/.chug/loopd.env        # default; override with CHUG_LOOPD_ENV
```

Format: `KEY=VALUE` lines, `#` comments, optional `export ` prefix and
one pair of quotes. Values are LITERAL (no `$()` expansion — the file is
parsed, never eval'd). ONLY three keys are applied — `CHUG_LAYA_CHECKPOINT`,
`HF_TOKEN`, `CHUG_HF_ENDPOINT` — and only when the operator has not
already exported them (explicit env wins). Rotation = edit the file +
restart loopd (the T47 lesson: no env mutation inside the cycle loop).

Hygiene: the file lives OUTSIDE any repo (never committed), and its
CONTENTS are never echoed or logged — loopd logs only the path and a
skipped-line count. This is the hard rule from the operator's env-dump
spill history; `tests/no_secret_spill.rs` pins the whole repo to the
same rule (HF_TOKEN appears as a NAME, never a value).

Degrade: absent file → silent no-op (public base laya); unauthed daemon
→ the 401 line above + fail-open; the loop never stalls on the judge.

## Publish contract (every org-bound fine-tune, before any consumer points at it)

1. **PRIVATE at creation** — org-membership gating, not HF's "gated".
2. **Secret-scanned over the training corpus** (gitleaks-class), scan
   result recorded in the model card. Hard gate, not advice.
3. **Revision-pinned by consumers** — `org/model@REV` in every env file.
4. Base laya mirroring to `org/laya-base` is OPTIONAL (availability
   pinning only).

## Operator checklist — PENDING 2026-10-05, do NOT execute early

1. Verify the Artifactory HF-repo option FIRST (version + repo type). If
   present it displaces the HF org.
2. Else verify the org HF org exists (else create with IT).
3. Create the private judge repo(s) — PRIVATE at inception.
4. Set org membership / Artifactory perms (machines: read-scoped token
   only; humans: membership).
5. Mint the read-scoped token INTO VAULT (never into this repo, never
   into `.chug/*.json` in git).
6. Decide the stop-judge migration (personal-transcript-derived IP → org
   asset call).

Out of scope here: training/publishing the F13 fine-tunes (F13 phases),
HF Enterprise/SSO, re-hosting base laya unless the mirror proves
necessary.

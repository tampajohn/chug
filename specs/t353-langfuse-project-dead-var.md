# T353 — profile contract: CHUG_LANGFUSE_PROJECT is a no-op — remove it + document per-lane visibility honestly

estimate: ~70 changed lines (install.sh profile template + docs/install-profiles.md + loader comment + a contract-audit test)

## Why

Fleet Primary live-verified 2026-11-09 (T4 clean-delivery run): grep
across src/scripts/docs finds ZERO consumers of CHUG_LANGFUSE_PROJECT.
observ.rs writes to exactly one Langfuse project — the one the key pair
scopes to. The T330 profile contract, the install.sh template, and the
pilot kit all teach a variable that does nothing; every adopter will
wire it and believe their spend is isolated when it is not. A documented
no-op in a contract is worse than an absent feature — it lies.

## What

1. **Remove CHUG_LANGFUSE_PROJECT** from the install.sh profile
   template, docs/install-profiles.md, and the loader's documented key
   list (leave a tombstone comment: removed 0.22, was a no-op, see
   per-lane visibility below).
2. **Document the two real mechanisms** in docs/install-profiles.md:
   - **Per-lane isolation**: a dedicated Langfuse project + its OWN
     pk/sk key pair per lane, wired via CHUG_LANGFUSE_PUBLIC_KEY /
     CHUG_LANGFUSE_SECRET_KEY (these DO map through the loader's
     CHUG_LANGFUSE_* -> LANGFUSE_* translation — verify and name them
     explicitly; if any of these is ALSO unconsumed, this item grows to
     wire it).
   - **Single-project filtering** (default): one chug project;
     per-automation reads use trace name (the loop names traces per TODO
     item), tags [chug, model], and sessionId — verified live by Fleet
     Primary's T4 run.
3. **Contract audit test** (the systemic fix): a test that every
   CHUG_* key documented in install-profiles.md has a consumer in src/
   or scripts/ (grep-based, allowlist for intentionally-reserved keys).
   This class of dead-var can never silently re-enter the contract.

## Tests

- The audit test fails on a planted dead var (RED-proven), passes on the
  cleaned contract.
- install.sh profile template renders without the removed key; the
  docs' per-lane section's key names each grep-hit in src/ or scripts/.

## Out of scope

- Building per-lane key provisioning (that's infra/operator work with
  Langfuse admin, not chug code).
- Changing observ.rs routing (the single-project-per-keypair model
  stays; this item makes the docs honest about it).

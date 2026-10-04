# T226 — close the T217+T215 validators' left-behind survivors (tests-only sweep)

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a; touch src/*.rs tests/*.rs; cargo test --test site_sync --test loopd_daemon_ensure --test daemon_feature_off

estimate: ~200 changed lines (three pin families, one RED-proof each; tests-only preferred — see req 2's fork)

## Concern

Two PASS verdicts left predicted survivors behind (the T224 pattern —
file them forward, sweep the family in ONE row):

- **T217 m4** (site-sync fail-closed, merge e242689): the kimi
  validator's bonus mutant m4 — deleting the git-log guard leg while the
  TODO.md and .chug/loopd legs stay intact — SURVIVED. The git-log leg
  has no isolated fixture pin (beyond the spec's Tests section;
  defense-in-depth).
- **T215 LOW-(a)** (loopd daemon-ensure binary, merge 04ac3f1):
  `resolve_daemon_bin` leg-(b) — the `~/.local/bin/chug daemon --help`
  probe does not discriminate a feature-OFF binary from a feature-on one
  (realistic carrier path safe: install.sh installs release.yml's
  feature-on tarballs — pinned, but a blind probe is one packaging drift
  from silently ensuring a stub).
- **T215 LOW-(b)**: the client-side `daemon_binary()` wrong-binary
  shape — mitigated by the latch:true fail-open the verdict verified,
  never pinned.

## Requirements

1. **T217 m4 pin** (tests/site_sync.rs): an isolated fixture where
   TODO.md and .chug/loopd are present and readable but the git log is
   EMPTY (or git absent from the fixture PATH — the impl picks the leg
   the guard actually reads); assert refusal: one named error, exit 4,
   zero writes, zero commits. RED-proof: delete the git-log guard leg →
   this pin RED; restore byte-clean.
2. **T215 LOW-(a)**: pin the probe's behavior against BOTH binary shapes
   through the loopd_daemon_ensure sandbox driver — a fixture
   `~/.local/bin/chug` whose `daemon --help` mimics the feature-off
   build, one mimicking feature-on. Preferred: if a cheap
   feature-on-only needle exists in the real `daemon --help` surface,
   the probe gains it and the pin asserts discrimination (a loopd.sh
   edit → the row flips to kimi-REQUIRED routing at review — say so in
   the ledger). Otherwise pin the CURRENT acceptance with a comment
   naming the accepted blindness (the install.sh carrier argument) so a
   future carrier regression is loud.
3. **T215 LOW-(b)**: a client-side pin for the wrong-binary shape — the
   daemon client resolving a feature-off stub latches fail-open exactly
   as the verdict verified (pin the latch; NOT a behavior change).
4. Sweep-the-family (T69): ONE RED-proven killing test per named
   survivor; every pin RED-proven against its named mutant in the
   worktree, reverted byte-clean (shasum-verified).

## Tests

- The three pin families above, each with its RED-proof recorded in the
  commit message (mutant name → red test name).
- The host families (site_sync, loopd_daemon_ensure, daemon_feature_off)
  stay green.

## Acceptance

- cargo build + clippy --all-targets -D warnings green; the check line
  green.
- Production diff empty UNLESS req 2's needle landed (then: loopd.sh
  only, and kimi validation is REQUIRED regardless of diff size).

## Out of scope

- Any production behavior change beyond req 2's optional needle.
- The reaper, the spoof guard, the other loopd families.

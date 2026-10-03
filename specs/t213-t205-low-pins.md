# T213 — close the T205 validator's three LOW survivors (canary pin, loader pin, retry-knob comment)

## Repo context

T205 (cb52c53, cycle 96) landed HF org hosting for laya checkpoints. Its
kimi validator PASSed with 3 LOW survivors (verdict d1791037771-9,
harvested `.chug/verdict-t205-20261003.md` + LEDGER) filed forward as
next-eval rows — this row closes all three in one bundle (one area:
T205's hygiene surface):

1. **M1 token-canary SURVIVED** — the auth honesty gate records
   `token_set` as a boolean and the shipped code is clean (a leak would
   need a code change), but no test drives a canary token VALUE through
   the auth path asserting the value never lands in any output. The
   surfaces: `src/hf_hosting.rs` `map_hub_fetch_error` (ONE stderr fix
   line ~:201), `note_auth_error` (ONE `judge_checkpoint_auth_error`
   events note, `token_set` boolean ~:236), and the error CHAIN returned
   to the caller (`auth_fix_clause`, ~:157/:205 — the chain is what
   lands in daemon.log/transcripts).
2. **M4 loopd-allowlist SURVIVED** — no automated pin drives loopd.sh's
   K7 env-file loader (loopd.sh:54-81: allowlist
   `CHUG_LAYA_CHECKPOINT|HF_TOKEN|CHUG_HF_ENDPOINT`, explicit env wins,
   quotes/`export `-prefix/CR tolerated, contents never logged — only
   the path + a skipped-line COUNT, absent file = silent no-op). The
   loader sits INLINE in loopd.sh, so a test must either spawn the whole
   supervisor (heavy) or get a sourceable seam.
3. **M3 retry-knob SURVIVED (structural, informational)** — the
   `judge_model` fetch is one attempt (`ApiBuilder` max_retries is 0,
   named in hf_hosting.rs's module docs ~:20) and the wobble metadata
   lives outside the retry block; the validator asked only for a
   comment naming that the one-attempt shape is DELIBERATE (a silent
   retry storm against a private org repo is worse than a fail-open).

**This row is SOLO** (loopd.sh is a doctrine carrier — the extraction
below edits it) and **kimi-REQUIRED at dispatch** (the diff touches
loopd.sh; the T189 lane's criterion (a) keeps loop/spec doctrine edits
at full validation).

estimate: ~200 changed lines (loopd.sh extraction ~35 moved lines +
tests/loopd_env_loader.rs ~120 + hf_hosting canary legs ~40 + comment;
test-dense per the cycle-98 eval calibration)

## Requirements

1. **Canary pin** (`src/hf_hosting.rs` test module): a test sets
   `HF_TOKEN` to a canary value (a distinctive string, e.g.
   `hf_CANARY7f3c9d1eNEVERLOG`) and drives the auth-error surface —
   `map_hub_fetch_error` with a 401 chain — asserting the canary string
   appears in NONE of: the stderr fix line (capture via the existing
   test seam or by refactoring the eprintln behind a
   `fn emit_fix_line(line: &str)` seam with a recording override), the
   returned error chain, and the events note written under a tempdir
   `note_cwd` (read the file back, assert `token_set: true` present and
   the canary absent). A second leg with NO token asserts
   `token_set: false` and the same absence (vacuous-guard: the canary
   constant must be asserted present in the fixture INPUT — env — so
   the test cannot pass by never wiring the token).
2. **Loader pin**: extract loopd.sh:54-81's loader block BYTE-IDENTICAL
   into `scripts/loopd_env_loader.sh` as a function
   `loopd_load_env_file` (depending on `$LOOPD_ENV_FILE`, `$LOG`, and
   `ts` — all three loopd.sh provides); loopd.sh sources the fragment
   (`. "$(dirname "$0")/scripts/loopd_env_loader.sh"` resolved relative
   to loopd.sh's own path, NOT the cwd) and calls
   `loopd_load_env_file` where the block stood. Behavior byte-identical;
   `bash -n loopd.sh scripts/loopd_env_loader.sh` clean; the T205
   comment block (loopd.sh:40-53) stays in loopd.sh with one added line
   naming the fragment's home.
3. New `tests/loopd_env_loader.rs` drives the fragment via `bash -c`
   with fixture env files in tempdirs (TS + LOG stubbed):
   - allowlisted key applied (CHUG_LAYA_CHECKPOINT reaches the env);
   - all three allowlisted keys accepted, a 4th non-allowlisted key
     (e.g. `AWS_SECRET=...`) NOT applied AND the log line carries the
     skipped COUNT and the path and ZERO value bytes from the fixture
     (assert the fixture's value strings never appear in LOG);
   - explicit env wins: pre-set `HF_TOKEN=fromenv` survives a fixture
     `HF_TOKEN=fromfile` (assert `fromenv`, assert no `fromfile`
     anywhere in env or LOG);
   - quotes / `export ` prefix / CR / blank / `#` comment legs;
   - absent file → no output, no LOG write, exit 0 (public-churn
     no-op).
4. **Retry-knob comment**: hf_hosting.rs's fetch path (at the
   `build_hub_api`/`ApiBuilder` site) gains a comment naming the
   one-attempt shape deliberate — the validator's M3 class: metadata
   outside the retry block is informational, and a retry storm against
   a private org repo is the worse failure (fail-open stands).
5. Nothing else in loopd.sh changes; the sourcing line is the ONLY
   loopd.sh diff besides the comment pointer (byte-identical loader
   move — pin it: the fragment's function body matches the extracted
   block modulo the function wrapper).

## Tests

Per leg above; plus the hygiene guards stay green:
`cargo test --bin chug hf_hosting` (canary legs run in the plain
profile — hf_hosting is ungated), `cargo test --test no_secret_spill`
(the repo-wide value ban must not flag the canary constant in the TEST
file — if it does, the canary leg belongs in the test with a
no_secret_spill allowlist entry NAMED in that guard's own terms; the
implementer chooses the cleaner of the two and says which in the commit
message), and `cargo test --test loopd_env_loader`.

## Acceptance

- All legs green; the canary test is RED-proven by a mutant that prints
  the token into the fix line (orchestrator or child proves RED, then
  restores byte-identical).
- The loader extraction is behavior-identical: `bash -n` clean, and one
  manual smoke (`CHUG_LOOPD_ENV=<fixture> bash -c '. scripts/loopd_env_loader.sh; LOOPD_ENV_FILE=<fixture>; LOG=/dev/stdout; ts(){ date -u +%FT%TZ; }; loopd_load_env_file'`) shown in the commit message with the count-only log line and no values.
- clippy `--all-targets -- -D warnings` clean; full suite green.

## Out of scope

- New allowlisted keys, a TCP/socket path, hot reload, or any daemon
  change (F15 phases-2+ deferrals stand).
- Restructuring loopd.sh beyond the one extraction.
- Widening no_secret_spill's matcher.

check: set -o pipefail; export CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared; touch src/*.rs tests/*.rs; cargo test --bin chug hf_hosting && cargo test --test loopd_env_loader --test no_secret_spill && bash -n loopd.sh scripts/loopd_env_loader.sh

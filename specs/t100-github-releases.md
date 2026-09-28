# T100 — GitHub releases: tag-triggered prebuilt binaries + notes

check: cargo test

## Concern

chug is installable only via `cargo build` from source. Operator
2026-09-28: "start doing releases of chug in github." External users
(Tony/Josh/Neil's teams, the Slack thread) need `curl | tar` install;
the loop itself needs a versioned artifact to point at. Semantic commits
make release notes mechanical; the repo's own GH Actions may be edited
(this is chug's repo — no policy-bot here).

## Repo context

- `Cargo.toml` version 0.1.0; `build.rs` already bakes CHUG_GIT_HASH into
  the startup banner (T11) — release artifacts are self-describing.
- `.github/workflows/`: exists? If absent, create one — the repo currently
  gates via local/loop gates only. Any workflow MUST pin actions by SHA
  (supply-chain hygiene).
- README quickstart: source-build only today; gains an install section.
- Binaries: macOS arm64 (primary), linux x86_64, linux arm64 (fleet boxes:
  K7 is mac-arm64, ucraft/sparks are linux-x64).

## Requirements

1. `.github/workflows/release.yml`: on `v*` tag push → build
   `chug` (release, locked) on macos-14 (arm64), ubuntu-22.04 x86_64 +
   aarch64 (cross or native runner); tar.gz each with sha256; create the
   GitHub Release with the archives attached. Actions pinned by SHA.
2. Release notes generated from semantic commits since the previous tag
   (grouped feat/fix/chore/docs; the repo's commit style makes this
   grep-able — see git log).
3. Version discipline: tags are `v<major>.<minor>.<patch>` matching
   Cargo.toml's version; a check step fails the release if they diverge.
4. README: install section — one-line `curl -L <latest tarball> | tar xz`
   per platform + `chug` into PATH; source-build stays documented below.
5. `install.sh` at the repo root (served as https://chug.sh/install.sh via
   the site repo): warp.dev-style one-liner — `curl -fsSL
   https://chug.sh/install.sh | sh`. Detects platform (uname -s/-m →
   macos-arm64 / linux-x86_64 / linux-aarch64), downloads the LATEST
   release tarball + sha256 from GitHub releases, VERIFIES the checksum,
   installs to ~/.local/bin (PATH hint if missing), prints the chug banner
   + quickstart pointer. POSIX sh, no bashisms, no sudo, failure messages
   name the fix (no release yet → build-from-source line). The site's
   get-started section gains the one-liner block (site-repo edit,
   verify.sh checks the block and the install.sh link).
6. Cutting a release is an OPERATOR action (`git tag v0.2.0 && git push
   origin v0.2.0`); the loop never self-tags (a loop that versions itself
   is a foot-gun — version bumps are human calls, like merges to prod).

## Tests

- Workflow-lint leg (actionlint if available, else YAML parse + SHA-pin
  grep: every `uses:` has a 40-hex ref).
- Tag/version divergence check is a script with a fixture test.
- Acceptance: first real tag (operator-cut) produces a release with 3
  artifacts; README install line verified once per platform by hand
  (recorded in Outcomes).

## Out of scope

- crates.io publish; homebrew tap; signing/notarization (macOS Gatekeeper
  note in README instead); changelogs beyond the generated notes; brew
  install support.

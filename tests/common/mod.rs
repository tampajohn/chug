//! Shared loopd.sh fixture plumbing (the derived-fragment copy).
//!
//! T213 extracted loopd.sh's env loader into `scripts/loopd_env_loader.sh`;
//! T330 added `scripts/loopd_profile_loader.sh`. loopd.sh sources every
//! fragment RELATIVE TO ITS OWN PATH under `set -euo pipefail`, so any test
//! that runs a COPIED loopd.sh needs every sourced fragment beside the copy
//! or the script dies before any mode dispatch. A hand-listed copy set goes
//! stale the moment loopd.sh gains a fragment (the T330 bite: five opaque
//! exit-1 routing/predicate fixtures), so the copy set is DERIVED from the
//! copied loopd.sh text and fails loudly if a referenced fragment is missing
//! from the checkout.

use std::path::Path;

/// Copy `loopd.sh` into `dir` plus every `scripts/*.sh` fragment it sources
/// via the `. "$(dirname "$0")/scripts/<name>.sh"` convention. Returns the
/// fixture `scripts` dir path.
/// Copy the derived loopd.sh fragment set into `dir` and return loopd.sh's
/// fixture path. Shared helper — individual harnesses use subsets, so the
/// per-unit dead-code allow is load-bearing (T329's mod-common addition to
/// loopd_daemon_ensure.rs trips it otherwise).
#[allow(dead_code)]
pub fn copy_loopd_fixture(dir: &Path) -> std::path::PathBuf {
    let repo_root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::write(
        dir.join("loopd.sh"),
        std::fs::read_to_string(repo_root.join("loopd.sh")).expect("loopd.sh readable"),
    )
    .expect("copy loopd.sh into fixture dir");
    let script_text = std::fs::read_to_string(dir.join("loopd.sh")).expect("reread copy");
    let scripts = dir.join("scripts");
    std::fs::create_dir_all(&scripts).expect("fixture scripts dir");
    for line in script_text.lines() {
        let Some(rest) = line.trim().strip_prefix(". \"$(dirname \"$0\")/") else {
            continue;
        };
        let Some(name) = rest.strip_suffix('"') else {
            continue;
        };
        let src = repo_root.join(name);
        assert!(
            src.is_file(),
            "loopd.sh sources {name} but it is missing from the checkout — \
             the loopd fixture cannot be built (add the fragment to the repo or \
             fix the sourcing line)"
        );
        let base = name.rsplit('/').next().expect("fragment has a basename");
        std::fs::copy(&src, scripts.join(base))
            .unwrap_or_else(|e| panic!("copy {name} into fixture dir: {e}"));
    }
    assert!(
        scripts.read_dir().expect("fixture scripts dir").count() >= 2,
        "the derived fragment set is implausibly small — loopd.sh should source \
         at least the env loader and the profile loader; check the sourcing lines"
    );
    scripts
}

/// T329 — pin the T213/T330 env-file loaders to ABSENT fixture paths, and
/// return the two paths (loopd.env, profile.env) for builder-chain call
/// sites that cannot take a `&mut Command`.
///
/// loopd.sh sources `$CHUG_LOOPD_ENV` (default `~/.chug/loopd.env`) and
/// `$CHUG_PROFILE_ENV` (default `~/.chug/profile.env`) at startup, and the
/// loaders run INSIDE the spawned script — AFTER any test-side
/// `env_remove`, so a host with a STAGED profile re-exports
/// `LOOP_ORCH_MODEL` / `CHUG_DECISION_PROVIDER` / `ANTHROPIC_BASE_URL` into
/// every fixture no matter what the harness deleted (the 2026-11-09 bite:
/// the operator-staged profile.env carried a `[1m]`-suffixed reasoning id
/// that leaked into the gate line's model field, and
/// `CHUG_DECISION_PROVIDER=proxy` starved the daemon-socket judge fixtures
/// onto the proxy — laya=down). The T332 hermeticity contract already
/// states the rule: no host environment may flip a fixture. The loaders'
/// contract (scripts/loopd_profile_loader.sh): an ABSENT profile file is a
/// silent no-op — and an EXISTING-but-incomplete one warns ("missing
/// required key(s)"), so the pin must point at paths that DO NOT EXIST,
/// never at freshly created empty files. The harness then sees EXACTLY the
/// defaults it pins, byte for byte. Harnesses that TEST the loaders set
/// their own files (loopd_env_loader.rs, loopd_profile_loader.rs) and skip
/// this helper.
pub fn write_empty_env_files(dir: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    // Deliberately NOT created: an absent file is the loaders' silent
    // no-op; a created-empty profile.env would trip the missing-required-
    // keys warning (the t329-absent names make the intent greppable).
    let loopd_env = dir.join("t329-absent-loopd.env");
    let profile_env = dir.join("t329-absent-profile.env");
    (loopd_env, profile_env)
}

/// The `&mut Command` form of [`write_empty_env_files`] — set both loader
/// overrides on a command under construction.
pub fn pin_loopd_env_files(cmd: &mut std::process::Command, dir: &Path) {
    let (loopd_env, profile_env) = write_empty_env_files(dir);
    cmd.env("CHUG_LOOPD_ENV", loopd_env);
    cmd.env("CHUG_PROFILE_ENV", profile_env);
}
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

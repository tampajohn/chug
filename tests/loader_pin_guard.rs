//! T346 — RED-proven guard: every loopd-spawning test harness pins the
//! env-file loaders (`CHUG_LOOPD_ENV` / `CHUG_PROFILE_ENV`) to ABSENT
//! fixture paths.
//!
//! The 2026-11-09 host bite (cycle-3172 post-wrap fix-up 7a26c410): the
//! operator-staged `~/.chug/profile.env` re-exported itself into every
//! spawned `loopd.sh` fixture AFTER each harness's `env_remove` calls —
//! the T330 profile loader runs INSIDE the spawned script. A `[1m]`-suffixed
//! `CHUG_MODEL_REASONING` landed in the gate line's model field, and the
//! profile's proxy config starved the daemon-socket judge fixtures onto the
//! proxy (laya=down): two `loopd_laya_triage` tests red under a full-suite
//! run while individually green in other orders. The repair pinned the two
//! loader overrides at 11 spawn sites across 7 harnesses
//! (`common::pin_loopd_env_files`), but nothing stops the NEXT harness from
//! forgetting the pin — the class is mechanical, so it is guarded here, not
//! remembered.
//!
//! The contract (specs/t346-loader-pin-guard.md): every test that execs
//! `loopd.sh` through a `Command::new("bash")` construction must either
//!   (a) call `common::pin_loopd_env_files(&mut cmd, <fixture root>)`, or
//!   (b) set BOTH `CHUG_LOOPD_ENV` and `CHUG_PROFILE_ENV` itself, to fixture
//!       paths that DO NOT EXIST (an absent file is the loaders' silent
//!       no-op; an existing-but-incomplete one warns), or
//!   (c) sit in a sanctioned-exception file — the harnesses that test the
//!       loaders themselves set their own files.
//!
//! The scan walks TEXT, not AST — the mechanical shape that survives
//! refactor noise (the T293 brittle-awk lesson). Walking text has a
//! false-positive surface, so the detector's own knobs are pinned below:
//! the exception list is capped and every entry is a named on-disk file,
//! and a floor on the number of exec sites keeps the walk from rotting into
//! a vacuous pass.
//!
//! One bounded carve-out, checked mechanically: a `bash -n <script>` chain
//! (tests/nextest_gate_runner.rs) PARSES loopd.sh without executing it — no
//! loader can run, so no pin is owed. The carve-out needle is the exact
//! text `.arg("-n")`; a site that actually execs the script never carries
//! it, and a future real exec added beside the parse check is still
//! scanned on its own chain.

use std::path::{Path, PathBuf};

/// The bash construction needle, assembled by concatenation so THIS file's
/// own source never contains it contiguously (the T48 pattern): the scan
/// walks this file too, there is no self-skip, and no self-exemption.
const BASH_CONSTRUCTION: &str = concat!("Command::", "new(", "\"bash\"", ")");

/// (a) — the helper call.
const PIN_CALL: &str = "pin_loopd_env_files(";
/// (b) — both overrides set directly on the command. The `env("` prefix
/// demands an actual `.env()`/`cmd.env()` call, never a comment mention.
const LOOPD_ENV_SET: &str = "env(\"CHUG_LOOPD_ENV\"";
const PROFILE_ENV_SET: &str = "env(\"CHUG_PROFILE_ENV\"";

/// The argv mention that puts a construction site in scope: an `.arg(`
/// line whose argument names the script.
const ARG_CALL: &str = ".arg(";
const SCRIPT_ARG: &str = "loopd.sh";
/// The parse-only carve-out needle (bash -n reads, never execs).
const PARSE_ONLY_ARG: &str = ".arg(\"-n\")";

/// The chain is over once the command RUNS; the terminal line belongs to
/// the chain (a pin placed right before it must be seen).
const TERMINALS: [&str; 3] = [".output()", ".spawn()", ".status()"];

/// Refactor-noise bound: no spawn chain in the tree runs longer than this
/// (the longest observed is ~30 lines). A longer chain reds rather than
/// silently swallowing a pin past the cap.
const REGION_CAP_LINES: usize = 120;

/// The sanctioned exceptions: the harnesses that test the T213/T330 loaders
/// themselves — they set their own files, which IS their subject. Adding a
/// name here is a deliberate edit to this list, reviewed as such.
const SANCTIONED_EXCEPTIONS: [&str; 2] = ["loopd_env_loader.rs", "loopd_profile_loader.rs"];

/// Short, per the spec: the list may grow only deliberately, and it may
/// not grow into a second way to say "everything".
const EXCEPTION_LIST_CAP: usize = 4;

/// Floor against detector rot (the T293 brittle-awk lesson applied to this
/// scan's own vacuity surface): the tree carries 16 exec sites today; if a
/// rename or a new chain shape drops the scan's find below this floor, the
/// DETECTOR broke — fix the detector, do not lower the floor to get green.
const EXEC_SITE_FLOOR: usize = 10;

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()));
    for entry in entries {
        let path = entry
            .unwrap_or_else(|e| panic!("dir entry under {}: {e}", dir.display()))
            .path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Every `Command::new("bash")` construction site as a half-open,
/// 0-based line range `[start, end)`: from the construction line to the
/// chain's terminal call, bounded by the next construction site and the
/// region cap.
fn construction_regions(lines: &[&str]) -> Vec<(usize, usize)> {
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.contains(BASH_CONSTRUCTION))
        .map(|(i, _)| i)
        .collect();
    let mut regions = Vec::with_capacity(starts.len());
    for (n, &start) in starts.iter().enumerate() {
        let next_site = starts.get(n + 1).copied().unwrap_or(lines.len());
        let hard_stop = next_site.min(start + REGION_CAP_LINES).max(start + 1);
        let mut end = hard_stop;
        for (j, line) in lines.iter().enumerate().take(hard_stop).skip(start + 1) {
            if TERMINALS.iter().any(|terminal| line.contains(terminal)) {
                end = j + 1;
                break;
            }
        }
        regions.push((start, end));
    }
    regions
}

#[test]
fn every_loopd_spawning_harness_pins_the_env_file_loaders() {
    // The repo root the binary RUNS against (cargo sets the test binary's
    // cwd to the package root) — never a compile-time manifest-dir macro.
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let mut files = Vec::new();
    collect_rs_files(&root.join("tests"), &mut files);
    assert!(
        !files.is_empty(),
        "the walk found no .rs files under tests/ — an empty scan would make \
         this guard vacuously green"
    );
    files.sort();

    // Needle sanity (the T48 pattern): a mangled needle must not pass
    // the guard silently.
    assert_eq!(BASH_CONSTRUCTION, "Command::new(\"bash\")");

    let mut exec_sites = 0usize;
    let mut violations: Vec<String> = Vec::new();
    for file in &files {
        let text =
            std::fs::read_to_string(file).unwrap_or_else(|e| panic!("reading {}: {e}", file.display()));
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf8 file name under tests/")
            .to_string();
        let sanctioned = SANCTIONED_EXCEPTIONS.contains(&name.as_str());
        let lines: Vec<&str> = text.lines().collect();
        for (start, end) in construction_regions(&lines) {
            let chain = &lines[start..end];
            // In scope: the chain passes loopd.sh as an argument …
            let args_loopd = chain
                .iter()
                .any(|line| line.contains(ARG_CALL) && line.contains(SCRIPT_ARG));
            if !args_loopd {
                continue;
            }
            // … and RUNS it: `bash -n` parses without executing, no loader
            // can fire, so no pin is owed.
            if chain.iter().any(|line| line.contains(PARSE_ONLY_ARG)) {
                continue;
            }
            exec_sites += 1;
            if sanctioned {
                continue;
            }
            let pinned = chain.iter().any(|line| line.contains(PIN_CALL));
            let overridden = chain.iter().any(|line| line.contains(LOOPD_ENV_SET))
                && chain.iter().any(|line| line.contains(PROFILE_ENV_SET));
            if pinned || overridden {
                continue;
            }
            violations.push(format!(
                "{}:{} — spawn chain lines {}-{} execs loopd.sh through a bash \
                 Command construction with neither common::pin_loopd_env_files \
                 nor both loader overrides (CHUG_LOOPD_ENV + CHUG_PROFILE_ENV) \
                 set to ABSENT fixture paths",
                file.strip_prefix(&root).unwrap_or(file).display(),
                start + 1,
                start + 1,
                end,
            ));
        }
    }
    assert!(
        exec_sites >= EXEC_SITE_FLOOR,
        "the scan found only {exec_sites} loopd-execing sites (floor \
         {EXEC_SITE_FLOOR}) — the detector rotted (a rename, a new chain \
         shape); fix the detector, do not lower the floor"
    );
    assert!(
        violations.is_empty(),
        "T346 guard fired — {} loopd-spawning site(s) leave the T213/T330 \
         env-file loaders unpinned, so an operator-staged ~/.chug/profile.env \
         (or loopd.env) re-exports itself INTO the spawned fixture after the \
         harness's env_remove calls (the 2026-11-09 bite):\n  - {}\n\
         Fix: call common::pin_loopd_env_files(&mut cmd, <fixture root>) \
         before the chain runs, or set both CHUG_LOOPD_ENV and \
         CHUG_PROFILE_ENV to fixture paths that DO NOT EXIST (absent is a \
         silent no-op; existing-but-incomplete warns). Harnesses that test \
         the loaders themselves are the sanctioned exceptions named in this \
         file's SANCTIONED_EXCEPTIONS: {SANCTIONED_EXCEPTIONS:?}.",
        violations.len(),
        violations.join("\n  - "),
    );
}

#[test]
fn sanctioned_exception_list_stays_short_and_named() {
    assert!(
        SANCTIONED_EXCEPTIONS.len() <= EXCEPTION_LIST_CAP,
        "the sanctioned-exception list grew to {} entries (cap \
         {EXCEPTION_LIST_CAP}) — every entry is a hole in the T346 contract; \
         growing it is a deliberate, reviewed act, not a way to get the scan \
         green",
        SANCTIONED_EXCEPTIONS.len()
    );
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    for name in SANCTIONED_EXCEPTIONS {
        let path = root.join("tests").join(name);
        assert!(
            path.is_file(),
            "sanctioned exception {name:?} no longer names an on-disk \
             tests/*.rs — update the list (the loader harnesses that set \
             their own files are the canonical entries)"
        );
    }
    // The canonical pair must stay NAMED: emptying the list would look
    // green today (both harnesses currently drive the loader fragments via
    // `bash -c`, so the main scan does not lean on the carve-out) and would
    // silently re-expose the first harness that execs loopd.sh to test a
    // loader.
    for canonical in ["loopd_env_loader.rs", "loopd_profile_loader.rs"] {
        assert!(
            SANCTIONED_EXCEPTIONS.contains(&canonical),
            "the canonical loader-harness exception {canonical:?} was dropped \
             from SANCTIONED_EXCEPTIONS — restore it or re-review the T346 \
             contract"
        );
    }
}

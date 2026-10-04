//! T213 — the loopd.sh K7 env-file loader pin (the T205 validator's M4
//! survivor). The loader moved BYTE-IDENTICAL out of inline loopd.sh into
//! scripts/loopd_env_loader.sh as `loopd_load_env_file` (the sourceable
//! seam), and these tests drive the REAL fragment via `bash -c` with
//! fixture env files in tempdirs — no supervisor spawn. Legs: the
//! allowlist (the three judge keys, nothing else), explicit-env-wins, the
//! tolerance shapes (quotes / `export ` prefix / CR / blank / `#`
//! comment), malformed lines counted, and the hygiene rule — the file's
//! CONTENTS never reach the log, only the path and a skipped-line COUNT;
//! an absent file is a silent no-op (public churn).
//!
//! House style: the tests/loopd_model_routing.rs pattern — drive the real
//! script from the checkout the binary RUNS against (cwd = the package
//! root, never the compile-time manifest-dir macro; T48), with static pins
//! on the loopd.sh wiring next to the behavioral legs.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

// --- fixture values (all fake) ----------------------------------------------
//
// Built into fixture text through `format!` placeholders so NO repo source
// line ever carries the `HF_TOKEN=<value>` assignment shape the repo-wide
// no_secret_spill guard bans — the same no-allowlist choice as the canary
// pin (verified green with these consts present).

const FX_CHECKPOINT: &str = "org/laya-judge@rev123";
const FX_TOKEN: &str = "hf_FIXTURE_TOKEN_VALUE_42";
const FX_ENDPOINT: &str = "https://artifactory.example.com/hf";
const FX_FOREIGN_KEY: &str = "AWS_SECRET";
const FX_FOREIGN_VALUE: &str = "AKIAFIXTURESECRET99";
const FX_MALFORMED: &str = "this line has no equals sign";
const FX_FROMENV: &str = "fromenv";
const FX_FROMFILE: &str = "fromfile";
const FX_DQ_CHECKPOINT: &str = "org/model@quoted";
const FX_SQ_TOKEN: &str = "hf_SINGLE_QUOTED_42";
const FX_CR_ENDPOINT: &str = "https://cr.example.com";

/// The probed keys, printed as `PROBE <key>=<value>` after the load. Each
/// is `env_remove`d from the child unless the leg presets it, so no host
/// environment can leak into a leg's expectation.
const PROBE_KEYS: [&str; 4] = [
    "CHUG_LAYA_CHECKPOINT",
    "HF_TOKEN",
    "CHUG_HF_ENDPOINT",
    "AWS_SECRET",
];

/// Run the REAL fragment under the supervisor's shell regime: source it,
/// point `$LOOPD_ENV_FILE` at `env_file` (the absent-file leg passes a path
/// that was never created), stub `ts`, aim `$LOG` at `log`, call
/// `loopd_load_env_file`, then print the probed keys' post-load values.
fn run_loader(env_file: &Path, preset: &[(&str, &str)], log: &Path) -> Output {
    let fragment = repo_root().join("scripts/loopd_env_loader.sh");
    let mut script = String::from("set -euo pipefail\n");
    script += &format!(". '{}'\n", fragment.display());
    script += &format!("LOOPD_ENV_FILE='{}'\n", env_file.display());
    script += &format!("LOG='{}'\n", log.display());
    script += "ts() { printf 'TS-STUB\\n'; }\n";
    script += "loopd_load_env_file\n";
    for key in PROBE_KEYS {
        script += &format!("printf 'PROBE {key}=%s\\n' \"${{{key}-}}\"\n");
    }
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(&script);
    for key in PROBE_KEYS {
        match preset.iter().find(|(k, _)| *k == key) {
            Some((_, v)) => {
                cmd.env(key, v);
            }
            None => {
                cmd.env_remove(key);
            }
        }
    }
    cmd.output().expect("bash -c runs the loader fragment")
}

/// The PROBE lines as (key, value) pairs.
fn probes(out: &Output) -> Vec<(String, String)> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("PROBE "))
        .map(|l| {
            let (k, v) = l.split_once('=').expect("PROBE <key>=<value>");
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn probe_value<'a>(ps: &'a [(String, String)], key: &str) -> &'a str {
    ps.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn assert_ok(out: &Output, what: &str) {
    assert!(
        out.status.success(),
        "{what}: bash exited {:?}\nstdout:\n{}\nstderr:\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

fn log_text(log: &Path) -> Option<String> {
    std::fs::read_to_string(log).ok()
}

/// The count-only log line the loader writes when something was skipped:
/// the path and the COUNT, never a value byte.
fn assert_count_line(text: &str, fixture: &Path, count: usize) {
    let mut lines = text.lines();
    let line = lines.next().unwrap_or_default();
    assert!(
        lines.next().is_none(),
        "exactly one log line: {text:?}"
    );
    assert!(
        line.contains(&format!("{count} non-allowlisted or malformed line(s) ignored")),
        "the skipped COUNT {count}: {line}"
    );
    assert!(
        line.contains(fixture.to_str().unwrap()),
        "the path: {line}"
    );
    assert!(line.contains("contents never logged"), "{line}");
}

// --- behavioral legs ---------------------------------------------------------

#[test]
fn allowlisted_key_is_applied() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("env-file");
    std::fs::write(&fixture, format!("CHUG_LAYA_CHECKPOINT={FX_CHECKPOINT}\n")).unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[], &log);
    assert_ok(&out, "allowlisted leg");
    assert_eq!(
        probe_value(&probes(&out), "CHUG_LAYA_CHECKPOINT"),
        FX_CHECKPOINT
    );
    // nothing skipped -> nothing logged
    assert!(
        log_text(&log).is_none(),
        "no log write when nothing is skipped"
    );
}

#[test]
fn all_three_allowlisted_fourth_key_skipped_with_count_only_logging() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("env-file");
    std::fs::write(
        &fixture,
        format!(
            "CHUG_LAYA_CHECKPOINT={FX_CHECKPOINT}\nHF_TOKEN={FX_TOKEN}\n\
             CHUG_HF_ENDPOINT={FX_ENDPOINT}\n{FX_FOREIGN_KEY}={FX_FOREIGN_VALUE}\n"
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[], &log);
    assert_ok(&out, "allowlist leg");
    let ps = probes(&out);
    assert_eq!(probe_value(&ps, "CHUG_LAYA_CHECKPOINT"), FX_CHECKPOINT);
    assert_eq!(probe_value(&ps, "HF_TOKEN"), FX_TOKEN);
    assert_eq!(probe_value(&ps, "CHUG_HF_ENDPOINT"), FX_ENDPOINT);
    assert_eq!(
        probe_value(&ps, FX_FOREIGN_KEY),
        "",
        "a non-allowlisted key is NOT applied"
    );

    // the log carries the skipped COUNT and the path — and ZERO value bytes
    let text = log_text(&log).expect("the skipped line is logged");
    assert_count_line(&text, &fixture, 1);
    for value in [FX_CHECKPOINT, FX_TOKEN, FX_ENDPOINT, FX_FOREIGN_VALUE] {
        assert!(
            !text.contains(value),
            "fixture value leaked to the log: {value}"
        );
    }
}

#[test]
fn explicit_env_wins_over_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("env-file");
    std::fs::write(
        &fixture,
        format!("HF_TOKEN={FX_FROMFILE}\nNOT_ALLOWLISTED=zzz-foreign-value\n"),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[("HF_TOKEN", FX_FROMENV)], &log);
    assert_ok(&out, "explicit-env-wins leg");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("PROBE HF_TOKEN=fromenv\n"),
        "the explicit env survived the load: {stdout:?}"
    );
    assert!(
        !stdout.contains(FX_FROMFILE),
        "no fromfile anywhere in the env: {stdout:?}"
    );
    let text = log_text(&log).expect("the foreign line is logged");
    assert!(!text.contains(FX_FROMFILE), "no fromfile in the log: {text}");
    assert!(
        !text.contains("zzz-foreign-value"),
        "no foreign value in the log: {text}"
    );
    assert_count_line(&text, &fixture, 1);
}

#[test]
fn quotes_export_prefix_cr_blank_and_comment_legs() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("env-file");
    std::fs::write(
        &fixture,
        format!(
            "# a comment line — ignored\n\n\
             export CHUG_LAYA_CHECKPOINT=\"{FX_DQ_CHECKPOINT}\"\n\
             HF_TOKEN='{FX_SQ_TOKEN}'\n\
             CHUG_HF_ENDPOINT={FX_CR_ENDPOINT}\r\n"
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[], &log);
    assert_ok(&out, "tolerance leg");
    let ps = probes(&out);
    assert_eq!(
        probe_value(&ps, "CHUG_LAYA_CHECKPOINT"),
        FX_DQ_CHECKPOINT,
        "`export ` prefix + surrounding double quotes tolerated"
    );
    assert_eq!(
        probe_value(&ps, "HF_TOKEN"),
        FX_SQ_TOKEN,
        "surrounding single quotes tolerated"
    );
    assert_eq!(
        probe_value(&ps, "CHUG_HF_ENDPOINT"),
        FX_CR_ENDPOINT,
        "a trailing CR (CRLF file) is stripped"
    );
    // comment + blank lines are NOT counted as skipped -> count 0, no log
    assert!(
        log_text(&log).is_none(),
        "comment/blank lines are not `skipped` lines"
    );
}

#[test]
fn malformed_lines_are_counted_but_not_applied() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("env-file");
    std::fs::write(
        &fixture,
        format!("{FX_MALFORMED}\nCHUG_LAYA_CHECKPOINT={FX_CHECKPOINT}\n"),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[], &log);
    assert_ok(&out, "malformed leg");
    assert_eq!(
        probe_value(&probes(&out), "CHUG_LAYA_CHECKPOINT"),
        FX_CHECKPOINT,
        "the malformed line does not stop the load"
    );
    let text = log_text(&log).expect("the malformed line is counted");
    assert_count_line(&text, &fixture, 1);
}

#[test]
fn absent_file_is_a_silent_no_op() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = tmp.path().join("never-created.env"); // never written
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&fixture, &[], &log);
    assert_ok(&out, "absent-file leg (exit 0)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for key in PROBE_KEYS {
        assert!(
            stdout.contains(&format!("PROBE {key}=\n")),
            "no value applied for {key}"
        );
    }
    assert!(
        !stdout.contains("non-allowlisted"),
        "the loader produces no output of its own"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).is_empty(),
        "stderr stays clean"
    );
    assert!(log_text(&log).is_none(), "no LOG write");
}

// --- static pins on the wiring ----------------------------------------------

#[test]
fn loopd_sources_the_fragment_and_the_inline_block_is_gone() {
    let loopd = read("loopd.sh");
    // the ONE sourcing line, resolved relative to the script's own path —
    // never the cwd (loopd.sh may be invoked or re-exec'd from anywhere)
    assert!(
        loopd.contains(r#". "$(dirname "$0")/scripts/loopd_env_loader.sh""#),
        "loopd.sh must source the fragment relative to its own path"
    );
    assert!(
        loopd.contains("loopd_load_env_file\n"),
        "loopd.sh calls the function where the block stood"
    );
    assert!(
        loopd.contains("the loader lives in scripts/loopd_env_loader.sh"),
        "the T205 comment block names the fragment's home"
    );
    // the loader body exists EXACTLY ONCE — in the fragment, not inline
    assert!(
        !loopd.contains("_loopd_env_skipped"),
        "the inline block moved out of loopd.sh"
    );
    assert_eq!(
        loopd.matches("loopd_load_env_file() {").count(),
        0,
        "no function definition inline"
    );
    let frag = read("scripts/loopd_env_loader.sh");
    assert_eq!(
        frag.matches("loopd_load_env_file() {").count(),
        1,
        "the fragment defines the function once"
    );
    // the load-bearing lines moved VERBATIM (the byte-identity pin on the
    // move itself is proven at extraction time: diff of the moved block vs
    // the pre-extraction lines — recorded in the T213 commit message)
    assert!(
        frag.contains("CHUG_LAYA_CHECKPOINT=*|HF_TOKEN=*|CHUG_HF_ENDPOINT=*)"),
        "the allowlist moved verbatim"
    );
    assert!(
        frag.contains(
            "non-allowlisted or malformed line(s) ignored (contents never logged)"
        ),
        "the count-only log line moved verbatim"
    );
    assert!(
        frag.contains("explicit process env wins over the file"),
        "the explicit-env-wins rule moved verbatim"
    );
    assert!(
        frag.contains("unset _line _key _val _loopd_env_skipped"),
        "the scratch vars are still unset"
    );
    // the sourcing contract names all three caller-provided dependencies
    for dep in ["$LOOPD_ENV_FILE", "$LOG", "ts()"] {
        assert!(frag.contains(dep), "the fragment documents {dep}");
    }
}

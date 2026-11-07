//! T330 — the loopd.sh profile loader pin: the LLM profile contract's
//! runtime half.
//!
//! `install.sh --profile` (tests/install_sh.rs) validates and copies an
//! operator profile to `~/.chug/profile.env`; loopd.sh loads it ONCE at
//! supervisor startup via `loopd_load_profile` in
//! scripts/loopd_profile_loader.sh, translating the profile-owned keys to
//! the loop's native env BEFORE the LOOP_*_MODEL defaults apply (see the
//! T330 block in loopd.sh). These tests drive the REAL fragment via
//! `bash -c` with fixture profiles in tempdirs — no supervisor spawn — the
//! tests/loopd_env_loader.rs pattern (the T213/T205 seam): the mapping
//! (base URL/key → ANTHROPIC_*, the model pair → LOOP_*, CHUG_LANGFUSE_* →
//! LANGFUSE_*), the *_CMD escape hatch (fetch-once, value never logged),
//! already-shaped-env-wins, the absent-profile no-op (today's behavior,
//! no flag day), count-only hygiene, and the missing-required-keys log.
//!
//! House style: drive the real script from the checkout the binary RUNS
//! against (cwd = the package root, never the compile-time manifest-dir
//! macro; T48), with static pins on the loopd.sh wiring next to the
//! behavioral legs.

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
// line ever carries the `CHUG_LLM_API_KEY=<value>` assignment shape the
// repo-wide no-secret guard bans (T205; T330 extends it to this key) — the
// same no-allowlist choice as tests/loopd_env_loader.rs's fixture consts.

const FX_BASE_URL: &str = "https://proxy.example.com";
const FX_TOKEN: &str = "sk-FIXTURE_TOKEN_VALUE_42";
const FX_CMD_TOKEN: &str = "sk-FROM-CMD-FIXTURE-42";
const FX_REASONING: &str = "anthropic/reasoner-full-id";
const FX_CODING: &str = "anthropic/coder-full-id";
const FX_LF_HOST: &str = "http://127.0.0.1:3000";
const FX_LF_PK: &str = "pk-lf-fixture-42";
const FX_LF_SK: &str = "sk-lf-fixture-42";
const FX_FROMENV: &str = "fromenv";
const FX_FOREIGN_KEY: &str = "NOT_PROFILE_OWNED";
const FX_FOREIGN_VALUE: &str = "foreign-value-should-not-apply";
const FX_MALFORMED: &str = "this line has no equals sign";

/// The probed keys — the NATIVE env the loader translates into — printed as
/// `PROBE <key>=<value>` after the load. Each is `env_remove`d from the
/// child unless the leg presets it, so no host environment can leak into a
/// leg's expectation.
const PROBE_KEYS: [&str; 8] = [
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "LOOP_ORCH_MODEL",
    "LOOP_ROUTINE_MODEL",
    "LOOP_IDEATION_MODEL",
    "LANGFUSE_HOST",
    "LANGFUSE_PUBLIC_KEY",
    "LANGFUSE_SECRET_KEY",
];

/// Run the REAL fragment under the supervisor's shell regime: source it,
/// point `$PROFILE_ENV_FILE` at `profile` (the absent-file leg passes a
/// path that was never created), stub `ts`, aim `$LOG` at `log`, call
/// `loopd_load_profile`, then print the probed keys' post-load values.
/// `preset` is exported BEFORE the call (the already-shaped-env-wins legs).
fn run_loader(profile: &Path, preset: &[(&str, &str)], log: &Path) -> Output {
    run_loader_guarded(profile, preset, log, false)
}

/// The guarded-call shape for the loud-failure leg: `loopd_load_profile` is
/// invoked inside if/else so a `return 1` reaches the probe script as an
/// observable RC instead of killing the harness before it prints.
fn run_loader_guarded(
    profile: &Path,
    preset: &[(&str, &str)],
    log: &Path,
    guarded: bool,
) -> Output {
    let fragment = repo_root().join("scripts/loopd_profile_loader.sh");
    let mut script = String::from("set -euo pipefail\n");
    script += &format!(". '{}'\n", fragment.display());
    script += &format!("PROFILE_ENV_FILE='{}'\n", profile.display());
    script += &format!("LOG='{}'\n", log.display());
    script += "ts() { printf 'TS-STUB\\n'; }\n";
    if guarded {
        script += "if loopd_load_profile; then printf 'LOADED\\n'; else printf 'RC=%s\\n' \"$?\"; fi\n";
    } else {
        script += "loopd_load_profile\n";
    }
    for key in PROBE_KEYS {
        script += &format!("printf 'PROBE {key}=%s\\n' \"${{{key}-}}\"\n");
    }
    // the foreign key is probed too — the allowlist leg reads its absence
    script += &format!(
        "printf 'PROBE {FX_FOREIGN_KEY}=%s\\n' \"${{{FX_FOREIGN_KEY}-}}\"\n"
    );
    let mut cmd = Command::new("bash");
    cmd.arg("-c").arg(&script);
    for key in PROBE_KEYS
        .iter()
        .copied()
        .chain(std::iter::once(FX_FOREIGN_KEY))
    {
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

/// The full-contract profile text every mapping leg starts from.
fn full_profile_text() -> String {
    format!(
        "# a comment line — ignored\n\n\
         export CHUG_LLM_BASE_URL=\"{FX_BASE_URL}\"\n\
         CHUG_LLM_API_KEY='{FX_TOKEN}'\n\
         export CHUG_MODEL_REASONING={FX_REASONING}\n\
         export CHUG_MODEL_CODING={FX_CODING}\n\
         export CHUG_LANGFUSE_HOST={FX_LF_HOST}\n\
         export CHUG_LANGFUSE_PUBLIC_KEY='{FX_LF_PK}'\n\
         export CHUG_LANGFUSE_SECRET_KEY=\"{FX_LF_SK}\"\n"
    )
}

// --- behavioral legs ---------------------------------------------------------

#[test]
fn the_full_profile_maps_to_the_native_env() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    std::fs::write(&profile, full_profile_text()).unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "full-mapping leg");
    let ps = probes(&out);
    assert_eq!(probe_value(&ps, "ANTHROPIC_BASE_URL"), FX_BASE_URL);
    assert_eq!(probe_value(&ps, "ANTHROPIC_AUTH_TOKEN"), FX_TOKEN);
    assert_eq!(probe_value(&ps, "LOOP_ORCH_MODEL"), FX_REASONING);
    assert_eq!(probe_value(&ps, "LOOP_ROUTINE_MODEL"), FX_CODING);
    assert_eq!(probe_value(&ps, "LANGFUSE_HOST"), FX_LF_HOST);
    assert_eq!(probe_value(&ps, "LANGFUSE_PUBLIC_KEY"), FX_LF_PK);
    assert_eq!(probe_value(&ps, "LANGFUSE_SECRET_KEY"), FX_LF_SK);
    // the profile does NOT own the ideation lane (T326 pins its own default)
    assert_eq!(probe_value(&ps, "LOOP_IDEATION_MODEL"), "");
    // every line was allowlisted -> nothing skipped -> no log write
    assert!(
        log_text(&log).is_none(),
        "no log write when nothing is skipped"
    );
}

#[test]
fn explicit_env_wins_over_the_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    std::fs::write(&profile, full_profile_text()).unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(
        &profile,
        &[
            ("ANTHROPIC_AUTH_TOKEN", FX_FROMENV),
            ("LOOP_ORCH_MODEL", FX_FROMENV),
        ],
        &log,
    );
    assert_ok(&out, "explicit-env-wins leg");
    let ps = probes(&out);
    assert_eq!(
        probe_value(&ps, "ANTHROPIC_AUTH_TOKEN"),
        FX_FROMENV,
        "an already-shaped ANTHROPIC_AUTH_TOKEN is never overridden"
    );
    assert_eq!(
        probe_value(&ps, "LOOP_ORCH_MODEL"),
        FX_FROMENV,
        "an already-shaped LOOP_ORCH_MODEL is never overridden"
    );
    // keys with no explicit env still apply — partial profiles compose
    assert_eq!(probe_value(&ps, "ANTHROPIC_BASE_URL"), FX_BASE_URL);
    assert_eq!(probe_value(&ps, "LOOP_ROUTINE_MODEL"), FX_CODING);
}

#[test]
fn absent_profile_is_a_silent_no_op() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("never-installed.env"); // never written
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "absent-profile leg (exit 0)");
    let stdout = String::from_utf8_lossy(&out.stdout);
    for key in PROBE_KEYS {
        assert!(
            stdout.contains(&format!("PROBE {key}=\n")),
            "no value applied for {key}"
        );
    }
    assert!(
        !stdout.contains("missing required"),
        "the loader produces no output of its own"
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).is_empty(),
        "stderr stays clean"
    );
    assert!(log_text(&log).is_none(), "no LOG write");
    // the loader function still exists (the fragment defines it) — the leg
    // proves the ABSENT FILE branch, not a missing fragment
    assert!(read("scripts/loopd_profile_loader.sh").contains("loopd_load_profile() {"));
}

#[test]
fn cmd_form_fetches_the_key_once_and_never_logs_it() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    std::fs::write(
        &profile,
        format!(
            "CHUG_LLM_BASE_URL={FX_BASE_URL}\n\
             CHUG_LLM_API_KEY_CMD=\"printf %s {FX_CMD_TOKEN}\"\n\
             CHUG_MODEL_REASONING={FX_REASONING}\n\
             CHUG_MODEL_CODING={FX_CODING}\n"
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "cmd-form leg");
    assert_eq!(
        probe_value(&probes(&out), "ANTHROPIC_AUTH_TOKEN"),
        FX_CMD_TOKEN,
        "the *_CMD stdout becomes ANTHROPIC_AUTH_TOKEN"
    );
    let text = log_text(&log).unwrap_or_default();
    assert!(
        !text.contains(FX_CMD_TOKEN) && !text.contains(FX_TOKEN),
        "the fetched (or any) key value never reaches the log: {text:?}"
    );
}

#[test]
fn literal_key_wins_over_cmd_and_env_wins_over_both() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    // The CMD would leave a marker file if it ran — the precedence assertions
    // below read its absence, not a probe value.
    let marker = tmp.path().join("cmd-ran");
    std::fs::write(
        &profile,
        format!(
            "CHUG_LLM_BASE_URL={FX_BASE_URL}\n\
             CHUG_LLM_API_KEY={FX_TOKEN}\n\
             CHUG_LLM_API_KEY_CMD=\"touch '{}'; printf %s {FX_CMD_TOKEN}\"\n\
             CHUG_MODEL_REASONING={FX_REASONING}\n\
             CHUG_MODEL_CODING={FX_CODING}\n",
            marker.display()
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "literal-vs-cmd leg");
    assert_eq!(
        probe_value(&probes(&out), "ANTHROPIC_AUTH_TOKEN"),
        FX_TOKEN,
        "the literal CHUG_LLM_API_KEY wins over the CMD form"
    );
    assert!(
        !marker.exists(),
        "the CMD is not evaluated when the literal key already answered"
    );
    // explicit env beats both — the CMD must not run even then
    let log2 = tmp.path().join("loopd2.log");
    let out = run_loader(&profile, &[("ANTHROPIC_AUTH_TOKEN", FX_FROMENV)], &log2);
    assert_ok(&out, "env-vs-both leg");
    assert_eq!(probe_value(&probes(&out), "ANTHROPIC_AUTH_TOKEN"), FX_FROMENV);
    assert!(!marker.exists(), "the CMD is not evaluated when env won");
    assert!(log_text(&log2).is_none(), "no log write on the env-wins path");
}

#[test]
fn cmd_failure_aborts_startup_loudly() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    std::fs::write(
        &profile,
        format!(
            "CHUG_LLM_BASE_URL={FX_BASE_URL}\n\
             CHUG_LLM_API_KEY_CMD=\"exit 3\"\n\
             CHUG_MODEL_REASONING={FX_REASONING}\n\
             CHUG_MODEL_CODING={FX_CODING}\n"
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader_guarded(&profile, &[], &log, true);
    assert_ok(&out, "the harness itself exits 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("RC=1\n"),
        "a failing fetch must return 1 (loopd.sh's set -e then aborts startup): {stdout:?}"
    );
    assert!(!stdout.contains("LOADED"), "{stdout:?}");
    let text = log_text(&log).expect("the failure is logged");
    assert!(
        text.contains("CHUG_LLM_API_KEY_CMD failed (rc=3)"),
        "the log names the failed fetch and its rc: {text:?}"
    );
    assert!(
        text.contains("set CHUG_LLM_API_KEY"),
        "the log names the fix: {text:?}"
    );
    assert!(!text.contains(FX_BASE_URL), "no values in the log: {text:?}");
}

#[test]
fn unknown_keys_are_skipped_with_count_only_logging() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    std::fs::write(
        &profile,
        format!(
            "CHUG_LLM_BASE_URL={FX_BASE_URL}\n\
             CHUG_LLM_API_KEY={FX_TOKEN}\n\
             CHUG_MODEL_REASONING={FX_REASONING}\n\
             CHUG_MODEL_CODING={FX_CODING}\n\
             {FX_FOREIGN_KEY}={FX_FOREIGN_VALUE}\n\
             {FX_MALFORMED}\n"
        ),
    )
    .unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "allowlist leg");
    let ps = probes(&out);
    assert_eq!(probe_value(&ps, "ANTHROPIC_BASE_URL"), FX_BASE_URL);
    assert_eq!(
        probe_value(&ps, FX_FOREIGN_KEY),
        "",
        "a non-profile-owned key is NOT applied"
    );
    // the log carries the skipped COUNT and the path — and ZERO value bytes
    let text = log_text(&log).expect("the skipped lines are logged");
    let mut lines = text.lines();
    let line = lines.next().unwrap_or_default();
    assert!(
        lines.next().is_none(),
        "exactly one log line (nothing required is missing): {text:?}"
    );
    assert!(
        line.contains("2 non-allowlisted or malformed line(s) ignored"),
        "the skipped COUNT 2: {line}"
    );
    assert!(line.contains("contents never logged"), "{line}");
    for value in [FX_BASE_URL, FX_TOKEN, FX_REASONING, FX_FOREIGN_VALUE] {
        assert!(
            !text.contains(value),
            "fixture value leaked to the log: {value}"
        );
    }
}

#[test]
fn a_required_key_missing_from_the_file_is_named_in_the_log() {
    let tmp = tempfile::tempdir().unwrap();
    let profile = tmp.path().join("profile.env");
    // base URL only — the models and the credential pair are absent
    std::fs::write(&profile, format!("CHUG_LLM_BASE_URL={FX_BASE_URL}\n")).unwrap();
    let log = tmp.path().join("loopd.log");
    let out = run_loader(&profile, &[], &log);
    assert_ok(&out, "missing-keys leg (the loader tolerates and applies)");
    assert_eq!(probe_value(&probes(&out), "ANTHROPIC_BASE_URL"), FX_BASE_URL);
    let text = log_text(&log).expect("the missing keys are logged");
    for key in [
        "CHUG_LLM_API_KEY (or CHUG_LLM_API_KEY_CMD)",
        "CHUG_MODEL_REASONING",
        "CHUG_MODEL_CODING",
    ] {
        assert!(text.contains(key), "the log names {key}: {text:?}");
    }
    assert!(text.contains("docs/install-profiles.md"), "{text:?}");
    assert!(!text.contains(FX_BASE_URL), "no values in the log: {text:?}");
}

// --- static pins on the wiring ----------------------------------------------

#[test]
fn loopd_sources_the_fragment_before_the_model_defaults() {
    let loopd = read("loopd.sh");
    // the ONE sourcing line, resolved relative to the script's own path —
    // never the cwd (loopd.sh may be invoked or re-exec'd from anywhere)
    assert!(
        loopd.contains(r#". "$(dirname "$0")/scripts/loopd_profile_loader.sh""#),
        "loopd.sh must source the profile fragment relative to its own path"
    );
    assert!(loopd.contains("loopd_load_profile\n"), "loopd.sh calls the loader");
    // the profile path: CHUG_PROFILE_ENV override, else $HOME/.chug/profile.env
    assert!(
        loopd.contains(r#"PROFILE_ENV_FILE="${CHUG_PROFILE_ENV:-$HOME/.chug/profile.env}""#),
        "loopd.sh must default the profile to $HOME/.chug/profile.env with the CHUG_PROFILE_ENV override"
    );
    // the load must run BEFORE the T81 model defaults — a profile's
    // CHUG_MODEL_* only reaches the cycle if the `${LOOP_*_MODEL:-default}`
    // assignments see it
    let load = loopd
        .find("loopd_load_profile\n")
        .expect("the loader call");
    let orch = loopd
        .find("LOOP_ORCH_MODEL=\"${LOOP_ORCH_MODEL:-")
        .expect("the orchestrator default");
    let routine = loopd
        .find("LOOP_ROUTINE_MODEL=\"${LOOP_ROUTINE_MODEL:-")
        .expect("the routine default");
    assert!(
        load < orch && load < routine,
        "loopd_load_profile must run BEFORE the LOOP_*_MODEL defaults apply \
         (profile values otherwise lose to the defaults they should replace)"
    );
    // the loader body exists EXACTLY ONCE — in the fragment, not inline
    assert_eq!(
        loopd.matches("loopd_load_profile() {").count(),
        0,
        "no function definition inline"
    );
    let frag = read("scripts/loopd_profile_loader.sh");
    assert_eq!(
        frag.matches("loopd_load_profile() {").count(),
        1,
        "the fragment defines the function once"
    );
    // the sourcing contract names all three caller-provided dependencies
    for dep in ["$PROFILE_ENV_FILE", "$LOG", "ts()"] {
        assert!(frag.contains(dep), "the fragment documents {dep}");
    }
}

#[test]
fn the_fragment_implements_exactly_the_profile_contract() {
    let frag = read("scripts/loopd_profile_loader.sh");
    // every profile-owned key appears in the allowlist pattern (the langfuse
    // class is asserted separately — it is a prefix, not a KEY=* alternative)
    for key in [
        "CHUG_LLM_BASE_URL",
        "CHUG_LLM_API_KEY_CMD",
        "CHUG_LLM_API_KEY",
        "CHUG_MODEL_REASONING",
        "CHUG_MODEL_CODING",
    ] {
        assert!(
            frag.contains(&format!("{key}=*")),
            "the allowlist must carry {key}"
        );
    }
    // the langfuse entry is the PREFIX class (CHUG_LANGFUSE_*), not a KEY=*
    // pattern — every other owned key is an exact KEY=* alternative
    assert!(
        frag.contains("CHUG_LANGFUSE_*)"),
        "the allowlist must carry the CHUG_LANGFUSE_* prefix class"
    );
    // ... and every translation the contract names
    for mapping in [
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_AUTH_TOKEN",
        "LOOP_ORCH_MODEL",
        "LOOP_ROUTINE_MODEL",
    ] {
        assert!(frag.contains(mapping), "the fragment maps to {mapping}");
    }
    assert!(
        frag.contains(r#"loopd_profile_apply "${_key#CHUG_}""#),
        "CHUG_LANGFUSE_* maps by prefix strip (CHUG_LANGFUSE_HOST -> LANGFUSE_HOST)"
    );
    // the docs the errors point at exist (the loader's log lines name them)
    assert!(
        repo_root().join("docs/install-profiles.md").is_file(),
        "docs/install-profiles.md must exist — the loader's missing-key log names it"
    );
    // the hygiene gate: the fragment carries the count-only wording and the
    // never-logged promise, as the T205 env loader does
    assert!(
        frag.contains("contents never logged"),
        "the hygiene promise is on every log line"
    );
}

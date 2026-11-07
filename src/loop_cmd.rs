//! T334 — the `chug loop` subcommand: init + run the continuous loop in
//! ANY repo, no clone of chug required.
//!
//! Two adoption-blocking flaws in the clone-based quickstart (the operator,
//! 2026-11-07): it told pilots to `git clone` chug to get `loopd.sh`, and
//! the cloned loop's first experience was the loop consuming CHUG'S OWN
//! backlog instead of the pilot's repo — exactly backwards. This module
//! fixes both: `chug loop init` scaffolds the CURRENT repo's own generic
//! loop contract (LOOP-SPEC.md, TODO.md with the empty queue + the T326
//! `## Proposed` lane, `.chug/`, the `.gitignore` line), and `chug loop
//! run` execs a generated pilot supervisor against the CURRENT directory —
//! the loop's queue is this repo's TODO.md, worktrees live under this
//! repo, commits land on this repo's branches. After `install.sh`, NO
//! clone of tampajohn/chug is ever required: the supervisor script and
//! both env/profile loader fragments (T205 + T330 — the same files chug's
//! own `loopd.sh` sources, embedded at build time from this checkout) are
//! written into the repo's gitignored `.chug/loopd/` and the cycle
//! launches the RUNNING binary, not a built-from-source one.
//!
//! `init` is idempotent: existing files are never clobbered (each is
//! reported `created` / `kept`); `--force` overwrites with a `.bak`
//! backup beside each replaced file. `run` verifies the scaffold BEFORE
//! any write — a missing scaffold prints `run chug loop init first` and
//! exits 2, never a half-start. Running `chug loop run` inside the chug
//! repo itself needs no special case: chug's own LOOP-SPEC.md/TODO.md are
//! the scaffold, `run` never writes them, and the generated supervisor
//! works chug's own files from the repo root exactly as `./loopd.sh` does.
//!
//! The one test/operator seam: `CHUG_LOOPD_BIN` overrides the supervisor
//! script (used as-is when executable; the `CHUG_DAEMON_BIN` precedent).
//! `chug loop run` always regenerates the derived assets first, then execs
//! the override when set — so a pilot can ship their own supervisor
//! without forking chug.

use std::path::{Path, PathBuf};

use anyhow::Context;

/// The generic LOOP-SPEC template: the queue rules, the check:-line
/// contract, the gate expectations. Deliberately NOT chug's own
/// self-referential doctrine (no EVALUATION.md freshness rule, no model
/// routing, no wrap assertions) — those are chug-repo-specific; a pilot
/// repo gets the minimal contract a cold loop can actually obey.
pub const LOOP_SPEC_TEMPLATE: &str = include_str!("loop_cmd/loop-spec-template.md");

/// The TODO.md scaffold: the header, the EMPTY queue table (a fresh pilot
/// repo has no rows — the supervisor's empty-queue gate keeps the loop
/// from manufacturing work), and the T326 `## Proposed` lane header.
pub const TODO_TEMPLATE: &str = include_str!("loop_cmd/todo-template.md");

/// The generated pilot supervisor. Sourced fragments resolve relative to
/// the script's own path (`$(dirname "$0")`), so no template
/// substitution is needed: written to `<repo>/.chug/loopd/loopd.sh`, its
/// `../..` IS the repo root.
const LOOPD_PILOT_TEMPLATE: &str = include_str!("loop_cmd/loopd-pilot.sh");

/// T205 — the judge env-file loader, embedded from the LIVE checkout file
/// (no copy to drift). `chug loop run` writes it beside the generated
/// supervisor, which sources it exactly as chug's own loopd.sh does.
const ENV_LOADER: &str = include_str!("../scripts/loopd_env_loader.sh");

/// T330 — the LLM profile loader, embedded from the LIVE checkout file
/// (no copy to drift). This is the "environment resolution identical to
/// today's loopd" leg: profile.env lands ANTHROPIC_*/model env the same
/// way at pilot-supervisor startup.
const PROFILE_LOADER: &str = include_str!("../scripts/loopd_profile_loader.sh");

/// The supervisor-override env var (the `CHUG_DAEMON_BIN` precedent): an
/// executable file set in the env wins over the generated script. The
/// loop_subcommand tests use it as the stubbed-exec seam.
const SUPERVISOR_OVERRIDE_ENV: &str = "CHUG_LOOPD_BIN";

/// The generated supervisor's path inside the repo's gitignored runtime
/// dir. Never collides with chug's own loopd state (loopd.log, cycle
/// logs, the dormant latch live beside it, not at this name).
fn generated_supervisor_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("loopd").join("loopd.sh")
}

/// `chug loop init` — scaffold the loop's files in `cwd`. Idempotent:
/// existing files are never clobbered; `--force` overwrites with a
/// `.bak` backup beside each replaced file. Returns one report line per
/// scaffolded artifact (the caller prints them), each naming the artifact
/// and its disposition (`created` / `kept` / the `--force` replacement
/// with its backup path).
pub fn init(cwd: &Path, force: bool) -> anyhow::Result<Vec<String>> {
    let mut report = Vec::new();

    // 1. The .chug/ runtime dir (created / kept — never a clobber leg).
    let chug_dir = cwd.join(".chug");
    if chug_dir.is_dir() {
        report.push("loop init: .chug/ kept".to_string());
    } else {
        std::fs::create_dir_all(&chug_dir)
            .with_context(|| format!("creating {}", chug_dir.display()))?;
        report.push("loop init: .chug/ created".to_string());
    }

    // 2. + 3. The two scaffolded files (the generic contract + the queue).
    for (name, template) in [
        ("LOOP-SPEC.md", LOOP_SPEC_TEMPLATE),
        ("TODO.md", TODO_TEMPLATE),
    ] {
        let path = cwd.join(name);
        if path.exists() {
            if force {
                let backup = cwd.join(format!("{name}.bak"));
                std::fs::copy(&path, &backup).with_context(|| {
                    format!("backing up {} to {}", path.display(), backup.display())
                })?;
                std::fs::write(&path, template)
                    .with_context(|| format!("overwriting {}", path.display()))?;
                report.push(format!(
                    "loop init: {name} replaced (backup {name}.bak)"
                ));
            } else {
                report.push(format!("loop init: {name} kept"));
            }
        } else {
            std::fs::write(&path, template)
                .with_context(|| format!("writing {}", path.display()))?;
            report.push(format!("loop init: {name} created"));
        }
    }

    // 4. The .gitignore line: append `.chug/` when absent (append-only —
    // the operator's existing ignores are never reordered or rewritten).
    let gitignore = cwd.join(".gitignore");
    let already_ignored = std::fs::read_to_string(&gitignore)
        .map(|text| text.lines().any(|line| line.trim() == ".chug/"))
        .unwrap_or(false);
    if already_ignored {
        report.push("loop init: .gitignore kept (.chug/ already ignored)".to_string());
    } else if gitignore.exists() {
        let mut text = std::fs::read_to_string(&gitignore)
            .with_context(|| format!("reading {}", gitignore.display()))?;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(".chug/\n");
        std::fs::write(&gitignore, text)
            .with_context(|| format!("updating {}", gitignore.display()))?;
        report.push("loop init: .gitignore updated (.chug/ appended)".to_string());
    } else {
        std::fs::write(&gitignore, ".chug/\n")
            .with_context(|| format!("writing {}", gitignore.display()))?;
        report.push("loop init: .gitignore created".to_string());
    }

    Ok(report)
}

/// Write the derived supervisor assets into `cwd`'s gitignored `.chug/`:
/// the pilot supervisor script (mode 755) plus both loader fragments it
/// sources. Regenerated on every `chug loop run` — the assets derive from
/// the RUNNING binary, so a dogfood-upgraded chug regenerates them too.
fn write_supervisor_assets(cwd: &Path) -> anyhow::Result<PathBuf> {
    let state = cwd.join(".chug").join("loopd");
    let scripts = state.join("scripts");
    std::fs::create_dir_all(&scripts)
        .with_context(|| format!("creating {}", scripts.display()))?;
    let script = generated_supervisor_path(cwd);
    std::fs::write(&script, LOOPD_PILOT_TEMPLATE)
        .with_context(|| format!("writing {}", script.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("chmod 755 {}", script.display()))?;
    }
    for (name, body) in [
        ("loopd_env_loader.sh", ENV_LOADER),
        ("loopd_profile_loader.sh", PROFILE_LOADER),
    ] {
        let path = scripts.join(name);
        std::fs::write(&path, body)
            .with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(script)
}

/// `chug loop run` — verify the scaffold, write the generated supervisor,
/// and exec it against `cwd`. On success (unix) this NEVER RETURNS: the
/// process image is replaced by the supervisor (`exec`, the same-pid
/// handoff — the supervisor's pid is the pid the operator backgrounded).
/// The missing-scaffold leg prints the init hint and exits 2 — before any
/// write, so a scaffold-less repo is never half-started.
pub fn run(cwd: &Path) -> anyhow::Result<i32> {
    // 1. Verify the scaffold BEFORE any write (no half-starts).
    let mut missing = Vec::new();
    if !cwd.join("LOOP-SPEC.md").is_file() {
        missing.push("LOOP-SPEC.md");
    }
    if !cwd.join("TODO.md").is_file() {
        missing.push("TODO.md");
    }
    if !cwd.join(".chug").is_dir() {
        missing.push(".chug/");
    }
    if !missing.is_empty() {
        eprintln!(
            "chug loop: missing scaffold ({}) in {} — run chug loop init first",
            missing.join(", "),
            cwd.display()
        );
        return Ok(2);
    }

    // 2. Regenerate the derived assets (idempotent; never touches the
    // scaffolded files above — running inside the chug repo keeps chug's
    // own LOOP-SPEC.md/TODO.md byte-identical).
    let generated = write_supervisor_assets(cwd)?;

    // 3. Resolve the supervisor: the explicit override wins when it is an
    // executable file (a set-but-unusable value is a hard error — an
    // override that silently fell through to the generated script would
    // run a different supervisor than the operator asked for).
    let override_path = std::env::var_os(SUPERVISOR_OVERRIDE_ENV)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    let supervisor = match override_path {
        Some(path) => {
            if !path.is_file() {
                anyhow::bail!(
                    "{SUPERVISOR_OVERRIDE_ENV}={} is not an executable file — fix the path or unset it to use the generated supervisor",
                    path.display()
                );
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&path)
                    .with_context(|| format!("statting {}", path.display()))?
                    .permissions()
                    .mode();
                if mode & 0o111 == 0 {
                    anyhow::bail!(
                        "{SUPERVISOR_OVERRIDE_ENV}={} is not executable — chmod +x it or unset it",
                        path.display()
                    );
                }
            }
            path
        }
        None => generated,
    };

    // 4. The handoff line — the exec below replaces this process, so the
    // operator (and the tests) see exactly which script took over and
    // where it runs, before the pid is gone.
    eprintln!("chug loop: supervisor {} (cwd {})", supervisor.display(), cwd.display());
    eprintln!(
        "chug loop: stop with `{} stop` (exits after the current cycle), status with `{} status`",
        supervisor.display(),
        supervisor.display()
    );

    // 5. Exec the supervisor against the CURRENT directory. The running
    // binary rides CHUG_LOOP_BIN so the cycles launch the installed chug —
    // never a build from a clone (the T334 contract).
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("chug"));
    let mut command = std::process::Command::new("bash");
    command
        .arg(&supervisor)
        .arg("run")
        .current_dir(cwd)
        .env("CHUG_LOOP_BIN", &exe);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        Err(anyhow::Error::from(err))
            .with_context(|| format!("execing supervisor {}", supervisor.display()))
    }
    #[cfg(not(unix))]
    {
        let status = command
            .status()
            .with_context(|| format!("running supervisor {}", supervisor.display()))?;
        Ok(status.code().unwrap_or(1))
    }
}

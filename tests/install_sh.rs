//! T100 — install.sh guard: the one-liner installer works, or fails loudly.
//!
//! `install.sh` is the public front door (served as chug.sh/install.sh, run
//! by strangers with `curl | sh`), so its failure legs matter as much as its
//! success path: a checksum that does not match must install NOTHING, a
//! missing release must name the build-from-source fix, an unsupported
//! platform must name the supported set, and the whole file must be POSIX sh
//! (`/bin/sh` on Debian is dash — one bashism and the one-liner is dead on
//! the fleet's linux boxes). Every fixture is a fake "release" served over
//! `file://` via CHUG_RELEASE_URL_BASE — no network, and the checksum sidecar
//! is computed with the same tool precedence the script itself uses.
//!
//! The uname→platform mapping is swept leg by leg (validator FINDING 2): the
//! original suite only exercised the CHUG_INSTALL_PLATFORM override, so the
//! real detection path had ZERO coverage and a platform-map-flip mutant
//! survived green. Every (os, mach) leg now has a killing test — driven via
//! the CHUG_INSTALL_OS/CHUG_INSTALL_MACH overrides the script consults
//! before `uname`, plus one wiring test with `uname` itself shimmed on PATH.
//!
//! T48 doctrine: the script under test is resolved from the RUNTIME checkout
//! (`current_dir()`), never the compile-time manifest-dir macro.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn install_sh() -> PathBuf {
    let p = std::env::current_dir()
        .expect("cargo sets the test cwd to the package root")
        .join("install.sh");
    assert!(
        p.is_file(),
        "install.sh must exist at the repo root — the one-liner was dropped"
    );
    p
}

/// The env override set every fixture run shares: file:// asset base, forced
/// platform, install dir under the fixture, minimal PATH (no install dir).
struct Run {
    script: PathBuf,
    release_dir: PathBuf,
    home: PathBuf,
    install_dir: PathBuf,
}

impl Run {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!("chug-t100-inst-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("fixture base");
        let release_dir = base.join("release");
        let home = base.join("home");
        std::fs::create_dir_all(&release_dir).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        Run {
            script: install_sh(),
            release_dir,
            home: home.clone(),
            install_dir: home.join(".local").join("bin"),
        }
    }

    /// Build a fake release pair: a tarball carrying an executable `chug`
    /// stub + a correct `<hash>  <archive>` sha256 sidecar.
    fn make_release(&self, platform: &str, stub_stdout: &str) -> PathBuf {
        let staging = self.release_dir.join(format!("stage-{platform}"));
        std::fs::create_dir_all(&staging).unwrap();
        let stub = staging.join("chug");
        std::fs::write(&stub, format!("#!/bin/sh\necho {stub_stdout}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let tarball = self.release_dir.join(format!("chug-{platform}.tar.gz"));
        let ok = Command::new("tar")
            .args(["czf"])
            .arg(&tarball)
            .arg("chug")
            .current_dir(&staging)
            .output()
            .expect("tar czf");
        assert!(ok.status.success(), "tar czf failed: {}", String::from_utf8_lossy(&ok.stderr));
        let hash = sha256(&tarball);
        std::fs::write(
            self.release_dir.join(format!("chug-{platform}.tar.gz.sha256")),
            format!("{hash}  chug-{platform}.tar.gz\n"),
        )
        .unwrap();
        tarball
    }

    fn run(&self, platform: &str, path_extra: Option<&Path>) -> Output {
        self.base_cmd(path_extra)
            .env("CHUG_INSTALL_PLATFORM", platform)
            .output()
            .expect("spawning sh install.sh")
    }

    /// Command every fixture run shares: file:// asset base, install dir under
    /// the fixture, minimal PATH (no install dir).
    fn base_cmd(&self, path_extra: Option<&Path>) -> Command {
        let mut path = "/usr/bin:/bin:/usr/sbin:/sbin".to_string();
        if let Some(extra) = path_extra {
            path = format!("{}:{}", extra.display(), path);
        }
        let mut cmd = Command::new("sh");
        cmd.arg(&self.script)
            .env("CHUG_RELEASE_URL_BASE", format!("file://{}", self.release_dir.display()))
            .env("CHUG_INSTALL_DIR", &self.install_dir)
            .env("HOME", &self.home)
            .env("PATH", path);
        cmd
    }

    /// Run the uname→platform mapping via the CHUG_INSTALL_OS/CHUG_INSTALL_MACH
    /// overrides the script consults before `uname` (POSIX env, no PATH games).
    /// The platform override is NOT set — the mapping under test is the only
    /// thing standing between the (os, mach) pair and the asset name.
    fn run_detect(&self, os: &str, mach: &str) -> Output {
        self.base_cmd(None)
            .env("CHUG_INSTALL_OS", os)
            .env("CHUG_INSTALL_MACH", mach)
            .output()
            .expect("spawning sh install.sh")
    }

    /// Run with NO overrides at all: detection must come from `uname` itself.
    /// `uname` is shimmed on PATH (reading FAKE_UNAME_OS/FAKE_UNAME_MACH) so
    /// the fixture stays deterministic on any host.
    fn run_uname_shimmed(&self, shim_dir: &Path, os: &str, mach: &str) -> Output {
        self.base_cmd(Some(shim_dir))
            .env("FAKE_UNAME_OS", os)
            .env("FAKE_UNAME_MACH", mach)
            .output()
            .expect("spawning sh install.sh")
    }

    /// A PATH dir whose `uname` reports FAKE_UNAME_OS / FAKE_UNAME_MACH.
    fn uname_shim(&self) -> PathBuf {
        let dir = self.home.join("uname-shim");
        std::fs::create_dir_all(&dir).unwrap();
        // POSIX sh, exactly the two invocations install.sh makes.
        std::fs::write(
            dir.join("uname"),
            "#!/bin/sh\ncase \"$1\" in\n  -s) printf '%s\\n' \"$FAKE_UNAME_OS\" ;;\n  -m) printf '%s\\n' \"$FAKE_UNAME_MACH\" ;;\n  *) printf 'shim-unexpected-arg\\n' ;;\nesac\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("uname"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        dir
    }

    fn installed(&self) -> PathBuf {
        self.install_dir.join("chug")
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Some(base) = self.home.parent() {
            let _ = std::fs::remove_dir_all(base);
        }
    }
}

fn out_text(o: &Output) -> String {
    format!(
        "--- stdout ---\n{}\n--- stderr ---\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// sha256 with the same precedence install.sh itself uses.
fn sha256(file: &Path) -> String {
    for (prog, args) in [
        ("sha256sum", vec![file.to_path_buf()]),
        ("shasum", vec![PathBuf::from("-a"), PathBuf::from("256"), file.to_path_buf()]),
    ] {
        if Command::new(prog).arg("--version").output().is_err() {
            continue; // not on PATH
        }
        let out = Command::new(prog).args(args).output().expect("sha tool");
        assert!(out.status.success(), "{prog} failed on {file:?}");
        let line = String::from_utf8_lossy(&out.stdout);
        let hash = line.split_whitespace().next().expect("hash field");
        assert_eq!(hash.len(), 64, "{prog} produced a non-sha256: {line}");
        return hash.to_string();
    }
    panic!("no sha256 tool (sha256sum/shasum) on PATH — cannot build the fixture sidecar");
}

// --- static legs ---------------------------------------------------------------

/// POSIX sh parse (`sh -n`) plus a bashism grep — the one-liner runs under
/// dash on the fleet's linux boxes, so `[[`, `function`, `local`, arrays,
/// `source`, herestrings and bash-only redirections are each forbidden.
#[test]
fn posix_sh_parse_and_no_bashisms() {
    let script = install_sh();
    let ok = Command::new("sh")
        .arg("-n")
        .arg(&script)
        .output()
        .expect("sh -n");
    assert!(
        ok.status.success(),
        "install.sh must parse under POSIX sh: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let text = std::fs::read_to_string(&script).unwrap();
    for banned in [
        "[[", "function ", " local ", "declare ", "typeset ", "source ", "<<<", "&>",
        "$RANDOM", "${var,", "BASH_",
    ] {
        assert!(
            !text.contains(banned),
            "install.sh must be POSIX sh — found bashism {banned:?}"
        );
    }
    // No sudo anywhere outside comments (the header may name what it avoids).
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with('#') {
            continue;
        }
        assert!(
            !t.contains("sudo"),
            "install.sh must never use sudo: {line}"
        );
    }
    // The documented env overrides exist (tests + mirrors depend on them).
    for needle in [
        "CHUG_INSTALL_REPO",
        "CHUG_INSTALL_OS",
        "CHUG_INSTALL_MACH",
        "CHUG_INSTALL_PLATFORM",
        "CHUG_INSTALL_DIR",
        "CHUG_RELEASE_URL_BASE",
        "CHUG_RELEASE_API_BASE",
    ] {
        assert!(text.contains(needle), "install.sh must keep the {needle} override");
    }
}

/// The three platform tokens install.sh knows are exactly the three the
/// workflow publishes and the README documents — one source of truth.
#[test]
fn platform_table_matches_the_release_matrix() {
    let text = std::fs::read_to_string(install_sh()).unwrap();
    for platform in ["macos-arm64", "linux-x86_64", "linux-aarch64"] {
        assert!(
            text.contains(platform),
            "install.sh must accept {platform} (the workflow publishes it)"
        );
    }
}

// --- happy paths ---------------------------------------------------------------

/// The full happy path: correct sidecar → binary installed, executable, no
/// sudo, banner + quickstart printed, and the PATH hint appears when the
/// install dir is NOT on PATH.
#[test]
fn happy_path_installs_and_hints_about_path() {
    let r = Run::new("happy");
    r.make_release("macos-arm64", "chug-fake-macos");
    let out = r.run("macos-arm64", None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let installed = r.installed();
    assert!(installed.is_file(), "binary must land in the install dir: {}", out_text(&out));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "the installed binary must be executable");
    }
    let text = out_text(&out);
    assert!(text.contains("quickstart"), "banner + quickstart pointer: {text}");
    assert!(text.contains("tampajohn/chug"), "the repo URL is printed: {text}");
    assert!(
        text.contains("NOT on your PATH"),
        "install dir absent from PATH → the hint must print: {text}"
    );
}

/// With the install dir already on PATH there is no hint, and the platform
/// override picks a different asset (linux-x86_64) than the host's uname
/// would — the override is what keeps the fixture offline.
#[test]
fn path_present_suppresses_hint_and_platform_override_selects_asset() {
    let r = Run::new("onpath");
    r.make_release("linux-x86_64", "chug-fake-linux");
    let out = r.run("linux-x86_64", Some(&r.install_dir));
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(r.installed().is_file(), "{}", out_text(&out));
    assert!(
        !out_text(&out).contains("NOT on your PATH"),
        "install dir on PATH → no hint: {}",
        out_text(&out)
    );
}

// --- failure legs (each message names the fix) ----------------------------------

/// A checksum mismatch installs NOTHING: the sidecar is valid hex but wrong,
/// so the script refuses before any binary exists.
#[test]
fn checksum_mismatch_installs_nothing() {
    let r = Run::new("badsum");
    r.make_release("macos-arm64", "chug-fake");
    let sidecar = r.release_dir.join("chug-macos-arm64.tar.gz.sha256");
    let wrong = "0".repeat(63) + "1";
    std::fs::write(&sidecar, format!("{wrong}  chug-macos-arm64.tar.gz\n")).unwrap();
    let out = r.run("macos-arm64", None);
    assert_ne!(out.status.code(), Some(0), "a wrong checksum must fail: {}", out_text(&out));
    assert!(
        !r.installed().exists(),
        "VERIFICATION BEFORE INSTALL: a mismatched tarball must never be installed"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("CHECKSUM MISMATCH"),
        "the failure must name the verification: {err}"
    );
    assert!(
        err.contains("build from source"),
        "the failure must name the fallback fix: {err}"
    );
}

/// A malformed sidecar (not a sha256) fails the format check — it must not be
/// able to "verify" against garbage.
#[test]
fn malformed_sidecar_fails_the_format_check() {
    let r = Run::new("badsformat");
    r.make_release("macos-arm64", "chug-fake");
    std::fs::write(
        r.release_dir.join("chug-macos-arm64.tar.gz.sha256"),
        "definitely-not-a-hash\n",
    )
    .unwrap();
    let out = r.run("macos-arm64", None);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("sha256"),
        "the failure must name the checksum format: {err}"
    );
    assert!(!r.installed().exists(), "nothing installs on a malformed sidecar");
}

/// A missing release asset (the "no release yet" case) names the
/// build-from-source fix explicitly.
#[test]
fn missing_release_names_the_build_from_source_fix() {
    let r = Run::new("norel");
    let out = r.run("macos-arm64", None); // empty release dir → file:// 404
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("build from source") && err.contains("cargo install"),
        "the no-release failure must carry the build-from-source line: {err}"
    );
    assert!(
        err.contains("no release has been published yet"),
        "the failure must name the likely cause: {err}"
    );
    assert!(!r.installed().exists());
}

/// An unsupported platform fails BEFORE downloading and names the supported
/// set — macos-x86_64 (Intel mac) has no published artifact by design.
#[test]
fn unsupported_platform_fails_before_downloading() {
    let r = Run::new("badplat");
    let out = r.run("macos-x86_64", None); // empty release dir: must not matter
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("no prebuilt binary"),
        "the platform failure must say why: {err}"
    );
    for platform in ["macos-arm64", "linux-x86_64", "linux-aarch64"] {
        assert!(err.contains(platform), "the failure must name {platform}: {err}");
    }
    assert!(
        err.contains("cargo install"),
        "the failure must name the build-from-source fix: {err}"
    );
}

// --- uname→platform mapping legs (validator FINDING-2 class sweep) --------------
//
// One killing test per leg of the OS-aware mapping. Each is RED-proven: with
// that leg's output flipped in install.sh the leg's test fails (proofs in the
// fix-up commit message), so a platform-map-flip mutant can no longer survive
// green. The fixtures build ONLY the expected platform's asset — a mutant that
// routes the leg to any other asset dies on the download, and the installed
// stub's echo pins the artifact identity, not just the URL string.

/// A successful mapping leg must install EXACTLY the published asset for its
/// platform: exit 0, the tarball URL requested, and the binary that landed is
/// that platform's artifact (the stub echoes its platform tag).
fn assert_installs_exact_asset(r: &Run, out: &Output, platform: &str) {
    assert_eq!(out.status.code(), Some(0), "{}", out_text(out));
    assert!(
        out_text(out).contains(&format!("chug-{platform}.tar.gz")),
        "the leg must request the EXACT published asset chug-{platform}.tar.gz: {}",
        out_text(out)
    );
    assert!(
        r.installed().is_file(),
        "the {platform} asset must install: {}",
        out_text(out)
    );
    let echo = Command::new(r.installed()).output().expect("running the installed stub");
    assert_eq!(
        String::from_utf8_lossy(&echo.stdout).trim(),
        format!("chug-fake-{platform}"),
        "the installed binary must be the {platform} artifact — a swapped leg installs \
         another platform's binary: {}",
        out_text(out)
    );
}

/// Darwin + arm64 → chug-macos-arm64.tar.gz (K7, the primary fleet box).
#[test]
fn detect_darwin_arm64_installs_the_macos_arm64_asset() {
    let r = Run::new("leg-macos");
    r.make_release("macos-arm64", "chug-fake-macos-arm64");
    let out = r.run_detect("Darwin", "arm64");
    assert_installs_exact_asset(&r, &out, "macos-arm64");
}

/// Linux + x86_64 → chug-linux-x86_64.tar.gz (ucraft/sparks).
#[test]
fn detect_linux_x86_64_installs_the_linux_x86_64_asset() {
    let r = Run::new("leg-linx64");
    r.make_release("linux-x86_64", "chug-fake-linux-x86_64");
    let out = r.run_detect("Linux", "x86_64");
    assert_installs_exact_asset(&r, &out, "linux-x86_64");
}

/// Linux + aarch64 → chug-linux-aarch64.tar.gz — THE FINDING-1 BUG: the old
/// arch-first mapping (`arm64|aarch64) arch=arm64`) produced `linux-arm64`,
/// which the allowlist rejected, turning every Linux ARM64 user away even
/// though the workflow publishes exactly this asset.
#[test]
fn detect_linux_aarch64_installs_the_linux_aarch64_asset() {
    let r = Run::new("leg-lina64");
    r.make_release("linux-aarch64", "chug-fake-linux-aarch64");
    let out = r.run_detect("Linux", "aarch64");
    assert_installs_exact_asset(&r, &out, "linux-aarch64");
}

/// Linux kernels that report `arm64` instead of `aarch64` alias onto the SAME
/// chug-linux-aarch64.tar.gz asset — one published tarball, both spellings.
#[test]
fn detect_linux_arm64_alias_installs_the_linux_aarch64_asset() {
    let r = Run::new("leg-linarm");
    r.make_release("linux-aarch64", "chug-fake-linux-aarch64");
    let out = r.run_detect("Linux", "arm64");
    assert_installs_exact_asset(&r, &out, "linux-aarch64");
}

/// An unsupported OS (FreeBSD) is rejected BEFORE any download and names the
/// detected OS plus the build-from-source fix. The release dir is empty: if a
/// mutant routes this leg anywhere else, the download error (not this
/// message) is what fails — the assertions below kill it.
#[test]
fn detect_unsupported_os_is_rejected_before_any_download() {
    let r = Run::new("leg-bados");
    let out = r.run_detect("FreeBSD", "arm64");
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("unsupported OS"), "must name the OS leg: {err}");
    assert!(err.contains("FreeBSD"), "must echo the detected OS: {err}");
    assert!(err.contains("cargo install"), "must name the fix: {err}");
    assert!(!r.installed().exists(), "nothing installs on an unsupported OS");
}

/// Unsupported arch, both reject shapes: a junk arch on a supported OS
/// (linux/sparc64) and a real arch with no published artifact on its OS
/// (darwin/x86_64 — Intel macs have no artifact by design). Both must fail
/// the ARCH leg's message, before any download.
#[test]
fn detect_unsupported_arch_is_rejected_before_any_download() {
    for (name, os, mach) in [("linux", "Linux", "sparc64"), ("darwin", "Darwin", "x86_64")] {
        let r = Run::new(&format!("leg-badarch-{name}"));
        let out = r.run_detect(os, mach);
        assert_ne!(out.status.code(), Some(0), "{os}/{mach}: {}", out_text(&out));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("unsupported architecture"),
            "{os}/{mach} must fail the ARCH leg, not some later one: {err}"
        );
        assert!(
            err.contains("cargo install"),
            "{os}/{mach} must name the build-from-source fix: {err}"
        );
        assert!(!r.installed().exists(), "{os}/{mach}: nothing installs");
    }
}

/// The mapping must be driven by `uname` ITSELF, not only by the test
/// overrides: with `uname` shimmed on PATH and zero env overrides, the real
/// detection path lands the right asset for a macos host and a linux-arm64
/// host (the FINDING-1 leg, end to end).
#[test]
fn real_uname_path_drives_the_mapping() {
    let r = Run::new("wire-macos");
    r.make_release("macos-arm64", "chug-fake-macos-arm64");
    let out = r.run_uname_shimmed(&r.uname_shim(), "Darwin", "arm64");
    assert_installs_exact_asset(&r, &out, "macos-arm64");

    let r = Run::new("wire-linux");
    r.make_release("linux-aarch64", "chug-fake-linux-aarch64");
    let out = r.run_uname_shimmed(&r.uname_shim(), "Linux", "aarch64");
    assert_installs_exact_asset(&r, &out, "linux-aarch64");
}

// --- T330 profile legs ----------------------------------------------------------
//
// `--profile <name>` is the one-command bridge from "binary installed" to
// "the loop talks to a model provider" (docs/install-profiles.md). The legs
// below mirror the T100 doctrine: every failure names its fix, validation
// happens BEFORE anything is written (never a half-written profile), and the
// committed example profile is executable documentation the battery itself
// executes. All fixtures are offline: the profile legs resolve/validate from
// the fixture HOME/cwd and only the success legs need the fake release.

impl Run {
    /// Write `~/.chug/profiles/<name>.env` in the fixture HOME — the private
    /// profile directory the resolution order prefers.
    fn write_profile(&self, name: &str, body: &str) -> PathBuf {
        let dir = self.home.join(".chug").join("profiles");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}.env"));
        std::fs::write(&p, body).unwrap();
        p
    }

    /// Run with extra argv (the `--profile` legs).
    fn run_args(&self, platform: &str, args: &[&str], path_extra: Option<&Path>) -> Output {
        self.base_cmd(path_extra)
            .env("CHUG_INSTALL_PLATFORM", platform)
            .args(args)
            .output()
            .expect("spawning sh install.sh --profile …")
    }

    fn installed_profile(&self) -> PathBuf {
        self.home.join(".chug").join("profile.env")
    }
}

/// A contract-valid profile in the CMD form (no literal key rests in the
/// file) — the shape docs/install-profiles.md recommends.
fn good_profile_body() -> String {
    "export CHUG_LLM_BASE_URL=\"http://127.0.0.1:8080\"\n\
     CHUG_LLM_API_KEY_CMD=\"printf %s fixture-token\"\n\
     CHUG_MODEL_REASONING=anthropic/reasoner-full-id\n\
     CHUG_MODEL_CODING=anthropic/coder-full-id\n"
        .to_string()
}

/// The full success path: a valid profile resolves from ~/.chug/profiles/,
/// installs to ~/.chug/profile.env (mode 600 — it may carry a literal key),
/// and the footer prints the loop launch line (the `chug loop` subcommand —
/// no clone of chug needed) with the profile sourced.
/// The CMD form must NOT print the literal-key note.
#[test]
fn profile_good_installs_and_prints_the_launch_line() {
    let r = Run::new("prof-good");
    r.make_release("macos-arm64", "chug-fake");
    r.write_profile("good", &good_profile_body());
    let out = r.run_args("macos-arm64", &["--profile", "good"], None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(r.installed().is_file(), "binary installs: {}", out_text(&out));
    let installed = r.installed_profile();
    assert!(installed.is_file(), "the profile is copied: {}", out_text(&out));
    assert_eq!(
        std::fs::read_to_string(&installed).unwrap(),
        good_profile_body(),
        "the copy is verbatim (comments and shapes survive for the loopd loader)"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&installed).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "the installed profile is mode 600 — it may carry a literal key"
        );
    }
    let text = out_text(&out);
    assert!(
        text.contains("chug loop init && chug loop run"),
        "the launch line is the loop subcommand (no clone needed): {text}"
    );
    assert!(
        !text.contains("./loopd.sh"),
        "the footer never tells the pilot to run a script only a clone has: {text}"
    );
    assert!(text.contains("profile good"), "the profile is named: {text}");
    assert!(
        !text.contains("literal CHUG_LLM_API_KEY"),
        "the CMD form prints no literal-key note: {text}"
    );
}

/// Resolution order: ~/.chug/profiles/<name>.env (private values) wins over
/// a repo checkout's ./profiles/<name>.env — the file a company operator
/// drops on the box beats anything in the checkout cwd.
#[test]
fn profile_resolution_prefers_the_private_home_dir() {
    let r = Run::new("prof-order");
    r.make_release("macos-arm64", "chug-fake");
    r.write_profile(
        "both",
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=k\nCHUG_MODEL_REASONING=r\n\
         CHUG_MODEL_CODING=c\n# marker: from the private home dir\n",
    );
    let checkout = r.home.parent().unwrap().join("checkout");
    let repo_profiles = checkout.join("profiles");
    std::fs::create_dir_all(&repo_profiles).unwrap();
    std::fs::write(
        repo_profiles.join("both.env"),
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=k\nCHUG_MODEL_REASONING=r\n\
         CHUG_MODEL_CODING=c\n# marker: from the repo checkout\n",
    )
    .unwrap();
    let mut cmd = r.base_cmd(None);
    cmd.current_dir(&checkout)
        .env("CHUG_INSTALL_PLATFORM", "macos-arm64")
        .args(["--profile", "both"]);
    let out = cmd.output().expect("spawning sh install.sh");
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let installed = std::fs::read_to_string(r.installed_profile()).unwrap();
    assert!(
        installed.contains("marker: from the private home dir"),
        "the HOME copy wins: {installed:?}"
    );
    // and with NO private file, the checkout's ./profiles/ resolves
    std::fs::remove_dir_all(r.home.join(".chug/profiles")).unwrap();
    std::fs::write(
        repo_profiles.join("only-repo.env"),
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=k\nCHUG_MODEL_REASONING=r\n\
         CHUG_MODEL_CODING=c\n# marker: repo-only profile\n",
    )
    .unwrap();
    let mut cmd = r.base_cmd(None);
    cmd.current_dir(&checkout)
        .env("CHUG_INSTALL_PLATFORM", "macos-arm64")
        .args(["--profile", "only-repo"]);
    let out = cmd.output().expect("spawning sh install.sh");
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let reinstalled = std::fs::read_to_string(r.installed_profile()).unwrap();
    assert!(
        reinstalled.contains("marker: repo-only profile"),
        "the checkout copy resolves when no private file exists: {reinstalled:?}"
    );
}

/// A missing required key fails with EVERY absent key NAMED, before any
/// download and before anything is written — never a half-written profile,
/// never a half-installed box. (The release dir is left empty on purpose: a
/// mutant that downloads before validating dies on the file:// 404.)
#[test]
fn profile_missing_keys_fail_named_before_anything_is_written() {
    let r = Run::new("prof-missing");
    r.write_profile("thin", "CHUG_LLM_BASE_URL=u\n");
    let err_text = {
        let out = r.run_args("macos-arm64", &["--profile", "thin"], None);
        assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
        String::from_utf8_lossy(&out.stderr).to_string()
    };
    assert!(
        err_text.contains("missing required key"),
        "the failure names the rule: {err_text}"
    );
    for key in [
        "CHUG_MODEL_REASONING",
        "CHUG_MODEL_CODING",
        "CHUG_LLM_API_KEY (or CHUG_LLM_API_KEY_CMD)",
    ] {
        assert!(err_text.contains(key), "the failure names {key}: {err_text}");
    }
    assert!(
        !r.installed_profile().exists(),
        "NEVER a half-written profile"
    );
    assert!(!r.installed().exists(), "nothing installs on a bad profile");
}

/// `--profile` with no matching file fails closed, naming every place
/// searched; `--profile` with no name names the usage.
#[test]
fn profile_without_a_file_fails_closed() {
    let r = Run::new("prof-nofile");
    let out = r.run_args("macos-arm64", &["--profile", "nosuch"], None);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("profile 'nosuch' not found"),
        "the failure names the profile: {err}"
    );
    assert!(
        err.contains(".chug/profiles/nosuch.env") && err.contains("./profiles/nosuch.env"),
        "the failure lists both searched places: {err}"
    );
    assert!(!r.installed().exists(), "nothing installs");

    let out = r.run_args("macos-arm64", &["--profile"], None);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("--profile needs a profile name"),
        "the no-name form names the usage: {}",
        out_text(&out)
    );
}

/// The contract takes exactly one credential form: both present is a named
/// failure (which one would the loader fetch?), as is any non-profile-owned
/// key (the loopd loader would silently skip it — install refuses so the
/// typo surfaces here) and any non-KEY=VALUE line.
#[test]
fn profile_contract_violations_fail_named() {
    // both credential forms
    let r = Run::new("prof-both");
    let body = "CHUG_LLM_BASE_URL=http://x\nCHUG_LLM_API_KEY=one\nCHUG_LLM_API_KEY_CMD=two\n\
         CHUG_MODEL_REASONING=r\nCHUG_MODEL_CODING=c\n";
    r.write_profile("both", body);
    let out = r.run_args("macos-arm64", &["--profile", "both"], None);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        err.contains("BOTH CHUG_LLM_API_KEY and CHUG_LLM_API_KEY_CMD"),
        "the conflict is named: {err}"
    );
    assert!(!r.installed_profile().exists());

    // a non-profile-owned key
    let r = Run::new("prof-unknown");
    r.write_profile(
        "unk",
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=k\nCHUG_MODEL_REASONING=r\n\
         CHUG_MODEL_CODING=c\nFOO=bar\n",
    );
    let out = r.run_args("macos-arm64", &["--profile", "unk"], None);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        err.contains("non-profile-owned") && err.contains("FOO"),
        "the stray key is named: {err}"
    );

    // a line that is not KEY=VALUE
    let r = Run::new("prof-malformed");
    r.write_profile(
        "mal",
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=k\nCHUG_MODEL_REASONING=r\n\
         CHUG_MODEL_CODING=c\njust some prose\n",
    );
    let out = r.run_args("macos-arm64", &["--profile", "mal"], None);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        err.contains("must be KEY=VALUE"),
        "the malformed line is named: {err}"
    );
}

/// A literal CHUG_LLM_API_KEY in the profile is legal but gets the note
/// steering operators at the CMD form (the key never rests in a file).
#[test]
fn profile_literal_key_installs_with_the_cmd_form_note() {
    let r = Run::new("prof-literal");
    r.make_release("macos-arm64", "chug-fake");
    r.write_profile(
        "lit",
        "CHUG_LLM_BASE_URL=u\nCHUG_LLM_API_KEY=fixture-placeholder\n\
         CHUG_MODEL_REASONING=r\nCHUG_MODEL_CODING=c\n",
    );
    let out = r.run_args("macos-arm64", &["--profile", "lit"], None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        out_text(&out).contains("literal CHUG_LLM_API_KEY"),
        "the note prints: {}",
        out_text(&out)
    );
}

/// The committed example IS the contract's executable documentation: the
/// battery installs it from the repo checkout itself (./profiles/ relative
/// to the test's cwd = the package root).
#[test]
fn the_committed_example_profile_installs() {
    let r = Run::new("prof-example");
    r.make_release("macos-arm64", "chug-fake");
    let out = r.run_args(
        "macos-arm64",
        &["--profile", "local-openai-compat"],
        None,
    );
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let installed = std::fs::read_to_string(r.installed_profile()).unwrap();
    assert!(
        installed.contains("CHUG_LLM_BASE_URL=")
            && installed.contains("CHUG_LLM_API_KEY_CMD=")
            && installed.contains("CHUG_MODEL_REASONING=")
            && installed.contains("CHUG_MODEL_CODING="),
        "the example carries the owned keys: {installed:?}"
    );
    assert!(
        !installed
            .lines()
            .any(|l| !l.trim_start().starts_with('#') && l.contains("CHUG_LLM_API_KEY=")),
        "the example's literal-KEY form stays commented (the CMD form is active)"
    );
}

/// T332 — the decision-provider example IS installable too: the decision
/// keys ride the T330 contract as optional pass-throughs, so a profile
/// carrying them (plus the required LLM keys) installs unchanged and lands
/// verbatim in ~/.chug/profile.env for the loopd loaders.
#[test]
fn the_decision_provider_example_profile_installs() {
    let r = Run::new("prof-decision");
    r.make_release("macos-arm64", "chug-fake");
    let out = r.run_args("macos-arm64", &["--profile", "decision-proxy"], None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let installed = std::fs::read_to_string(r.installed_profile()).unwrap();
    for needle in [
        "CHUG_DECISION_PROVIDER=",
        "CHUG_DECISION_PROXY_URL=",
        "CHUG_DECISION_PROXY_TOKEN=",
        "CHUG_LLM_BASE_URL=",
        "CHUG_MODEL_REASONING=",
        "CHUG_MODEL_CODING=",
    ] {
        assert!(
            installed.contains(needle),
            "the decision example carries {needle}: {installed:?}"
        );
    }
    // placeholder values only — the real endpoint/token never land in the repo
    assert!(
        installed.contains("REPLACE-ME"),
        "the example's values are placeholders: {installed:?}"
    );
}

/// Static pins: install.sh enforces exactly the documented key set, and the
/// no-profile invocation keeps today's footer (plus the discovery hint).
#[test]
fn profile_static_pins_and_the_no_profile_footer() {
    let text = std::fs::read_to_string(install_sh()).unwrap();
    for needle in [
        "--profile",
        "CHUG_LLM_BASE_URL",
        "CHUG_LLM_API_KEY",
        "CHUG_LLM_API_KEY_CMD",
        "CHUG_MODEL_REASONING",
        "CHUG_MODEL_CODING",
        "CHUG_LANGFUSE_",
        "CHUG_DECISION_PROVIDER",
        "CHUG_DECISION_PROXY_URL",
        "CHUG_DECISION_PROXY_TOKEN",
        "chmod 600",
        "docs/install-profiles.md",
    ] {
        assert!(text.contains(needle), "install.sh must pin {needle}");
    }
    let example = std::fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("profiles/local-openai-compat.env"),
    )
    .expect("the committed example profile exists");
    assert!(
        example.contains("docs/install-profiles.md"),
        "the example links the contract doc"
    );

    // no-arg invocation: classic footer + the one-line profile hint
    let r = Run::new("prof-classic");
    r.make_release("macos-arm64", "chug-fake");
    let out = r.run("macos-arm64", None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(text.contains("quickstart"), "the classic footer rides: {text}");
    assert!(
        text.contains("install.sh --profile <name>"),
        "the discovery hint prints when no profile was requested: {text}"
    );
    assert!(
        !text.contains("  profile:"),
        "no profile block without --profile: {text}"
    );
}

// ---------- T341: the meta-broker pack (--with-meta-pack legs) ----------

/// A fake release whose tarball carries the pack: `chug` + `hooks/` — the
/// layout the release workflow's package step ships (the tarball-carries-
/// the-pack pin in tests/release_workflow.rs).
impl Run {
    fn make_release_with_pack(&self, platform: &str) -> PathBuf {
        let staging = self.release_dir.join(format!("stage-pack-{platform}"));
        std::fs::create_dir_all(&staging).unwrap();
        let stub = staging.join("chug");
        std::fs::write(&stub, "#!/bin/sh\necho chug-stub\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let staging_hooks = staging.join("hooks");
        std::fs::create_dir_all(&staging_hooks).unwrap();
        // BOTH packs ride the tarball (the release workflow packages
        // hooks/ whole): the T341 broker and the T342 status mod.
        let broker_src = install_sh().parent().unwrap().join("hooks/meta-broker");
        let dst = staging_hooks.join("meta-broker");
        std::fs::create_dir_all(&dst).unwrap();
        for file in ["hook.sh", "hook.py", "wire.py", "README.md"] {
            std::fs::copy(broker_src.join(file), dst.join(file)).unwrap();
        }
        let status_src = install_sh().parent().unwrap().join("hooks/meta-status");
        let status_dst = staging_hooks.join("meta-status");
        std::fs::create_dir_all(&status_dst).unwrap();
        for file in ["hook.sh", "wire.py", "README.md"] {
            std::fs::copy(status_src.join(file), status_dst.join(file)).unwrap();
        }
        // T343: the usage emitter rides the same tarball (three packs now).
        let usage_src = install_sh().parent().unwrap().join("hooks/meta-usage");
        let usage_dst = staging_hooks.join("meta-usage");
        std::fs::create_dir_all(&usage_dst).unwrap();
        for file in ["hook.sh", "wire.py", "README.md"] {
            std::fs::copy(usage_src.join(file), usage_dst.join(file)).unwrap();
        }
        // The tarball-source marker: lets the fixtures PROVE which pack
        // source won (tarball vs repo checkout) instead of implying it —
        // the checkout-wins fixture below asserts this file did NOT land.
        std::fs::write(dst.join("tarball-pack-marker.txt"), "from-the-verified-tarball\n").unwrap();
        std::fs::write(status_dst.join("tarball-pack-marker.txt"), "from-the-verified-tarball\n").unwrap();
        std::fs::write(usage_dst.join("tarball-pack-marker.txt"), "from-the-verified-tarball\n").unwrap();
        let tarball = self.release_dir.join(format!("chug-{platform}.tar.gz"));
        let ok = Command::new("tar")
            .args(["czf"])
            .arg(&tarball)
            .args(["chug", "hooks"])
            .current_dir(&staging)
            .output()
            .expect("tar czf");
        assert!(ok.status.success(), "tar czf failed: {}", String::from_utf8_lossy(&ok.stderr));
        let hash = sha256(&tarball);
        std::fs::write(
            self.release_dir.join(format!("chug-{platform}.tar.gz.sha256")),
            format!("{hash}  chug-{platform}.tar.gz\n"),
        )
        .unwrap();
        tarball
    }

/// Run a COPY of install.sh from a non-checkout dir (no hooks/ sibling):
/// the $0-relative checkout source must NOT win, so the tarball leg —
/// extract the pack from the SAME verified tarball — is what runs.
fn run_pack_leg(&self, platform: &str, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let script_copy = self.release_dir.join(format!("install-{platform}.sh"));
    std::fs::copy(install_sh(), &script_copy).unwrap();
    let mut cmd = Command::new("sh");
    cmd.arg(&script_copy)
        .args(args)
        .env("CHUG_RELEASE_URL_BASE", format!("file://{}", self.release_dir.display()))
        .env("CHUG_INSTALL_DIR", &self.install_dir)
        .env("HOME", &self.home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawning sh install.sh")
}

/// Run install.sh FROM `checkout` — a directory that carries its own
/// `hooks/meta-broker/` (the repo-checkout shape; the script's $0-relative
/// gate decides, exactly like ./profiles/ does). The checkout source WINS
/// over the tarball: repo-checkout users install the pack from the working
/// tree.
fn run_from_checkout(&self, checkout: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("sh");
    cmd.arg(checkout.join("install.sh"))
        .args(args)
        .env("CHUG_RELEASE_URL_BASE", format!("file://{}", self.release_dir.display()))
        .env("CHUG_INSTALL_DIR", &self.install_dir)
        .env("HOME", &self.home)
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawning sh <checkout>/install.sh")
}
}

fn claude_dir(home: &Path) -> PathBuf {
    home.join(".claude")
}

/// --with-meta-pack installs the pack from the verified tarball and wires
/// the hook ADDITIVELY: pre-existing hooks untouched, the pack entry
/// appended, a re-run a no-op.
#[test]
fn with_meta_pack_installs_from_the_tarball_and_wires_additively() {
    let run = Run::new("with-meta-pack-additive");
    run.make_release_with_pack("macos-arm64");
    let claude = claude_dir(&run.home);
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("settings.json"),
        r#"{"model":"opus","hooks":{"PreToolUse":[{"matcher":"WebFetch","hooks":[{"type":"command","command":"echo keep-me"}]}]}}"#,
    )
    .unwrap();
    let out = run.run_pack_leg("macos-arm64", &["--with-meta-pack"], &[("CHUG_INSTALL_PLATFORM", "macos-arm64")]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let pack = claude.join("hooks").join("meta-broker");
    for file in ["hook.sh", "hook.py", "wire.py"] {
        assert!(pack.join(file).is_file(), "{file} landed");
    }
    assert!(
        pack.join("tarball-pack-marker.txt").is_file(),
        "PROOF of source: the installed pack carries the tarball marker — the \
         pack came out of the verified tarball, not some other tree"
    );
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude.join("settings.json")).unwrap()).unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(
        entries.len(),
        3,
        "BOTH packs' entries appended, nothing else moved: {entries:?}"
    );
    assert_eq!(entries[0]["hooks"][0]["command"], "echo keep-me", "existing hooks untouched");
    assert!(
        entries[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hooks/meta-broker/hook.sh"),
        "the broker's wired command points at the installed pack"
    );
    assert_eq!(
        entries[2]["matcher"], "AskUserQuestion",
        "the status mod's PreToolUse entry matches the question tool: {entries:?}"
    );
    // The status mod is wired on its six events (the non-tool events
    // carry no matcher).
    for event in ["PostToolUse", "UserPromptSubmit", "Notification", "Stop", "SessionEnd"] {
        let entries = settings["hooks"][event].as_array().unwrap();
        assert!(
            entries
                .iter()
                .any(|e| e["hooks"][0]["command"].as_str().is_some_and(|c| c.contains("meta-status"))),
            "{event} carries the status mod's wiring"
        );
    }
    assert_eq!(settings["model"], "opus", "unrelated keys untouched");
    // Idempotent re-run: still exit 0, still exactly one pack entry.
    let out = run.run_pack_leg("macos-arm64", &["--with-meta-pack"], &[("CHUG_INSTALL_PLATFORM", "macos-arm64")]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude.join("settings.json")).unwrap()).unwrap();
    assert_eq!(
        settings["hooks"]["PreToolUse"].as_array().unwrap().len(),
        3,
        "no duplicate wiring (both packs idempotent)"
    );
    assert_eq!(
        settings["hooks"]["Stop"].as_array().unwrap().len(),
        2,
        "no duplicate wiring: Stop now carries the status mod AND the usage emitter"
    );
    assert!(
        settings["hooks"]["Stop"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["hooks"][0]["command"].as_str().is_some_and(|c| c.contains("meta-usage"))),
        "the usage emitter is wired on Stop"
    );
    let usage_pack = claude.join("hooks").join("meta-usage");
    for file in ["hook.sh", "wire.py", "README.md"] {
        assert!(usage_pack.join(file).is_file(), "{file} landed");
    }
    // The status dir exists from install (the hook never mkdirs on the
    // hot path).
    assert!(claude.join("meta-status").is_dir(), "the status dir is created at install");
    // The usage log, by contrast, is NOT created at install: the hook
    // appends it on the first real turn, and install writes no telemetry.
    assert!(
        !claude.join("meta-usage.jsonl").exists(),
        "no usage log until a turn completes"
    );
}

/// --uninstall-meta-pack is the exact reverse: the pack's block removed
/// (the foreign hook survives), the pack dir gone, and it runs OFFLINE —
/// on an unsupported platform too (no download, before the platform
/// detect). A second uninstall is a no-op.
#[test]
fn uninstall_meta_pack_removes_exactly_its_own_block_offline() {
    let run = Run::new("with-meta-pack-uninstall");
    run.make_release_with_pack("macos-arm64");
    let claude = claude_dir(&run.home);
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"WebFetch","hooks":[{"type":"command","command":"echo keep-me"}]}]}}"#,
    )
    .unwrap();
    let envs = [("CHUG_INSTALL_PLATFORM", "macos-arm64")];
    assert!(run.run_pack_leg("macos-arm64", &["--with-meta-pack"], &envs).status.success());
    // Offline uninstall on an unsupported platform — no platform error.
    let out = run.run_pack_leg(
        "macos-arm64",
        &["--uninstall-meta-pack"],
        &[("CHUG_INSTALL_OS", "FreeBSD")],
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude.join("settings.json")).unwrap()).unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "only the packs' blocks removed");
    assert_eq!(entries[0]["hooks"][0]["command"], "echo keep-me", "the foreign hook survives");
    assert!(!claude.join("hooks").join("meta-broker").exists(), "the broker pack dir is gone");
    assert!(!claude.join("hooks").join("meta-status").exists(), "the status pack dir is gone");
    assert!(!claude.join("hooks").join("meta-usage").exists(), "the usage pack dir is gone");
    for event in ["PostToolUse", "UserPromptSubmit", "Notification", "Stop", "SessionEnd"] {
        assert!(
            settings["hooks"].get(event).is_none(),
            "{event} was the status mod's event: unwired"
        );
    }
    assert!(!claude.join("meta-status").exists(), "the status files went with the uninstall");
    // The usage log is a HISTORICAL RECORD, not state: uninstall removes
    // the emitter (code + wiring), never recorded telemetry.
    std::fs::write(claude.join("meta-usage.jsonl"), "{}\n").unwrap();
    assert!(
        run.run_pack_leg("macos-arm64", &["--uninstall-meta-pack"], &[("CHUG_INSTALL_OS", "FreeBSD")])
            .status
            .success()
    );
    assert!(
        claude.join("meta-usage.jsonl").is_file(),
        "recorded usage survives the uninstall (data, not state)"
    );
    // Idempotent: a second uninstall is a no-op exit 0.
    let out = run.run_pack_leg(
        "macos-arm64",
        &["--uninstall-meta-pack"],
        &[("CHUG_INSTALL_OS", "FreeBSD")],
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// --uninstall-meta-pack refuses --profile with a NAMED error (the t341
/// validator's finding 5: the guard ignored the flag — the uninstall leg
/// runs before profile resolution, so the flag was silently dropped on the
/// floor). Nothing moves: the pack stays installed and wired, and no
/// profile file is written. The refusal is offline like the whole leg.
#[test]
fn uninstall_meta_pack_refuses_profile_named_and_moves_nothing() {
    let run = Run::new("with-meta-pack-uninstall-profile");
    run.make_release_with_pack("macos-arm64");
    let claude = claude_dir(&run.home);
    std::fs::create_dir_all(&claude).unwrap();
    std::fs::write(
        claude.join("settings.json"),
        r#"{"hooks":{"PreToolUse":[{"matcher":"WebFetch","hooks":[{"type":"command","command":"echo keep-me"}]}]}}"#,
    )
    .unwrap();
    let envs = [("CHUG_INSTALL_PLATFORM", "macos-arm64")];
    assert!(run.run_pack_leg("macos-arm64", &["--with-meta-pack"], &envs).status.success());

    // The refused combination — on an unsupported platform, proving the
    // guard fires before any platform work (the offline leg, unchanged).
    let out = run.run_pack_leg(
        "macos-arm64",
        &["--uninstall-meta-pack", "--profile", "some-profile"],
        &[("CHUG_INSTALL_OS", "FreeBSD")],
    );
    assert!(!out.status.success(), "the combination must refuse");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        text.contains("--profile") && text.contains("--uninstall-meta-pack"),
        "the refusal names both flags: {text}"
    );

    // Nothing moved: the pack is still installed and wired...
    assert!(
        claude.join("hooks").join("meta-broker").join("wire.py").is_file(),
        "the pack dir is untouched"
    );
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(claude.join("settings.json")).unwrap())
            .unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 3, "the wiring is untouched");
    assert!(
        entries[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("meta-broker"),
        "the broker entry survives the refused run"
    );
    assert!(
        entries[2]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .contains("meta-status"),
        "the status mod entry survives the refused run"
    );
    // ...and the refused profile was never consumed.
    assert!(
        !run.home.join(".chug/profile.env").exists(),
        "no profile written by a refused uninstall"
    );
}

/// The CHECKOUT WINS: a repo checkout carrying hooks/meta-broker/ installs
/// the pack from the WORKING TREE — even though the tarball it also
/// downloads carries its own (different) copy of the pack. Both sources
/// exist with distinguishable content; the installed pack must carry the
/// checkout's marker and NOT the tarball's (the t341 validator's finding
/// 4: the checkout-wins leg was unfixtured — a source-order mutant would
/// silently flip which tree users actually wire).
#[test]
fn a_checkout_carrying_the_pack_installs_from_the_working_tree() {
    let run = Run::new("with-meta-pack-checkout-wins");
    run.make_release_with_pack("macos-arm64"); // the tarball carries its own pack (tarball marker)

    // The fake checkout: install.sh + the real pack + the checkout marker.
    // Sits under the fixture's own tempdir (next to release/) — it is the
    // repo-checkout shape, not the release dir itself (which must stay a
    // non-checkout for the tarball fixtures).
    let checkout = run.release_dir.parent().unwrap().join("checkout");
    let checkout_hooks = checkout.join("hooks");
    std::fs::create_dir_all(&checkout_hooks).unwrap();
    std::fs::copy(install_sh(), checkout.join("install.sh")).unwrap();
    let repo_hooks = install_sh().parent().unwrap().join("hooks");
    for (pack, files) in [
        ("meta-broker", vec!["hook.sh", "hook.py", "wire.py", "README.md"]),
        ("meta-status", vec!["hook.sh", "wire.py", "README.md"]),
    ] {
        let dst = checkout_hooks.join(pack);
        std::fs::create_dir_all(&dst).unwrap();
        for file in files {
            std::fs::copy(repo_hooks.join(pack).join(file), dst.join(file)).unwrap();
        }
        std::fs::write(dst.join("checkout-pack-marker.txt"), "from-the-working-tree\n").unwrap();
    }

    // Install from the checkout. `sh <checkout>/install.sh` puts the
    // checkout's hooks/ sibling in play via the $0-relative gate.
    let out = run.run_from_checkout(
        &checkout,
        &["--with-meta-pack"],
        &[("CHUG_INSTALL_PLATFORM", "macos-arm64")],
    );
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    // The working tree won: the checkout marker landed, the tarball's did
    // not — a tarball-wins mutant fails the first assert, a checkout-ignored
    // mutant fails it too (it would have installed the tarball's copy).
    let pack = claude_dir(&run.home).join("hooks").join("meta-broker");
    assert!(
        pack.join("checkout-pack-marker.txt").is_file(),
        "the installed pack is the checkout's working-tree copy"
    );
    assert!(
        !pack.join("tarball-pack-marker.txt").is_file(),
        "the tarball's pack did NOT win over the checkout"
    );
    for file in ["hook.sh", "hook.py", "wire.py"] {
        assert!(pack.join(file).is_file(), "{file} landed from the checkout");
    }
    let status_pack = claude_dir(&run.home).join("hooks").join("meta-status");
    assert!(
        status_pack.join("checkout-pack-marker.txt").is_file()
            && !status_pack.join("tarball-pack-marker.txt").is_file(),
        "BOTH packs install from the working tree when the checkout carries them"
    );

    // The wiring still happened, and points at the installed copy.
    let settings: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(claude_dir(&run.home).join("settings.json")).unwrap(),
    )
    .unwrap();
    let entries = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(entries.len(), 2, "exactly the two packs' entries wired");
    assert!(
        entries[0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hooks/meta-broker/hook.sh"),
        "the wired command points at the installed pack"
    );
    assert!(
        entries[1]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("hooks/meta-status/hook.sh"),
        "the status mod's entry points at the installed pack"
    );
}

/// A release tarball that predates the pack fails NAMED — before anything
/// is wired or installed (the old-layout tarball: `chug` only).
#[test]
fn a_pack_less_tarball_fails_named() {
    let run = Run::new("with-meta-pack-old-tarball");
    run.make_release("macos-arm64", "chug-stub");
    let out = run.run_pack_leg("macos-arm64", &["--with-meta-pack"], &[("CHUG_INSTALL_PLATFORM", "macos-arm64")]);
    assert!(!out.status.success(), "a pack-less tarball must not install");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(text.contains("meta pack"), "the failure names the missing pack: {text}");
    assert!(
        !claude_dir(&run.home).join("hooks").exists(),
        "nothing installed on the failed leg"
    );
}

// ---------- T348: the release-staleness gate + the service legs ----------//
//
// Two targeted gaps found during the K7 update (2026-11-09): install.sh
// succeeded while installing a MONTH-OLD binary (releases/latest is a moving
// pointer — the T347 outage made every pilot install a silent downgrade),
// and the managed launchd jobs for chugd/loopd were tribal knowledge (the
// scripts/ plists were hand-installed on K7). The battery covers: the
// staleness gate (probe BEFORE download, --allow-stale as the non-
// interactive escape hatch, the installed --version echo for logs), and the
// --with-daemon/--with-loopd legs (idempotent bootstrap → kickstart-on-
// rerun via a MOCK launchctl — no system job is ever consulted — the
// macOS-only Linux TODO note, and the loop-scaffold gate).

/// Howard Hinnant's civil_from_days: days since 1970-01-01 → (y, m, d).
/// No external `date` call: BSD and GNU disagree on `-d`/`-v`, and the
/// freshness fixtures must be pinned relative to the REAL clock.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }), m as u32, d)
}

/// The UTC date `n` days before now, in the YYYY-MM-DD form the release
/// metadata's published_at carries.
fn days_ago_utc(n: i64) -> String {
    let now_days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs() as i64
        / 86_400;
    let (y, m, d) = civil_from_days(now_days - n);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A fixed date far beyond the 7-day window, whatever day the suite runs.
const STALE_PUBLISHED_AT: &str = "2026-01-01T12:00:00Z";

impl Run {
    /// The release-metadata fixture for the staleness probe: the JSON the
    /// GitHub API returns for repos/<repo>/releases/latest, served from the
    /// fixture's fake API base (CHUG_RELEASE_API_BASE=file://<release>/api).
    /// Decoy fields ride it so the script's extraction must key on the FIELD
    /// NAME (published_at), not on line position.
    fn write_release_meta(&self, published_at: &str, tag: &str) -> PathBuf {
        let api = self.release_dir.join("api/repos/tampajohn/chug/releases");
        std::fs::create_dir_all(&api).unwrap();
        let meta = api.join("latest");
        std::fs::write(
            &meta,
            format!(
                "{{\n  \"url\": \"https://api.github.com/repos/tampajohn/chug/releases/1\",\n  \
                 \"tag_name\": \"{tag}\",\n  \"name\": \"{tag}\",\n  \
                 \"published_at\": \"{published_at}\",\n  \"assets\": []\n}}\n"
            ),
        )
        .unwrap();
        meta
    }

    /// Run with the staleness probe pointed at the fixture's fake API base.
    /// The probe runs ONLY when CHUG_RELEASE_API_BASE is set on top of the
    /// base_cmd's CHUG_RELEASE_URL_BASE (the skip rule keeps every other
    /// fixture — and every mirror run — offline).
    fn run_with_api(&self, platform: &str, args: &[&str]) -> Output {
        self.base_cmd(None)
            .env("CHUG_INSTALL_PLATFORM", platform)
            .env(
                "CHUG_RELEASE_API_BASE",
                format!("file://{}/api", self.release_dir.display()),
            )
            .args(args)
            .output()
            .expect("spawning sh install.sh")
    }

    /// A fake release whose tarball carries the service plists (`chug` +
    /// `scripts/`) — the layout a release packages once the service legs
    /// ship. The plist is the checked-in scripts/com.tampajohn.chugd.plist.
    fn make_release_with_scripts(&self, platform: &str, stub_stdout: &str) -> PathBuf {
        let staging = self.release_dir.join(format!("stage-scripts-{platform}"));
        std::fs::create_dir_all(staging.join("scripts")).unwrap();
        let stub = staging.join("chug");
        std::fs::write(&stub, format!("#!/bin/sh\necho {stub_stdout}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let repo_scripts = install_sh().parent().unwrap().join("scripts");
        std::fs::copy(
            repo_scripts.join("com.tampajohn.chugd.plist"),
            staging.join("scripts").join("com.tampajohn.chugd.plist"),
        )
        .expect("the committed chugd plist exists in scripts/");
        let tarball = self.release_dir.join(format!("chug-{platform}.tar.gz"));
        let ok = Command::new("tar")
            .args(["czf"])
            .arg(&tarball)
            .args(["chug", "scripts"])
            .current_dir(&staging)
            .output()
            .expect("tar czf");
        assert!(
            ok.status.success(),
            "tar czf failed: {}",
            String::from_utf8_lossy(&ok.stderr)
        );
        let hash = sha256(&tarball);
        std::fs::write(
            self.release_dir.join(format!("chug-{platform}.tar.gz.sha256")),
            format!("{hash}  chug-{platform}.tar.gz\n"),
        )
        .unwrap();
        tarball
    }

    /// The state dir the mock launchctl keeps its gui-domain registry in.
    fn launchctl_state(&self) -> PathBuf {
        self.home.join("launchctl-state")
    }

    /// A PATH-shim `launchctl`: models the gui-domain job registry as
    /// marker files (loaded/<label>) and appends EVERY invocation to
    /// calls.log. The fixture harness that makes the bootstrap/kickstart
    /// legs deterministic without touching the real launchd — no system
    /// job is ever consulted, exactly like the uname shim above.
    fn launchctl_shim(&self) -> PathBuf {
        let dir = self.home.join("launchctl-shim");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("launchctl"),
            "#!/bin/sh\n\
             state=$MOCK_LAUNCHCTL_STATE\n\
             mkdir -p \"$state\"\n\
             printf '%s\\n' \"$*\" >> \"$state/calls.log\"\n\
             case \"$1\" in\n\
               print)\n\
                 label=${2##*/}\n\
                 if [ -f \"$state/loaded/$label\" ]; then exit 0; fi\n\
                 exit 1 ;;\n\
               bootstrap)\n\
                 mkdir -p \"$state/loaded\"\n\
                 touch \"$state/loaded/$(basename \"$3\" .plist)\"\n\
                 exit 0 ;;\n\
               kickstart)\n\
                 printf '%s\\n' \"$*\" >> \"$state/kickstarts.log\"\n\
                 exit 0 ;;\n\
             esac\n\
             exit 0\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.join("launchctl"), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        dir
    }

    /// Run a COPY of install.sh from a non-checkout dir (no scripts/ sibling:
    /// the tarball leg runs) with the mock launchctl on PATH and the OS leg
    /// forced (the plist fixtures must be host-independent). `cwd` relocates
    /// the run — the --with-loopd scaffold gate reads the current directory.
    fn run_service_leg(
        &self,
        platform: &str,
        args: &[&str],
        os: &str,
        cwd: Option<&Path>,
    ) -> Output {
        let script_copy = self
            .release_dir
            .join(format!("install-service-{platform}.sh"));
        std::fs::copy(install_sh(), &script_copy).unwrap();
        self.run_service_leg_at(&script_copy, platform, args, os, cwd)
    }

    /// The same, but against an explicit script path (the checkout-wins
    /// fixture runs the checkout's own install.sh).
    fn run_service_leg_at(
        &self,
        script: &Path,
        platform: &str,
        args: &[&str],
        os: &str,
        cwd: Option<&Path>,
    ) -> Output {
        let shim = self.launchctl_shim();
        let mut cmd = Command::new("sh");
        cmd.arg(script)
            .args(args)
            .env(
                "CHUG_RELEASE_URL_BASE",
                format!("file://{}", self.release_dir.display()),
            )
            .env("CHUG_INSTALL_DIR", &self.install_dir)
            .env("CHUG_INSTALL_PLATFORM", platform)
            .env("CHUG_INSTALL_OS", os)
            .env("HOME", &self.home)
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", shim.display()),
            )
            .env("MOCK_LAUNCHCTL_STATE", self.launchctl_state());
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        cmd.output()
            .expect("spawning sh install.sh --with-daemon/--with-loopd")
    }

    /// A loop-scaffolded repo (exactly the three artifacts `chug loop run`
    /// verifies): the shape `chug loop init` leaves behind. Canonicalized:
    /// the script's $PWD is the getcwd-resolved path (macOS /var →
    /// /private/var), and the plist it writes carries THAT path.
    fn initialized_loop_repo(&self) -> PathBuf {
        let repo = self.home.join("pilot-repo");
        std::fs::create_dir_all(repo.join(".chug")).unwrap();
        std::fs::write(repo.join("LOOP-SPEC.md"), "spec\n").unwrap();
        std::fs::write(repo.join("TODO.md"), "todo\n").unwrap();
        repo.canonicalize().unwrap()
    }
}

// --- the staleness gate ----------------------------------------------------------

/// A stale release gates the install BEFORE any download: the loud warning
/// names the date and the tag, suggests the build-from-source and mirror
/// fixes, and refuses without --allow-stale. The fixture carries NO tarball:
/// a mutant that downloads before gating dies on the file:// 404 with the
/// download message — the gate's message is what must fire.
#[test]
fn stale_release_gates_the_install_before_any_download() {
    let r = Run::new("t348-gate");
    r.write_release_meta(STALE_PUBLISHED_AT, "v0.17.27");
    let out = r.run_with_api("macos-arm64", &[]);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(text.contains("STALE RELEASE"), "the loud warning prints: {text}");
    assert!(text.contains("2026-01-01"), "the warning names the date: {text}");
    assert!(text.contains("v0.17.27"), "the warning names the release tag: {text}");
    assert!(
        text.contains("--allow-stale"),
        "the gate names the non-interactive escape hatch: {text}"
    );
    assert!(
        text.contains("cargo install"),
        "the warning suggests building from source: {text}"
    );
    assert!(
        text.contains("CHUG_RELEASE_URL_BASE"),
        "the warning names the mirror override: {text}"
    );
    assert!(
        !text.contains("download failed"),
        "the gate fires BEFORE the tarball download: {text}"
    );
    assert!(!r.installed().exists(), "a gated install writes nothing");
}

/// --allow-stale is the escape hatch: the warning still prints (loud, in the
/// log), the install proceeds, and the installed `chug --version` is echoed
/// so the staleness stays visible even past the gate.
#[test]
fn stale_release_with_allow_stale_installs_with_the_loud_warning() {
    let r = Run::new("t348-allow");
    r.make_release("macos-arm64", "chug-fake-0.17.27");
    r.write_release_meta(STALE_PUBLISHED_AT, "v0.17.27");
    let out = r.run_with_api("macos-arm64", &["--allow-stale"]);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(text.contains("STALE RELEASE"), "the warning still prints: {text}");
    assert!(text.contains("2026-01-01"), "…naming the date: {text}");
    assert!(
        text.contains("proceeding with the stale release"),
        "…and logging the opt-in: {text}"
    );
    assert!(r.installed().is_file(), "the install proceeds: {text}");
    assert!(
        text.contains("installed version: chug-fake-0.17.27"),
        "the installed chug --version is echoed for the log: {text}"
    );
}

/// A fresh release passes silently (no staleness warning at all) and the
/// --version echo still appears — it is on every install, not just the
/// gated ones.
#[test]
fn fresh_release_passes_silently_and_still_echoes_the_version() {
    let r = Run::new("t348-fresh");
    r.make_release("macos-arm64", "chug-fake-0.17.27");
    r.write_release_meta(&format!("{}T12:00:00Z", days_ago_utc(1)), "v0.18.0");
    let out = r.run_with_api("macos-arm64", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(
        !text.contains("STALE"),
        "a fresh release prints no staleness warning: {text}"
    );
    assert!(r.installed().is_file(), "{text}");
    assert!(
        text.contains("installed version: chug-fake-0.17.27"),
        "the --version echo appears on every install: {text}"
    );
}

/// The window comparison is `age > days` at the DAY boundary: exactly 7
/// days old is inside the window (silent), 8 days is gated. CHUG_STALE_DAYS
/// widens the window for pilots who deliberately run behind.
#[test]
fn staleness_window_boundary_seven_passes_eight_gates_and_the_env_widens_it() {
    let r = Run::new("t348-edge7");
    r.make_release("macos-arm64", "chug-fake");
    r.write_release_meta(&format!("{}T00:00:00Z", days_ago_utc(7)), "v0.18.0");
    let out = r.run_with_api("macos-arm64", &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "7 days old is NOT older than the 7-day window: {}",
        out_text(&out)
    );
    assert!(!out_text(&out).contains("STALE"), "{}", out_text(&out));

    let r = Run::new("t348-edge8");
    r.make_release("macos-arm64", "chug-fake");
    r.write_release_meta(&format!("{}T00:00:00Z", days_ago_utc(8)), "v0.18.0");
    let out = r.run_with_api("macos-arm64", &[]);
    assert_ne!(
        out.status.code(),
        Some(0),
        "8 days old IS older than the window: {}",
        out_text(&out)
    );
    assert!(
        out_text(&out).contains("--allow-stale"),
        "{}",
        out_text(&out)
    );

    let r = Run::new("t348-edge-widened");
    r.make_release("macos-arm64", "chug-fake");
    r.write_release_meta(STALE_PUBLISHED_AT, "v0.17.27");
    let out = r
        .base_cmd(None)
        .env("CHUG_INSTALL_PLATFORM", "macos-arm64")
        .env(
            "CHUG_RELEASE_API_BASE",
            format!("file://{}/api", r.release_dir.display()),
        )
        .env("CHUG_STALE_DAYS", "3650")
        .output()
        .expect("spawning sh install.sh");
    assert_eq!(
        out.status.code(),
        Some(0),
        "CHUG_STALE_DAYS widens the window: {}",
        out_text(&out)
    );
    assert!(!out_text(&out).contains("STALE"), "{}", out_text(&out));
}

/// An UNREACHABLE probe degrades to a note and the install continues:
/// api.github.com rate limits are real, the checksum still guards
/// integrity, and staleness blindness must not be traded for a new way to
/// fail every install.
#[test]
fn unreachable_release_metadata_degrades_to_a_note() {
    let r = Run::new("t348-nometa");
    r.make_release("macos-arm64", "chug-fake");
    // NO metadata fixture: the fake API base 404s.
    let out = r.run_with_api("macos-arm64", &[]);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(
        text.contains("WITHOUT the staleness check"),
        "the degrade names what did not happen: {text}"
    );
    assert!(r.installed().is_file(), "{text}");
}

// --- the service legs (--with-daemon / --with-loopd) ------------------------------

/// --with-daemon installs the chugd plist into ~/Library/LaunchAgents,
/// bootstraps it in the gui domain, prints the socket path + the fail-open
/// note — and is IDEMPOTENT: the rerun detects the already-bootstrapped job
/// and kickstarts it instead (never a second bootstrap).
#[test]
fn with_daemon_installs_bootstraps_and_kickstarts_on_rerun() {
    let r = Run::new("t348-daemon");
    r.make_release_with_scripts("macos-arm64", "chug-fake");
    let out = r.run_service_leg("macos-arm64", &["--with-daemon"], "Darwin", None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let plist = r.home.join("Library/LaunchAgents/com.tampajohn.chugd.plist");
    assert!(
        plist.is_file(),
        "the plist lands in ~/Library/LaunchAgents: {}",
        out_text(&out)
    );
    let body = std::fs::read_to_string(&plist).unwrap();
    assert!(
        !body.contains("/Users/jadams"),
        "the committed plist's authoring-box paths are rewritten to this box: {body}"
    );
    assert!(
        body.contains(&format!("{}/chug", r.install_dir.display())),
        "the plist runs the binary install.sh JUST installed: {body}"
    );
    let text = out_text(&out);
    assert!(text.contains(".chug/daemon.sock"), "the socket path prints: {text}");
    assert!(text.contains("fails OPEN"), "the fail-open note prints: {text}");
    let calls = std::fs::read_to_string(r.launchctl_state().join("calls.log")).unwrap();
    assert!(
        calls
            .lines()
            .any(|l| l.starts_with("bootstrap gui/") && l.contains("/com.tampajohn.chugd.plist")),
        "a fresh job is BOOTSTRAPPED in the gui domain: {calls}"
    );
    assert!(
        !calls.contains("kickstart"),
        "a fresh job is never kickstarted: {calls}"
    );

    // Rerun: idempotent — the loaded job is detected and KICKSTARTED.
    let out = r.run_service_leg("macos-arm64", &["--with-daemon"], "Darwin", None);
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    assert!(
        out_text(&out).contains("already bootstrapped"),
        "the rerun detects the loaded job: {}",
        out_text(&out)
    );
    let calls = std::fs::read_to_string(r.launchctl_state().join("calls.log")).unwrap();
    assert_eq!(
        calls.matches("bootstrap gui/").count(),
        1,
        "no second bootstrap: {calls}"
    );
    let kicks = std::fs::read_to_string(r.launchctl_state().join("kickstarts.log")).unwrap();
    assert!(
        kicks
            .lines()
            .any(|l| l.starts_with("kickstart -k gui/") && l.ends_with("/com.tampajohn.chugd")),
        "the rerun kickstarts the job: {kicks}"
    );
}

/// The checkout's scripts/ plist wins over the tarball's — the same
/// working-tree rule the meta packs follow (repo-checkout users install
/// from the tree). Distinguishable copies: the checkout's carries a marker
/// comment the tarball's does not.
#[test]
fn daemon_plist_checkout_wins_over_the_tarball() {
    let r = Run::new("t348-daemon-checkout");
    r.make_release_with_scripts("macos-arm64", "chug-fake"); // the tarball carries its own
    let checkout = r.release_dir.parent().unwrap().join("checkout");
    std::fs::create_dir_all(checkout.join("scripts")).unwrap();
    std::fs::copy(install_sh(), checkout.join("install.sh")).unwrap();
    let original = std::fs::read_to_string(
        install_sh()
            .parent()
            .unwrap()
            .join("scripts/com.tampajohn.chugd.plist"),
    )
    .unwrap();
    std::fs::write(
        checkout.join("scripts/com.tampajohn.chugd.plist"),
        original.replacen(
            "<plist version=\"1.0\">",
            "<plist version=\"1.0\">\n<!-- checkout-plist-marker -->",
            1,
        ),
    )
    .unwrap();
    let out = r.run_service_leg_at(
        &checkout.join("install.sh"),
        "macos-arm64",
        &["--with-daemon"],
        "Darwin",
        Some(&checkout),
    );
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let body =
        std::fs::read_to_string(r.home.join("Library/LaunchAgents/com.tampajohn.chugd.plist"))
            .unwrap();
    assert!(
        body.contains("checkout-plist-marker"),
        "the WORKING TREE plist won over the tarball's: {body}"
    );
}

/// The daemon leg with NO plist anywhere (plain tarball, script copy away
/// from any checkout scripts/ sibling) fails NAMED with the hand-install
/// fix, and installs nothing.
#[test]
fn daemon_leg_without_a_plist_anywhere_fails_named() {
    let r = Run::new("t348-noplist");
    r.make_release("macos-arm64", "chug-fake"); // chug only — no scripts/
    let out = r.run_service_leg("macos-arm64", &["--with-daemon"], "Darwin", None);
    assert_ne!(out.status.code(), Some(0), "{}", out_text(&out));
    let text = out_text(&out);
    assert!(
        text.contains("plist is unavailable"),
        "the failure names the missing plist: {text}"
    );
    assert!(
        text.contains("launchctl bootstrap"),
        "the failure names the hand-install fix: {text}"
    );
    assert!(
        !r.home.join("Library/LaunchAgents").exists(),
        "nothing installed on the failed leg"
    );
}

/// Both legs are macOS-only: on Linux each prints the systemd TODO note and
/// the install CONTINUES (exit 0), and nothing lands in ~/Library.
#[test]
fn service_legs_on_linux_print_the_systemd_todo_and_continue() {
    let r = Run::new("t348-linux");
    r.make_release_with_scripts("linux-x86_64", "chug-fake-linux");
    let repo = r.initialized_loop_repo();
    let out = r.run_service_leg(
        "linux-x86_64",
        &["--with-daemon", "--with-loopd"],
        "Linux",
        Some(&repo),
    );
    assert_eq!(
        out.status.code(),
        Some(0),
        "the legs continue on linux: {}",
        out_text(&out)
    );
    let text = out_text(&out);
    assert!(
        text.contains("systemd unit is not packaged yet"),
        "the TODO note prints: {text}"
    );
    assert!(
        !r.home.join("Library").exists(),
        "no LaunchAgents on linux: {text}"
    );
}

/// --with-loopd OUTSIDE a loop-scaffolded repo SKIPS (exit 0) with the init
/// hint, and installs no plist.
#[test]
fn with_loopd_outside_an_initialized_repo_skips_with_the_init_hint() {
    let r = Run::new("t348-loopd-skip");
    r.make_release_with_scripts("macos-arm64", "chug-fake");
    let bare = r.home.join("bare-dir"); // no LOOP-SPEC.md / TODO.md / .chug/
    std::fs::create_dir_all(&bare).unwrap();
    let out = r.run_service_leg("macos-arm64", &["--with-loopd"], "Darwin", Some(&bare));
    assert_eq!(
        out.status.code(),
        Some(0),
        "a skip is not a failure: {}",
        out_text(&out)
    );
    let text = out_text(&out);
    assert!(text.contains("--with-loopd skipped"), "{text}");
    assert!(
        text.contains("chug loop init"),
        "the hint names the init command: {text}"
    );
    assert!(
        !r.home
            .join("Library/LaunchAgents/com.tampajohn.chug-loopd.plist")
            .exists(),
        "no plist installed outside an initialized repo"
    );
}

/// --with-loopd inside an initialized repo installs the supervisor plist
/// generated for THIS repo (WorkingDirectory = the repo, ProgramArguments =
/// caffeinate -dims <installed chug> loop run, logs under the repo's
/// .chug/loopd/) and bootstraps it — idempotently (kickstart on rerun).
#[test]
fn with_loopd_installs_the_supervisor_in_an_initialized_repo() {
    let r = Run::new("t348-loopd");
    r.make_release_with_scripts("macos-arm64", "chug-fake");
    let repo = r.initialized_loop_repo();
    let out = r.run_service_leg("macos-arm64", &["--with-loopd"], "Darwin", Some(&repo));
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let plist = r
        .home
        .join("Library/LaunchAgents/com.tampajohn.chug-loopd.plist");
    assert!(
        plist.is_file(),
        "the supervisor plist installs: {}",
        out_text(&out)
    );
    let body = std::fs::read_to_string(&plist).unwrap();
    assert!(
        body.contains(&format!(
            "<key>WorkingDirectory</key><string>{}</string>",
            repo.display()
        )),
        "the job runs against THIS repo: {body}"
    );
    assert!(
        body.contains(&format!(
            "<string>{}</string>",
            repo.join(".chug/loopd/launchd.out.log").display()
        )),
        "the logs land in the repo's .chug/loopd/: {body}"
    );
    for arg in ["<string>loop</string>", "<string>run</string>"] {
        assert!(
            body.contains(arg),
            "ProgramArguments rides `chug loop run` (the installed binary, no clone): {body}"
        );
    }
    assert!(
        body.contains(&format!("<string>{}/chug</string>", r.install_dir.display())),
        "the supervisor is the INSTALLED binary: {body}"
    );
    assert!(
        body.contains("<key>RunAtLoad</key><true/>"),
        "launchd starts the loop at login: {body}"
    );
    let calls = std::fs::read_to_string(r.launchctl_state().join("calls.log")).unwrap();
    assert!(
        calls
            .lines()
            .any(|l| l.starts_with("bootstrap gui/") && l.contains("/com.tampajohn.chug-loopd")),
        "a fresh job is bootstrapped: {calls}"
    );

    // Rerun: kickstart, never a second bootstrap.
    let out = r.run_service_leg("macos-arm64", &["--with-loopd"], "Darwin", Some(&repo));
    assert_eq!(out.status.code(), Some(0), "{}", out_text(&out));
    let calls = std::fs::read_to_string(r.launchctl_state().join("calls.log")).unwrap();
    assert_eq!(
        calls.matches("bootstrap gui/").count(),
        1,
        "no duplicate bootstrap: {calls}"
    );
    assert!(calls.contains("kickstart"), "the rerun kickstarts: {calls}");
}

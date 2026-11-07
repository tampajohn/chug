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

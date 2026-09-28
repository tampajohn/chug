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
        let mut path = "/usr/bin:/bin:/usr/sbin:/sbin".to_string();
        if let Some(extra) = path_extra {
            path = format!("{}:{}", extra.display(), path);
        }
        Command::new("sh")
            .arg(&self.script)
            .env("CHUG_RELEASE_URL_BASE", format!("file://{}", self.release_dir.display()))
            .env("CHUG_INSTALL_PLATFORM", platform)
            .env("CHUG_INSTALL_DIR", &self.install_dir)
            .env("HOME", &self.home)
            .env("PATH", path)
            .output()
            .expect("spawning sh install.sh")
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

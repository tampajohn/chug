//! T352 — `chug update` + the opt-in loop integration: the fixtures run
//! the REAL `chug update` against a FAKE release server (newer/same/older,
//! checksum mismatch, staleness + source fallback), the atomicity proof
//! (rename, never truncate), the `--check` exit-code contract, and the
//! loopd wiring (CHUG_AUTO_UPDATE=1 schedules the daily check BETWEEN
//! cycles — the cycle-id capture asserts the swap never interleaves with a
//! running cycle).
//!
//! The release contract is install.sh's; the static pins keep the two
//! sides byte-honest (same asset naming, same sidecar format, same
//! profile-owned keys).

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use sha2::{Digest as _, Sha256};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

/// sha256 hex of bytes (the same digest the sidecar carries).
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- the fake release server ----------

/// One HTTP exchange served by the stub: a status, headers, and a body.
struct Resp {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Resp {
    fn json(body: String) -> Self {
        Resp {
            status: 200,
            headers: vec![("content-type".to_string(), "application/json".to_string())],
            body: body.into_bytes(),
        }
    }

    fn bytes(content_type: &str, body: Vec<u8>) -> Self {
        Resp {
            status: 200,
            headers: vec![("content-type".to_string(), content_type.to_string())],
            body,
        }
    }

    fn not_found() -> Self {
        Resp { status: 404, headers: vec![], body: b"not found".to_vec() }
    }
}

/// Serve the handler's responses on 127.0.0.1, one connection per request
/// (every response carries `connection: close`). The stub exits after ~5s
/// of idle (or 64 requests), so a leg that makes FEWER requests than
/// expected still lets `join()` return instead of hanging the suite — an
/// over-quota leg fails its assertions loudly instead.
fn serve(handler: impl Fn(&str) -> Resp + Send + 'static) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the stub release server");
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let mut served = 0;
        let mut idle = Instant::now();
        while served < 64 && idle.elapsed() < Duration::from_secs(5) {
            listener.set_nonblocking(true).expect("nonblocking accept");
            let Ok((mut stream, _)) = listener.accept() else {
                thread::sleep(Duration::from_millis(20));
                continue;
            };
            listener.set_nonblocking(false).ok();
            idle = Instant::now();
            served += 1;
            // Read one request (headers end at CRLFCRLF; the bodies are GETs
            // with no body).
            let mut buf = vec![0u8; 8192];
            let mut req = Vec::new();
            loop {
                let read = stream.read(&mut buf).unwrap_or(0);
                if read == 0 {
                    break;
                }
                req.extend_from_slice(&buf[..read]);
                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&req);
            let target = text
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let resp = handler(&target);
            let mut out = format!("HTTP/1.1 {}\r\nconnection: close\r\n", resp.status);
            for (k, v) in &resp.headers {
                out.push_str(&format!("{k}: {v}\r\n"));
            }
            out.push_str(&format!("content-length: {}\r\n\r\n", resp.body.len()));
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.write_all(&resp.body);
            let _ = stream.flush();
        }
    });
    (url, handle)
}

/// The release-metadata document the API endpoint returns. `published_at`
/// controls the staleness verdict (the T348 rule).
fn release_doc(tag: &str, published_at: &str) -> String {
    format!(r#"{{"tag_name":"{tag}","name":"{tag}","published_at":"{published_at}","assets":[]}}"#)
}

// ---------- the fake release artifact ----------

/// A stub `chug` binary for the tarball: `--version` prints the named
/// version, `capabilities` prints a contract_version document, `daemon
/// --help` exits 0 (the daemon surface IS live in this stub).
fn stub_chug_script(version: &str, contract_version: &str) -> String {
    format!(
        "#!/bin/sh\n\
         case \"$1\" in\n\
         --version) echo 'chug {version}' ;;\n\
         capabilities) echo '{{\"contract_version\":\"{contract_version}\",\"features\":{{}}}}' ;;\n\
         daemon) [ \"$2\" = --help ] && exit 0 || exit 1 ;;\n\
         esac\nexit 0\n"
    )
}

/// Package `content` as the `chug-<platform>.tar.gz` bytes + the matching
/// sha256 sidecar string (the release workflow's packaging, miniaturized:
/// tar -czf with the binary at the archive root).
fn package_release(content: &str) -> (Vec<u8>, String) {
    let dir = tempfile::tempdir().expect("staging dir");
    let bin = dir.path().join("chug");
    fs::write(&bin, content).expect("write the stub binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).expect("chmod the stub");
    }
    let tarball = dir.path().join("chug-fixture.tar.gz");
    let status = Command::new("tar")
        .arg("-czf")
        .arg(&tarball)
        .arg("-C")
        .arg(dir.path())
        .arg("chug")
        .status()
        .expect("spawn tar for the fixture package");
    assert!(status.success(), "tar packaging failed");
    let tar_bytes = fs::read(&tarball).expect("read the tarball");
    let sidecar = format!("{}  chug-fixture.tar.gz\n", sha256_hex(&tar_bytes));
    (tar_bytes, sidecar)
}

/// The installed-binary fixture: a COPY of the real test binary at
/// `<install>/chug`, and CHUG_UPDATE_TARGET pointing at it — the seam that
/// keeps the update from ever touching the cargo-built binary itself.
fn install_fixture() -> (tempfile::TempDir, PathBuf) {
    let install = tempfile::tempdir().expect("install dir");
    let target = install.path().join("chug");
    fs::copy(repo_root().join(env!("CARGO_BIN_EXE_chug")), &target)
        .expect("copy the real binary as the installed chug");
    (install, target)
}

/// Spawn + bounded wait: a wedged child fails its test in seconds, never
/// hangs the suite.
fn bounded_output(cmd: &mut Command) -> std::process::Output {
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn chug");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Ok(Some(_)) = child.try_wait() {
            return child.wait_with_output().expect("collect output");
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("chug update never finished in 60s");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

/// The standard newer-release scenario wiring: a server carrying the
/// metadata endpoint and both asset endpoints.
fn newer_release_server(
    tag: &str,
    published_at: &str,
    tarball: Vec<u8>,
    sidecar: &str,
    sidecar_override: Option<&str>,
) -> (String, thread::JoinHandle<()>) {
    let tag = tag.to_string();
    let published_at = published_at.to_string();
    let good_sidecar = sidecar.to_string();
    let bad_sidecar = sidecar_override.map(str::to_string);
    serve(move |target| {
        if target.ends_with("/releases/latest") {
            Resp::json(release_doc(&tag, &published_at))
        } else if target.ends_with(".tar.gz.sha256") {
            match &bad_sidecar {
                Some(sc) => Resp::bytes("text/plain", sc.clone().into_bytes()),
                None => Resp::bytes("text/plain", good_sidecar.clone().into_bytes()),
            }
        } else if target.ends_with(".tar.gz") {
            Resp::bytes("application/gzip", tarball.clone())
        } else {
            Resp::not_found()
        }
    })
}

// ---------- the legs ----------

/// newer → downloaded, checksum-verified, atomically swapped, `old -> new`
/// printed with the contract_version change, exit 0.
#[test]
fn a_newer_release_is_downloaded_verified_and_swapped() {
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    let (url, stub) = newer_release_server("v9.9.9", "2026-11-09T00:00:00Z", tarball, &sidecar, None);
    let (install, target) = install_fixture();
    let old_bytes = fs::read(&target).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("the stub served exactly 3 requests");
    assert_eq!(out.status.code(), Some(0), "swap exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("updated: ") && stdout.contains("-> v9.9.9"),
        "the updated line names old -> new: {stdout}"
    );
    // The installed file now carries the STUB's bytes (checksum verified —
    // a wrong checksum leg has its own test below).
    let new_bytes = fs::read(&target).unwrap();
    assert_ne!(new_bytes, old_bytes, "the swap replaced the bytes");
    assert!(
        new_bytes.starts_with(b"#!/bin/sh"),
        "the installed binary is the served artifact"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o755,
            "the exec bit travels with the new inode"
        );
    }
    let _ = install; // hold the tempdir open
}

/// The ATOMICITY fixture: an open handle on the installed binary keeps
/// reading the OLD image after the swap — the rename property that makes
/// replacing a running Mach-O safe; an in-place truncate would corrupt it.
#[test]
fn the_swap_is_a_rename_the_old_image_survives_for_running_processes() {
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    let (url, stub) = newer_release_server("v9.9.9", "2026-11-09T00:00:00Z", tarball, &sidecar, None);
    let (install, target) = install_fixture();
    let mut held = fs::File::open(&target).expect("open the installed binary (the running image)");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub complete");
    assert_eq!(out.status.code(), Some(0), "swap exit 0");
    // The open handle still reads the OLD image, whole.
    let mut still = Vec::new();
    held.read_to_end(&mut still).expect("read the open inode");
    assert_eq!(
        still,
        fs::read(repo_root().join(env!("CARGO_BIN_EXE_chug"))).unwrap(),
        "a running process keeps its image: the open inode was never truncated"
    );
    // And the path carries the NEW bytes.
    assert!(fs::read(&target).unwrap().starts_with(b"#!/bin/sh"));
    let _ = install;
}

/// The current release is a no-op: `already current`, no swap, exit 0.
/// The tag must equal the RUNNING binary's package version — that is what
/// the comparison uses (the tarball stub's own version is irrelevant).
#[test]
fn the_current_release_is_a_no_op() {
    let (tarball, sidecar) = package_release(&stub_chug_script("0.1.0", "1"));
    let current_tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let (url, stub) = newer_release_server(&current_tag, "2026-11-09T00:00:00Z", tarball, &sidecar, None);
    let (install, target) = install_fixture();
    let before = fs::read(&target).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served 1 request");
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("already current"), "{stdout}");
    assert_eq!(fs::read(&target).unwrap(), before, "no swap happened");
    let _ = install;
}

/// older → REFUSED (no silent downgrade), non-zero exit, no swap;
/// `--check` reports the refusal at exit 0; `--force` installs it.
#[test]
fn an_older_release_is_refused_unless_forced() {
    let (tarball, sidecar) = package_release(&stub_chug_script("0.1.0", "1"));
    // Apply leg: refused.
    let (url, stub) = newer_release_server("v0.1.0", "2026-11-09T00:00:00Z", tarball.clone(), &sidecar, None);
    let (install, target) = install_fixture();
    let before = fs::read(&target).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served 1 request");
    assert_eq!(out.status.code(), Some(1), "a refusal is non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("refusing to downgrade"),
        "the refusal is named: {stderr}"
    );
    assert_eq!(fs::read(&target).unwrap(), before, "no swap happened");

    // --check leg: exit 0 with the refusal note (not 10, not 2).
    let (url2, stub2) = newer_release_server("v0.1.0", "2026-11-09T00:00:00Z", tarball.clone(), &sidecar, None);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--check");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url2)
        .env("CHUG_RELEASE_API_BASE", &url2)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub2.join().expect("stub served 1 request");
    assert_eq!(out.status.code(), Some(0), "check reports the refusal at 0");
    assert!(String::from_utf8_lossy(&out.stdout).contains("refusing to downgrade"));

    // --force leg: the downgrade installs.
    let (url3, stub3) = newer_release_server("v0.1.0", "2026-11-09T00:00:00Z", tarball.clone(), &sidecar, None);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--force");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url3)
        .env("CHUG_RELEASE_API_BASE", &url3)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub3.join().expect("stub served 3 requests");
    assert_eq!(out.status.code(), Some(0), "forced downgrade installs");
    assert!(fs::read(&target).unwrap().starts_with(b"#!/bin/sh"), "swapped");
    let _ = install;
}

/// checksum mismatch → no swap, error (verify BEFORE install, install.sh's
/// ordering; a tampered artifact must never reach the disk as `chug`).
#[test]
fn a_checksum_mismatch_refuses_the_swap() {
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    let wrong = format!("{}  chug-fixture.tar.gz\n", "0".repeat(64));
    let (url, stub) = newer_release_server(
        "v9.9.9",
        "2026-11-09T00:00:00Z",
        tarball,
        &sidecar,
        Some(&wrong),
    );
    let (install, target) = install_fixture();
    let before = fs::read(&target).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served both asset requests");
    assert_eq!(out.status.code(), Some(1), "a mismatch is an error");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("CHECKSUM MISMATCH"), "{stderr}");
    assert_eq!(fs::read(&target).unwrap(), before, "NO swap on mismatch");
    let _ = install;
}

/// `--check` exit codes are the machine contract: 0 current / 10
/// update-available / 2 error.
#[test]
fn check_exit_codes_are_the_machine_contract() {
    // 10: newer release.
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    let (url, stub) = newer_release_server("v9.9.9", "2026-11-09T00:00:00Z", tarball.clone(), &sidecar, None);
    let (install, target) = install_fixture();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--check");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served 1 request");
    assert_eq!(out.status.code(), Some(10), "update available = 10");
    assert!(String::from_utf8_lossy(&out.stdout).contains("update available"));

    // 0: current release (the tag matches the running binary's version).
    let current_tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    let (url2, stub2) = newer_release_server(&current_tag, "2026-11-09T00:00:00Z", tarball, &sidecar, None);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--check");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url2)
        .env("CHUG_RELEASE_API_BASE", &url2)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub2.join().expect("stub served 1 request");
    assert_eq!(out.status.code(), Some(0), "current = 0");
    assert!(String::from_utf8_lossy(&out.stdout).contains("already current"));

    // 2: the check itself failed (a dead release endpoint), never 1.
    let dead = "http://127.0.0.1:1";
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--check");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", dead)
        .env("CHUG_RELEASE_API_BASE", dead);
    let out = bounded_output(&mut cmd);
    assert_eq!(out.status.code(), Some(2), "a failed check = 2: {}", out.status);
    assert!(String::from_utf8_lossy(&out.stderr).contains("update --check failed"));
    let _ = install;
}

// ---------- the source-build leg's fixture ----------

/// The source-build leg's fixture: a fake checkout (a Cargo.toml at its
/// root), a stub `cargo` on PATH that records its argv to
/// `<checkout>/cargo-argv.log` and leaves a "built" stub chug at
/// `<checkout>/target/release/chug` whose `--version` reports
/// `built_version`, and the installed-binary seam at `<install>/chug` —
/// the REAL test binary (the feature-lean default build,
/// tests/daemon_feature_off.rs's doctrine) unless `installed` names a stub
/// script to install instead.
struct SourceLegFixture {
    checkout: tempfile::TempDir,
    bin: tempfile::TempDir,
    /// Held, never read: the tempdir must outlive the run (the same hold
    /// the `let _ = install;` tail gives the standalone fixtures).
    _install: tempfile::TempDir,
    target: PathBuf,
}

impl SourceLegFixture {
    fn new(built_version: &str) -> Self {
        Self::with_installed(None, built_version)
    }

    fn with_installed(installed: Option<&str>, built_version: &str) -> Self {
        let checkout = tempfile::tempdir().expect("fake checkout");
        fs::write(checkout.path().join("Cargo.toml"), "[package]\nname=\"x\"\n")
            .expect("write the manifest");
        let target_rel = checkout.path().join("target/release");
        fs::create_dir_all(&target_rel).expect("target dir");
        let built = target_rel.join("chug");
        fs::write(&built, stub_chug_script(built_version, "1")).expect("write the built stub");
        let bin = tempfile::tempdir().expect("bin dir");
        let cargo_stub = bin.path().join("cargo");
        // The stub logs only BUILD invocations: update.rs's
        // cargo-availability probe (`cargo --version`) runs the same stub
        // with the test process's cwd — logging there would litter the
        // worktree root with a stray cargo-argv.log.
        fs::write(
            &cargo_stub,
            "#!/bin/sh\n[ \"$1\" = build ] && printf '%s\\n' \"$*\" >> \"$PWD/cargo-argv.log\"\n\
                 printf 'cargo build --release would compile here\\n' >&2\nexit 0\n",
        )
        .expect("write the cargo stub");
        let (install, target) = match installed {
            Some(script) => {
                let install = tempfile::tempdir().expect("install dir");
                let target = install.path().join("chug");
                fs::write(&target, script).expect("write the installed stub");
                (install, target)
            }
            None => install_fixture(),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&built, fs::Permissions::from_mode(0o755)).expect("chmod built");
            fs::set_permissions(&cargo_stub, fs::Permissions::from_mode(0o755))
                .expect("chmod cargo");
            if installed.is_some() {
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
                    .expect("chmod the installed stub");
            }
        }
        Self { checkout, bin, _install: install, target }
    }

    /// The cargo stub's argv capture (inside the checkout — the stub's
    /// `$PWD` is where cargo runs it).
    fn argv_log(&self) -> PathBuf {
        self.checkout.path().join("cargo-argv.log")
    }

    /// The `chug update` command wired for this fixture: `flags` between
    /// `update` and `--source`, CHUG_UPDATE_TARGET at the installed binary,
    /// CARGO_TARGET_DIR pinned to the checkout's target (the harness itself
    /// may carry a shared-cache one), the stub cargo leading PATH, and both
    /// release URL bases at the fake server.
    fn update_cmd(&self, flags: &[&str], url: &str) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
        cmd.arg("update");
        for flag in flags {
            cmd.arg(flag);
        }
        cmd.arg("--source").arg(self.checkout.path());
        cmd.env("CHUG_UPDATE_TARGET", &self.target)
            .env("CHUG_RELEASE_URL_BASE", url)
            .env("CHUG_RELEASE_API_BASE", url)
            .env("CHUG_INSTALL_PLATFORM", "linux-x86_64")
            .env("CARGO_TARGET_DIR", self.checkout.path().join("target"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.path().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            );
        cmd
    }
}

/// stale release (8+ days old, the T348 window) + `--source <checkout>` +
/// a cargo on PATH → the SOURCE path is taken: `cargo build --release`
/// runs and its result is swapped in. THE PROBE FACET is finding 1's
/// killing fixture: the installed binary is the REAL feature-off test
/// build, so the probe must read it feature-OFF and the stub cargo must
/// NOT be handed `--features daemon` (the heaviest build chug has — the
/// vacuous probe, `daemon --help` exit 0, read this exact binary as
/// daemon-live and silently flipped feature-off installs to daemon-on).
#[test]
fn a_stale_release_takes_the_source_build_path() {
    // The release is NEWER but stale — the source path wins (the T347
    // smell: the pipeline may be frozen; the checkout is the operator's
    // freshness source).
    let (tarball, sidecar) = package_release(&stub_chug_script("8.8.8", "1"));
    let old = "2026-01-01T00:00:00Z";
    let (url, stub) = newer_release_server("v8.8.8", old, tarball, &sidecar, None);
    let fix = SourceLegFixture::new("8.8.8");

    // Fixture validity, on the real bytes: the installed binary IS the
    // feature-lean default build — its bytes carry the compiled-in
    // feature-off refusal literal (the grep loopd's resolve_daemon_bin
    // runs). If this fails the suite was built with --features daemon;
    // the ground-truth assert below needs a real feature-off binary.
    let validity = Command::new("grep")
        .args(["-aq", "built without the judge daemon"])
        .arg(&fix.target)
        .output()
        .expect("grep the fixture binary");
    assert!(
        validity.status.success(),
        "the fixture binary must be the feature-off default build (its bytes carry the \
         compiled-in refusal literal) — the suite must run with default features"
    );
    // The blindness anchor, on the same real bytes: `daemon --help` exits 0
    // — the vacuous probe's sole signal — so this fixture's discrimination
    // is real, not vacuously green (against the old mechanism the argv
    // below carried the flag and the assert failed).
    let help = Command::new(&fix.target)
        .args(["daemon", "--help"])
        .output()
        .expect("spawn the real binary daemon --help");
    assert_eq!(
        help.status.code(),
        Some(0),
        "clap answers --help at parse time on the feature-off build too — the vacuous \
         probe would have read this binary as daemon-live"
    );

    let mut cmd = fix.update_cmd(&[], &url);
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(0), "source swap exit 0");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("updated: ") && stdout.contains("source build from"),
        "the source-build swap is named: {stdout}"
    );
    assert!(
        stdout.contains("-> 8.8.8"),
        "the source leg renders the built version in dotted form, not a Debug tuple \
         (finding 3): {stdout}"
    );
    assert!(
        fs::read(&fix.target).unwrap().starts_with(b"#!/bin/sh"),
        "the swapped-in binary is the checkout's build (the stub), not a stray artifact"
    );
    // THE PROBE ASSERT (the killing one): the cargo invocation follows the
    // probe of the binary BEING REPLACED — a REAL feature-off binary — so
    // the argv must NOT carry `--features daemon`. Against the vacuous
    // probe mechanism this argv carried the flag (the blindness anchor
    // above proves the old probe would have been fooled by this exact
    // binary) and this assert failed.
    let argv = fs::read_to_string(fix.argv_log()).expect("cargo argv");
    assert!(argv.contains("build --release"), "{argv}");
    assert!(
        !argv.contains("--features daemon"),
        "the REAL feature-off installed binary must not flip the fallback build to \
         daemon-on: {argv}"
    );
}

/// The ON leg of the probe sweep: a daemon-live installed binary (the stub
/// chug — its daemon surface answers `--help`, and, what the probe
/// actually keys on, its bytes carry NO feature-off refusal literal) hands
/// `--features daemon` to the fallback build. A real feature-on release
/// binary is out of the suite's reach (the candle stack is a minutes-long
/// build and the shared cache is single-tenant); the stub's literal-absence
/// is exactly the byte property a real feature-on release binary has, and
/// the OFF leg above pins the real-binary discrimination.
#[test]
fn the_daemon_probe_hands_features_daemon_to_a_live_daemon_binary() {
    let (tarball, sidecar) = package_release(&stub_chug_script("8.8.8", "1"));
    let (url, stub) = newer_release_server("v8.8.8", "2026-01-01T00:00:00Z", tarball, &sidecar, None);
    let fix = SourceLegFixture::with_installed(Some(&stub_chug_script("0.22.0", "1")), "8.8.8");
    let mut cmd = fix.update_cmd(&[], &url);
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(0), "the source leg applied");
    let argv = fs::read_to_string(fix.argv_log()).expect("the cargo stub ran");
    assert!(
        argv.contains("--features daemon"),
        "a daemon-live installed binary must flip the fallback build to daemon-on: {argv}"
    );
}

/// The source-build version floor (finding 2, mutant m8's killing fixture):
/// a source build OLDER than the running binary is REFUSED without --force
/// — the floor checks the RESULT's self-reported version, not the release
/// tag. Dropping the floor check swaps the older build in and fails the
/// no-swap / non-zero-exit asserts.
#[test]
fn an_older_source_build_is_refused_unless_forced() {
    let (tarball, sidecar) = package_release(&stub_chug_script("8.8.8", "1"));
    // Apply leg: the built 0.1.0 is older than the running binary → refused.
    let (url, stub) = newer_release_server(
        "v8.8.8",
        "2026-01-01T00:00:00Z",
        tarball.clone(),
        &sidecar,
        None,
    );
    let fix = SourceLegFixture::new("0.1.0");
    let before = fs::read(&fix.target).unwrap();
    let mut cmd = fix.update_cmd(&[], &url);
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(1), "a floor refusal is non-zero");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("refusing to downgrade") && stderr.contains("0.1.0"),
        "the refusal names the source build's own version: {stderr}"
    );
    // The build RAN — the floor is a check on the RESULT, not a pre-filter
    // that never reaches cargo.
    let argv = fs::read_to_string(fix.argv_log()).expect("the cargo stub ran before the floor");
    assert!(argv.contains("build --release"), "{argv}");
    assert_eq!(fs::read(&fix.target).unwrap(), before, "NO swap on the floor refusal");

    // --force leg: the older source build installs (the floor is
    // force-licensable, the same rule as the download leg).
    let (url2, stub2) = newer_release_server("v8.8.8", "2026-01-01T00:00:00Z", tarball, &sidecar, None);
    let mut cmd = fix.update_cmd(&["--force"], &url2);
    let out = bounded_output(&mut cmd);
    stub2.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(0), "--force licenses the downgrade");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("updated: ") && stdout.contains("-> 0.1.0"),
        "the forced source downgrade renders the built version in dotted form: {stdout}"
    );
    assert!(
        fs::read(&fix.target).unwrap().starts_with(b"#!/bin/sh"),
        "the older source build was swapped in under --force"
    );
}

/// The `--check --source` leg (finding 2): a stale release + a named
/// checkout reports the SOURCE path at exit 10 — the machine contract's
/// update-available — and the check never builds (a dry-run: the cargo
/// stub never runs).
#[test]
fn check_with_source_reports_the_source_path_at_exit_10() {
    let (tarball, sidecar) = package_release(&stub_chug_script("8.8.8", "1"));
    let (url, stub) = newer_release_server("v8.8.8", "2026-01-01T00:00:00Z", tarball, &sidecar, None);
    let fix = SourceLegFixture::new("8.8.8");
    let mut cmd = fix.update_cmd(&["--check"], &url);
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(10), "the source path is update-available");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("update available") && stdout.contains("source build from"),
        "the check names the source path: {stdout}"
    );
    assert!(
        !fix.argv_log().exists(),
        "the check never builds (a dry-run) — the cargo stub never ran"
    );
}

/// Finding 5b: a FRESH release wins over a named-but-unusable `--source` —
/// the checkout is validated only when the source path is actually taken,
/// so an unusable checkout cannot hard-error a run whose release already
/// answers the question. Against the pre-fix order (validate-then-decide)
/// this run exits 1 with "does not exist".
#[test]
fn a_fresh_release_wins_over_an_unusable_named_source() {
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    // 2099: always fresh — ahead of any suite run (the staleness window is
    // 7 days behind whatever today is).
    let (url, stub) = newer_release_server("v9.9.9", "2099-01-01T00:00:00Z", tarball, &sidecar, None);
    let (install, target) = install_fixture();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--source").arg("/definitely/not/here/t352");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the download requests");
    assert_eq!(out.status.code(), Some(0), "the fresh-release path applied");
    assert!(
        fs::read(&target).unwrap().starts_with(b"#!/bin/sh"),
        "the release tarball was installed — the source path was moot, not an error"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("source build from"),
        "the source path was not taken: {stdout}"
    );
    assert!(
        !stdout.contains("NOTE: the release is"),
        "a fresh release carries no staleness note: {stdout}"
    );
    let _ = install;
}

/// Finding 5a: a newer-but-STALE release still installs when no source is
/// named (the checkout path is the operator's option, not a requirement),
/// and the install line names the T347 smell and its remedy; the --check
/// line carries the same note.
#[test]
fn a_stale_release_install_carries_the_staleness_note() {
    let (tarball, sidecar) = package_release(&stub_chug_script("9.9.9", "1"));
    let (url, stub) = newer_release_server(
        "v9.9.9",
        "2026-01-01T00:00:00Z",
        tarball.clone(),
        &sidecar,
        None,
    );
    let (install, target) = install_fixture();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url)
        .env("CHUG_RELEASE_API_BASE", &url)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub.join().expect("stub served the download requests");
    assert_eq!(out.status.code(), Some(0), "the stale release still installs");
    assert!(
        fs::read(&target).unwrap().starts_with(b"#!/bin/sh"),
        "the stale release was installed"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("updated: ") && stdout.contains("NOTE: the release is"),
        "the stale install names the T347 smell: {stdout}"
    );
    assert!(
        stdout.contains("--source"),
        "the note names the remedy (the source fallback): {stdout}"
    );

    // The --check line carries the same note (the machine surface loopd
    // consumes sees the staleness too).
    let (url2, stub2) = newer_release_server("v9.9.9", "2026-01-01T00:00:00Z", tarball, &sidecar, None);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_chug"));
    cmd.arg("update").arg("--check");
    cmd.env("CHUG_UPDATE_TARGET", &target)
        .env("CHUG_RELEASE_URL_BASE", &url2)
        .env("CHUG_RELEASE_API_BASE", &url2)
        .env("CHUG_INSTALL_PLATFORM", "linux-x86_64");
    let out = bounded_output(&mut cmd);
    stub2.join().expect("stub served the metadata request only");
    assert_eq!(out.status.code(), Some(10));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("update available") && stdout.contains("NOTE: the release is"),
        "the check line carries the staleness note too: {stdout}"
    );
    let _ = install;
}

// ---------- the loop integration ----------

/// The wiring is the contract: loopd.sh sources the fragment, calls the
/// check BETWEEN cycles (before the single-driver probe and any cycle
/// launch), the fragment is opt-in + daily-cadence + exit-10-apply, and
/// the profile/install allowlists carry the flag.
#[test]
fn the_loop_wiring_is_the_contract() {
    let loopd = fs::read_to_string(repo_root().join("loopd.sh")).expect("loopd.sh");
    let frag = fs::read_to_string(repo_root().join("scripts/loopd_auto_update.sh"))
        .expect("scripts/loopd_auto_update.sh");
    let loader = fs::read_to_string(repo_root().join("scripts/loopd_profile_loader.sh"))
        .expect("scripts/loopd_profile_loader.sh");
    let install_sh = fs::read_to_string(repo_root().join("install.sh")).expect("install.sh");

    // loopd.sh sources the fragment and calls it inside the while body.
    assert!(
        loopd.contains(r#". "$(dirname "$0")/scripts/loopd_auto_update.sh""#),
        "loopd.sh sources the T352 fragment"
    );
    // Line-anchored two-space indent — the finding-4 pin: the probe arm
    // (`update-check)`) carries the same call at a DEEPER indent and
    // substring-matched the old `"  loopd_auto_update_check\n"` pin, so the
    // pin passed even with the body call deleted. The line-start anchor
    // matches ONLY the while-body call site: delete it and this pin fails.
    let call_pos = loopd
        .find("\n  loopd_auto_update_check\n")
        .expect("loopd.sh calls the check in the while body (line-anchored two-space indent — \
                 the deeper-indented probe arm must not satisfy this pin)");
    let driver_pos = loopd
        .find("Single-driver: never overlap another LOOP-SPEC run")
        .expect("the single-driver probe comment anchors the body order");
    assert!(
        call_pos < driver_pos,
        "the check runs BETWEEN cycles — before the single-driver probe and any cycle launch"
    );

    // The fragment: opt-in, daily cadence, the exit-10 apply, the
    // between-cycles disposition, and the machine exit codes.
    assert!(
        frag.contains(r#"[ "${CHUG_AUTO_UPDATE:-0}" = "1" ] || return 0"#),
        "opt-in gate"
    );
    assert!(frag.contains("86400"), "the daily cadence (CHECK_INTERVAL_SECS)");
    assert!(frag.contains("chug update --check"), "the machine surface");
    assert!(frag.contains("applying between cycles"), "the apply disposition");
    assert!(
        frag.contains("never swaps a binary a running cycle is executing"),
        "the between-cycles disposition line says so"
    );

    // The flag is profile-owned end to end (the loader allowlist + the
    // identity pass-through + install.sh's owns-exactly line).
    assert!(loader.contains("CHUG_AUTO_UPDATE=*"), "the loader allowlist carries the flag");
    assert!(loader.contains("CHUG_AUTO_UPDATE)"), "the loader applies it by identity");
    assert!(install_sh.contains("CHUG_AUTO_UPDATE)"), "install.sh owns the key");
    assert!(
        install_sh.contains("CHUG_DECISION_PROXY_TOKEN, CHUG_AUTO_UPDATE;"),
        "install.sh's owns-exactly line names it"
    );

    // And `loopd.sh update-check` is the operator probe (the routing/
    // predicate pattern).
    assert!(
        loopd.contains("update-check)"),
        "the update-check probe mode exists"
    );
}

/// The behavioral surface: `bash loopd.sh update-check` with a stub `chug`
/// on PATH — CHUG_AUTO_UPDATE=1 runs the check and stamps the daily latch;
/// a second probe is cadence-gated (one check per day); the flag unset is
/// a silent no-op.
#[cfg(unix)]
#[test]
fn the_update_check_probe_schedules_and_is_daily_gated() {
    let root = tempfile::tempdir().expect("fixture root");
    // The sandbox carries the REAL loopd.sh too — the probe resolves the
    // fragments relative to loopd.sh's own path, never the cwd.
    fs::copy(repo_root().join("loopd.sh"), root.path().join("loopd.sh")).expect("copy loopd.sh");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    // loopd.sh sources its scripts/ fragments relative to ITS OWN path —
    // the fixture carries them (the run-mode fixture's shape).
    let scripts = root.path().join("scripts");
    fs::create_dir_all(&scripts).expect("scripts dir");
    for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
        let entry = entry.expect("scripts entry");
        if entry.file_type().expect("type").is_file() {
            fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
        }
    }
    // The stub chug records EVERY invocation to the capture log and exits
    // 10 (update available) for --check — the loop would apply next.
    let stub_chug = bin.join("chug");
    fs::write(
        &stub_chug,
        "#!/bin/sh\nprintf 'chug %s\\n' \"$*\" >> \"$PWD/update-capture.log\"\n\
         [ \"$1\" = update ] && [ \"$2\" = --check ] && exit 10\nexit 0\n",
    )
    .expect("write the stub chug");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub_chug, fs::Permissions::from_mode(0o755)).expect("chmod stub");
    }
    fs::write(root.path().join("NO-SUCH-SITE"), "").expect("site sentinel");

    let probe = |env_extra: &[(&str, &str)]| {
        let mut cmd = Command::new("bash");
        cmd.arg("loopd.sh").arg("update-check").current_dir(root.path());
        cmd.env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()))
            .env("CHUG_SITE_DIR", root.path().join("NO-SUCH-SITE"))
            .env("CHUG_SITE_SYNC_NO_PUSH", "1");
        for (k, v) in env_extra {
            cmd.env(k, v);
        }
        common::pin_loopd_env_files(&mut cmd, root.path());
        cmd.output().expect("spawn bash loopd.sh update-check")
    };

    // Flag unset: a silent no-op — no check, no latch, opt-in by
    // construction.
    let out = probe(&[]);
    assert!(
        out.status.success(),
        "the flag-unset probe must succeed — stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !root.path().join("update-capture.log").exists(),
        "no CHUG_AUTO_UPDATE=1 → the check never runs"
    );

    // Flag set: the check runs, the latch is stamped.
    let out = probe(&[("CHUG_AUTO_UPDATE", "1")]);
    assert!(out.status.success(), "{:?}", out.status);
    let capture = fs::read_to_string(root.path().join("update-capture.log")).expect("capture");
    assert!(
        capture.contains("chug update --check"),
        "the daily check ran: {capture}"
    );
    let latch = root.path().join(".chug/loopd/update-check.json");
    assert!(latch.is_file(), "the cadence latch exists");
    assert!(
        fs::read_to_string(&latch).unwrap().contains("last_check_ts"),
        "the latch carries the stamp: {}",
        fs::read_to_string(&latch).unwrap()
    );

    // Second probe: cadence-gated — one check per day, no second capture.
    let before = capture.lines().count();
    let out = probe(&[("CHUG_AUTO_UPDATE", "1")]);
    assert!(out.status.success());
    let capture2 = fs::read_to_string(root.path().join("update-capture.log")).unwrap();
    assert_eq!(
        capture2.lines().count(),
        before,
        "the daily latch gates the second check"
    );
}

/// The loop fixture (run mode): with CHUG_AUTO_UPDATE=1, the check is
/// SCHEDULED in the supervisor's body, and — the cycle-id capture — the
/// update never runs while a cycle is executing: the stub `chug` records
/// whether the cycle's CYCLE-ACTIVE marker was live at check time, and
/// every captured line must read `cycle_active=no`.
#[cfg(unix)]
#[test]
fn the_run_loop_checks_between_cycles_never_mid_cycle() {
    use std::process::Child;
    let root = tempfile::tempdir().expect("fixture root");
    let bin = root.path().join("bin");
    fs::create_dir_all(&bin).expect("bin dir");
    fs::copy(repo_root().join("loopd.sh"), root.path().join("loopd.sh")).expect("copy loopd.sh");
    let scripts = root.path().join("scripts");
    fs::create_dir_all(&scripts).expect("scripts dir");
    for entry in fs::read_dir(repo_root().join("scripts")).expect("scripts dir") {
        let entry = entry.expect("scripts entry");
        if entry.file_type().expect("type").is_file() {
            fs::copy(entry.path(), scripts.join(entry.file_name())).expect("copy script");
        }
    }
    // ps: a successful single-driver probe (no other driver).
    let ps = bin.join("ps");
    fs::write(&ps, "#!/bin/sh\nexit 0\n").expect("ps stub");
    // cargo: the build gate produces the cycle binary (a stub) and exits 0.
    let cargo = bin.join("cargo");
    fs::write(
        &cargo,
        "#!/bin/sh\nmkdir -p \"$PWD/target/release\"\n\
         printf '%s\\n' '#!/bin/sh' > \"$PWD/target/release/chug\"\n\
         printf '%s\\n' 'mkdir -p .chug/loopd && touch .chug/loopd/CYCLE-ACTIVE' >> \"$PWD/target/release/chug\"\n\
         printf '%s\\n' 'sleep 0.2' >> \"$PWD/target/release/chug\"\n\
         printf '%s\\n' 'rm -f .chug/loopd/CYCLE-ACTIVE' >> \"$PWD/target/release/chug\"\n\
         printf '%s\\n' 'printf \"chug: goal complete\\\\nsummary: fixture cycle\\\\n\"' >> \"$PWD/target/release/chug\"\n\
         chmod 755 \"$PWD/target/release/chug\"\nexit 0\n",
    )
    .expect("cargo stub");
    // chug on PATH: the update-check stub, recording the cycle-id state.
    let chug = bin.join("chug");
    fs::write(
        &chug,
        "#!/bin/sh\n\
         printf 'chug %s cycle_active=%s\\n' \"$*\" \\\n\
           \"$(test -f \"$PWD/.chug/loopd/CYCLE-ACTIVE\" && echo yes || echo no)\" \\\n\
           >> \"$PWD/update-capture.log\"\n\
         [ \"$1\" = update ] && [ \"$2\" = --check ] && exit 10\nexit 0\n",
    )
    .expect("chug stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in [&ps, &cargo, &chug] {
            fs::set_permissions(p, fs::Permissions::from_mode(0o755)).expect("chmod stub");
        }
    }
    fs::write(root.path().join("NO-SUCH-SITE"), "").expect("site sentinel");

    let mut cmd = Command::new("bash");
    cmd.arg("loopd.sh").arg("run").current_dir(root.path());
    cmd.env(
        "PATH",
        format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default()),
    )
    .env("CHUG_AUTO_UPDATE", "1")
    .env("CHUG_SITE_DIR", root.path().join("NO-SUCH-SITE"))
    .env("CHUG_SITE_SYNC_NO_PUSH", "1")
    .env("LOOPD_SLEEP_OK", "1")
    .env("LOOPD_SLEEP_FAIL", "1")
    .env("LOOPD_MAX_LOOPS", "2");
    common::pin_loopd_env_files(&mut cmd, root.path());
    let mut child: Child = cmd.spawn().expect("spawn bash loopd.sh run");
    // Wait until the fixture cycle has completed (the goal-complete line
    // in the supervisor log), then kill the loop — the bounded-wait shape.
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let log = root.path().join(".chug/loopd/loopd.log");
        let content = fs::read_to_string(&log).unwrap_or_default();
        if content.contains("cycle OK:") && content.contains("auto-update:") {
            break;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the loop never reached the update check + cycle in 30s. log:\n{content}"
            );
        }
        thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();

    // THE CYCLE-ID CAPTURE: every update invocation saw NO running cycle.
    let capture = fs::read_to_string(root.path().join("update-capture.log")).expect("capture");
    let check_lines: Vec<&str> = capture.lines().filter(|l| l.contains("--check")).collect();
    assert!(
        !check_lines.is_empty(),
        "CHUG_AUTO_UPDATE=1 scheduled the check: {capture}"
    );
    for line in &check_lines {
        assert!(
            line.contains("cycle_active=no"),
            "the update check ran DURING a cycle — the between-cycles contract is broken: {line}"
        );
    }
    // The disposition rode the log: the apply happened after the check.
    let log = fs::read_to_string(root.path().join(".chug/loopd/loopd.log")).unwrap_or_default();
    assert!(
        log.contains("auto-update: update available — applying between cycles"),
        "the disposition line is present: {log}"
    );
    assert!(
        log.find("auto-update: update available").unwrap_or(usize::MAX)
            < log.find("cycle OK:").unwrap_or(0),
        "the update ran BETWEEN cycles (before the cycle verdict), never mid-cycle"
    );
}

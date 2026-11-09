//! T352 — `chug update`: self-update with version-floor protection and a
//! source-build fallback.
//!
//! Every box drifts alone: F94 ran chug 0.9.0 against a 5,753-commit-stale
//! repo, K7's daemon was a month old, and releases can freeze silently
//! (T347) — the Fleet Primary found all of it by hand. This module is the
//! box-side remedy: one subcommand that resolves the latest release,
//! compares it against the running binary, and — only when the release is
//! strictly newer (or `--force` names an explicit downgrade) — downloads
//! it, verifies the sha256 sidecar, and swaps the binary ATOMICALLY
//! (temp file + rename, never an in-place overwrite of a running Mach-O).
//!
//! The release contract is install.sh's, not a fork of it: the same asset
//! base (honoring `CHUG_RELEASE_URL_BASE`), the same
//! `chug-<platform>.tar.gz` + `.sha256` sidecar naming, the same
//! first-whitespace-field sidecar format, the same verify-BEFORE-install
//! ordering, and the same platform derivation (`macos-arm64`,
//! `linux-x86_64`, `linux-aarch64`). tests/update_cmd.rs pins both sides
//! of that contract so they cannot drift apart.
//!
//! The T348 staleness rule rides along: a release older than
//! [`STALE_DAYS`] (7) smells like a stalled pipeline, so when a checkout +
//! cargo toolchain are available (`--source <path>` / `$CHUG_SOURCE`) the
//! update is built FROM SOURCE instead (the T347-era path — the Fleet
//! Primary's 9.7s cached build is the proof it is cheap), with
//! `--features daemon` iff the binary being replaced has the daemon
//! subcommand live (the loopd `judge_probe_ok` shape). Version-floor
//! protection still applies to the RESULT: a source build older than the
//! running binary is refused without `--force`.
//!
//! Opt-in by construction: the subcommand changes nothing unless invoked,
//! and the loopd integration (CHUG_AUTO_UPDATE=1 in the profile) only
//! runs the `--check` probe once per day, BETWEEN cycles — a resident
//! supervisor never swaps a binary a running cycle is executing.
//!
//! Exit codes (`--check` is the machine surface loopd consumes): 0 =
//! current / nothing to do, 10 = an update is available, 2 = the check
//! itself failed.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::build_info::VERSION;

/// The release repo (install.sh's `REPO`, overridable with
/// `CHUG_INSTALL_REPO` for mirrors).
pub const RELEASE_REPO: &str = "tampajohn/chug";

/// The T348 staleness window in days: a release older than this smells
/// like a stalled pipeline (T347's exact failure) and the source-build
/// fallback becomes the fresher path when a checkout is available.
pub const STALE_DAYS: u64 = 7;

/// The once-per-day check cadence (the cheap-exit-era rule the loopd
/// integration holds its latch against).
// The consumer is loopd's bash latch (scripts/loopd_auto_update.sh carries
// the same 86400; tests/update_cmd.rs pins both sides) — the Rust-side
// caller is the contract pin, so the const is deliverable-without-caller
// today (the capabilities.rs posture).
#[allow(dead_code)]
pub const CHECK_INTERVAL_SECS: u64 = 24 * 60 * 60;

/// `chug update --check` exit code: an update is available.
pub const EXIT_UPDATE_AVAILABLE: i32 = 10;

/// `chug update --check` exit code: the check itself failed.
pub const EXIT_CHECK_ERROR: i32 = 2;

/// Bounded download: the largest tarball this update will accept. Release
/// binaries ship the judge daemon (metal/cpu candle stacks), so the
/// artifact is tens of MB — half a GB is a generous corruption guard, not
/// a real size, and anything bigger is by construction not a release
/// artifact.
const MAX_TARBALL_BYTES: u64 = 512 * 1024 * 1024;

/// Bounded download: the metadata/sidecar budget (a JSON document and a
/// 65-byte digest — anything bigger is not what was asked for).
const MAX_SMALL_BODY_BYTES: u64 = 1024 * 1024;

/// Metadata + sidecar fetch budget (webfetch's total bound: these answers
/// are small, and a wedged mirror must cost seconds, not minutes).
const METADATA_TIMEOUT: Duration = Duration::from_secs(30);

/// Tarball fetch budget: a ~100MB daemon-bearing binary over a slow link
/// needs minutes; 300s is the honest outer bound (install.sh's curl has
/// none — the update is the more careful surface).
const TARBALL_TIMEOUT: Duration = Duration::from_secs(300);

// ---------- version arithmetic ----------

/// A parsed three-component semver: `(major, minor, patch)`.
pub type SemVer = (u64, u64, u64);

/// Parse `X.Y.Z` (an optional leading `v`/`V` is stripped, as release tags
/// carry it). Anything else — four components, non-numeric, empty — is
/// `None`: an unparseable version is never silently compared.
pub fn parse_version(raw: &str) -> Option<SemVer> {
    let trimmed = raw.trim();
    let stripped = trimmed
        .strip_prefix('v')
        .or_else(|| trimmed.strip_prefix('V'))
        .unwrap_or(trimmed);
    let mut parts = stripped.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

/// Three-way compare: `Equal`, `Greater` (a newer than b), `Less`.
pub fn cmp_versions(a: &SemVer, b: &SemVer) -> Ordering {
    a.cmp(b)
}

/// The running binary's own package version, parsed. The binary is built
/// from this crate, so this parse cannot fail — but it returns a Result
/// anyway so a future version format change fails loudly, never compares
/// against garbage.
fn current_version() -> anyhow::Result<SemVer> {
    parse_version(VERSION).ok_or_else(|| anyhow!("unparseable built-in version {VERSION:?}"))
}

// ---------- env seams (thin wrappers over pure cores) ----------

fn nonempty_env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// The release repo: install.sh's `CHUG_INSTALL_REPO` override, else the
/// default.
fn release_repo() -> String {
    nonempty_env("CHUG_INSTALL_REPO").unwrap_or_else(|| RELEASE_REPO.to_string())
}

/// The asset base URL: `CHUG_RELEASE_URL_BASE` (a mirror pins its own
/// artifact base), else GitHub's moving `releases/latest/download`
/// pointer — byte-identical to install.sh's `base=` line.
pub fn asset_base() -> String {
    asset_base_from(
        nonempty_env("CHUG_RELEASE_URL_BASE").as_deref(),
        &release_repo(),
    )
}

/// The pure core of [`asset_base`]: the URL the release contract names for
/// a given mirror override (or none) and repo.
fn asset_base_from(url_base: Option<&str>, repo: &str) -> String {
    url_base
        .map(str::to_string)
        .unwrap_or_else(|| format!("https://github.com/{repo}/releases/latest/download"))
}

/// The release-metadata base: `CHUG_RELEASE_API_BASE`, else api.github.com
/// (install.sh's `api_base=` line).
pub fn api_base() -> String {
    api_base_from(nonempty_env("CHUG_RELEASE_API_BASE").as_deref())
}

/// The pure core of [`api_base`].
fn api_base_from(api: Option<&str>) -> String {
    api.map(str::to_string)
        .unwrap_or_else(|| "https://api.github.com".to_string())
}

/// install.sh's mirror rule: a mirror that sets `CHUG_RELEASE_URL_BASE`
/// WITHOUT an explicit API base controls its own freshness — the staleness
/// question is skipped (every mirror run stays offline-able).
pub fn staleness_skipped() -> bool {
    staleness_skipped_from(
        nonempty_env("CHUG_RELEASE_URL_BASE").as_deref(),
        nonempty_env("CHUG_RELEASE_API_BASE").as_deref(),
    )
}

/// The pure core of [`staleness_skipped`].
fn staleness_skipped_from(url_base: Option<&str>, api_base: Option<&str>) -> bool {
    url_base.is_some() && api_base.is_none()
}

/// The asset platform tag — install.sh's exact derivation (the same three
/// published platforms), from the compile-time target rather than spawning
/// `uname` (the target IS what the binary runs as). `CHUG_INSTALL_PLATFORM`
/// overrides, the same knob install.sh honors (and the fixtures' seam).
pub fn platform() -> anyhow::Result<String> {
    platform_from(nonempty_env("CHUG_INSTALL_PLATFORM").as_deref())
}

/// The pure core of [`platform`]: the override when valid, else the
/// compile-time derivation for the target this binary was built for.
fn platform_from(override_tag: Option<&str>) -> anyhow::Result<String> {
    if let Some(tag) = override_tag {
        return validate_platform(tag);
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let derived = "macos-arm64";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    let derived = "linux-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    let derived = "linux-aarch64";
    #[cfg(not(any(
        all(target_os = "macos", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64")
    )))]
    let derived = "";
    if derived.is_empty() {
        bail!(
            "no prebuilt binary for this platform ({}/{}) — chug publishes macos-arm64, \
             linux-x86_64 and linux-aarch64; build from source: pass --source <checkout> \
             (or set CHUG_SOURCE) with a cargo toolchain on PATH",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
    }
    Ok(derived.to_string())
}

/// The platform allowlist — the three published tags, nothing else.
fn validate_platform(tag: &str) -> anyhow::Result<String> {
    match tag {
        "macos-arm64" | "linux-x86_64" | "linux-aarch64" => Ok(tag.to_string()),
        other => bail!(
            "no prebuilt binary for platform '{other}' — supported: macos-arm64, \
             linux-x86_64, linux-aarch64"
        ),
    }
}

// ---------- release resolution (GitHub API, install.sh's contract) ----------

/// One resolved latest-release record: the raw tag, its parsed version,
/// and the `published_at` DATE part (`YYYY-MM-DD`) when the metadata
/// carried one (the T348 staleness input).
pub struct LatestRelease {
    pub tag: String,
    pub version: SemVer,
    pub published_date: Option<String>,
}

/// ONE bounded GET returning the raw body — the same posture as
/// webfetch's transport (fixed chug UA, connect/total timeouts, one
/// attempt, non-2xx named with the status) but for BYTES: the tarball is
/// binary, so a lossy `String` read would corrupt it.
fn http_get(url: &str, total_timeout: Duration, max_bytes: u64) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("chug/{VERSION}"))
        .connect_timeout(Duration::from_secs(10))
        .timeout(total_timeout)
        .build()
        .context("building http client")?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("fetching {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("HTTP {status}: non-2xx response from GET {url}");
    }
    if let Some(len) = resp.content_length().filter(|&len| len > max_bytes) {
        bail!("GET {url} returned {len} bytes (cap {max_bytes}) — not a release artifact");
    }
    let mut bytes = Vec::new();
    resp.take(max_bytes)
        .read_to_end(&mut bytes)
        .with_context(|| format!("reading {url}"))?;
    Ok(bytes)
}

/// Resolve the latest release: `GET <api_base>/repos/<repo>/releases/latest`,
/// the same endpoint install.sh's staleness gate probes. `tag_name` is the
/// version source; `published_at`'s date part is the staleness input.
/// A missing `tag_name` is an error (there is nothing honest to compare);
/// a missing `published_at` degrades to an unknown staleness (never a
/// hard failure — the version comparison still works).
pub fn fetch_latest_release() -> anyhow::Result<LatestRelease> {
    let url = format!("{}/repos/{}/releases/latest", api_base(), release_repo());
    let body = http_get(&url, METADATA_TIMEOUT, MAX_SMALL_BODY_BYTES)?;
    let value: Value = serde_json::from_slice(&body).with_context(|| format!("parsing {url}"))?;
    let tag = value
        .get("tag_name")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("release metadata carried no tag_name ({url})"))?
        .trim()
        .to_string();
    let version = parse_version(&tag)
        .ok_or_else(|| anyhow!("release tag {tag:?} is not a parseable X.Y.Z version"))?;
    let published_date = value
        .get("published_at")
        .and_then(Value::as_str)
        .and_then(published_date_part);
    Ok(LatestRelease {
        tag,
        version,
        published_date,
    })
}

/// The `YYYY-MM-DD` prefix of an ISO `published_at` timestamp, when it
/// parses as one (GitHub's form is `2026-11-09T14:30:00Z`).
fn published_date_part(published_at: &str) -> Option<String> {
    let date = published_at.trim().get(..10)?;
    if date.len() != 10 {
        return None;
    }
    let mut it = date.split('-');
    let (y, m, d) = (it.next()?, it.next()?, it.next()?);
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let valid = y.len() == 4 && m.len() == 2 && d.len() == 2 && numeric(y) && numeric(m) && numeric(d);
    valid.then(|| date.to_string())
}

/// Howard Hinnant's `days_from_civil` (the same integer-only algorithm
/// install.sh shells) — days since 1970-01-01 for a civil date, exact for
/// every year >= 0.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let m = if m <= 2 { m + 12 } else { m };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Days today (UTC) since 1970-01-01.
fn today_days() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64 / 86_400)
        .unwrap_or(0)
}

/// The age of a `YYYY-MM-DD` release date vs `today` (days since epoch),
/// in whole days: `Some(age)` when the date parsed, `None` when unknown
/// (an unparseable or absent date is never treated as stale — install.sh's
/// degrade leg).
fn age_days_from(today: i64, date: Option<&str>) -> Option<i64> {
    let date = date?;
    let mut it = date.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    Some(today - days_from_civil(y, m, d))
}

/// Age of the release's `published_at` date vs today (UTC), in whole days.
pub fn release_age_days(release: &LatestRelease) -> Option<i64> {
    age_days_from(today_days(), release.published_date.as_deref())
}

/// The staleness verdict behind the source-build fallback: `Some(true)` =
/// the release is older than [`STALE_DAYS`], `Some(false)` = fresh enough,
/// `None` = unknown or skipped (a mirror run without an API base, or a
/// metadata body with no usable date).
pub fn staleness(release: &LatestRelease) -> Option<bool> {
    staleness_from(today_days(), staleness_skipped(), release)
}

/// The pure core of [`staleness`]: the verdict for a given today/today-days
/// and skip flag — every input the wall clock and the environment would
/// otherwise bake in, made explicit so the test pins are deterministic.
fn staleness_from(today: i64, skipped: bool, release: &LatestRelease) -> Option<bool> {
    if skipped {
        return None;
    }
    age_days_from(today, release.published_date.as_deref()).map(|age| age > STALE_DAYS as i64)
}

// ---------- download + verify (install.sh's sidecar contract) ----------

/// The sha256 hex digest of `bytes`, lowercase hex (comparable with the
/// sidecar and `shasum -a 256` out-of-band).
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Parse the sidecar: the FIRST whitespace-separated field, which must be
/// 64 hex chars — install.sh's `awk '{print $1}'` + 64-hex gate, byte for
/// byte. `None` = malformed (the caller refuses the install).
pub fn parse_sha_sidecar(raw: &str) -> Option<String> {
    let field = raw.split_whitespace().next()?;
    let all_hex = field.chars().all(|c| c.is_ascii_hexdigit());
    (all_hex && field.len() == 64).then(|| field.to_ascii_lowercase())
}

/// A scratch directory that removes itself on drop — the update's
/// download/extract workspace (mktemp + trap in install.sh's shape). Named
/// with the pid + a nanosecond suffix so concurrent updates never collide.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> anyhow::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir =
            std::env::temp_dir().join(format!("chug-update-{label}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir)
            .with_context(|| format!("creating scratch dir {}", dir.display()))?;
        Ok(Self(dir))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        // Best-effort cleanup: a leftover scratch dir is garbage in /tmp,
        // never a failure (the trap-on-EXIT shape).
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The downloaded-and-verified release archive, extracted. Holds the
/// scratch dir so the extracted binary stays alive until the caller has
/// swapped its bytes into place; dropping it cleans up (the trap shape).
struct Extracted {
    _scratch: ScratchDir,
    bin: PathBuf,
}

impl Extracted {
    fn bin(&self) -> &Path {
        &self.bin
    }
}

/// Download the platform tarball + its `.sha256` sidecar from `base`,
/// VERIFY the checksum (before anything runs, install.sh's ordering), and
/// extract with the system `tar` (the same tool install.sh invokes — no
/// new tar/gzip dependency for a format the platform guarantees). Any
/// failure leaves nothing behind but the self-cleaning scratch dir.
fn download_and_extract(base: &str, platform_tag: &str) -> anyhow::Result<Extracted> {
    let tarball_url = format!("{base}/chug-{platform_tag}.tar.gz");
    let sha_url = format!("{tarball_url}.sha256");
    let tarball = http_get(&tarball_url, TARBALL_TIMEOUT, MAX_TARBALL_BYTES)
        .with_context(|| format!("downloading {tarball_url}"))?;
    let sidecar_raw = http_get(&sha_url, METADATA_TIMEOUT, MAX_SMALL_BODY_BYTES).with_context(
        || {
            format!(
                "downloading {sha_url} — the tarball exists but its .sha256 sidecar is \
                 missing; re-check the release assets"
            )
        },
    )?;
    let sidecar = String::from_utf8_lossy(&sidecar_raw);
    let want = parse_sha_sidecar(&sidecar).ok_or_else(|| {
        anyhow!(
            "checksum file did not contain a sha256 (got {:?}) — the .sha256 sidecar is \
             malformed",
            sidecar.get(..80).unwrap_or(&sidecar)
        )
    })?;
    let got = sha256_hex(&tarball);
    if want != got {
        bail!(
            "CHECKSUM MISMATCH for {tarball_url} (want {want}, got {got}) — do not install \
             a tampered or corrupt download; re-run later or build from source with \
             --source <checkout>"
        );
    }
    let scratch = ScratchDir::new("download")?;
    let tar_path = scratch.path().join("chug.tar.gz");
    fs::write(&tar_path, &tarball).with_context(|| format!("writing {}", tar_path.display()))?;
    let dest = scratch.path();
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(&tar_path)
        .arg("-C")
        .arg(dest)
        .status()
        .context("spawning tar for the release archive")?;
    if !status.success() {
        bail!(
            "tar extraction failed for the downloaded archive (exit {status}) — the tarball \
             may be corrupt despite the matching checksum"
        );
    }
    let bin = dest.join("chug");
    if !bin.is_file() {
        bail!(
            "archive did not contain a 'chug' binary — the release asset layout changed; \
             inspect {tarball_url}"
        );
    }
    Ok(Extracted {
        _scratch: scratch,
        bin,
    })
}

// ---------- the swap (temp file + rename, never an in-place overwrite) ----------

/// The binary this update replaces: `CHUG_UPDATE_TARGET` when set (the
/// test/operator seam — the same convention as CHUG_LOOPD_BIN — and the
/// loopd integration's way of naming the installed binary it updates),
/// else the running executable itself.
pub fn update_target() -> anyhow::Result<PathBuf> {
    update_target_from(nonempty_env("CHUG_UPDATE_TARGET").as_deref())
}

/// The pure core of [`update_target`].
fn update_target_from(target_env: Option<&str>) -> anyhow::Result<PathBuf> {
    match target_env {
        Some(p) => Ok(PathBuf::from(p)),
        None => std::env::current_exe().context("resolving the running binary (current_exe)"),
    }
}

/// Replace `target` with the bytes of `new_bin` ATOMICALLY: the bytes are
/// written to a pid-suffixed `.tmp` sibling IN THE TARGET'S DIRECTORY
/// (rename stays on one filesystem), fsync'd, made executable, then
/// `fs::rename`d over the target — the POSIX rename replaces the directory
/// entry atomically, so the target only ever holds whole old or whole new
/// bytes, and a RUNNING binary (whose inode the old entry still maps)
/// keeps executing its image untouched. This is fsatomic::write_atomic's
/// shape with one step inserted before the rename: the exec bit travels
/// WITH the new inode, so there is no window where the installed path is
/// non-executable. Any failure removes the temp file and leaves the target
/// byte-untouched.
fn install_swapped(new_bin: &Path, target: &Path) -> anyhow::Result<()> {
    let bytes =
        fs::read(new_bin).with_context(|| format!("reading the new binary {}", new_bin.display()))?;
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow!("unusable update target {}", target.display()))?;
    let tmp = target.with_file_name(format!("{name}.{}.update-tmp", std::process::id()));
    let write = || -> anyhow::Result<()> {
        let mut file =
            File::create(&tmp).with_context(|| format!("opening temp file {}", tmp.display()))?;
        file.write_all(&bytes)
            .with_context(|| format!("writing {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("syncing {}", tmp.display()))?;
        drop(file);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))
                .with_context(|| format!("chmod 755 {}", tmp.display()))?;
        }
        fs::rename(&tmp, target).with_context(|| {
            format!(
                "renaming {} over {} — the atomic swap failed, the installed binary is \
                 untouched",
                tmp.display(),
                target.display()
            )
        })
    };
    match write() {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

// ---------- the binary probes ----------

/// The daemon feature probe (loopd's `judge_probe_ok` shape): does THIS
/// binary host the judge daemon (`daemon --help` exits 0)? The feature-off
/// build refuses the subcommand with the fail-open stub, so the probe is
/// the honest answer about the binary being replaced, not a compile-time
/// guess about a different one.
pub fn daemon_feature_live(bin: &Path) -> bool {
    Command::new(bin)
        .args(["daemon", "--help"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// A binary's self-reported version (`chug --version` → `chug X.Y.Z`):
/// the LAST whitespace token parsed as X.Y.Z. Errors name the binary —
/// the floor comparison must never compare against garbage.
fn binary_version(bin: &Path) -> anyhow::Result<SemVer> {
    let out = Command::new(bin)
        .arg("--version")
        .output()
        .with_context(|| format!("running {} --version", bin.display()))?;
    if !out.status.success() {
        bail!(
            "{} --version exited {} — the binary does not report a version",
            bin.display(),
            out.status
        );
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let token = text.split_whitespace().last().unwrap_or_default();
    parse_version(token)
        .ok_or_else(|| anyhow!("{:?} reported an unparseable version {token:?}", bin.display()))
}

/// A binary's capabilities `contract_version` (best-effort): the printed
/// document's `contract_version` string, `None` on any failure — the
/// `old -> new` line degrades to the plain version pair, never a failure.
fn contract_version_of(bin: &Path) -> Option<String> {
    let out = Command::new(bin).arg("capabilities").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let value: Value = serde_json::from_slice(&out.stdout).ok()?;
    value
        .get("contract_version")
        .and_then(Value::as_str)
        .map(str::to_string)
}

// ---------- the source-build fallback (the T347-era path) ----------

/// The source checkout: `--source <path>`, else `$CHUG_SOURCE`.
fn source_checkout(explicit: &Option<PathBuf>) -> Option<PathBuf> {
    explicit
        .clone()
        .or_else(|| nonempty_env("CHUG_SOURCE").map(PathBuf::from))
}

/// Is `dir` a Rust checkout this module can build (a Cargo.toml at its
/// root)? The cheap structural probe; cargo itself is probed separately.
fn is_rust_checkout(dir: &Path) -> bool {
    dir.join("Cargo.toml").is_file()
}

/// Is a cargo toolchain on PATH?
fn cargo_available() -> bool {
    Command::new("cargo")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Validate a NAMED source checkout (an explicit `--source` or
/// `$CHUG_SOURCE`): an unusable one is an ERROR, never a silent
/// fall-through to the release download — the flag was given to solve
/// exactly the problem hiding behind it.
fn validate_named_source(dir: &Path) -> anyhow::Result<()> {
    if !dir.is_dir() {
        bail!(
            "source checkout {} does not exist — point --source (or $CHUG_SOURCE) at the \
             chug repo checkout",
            dir.display()
        );
    }
    if !is_rust_checkout(dir) {
        bail!(
            "{} is not a Rust checkout (no Cargo.toml at its root) — point --source (or \
             $CHUG_SOURCE) at the chug repo checkout",
            dir.display()
        );
    }
    if !cargo_available() {
        bail!(
            "no cargo toolchain on PATH — install rustup/cargo to use the source-build \
             fallback named by --source {}",
            dir.display()
        );
    }
    Ok(())
}

/// Build the checkout: `cargo build --release` (+ `--features daemon` iff
/// `want_daemon` — the replaced binary's own live surface, probed, not
/// assumed). The binary lands at `$CARGO_TARGET_DIR/release/chug` when the
/// caller's environment names a shared cache, else
/// `<checkout>/target/release/chug`. Build output is CAPTURED (a source
/// build inside `chug update` must not scribble on the caller's terminal);
/// a failing build errors with the captured stderr tail.
fn source_build(checkout: &Path, want_daemon: bool) -> anyhow::Result<PathBuf> {
    if !checkout.is_dir() {
        bail!("source checkout {} is not a directory", checkout.display());
    }
    if !is_rust_checkout(checkout) {
        bail!(
            "{} is not a Rust checkout (no Cargo.toml at its root) — point --source (or \
             $CHUG_SOURCE) at the chug repo checkout",
            checkout.display()
        );
    }
    if !cargo_available() {
        bail!(
            "no cargo toolchain on PATH — install rustup/cargo to use the source-build \
             fallback (or update from the release: re-run without --source)"
        );
    }
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("--release").current_dir(checkout);
    if want_daemon {
        cmd.arg("--features").arg("daemon");
    }
    let out = cmd
        .output()
        .context("spawning cargo build --release for the source fallback")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: String = stderr.lines().rev().take(10).collect::<Vec<_>>().join("\n");
        bail!(
            "cargo build --release failed in {} ({}) — the checkout did not build:\n{tail}",
            checkout.display(),
            out.status
        );
    }
    let target_dir = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| checkout.join("target"));
    let bin = target_dir.join("release").join("chug");
    if !bin.is_file() {
        bail!(
            "cargo build --release succeeded but {} does not exist — CARGO_TARGET_DIR ({}) \
             does not name where the binary landed",
            bin.display(),
            target_dir.display()
        );
    }
    Ok(bin)
}

// ---------- the command ----------

/// `chug update` (T352). `check` is the dry-run (`--check`): it resolves
/// and reports, never writes — exit 0 current / 10 update-available /
/// 2 error. `force` licenses an explicit downgrade. `source` is the
/// source-build fallback checkout (`--source`, else `$CHUG_SOURCE`).
///
/// The floor rule: a release OLDER than the running binary is REFUSED (no
/// silent downgrade) without `--force`; the same rule applies to a source
/// build's result. The staleness rule: when the latest release is older
/// than [`STALE_DAYS`] AND a usable checkout + cargo exist, the update is
/// built from source instead of downloaded (the T347-era path).
pub fn run(check: bool, force: bool, source: &Option<PathBuf>) -> anyhow::Result<i32> {
    // The check's error leg is part of its machine contract: a failed
    // check is exit 2 + a stderr note, never a propagated anyhow error
    // (that would exit 1 and read as an apply-mode failure to loopd).
    match run_inner(check, force, source) {
        Ok(code) => Ok(code),
        Err(e) if check => {
            eprintln!("chug: update --check failed: {e:#}");
            Ok(EXIT_CHECK_ERROR)
        }
        Err(e) => Err(e),
    }
}

fn run_inner(check: bool, force: bool, source: &Option<PathBuf>) -> anyhow::Result<i32> {
    let current = current_version()?;
    let release = fetch_latest_release()?;
    let stale = staleness(&release);

    // The source fallback is a first-class path only when the operator
    // named a checkout — and an EXPLICITLY named checkout that cannot
    // build is an error (a silent fall-through to the stale release would
    // hide exactly the problem the flag was given to solve).
    let named_source = source_checkout(source);
    let source_usable = match &named_source {
        Some(dir) => {
            validate_named_source(dir)?;
            true
        }
        None => false,
    };

    // The source path wins whenever the release is STALE and a usable
    // checkout was named: a stale release is the T347 smell (the pipeline
    // may be frozen while the repo runs ahead), and the checkout is the
    // operator's own answer to that. A mirror-skipped or unknown
    // staleness never takes the source path on its own.
    let take_source = stale == Some(true) && source_usable;
    if take_source {
        let checkout = named_source.expect("source_usable implies a named checkout");
        return source_leg(check, force, current, &release, &checkout);
    }

    match cmp_versions(&release.version, &current) {
        Ordering::Greater => {
            if check {
                println!("update available: {} -> {}", VERSION, release.tag);
                return Ok(EXIT_UPDATE_AVAILABLE);
            }
            apply_download(&release, force)
        }
        Ordering::Equal => {
            // Fresh release at the running version: current. A stale one
            // with no checkout named says so — the pipeline may be stalled
            // and the remedy is the source fallback, named (the
            // remedy-naming doctrine).
            println!(
                "already current: {} (latest release {}){}",
                VERSION,
                release.tag,
                stale_note(&release, stale)
            );
            Ok(0)
        }
        Ordering::Less => {
            if force {
                if check {
                    println!(
                        "downgrade available (forced): {} -> {}",
                        VERSION, release.tag
                    );
                    return Ok(EXIT_UPDATE_AVAILABLE);
                }
                return apply_download(&release, force);
            }
            let msg = format!(
                "refusing to downgrade: installed {} > latest release {} (no silent \
                 downgrades — re-run with --force to install it anyway)",
                VERSION, release.tag
            );
            if check {
                println!("{msg}");
                Ok(0)
            } else {
                bail!("{msg}")
            }
        }
    }
}

/// The staleness note for the already-current / refused legs: the one
/// line that names the T347 smell and its remedy. A mirror run
/// (staleness skipped) and unknown staleness say nothing — install.sh's
/// degrade posture, not invented certainty.
fn stale_note(release: &LatestRelease, stale: Option<bool>) -> String {
    match stale {
        Some(true) => {
            let age = release_age_days(release).unwrap_or(0);
            format!(
                " — NOTE: the release is {age} days old (window: {STALE_DAYS}); the \
                 pipeline may be stalled (T347) — to update from a checkout instead, \
                 pass --source <path> or set CHUG_SOURCE"
            )
        }
        _ => String::new(),
    }
}

/// The source-build leg: build the checkout, floor-check the RESULT (the
/// checkout's own version is the truth here, not the release tag), swap,
/// report. `--check` stops after the usability facts and reports exit 10.
fn source_leg(
    check: bool,
    force: bool,
    current: SemVer,
    release: &LatestRelease,
    checkout: &Path,
) -> anyhow::Result<i32> {
    if check {
        println!(
            "update available (source build from {}): {} -> a build of that checkout \
             (the latest release {} is stale; the T347 rule prefers the source path)",
            checkout.display(),
            VERSION,
            release.tag
        );
        return Ok(EXIT_UPDATE_AVAILABLE);
    }
    // The daemon feature of the binary BEING REPLACED is the one that
    // matters: rebuild with the same surface (probe, never assume).
    let target = update_target()?;
    let want_daemon = daemon_feature_live(&target);
    let built = source_build(checkout, want_daemon)?;
    let built_version = binary_version(&built)?;
    if cmp_versions(&built_version, &current) == Ordering::Less && !force {
        bail!(
            "refusing to downgrade: the source build reports {built_version:?} but the \
             installed binary is {} (no silent downgrades — re-run with --force to \
             install it anyway)",
            VERSION
        );
    }
    let old_contract = contract_version_of(&target);
    install_swapped(&built, &target)?;
    println!(
        "updated: {} -> {built_version:?} (source build from {}){}",
        VERSION,
        checkout.display(),
        contract_note(old_contract, &target)
    );
    Ok(0)
}

/// The download leg: fetch + verify + extract + probe + swap + report.
/// The floor was already checked against the release tag; `force` exists
/// here only for the forced-downgrade leg, the one caller that arrives
/// with a release older than the running binary.
fn apply_download(release: &LatestRelease, force: bool) -> anyhow::Result<i32> {
    let platform_tag = platform()?;
    let target = update_target()?;
    let extracted = download_and_extract(&asset_base(), &platform_tag)?;
    let old_contract = contract_version_of(&target);
    install_swapped(extracted.bin(), &target)?;
    drop(extracted);
    let label = if force {
        "updated (forced downgrade)"
    } else {
        "updated"
    };
    println!(
        "{label}: {} -> {}{}",
        VERSION,
        release.tag,
        contract_note(old_contract, &target)
    );
    Ok(0)
}

/// The contract_version clause of the `updated:` line: ` (contract_version
/// a -> b)` when the NEW binary reports a parseable document and it
/// differs from the replaced binary's; ` (contract_version b)` when the
/// replaced binary's own version was unreadable (harnesses plan around
/// the value that WILL be live); nothing when the probe fails entirely
/// (best-effort — the version pair is the contract of the line).
fn contract_note(old: Option<String>, new_bin: &Path) -> String {
    let Some(new) = contract_version_of(new_bin) else {
        return String::new();
    };
    match old {
        Some(o) if o != new => format!(" (contract_version {o} -> {new})"),
        Some(_) => String::new(),
        None => format!(" (contract_version {new})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------- parse_version ----------

    #[test]
    fn parse_version_legs() {
        assert_eq!(parse_version("0.22.0"), Some((0, 22, 0)));
        assert_eq!(parse_version("v0.9.0"), Some((0, 9, 0)));
        assert_eq!(parse_version("V1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version(" 1.2.3 "), Some((1, 2, 3)));
        assert_eq!(parse_version("0.22"), None, "two components");
        assert_eq!(parse_version("0.22.0.1"), None, "four components");
        assert_eq!(parse_version("vX.Y.Z"), None, "non-numeric");
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("v"), None);
    }

    #[test]
    fn cmp_versions_orders() {
        let a = (0, 22, 0);
        assert_eq!(cmp_versions(&a, &a), Ordering::Equal);
        assert_eq!(cmp_versions(&a, &(0, 9, 0)), Ordering::Greater);
        assert_eq!(cmp_versions(&a, &(1, 0, 0)), Ordering::Less);
        assert_eq!(cmp_versions(&a, &(0, 23, 0)), Ordering::Less);
        assert_eq!(cmp_versions(&a, &(0, 22, 1)), Ordering::Less);
    }

    #[test]
    fn current_version_is_the_package_version() {
        assert_eq!(current_version().unwrap(), parse_version(VERSION).unwrap());
    }

    #[test]
    fn the_check_cadence_is_one_day() {
        // The cheap-exit-era cadence the loopd integration holds its latch
        // against: one check per day, no more (scripts/loopd_auto_update.sh
        // carries the same number — tests/update_cmd.rs pins both sides).
        assert_eq!(CHECK_INTERVAL_SECS, 24 * 60 * 60);
    }

    // ---------- the release contract: install.sh's URLs ----------

    #[test]
    fn default_urls_match_install_sh() {
        // With every override unset, the URL construction is byte-identical
        // to install.sh's base/api lines (the no-fork contract).
        assert_eq!(
            asset_base_from(None, RELEASE_REPO),
            format!("https://github.com/{RELEASE_REPO}/releases/latest/download")
        );
        assert_eq!(api_base_from(None), "https://api.github.com");
        assert!(!staleness_skipped_from(None, None));
    }

    #[test]
    fn mirror_overrides_rebase_both_urls() {
        // The fixture seam: a mirror pins the artifact base and (optionally)
        // the metadata base. URL_BASE alone also SKIPS the staleness gate —
        // install.sh's rule (the mirror controls its own freshness).
        assert_eq!(
            asset_base_from(Some("http://mirror.test/assets"), "other/repo"),
            "http://mirror.test/assets",
            "the mirror base replaces the GitHub pointer wholesale"
        );
        assert_eq!(api_base_from(Some("http://mirror.test/api")), "http://mirror.test/api");
        assert!(staleness_skipped_from(Some("http://mirror.test/assets"), None));
        assert!(!staleness_skipped_from(Some("http://m.test/a"), Some("http://m.test/api")));
        assert!(!staleness_skipped_from(None, None));
    }

    #[test]
    fn platform_override_and_validation() {
        assert_eq!(platform_from(Some("linux-x86_64")).unwrap(), "linux-x86_64");
        assert!(platform_from(Some("windows-x64")).is_err());
        // The compile-time derivation is one of the three published tags.
        let derived = platform_from(None).unwrap();
        assert!(
            ["macos-arm64", "linux-x86_64", "linux-aarch64"].contains(&derived.as_str()),
            "derived platform {derived} must be a published tag"
        );
    }

    // ---------- staleness ----------

    #[test]
    fn published_date_part_parses_iso_shapes() {
        assert_eq!(
            published_date_part("2026-11-09T14:30:00Z"),
            Some("2026-11-09".to_string())
        );
        assert_eq!(
            published_date_part("2026-11-09"),
            Some("2026-11-09".to_string())
        );
        assert_eq!(published_date_part("garbage"), None);
        assert_eq!(published_date_part("2026-1-9T00:00:00Z"), None, "unpadded");
        assert_eq!(published_date_part(""), None);
    }

    #[test]
    fn days_from_civil_matches_known_anchors() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1970, 1, 2), 1);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        assert_eq!(days_from_civil(2026, 11, 9), 20_766);
        // Leap-year day: 2024-03-01 follows 2024-02-29.
        assert_eq!(days_from_civil(2024, 2, 29) + 1, days_from_civil(2024, 3, 1));
    }

    #[test]
    fn staleness_reads_the_seven_day_window() {
        let today = days_from_civil(2026, 11, 9);
        let mk = |date: &str| LatestRelease {
            tag: "v0.22.0".into(),
            version: (0, 22, 0),
            published_date: Some(date.to_string()),
        };
        assert_eq!(staleness_from(today, false, &mk("2026-11-09")), Some(false));
        assert_eq!(
            staleness_from(today, false, &mk("2026-11-03")),
            Some(false),
            "6 days: inside the window"
        );
        assert_eq!(
            staleness_from(today, false, &mk("2026-11-02")),
            Some(false),
            "exactly 7 days: the window edge is not stale"
        );
        assert_eq!(
            staleness_from(today, false, &mk("2026-11-01")),
            Some(true),
            "8 days: past the window"
        );
        // Unknown staleness: no date at all.
        let no_date = LatestRelease {
            tag: "v0.22.0".into(),
            version: (0, 22, 0),
            published_date: None,
        };
        assert_eq!(staleness_from(today, false, &no_date), None, "no date is never stale");
        // Skipped: the mirror rule (URL_BASE alone).
        assert_eq!(staleness_from(today, true, &mk("2020-01-01")), None, "mirror skip");
    }

    #[test]
    fn age_days_legs() {
        let today = days_from_civil(2026, 11, 9);
        assert_eq!(age_days_from(today, Some("2026-11-09")), Some(0));
        assert_eq!(age_days_from(today, Some("2026-11-08")), Some(1));
        assert_eq!(age_days_from(today, Some("2026-11-10")), Some(-1), "future date");
        assert_eq!(age_days_from(today, None), None);
        assert_eq!(age_days_from(today, Some("nope")), None);
    }

    // ---------- the sidecar contract ----------

    #[test]
    fn sidecar_parses_the_first_field_only() {
        let hex = "a".repeat(64);
        assert_eq!(parse_sha_sidecar(&hex), Some(hex.clone()));
        assert_eq!(
            parse_sha_sidecar(&format!("{hex}  chug-macos-arm64.tar.gz\n")),
            Some(hex.clone()),
            "the filename field is ignored"
        );
        assert_eq!(
            parse_sha_sidecar(&format!("{}\n", "A".repeat(64))),
            Some(hex),
            "uppercase normalizes to lowercase"
        );
        assert_eq!(parse_sha_sidecar(""), None);
        assert_eq!(parse_sha_sidecar("nothex"), None);
        assert_eq!(parse_sha_sidecar(&"g".repeat(64)), None, "g is not hex");
        assert_eq!(parse_sha_sidecar(&"a".repeat(65)), None, "65 chars");
        assert_eq!(parse_sha_sidecar("abc123"), None, "63 chars");
    }

    #[test]
    fn sha256_hex_matches_the_known_vectors() {
        // The empty-string and "abc" FIPS vectors — the implementation is
        // pinned against the standard, not against itself.
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    // ---------- the swap ----------

    #[test]
    fn install_swapped_replaces_by_rename_and_cleans_up() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("chug");
        fs::write(&target, b"old binary bytes").unwrap();
        let new_bin = tmp.path().join("new-chug");
        fs::write(&new_bin, b"new binary bytes").unwrap();
        install_swapped(&new_bin, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new binary bytes");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o755,
                "the exec bit travels with the new inode"
            );
        }
        // No temp sibling remains.
        let strays: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("update-tmp"))
            .collect();
        assert!(strays.is_empty(), "no temp leftover: {strays:?}");
    }

    #[test]
    fn install_swapped_failure_leaves_the_target_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("chug");
        fs::write(&target, b"old binary bytes").unwrap();
        // A missing source binary fails the read, before anything touches
        // the target.
        let err = install_swapped(&tmp.path().join("missing"), &target).unwrap_err();
        assert!(err.to_string().contains("reading"), "{err}");
        assert_eq!(fs::read(&target).unwrap(), b"old binary bytes");
        let strays: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains("update-tmp"))
            .collect();
        assert!(strays.is_empty(), "no temp leftover: {strays:?}");
    }

    /// The atomicity proof the spec asks for: an OPEN handle on the target
    /// keeps reading the OLD bytes after the swap — rename semantics, the
    /// exact property that makes replacing a running Mach-O safe. An
    /// in-place truncate/overwrite (the cp-overwrite hazard) would change
    /// what the open handle reads.
    #[cfg(unix)]
    #[test]
    fn install_swapped_is_rename_not_truncate() {
        use std::io::Read as _;
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("chug");
        fs::write(&target, b"old binary bytes").unwrap();
        let mut held = File::open(&target).unwrap();
        let new_bin = tmp.path().join("new-chug");
        fs::write(&new_bin, b"new binary bytes").unwrap();
        install_swapped(&new_bin, &target).unwrap();
        let mut still = String::new();
        held.read_to_string(&mut still).unwrap();
        assert_eq!(still, "old binary bytes", "the open inode kept the old image");
        assert_eq!(fs::read(&target).unwrap(), b"new binary bytes");
    }

    // ---------- the probes ----------

    /// Write an executable shell stub (unix only — the suite's release
    /// platforms; the fixtures' bin/ shape).
    #[cfg(unix)]
    fn write_stub(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::write(path, body).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn daemon_probe_reads_the_live_subcommand() {
        let tmp = tempfile::tempdir().unwrap();
        // "Live": exit 0 for `daemon --help`.
        let live = tmp.path().join("live");
        write_stub(&live, "#!/bin/sh\n[ \"$1\" = daemon ] && exit 0\nexit 1\n");
        assert!(daemon_feature_live(&live));
        // The feature-off stub shape: nonzero for the subcommand.
        let off = tmp.path().join("off");
        write_stub(&off, "#!/bin/sh\nexit 1\n");
        assert!(!daemon_feature_live(&off));
        // A missing file: spawn failure → false, never a panic.
        assert!(!daemon_feature_live(&tmp.path().join("absent")));
    }

    #[cfg(unix)]
    #[test]
    fn binary_version_reads_the_last_token() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        write_stub(&bin, "#!/bin/sh\necho 'chug 9.9.9'\n");
        assert_eq!(binary_version(&bin).unwrap(), (9, 9, 9));
        // An unparseable report is an error, never a garbage comparison.
        let bad = tmp.path().join("bad");
        write_stub(&bad, "#!/bin/sh\necho 'chug unknown'\n");
        assert!(binary_version(&bad).is_err());
        // A failing --version is an error naming the binary.
        let fail = tmp.path().join("fail");
        write_stub(&fail, "#!/bin/sh\nexit 3\n");
        let err = binary_version(&fail).unwrap_err();
        assert!(err.to_string().contains("does not report a version"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn contract_version_probe_degrades_to_none() {
        let tmp = tempfile::tempdir().unwrap();
        // A working capabilities printer.
        let good = tmp.path().join("good");
        write_stub(
            &good,
            "#!/bin/sh\necho '{\"contract_version\":\"2\",\"features\":{}}'\n",
        );
        assert_eq!(contract_version_of(&good), Some("2".to_string()));
        // A failing printer: None, never an error.
        let bad = tmp.path().join("bad");
        write_stub(&bad, "#!/bin/sh\nexit 1\n");
        assert_eq!(contract_version_of(&bad), None);
        // A printer of non-JSON: None.
        let junk = tmp.path().join("junk");
        write_stub(&junk, "#!/bin/sh\necho 'not json'\n");
        assert_eq!(contract_version_of(&junk), None);
        // Absent binary: None.
        assert_eq!(contract_version_of(&tmp.path().join("absent")), None);
    }

    #[cfg(unix)]
    #[test]
    fn contract_note_renders_the_change_clauses() {
        let tmp = tempfile::tempdir().unwrap();
        let v1 = tmp.path().join("v1");
        write_stub(&v1, "#!/bin/sh\necho '{\"contract_version\":\"1\"}'\n");
        let v2 = tmp.path().join("v2");
        write_stub(&v2, "#!/bin/sh\necho '{\"contract_version\":\"2\"}'\n");
        let mute = tmp.path().join("mute");
        write_stub(&mute, "#!/bin/sh\nexit 0\n");
        assert_eq!(contract_note(Some("1".into()), &v2), " (contract_version 1 -> 2)");
        assert_eq!(contract_note(Some("1".into()), &v1), "", "same version: no clause");
        assert_eq!(
            contract_note(None, &v2),
            " (contract_version 2)",
            "an unreadable old document names the new one"
        );
        assert_eq!(contract_note(Some("1".into()), &mute), "", "a failed probe stays silent");
    }

    // ---------- the source fallback ----------

    #[test]
    fn source_checkout_prefers_the_explicit_flag() {
        let explicit = Some(PathBuf::from("/repo/a"));
        assert_eq!(
            source_checkout(&explicit),
            Some(PathBuf::from("/repo/a")),
            "--source wins over the env"
        );
        assert_eq!(source_checkout(&None), None);
    }

    #[test]
    fn is_rust_checkout_needs_a_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_rust_checkout(tmp.path()));
        fs::write(tmp.path().join("Cargo.toml"), "[package]\n").unwrap();
        assert!(is_rust_checkout(tmp.path()));
    }

    #[test]
    fn source_build_refuses_a_non_checkout_and_a_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let err = source_build(tmp.path(), false).unwrap_err();
        assert!(err.to_string().contains("not a Rust checkout"), "{err}");
        let err = source_build(Path::new("/definitely/not/here"), false).unwrap_err();
        assert!(err.to_string().contains("not a directory"), "{err}");
    }

    #[test]
    fn update_target_prefers_the_seam() {
        // The test/operator seam: CHUG_UPDATE_TARGET names the file to
        // replace instead of current_exe (the fixtures' whole mechanism,
        // the CHUG_LOOPD_BIN convention).
        assert_eq!(
            update_target_from(Some("/tmp/fake-install/chug")).unwrap(),
            PathBuf::from("/tmp/fake-install/chug")
        );
        assert!(update_target_from(None).is_ok(), "the default resolves current_exe");
    }
}

//! Run-start housekeeping: rotate a previous session's state file into a
//! timestamped `.chug/` archive so a fresh run never inherits foreign state
//! (T3 ledger, T7 transcript). Archive — never delete — and never abort a
//! run over housekeeping: failures surface as [`Outcome::Failed`] for the
//! caller to warn about.

use std::fs;
use std::path::{Path, PathBuf};

/// Outcome of a best-effort archive attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing worth archiving (source absent, or content not worth keeping).
    Skipped,
    /// Source rotated to this archive path.
    Archived(PathBuf),
    /// Rotation failed (the run continues with the file as-is). Payload is a
    /// human-readable reason for the stderr warning.
    Failed(String),
}

/// Rename `src` to `<archive_dir>/<stem>-<YYYYMMDD-HHMMSS><ext>` (UTC),
/// creating `archive_dir` if needed. Same-second collisions get `-2`, `-3`,
/// … suffixes. Best-effort: errors come back as [`Outcome::Failed`].
pub fn rotate(src: &Path, archive_dir: &Path, stem: &str, ext: &str) -> Outcome {
    if !src.exists() {
        return Outcome::Skipped;
    }
    if let Err(e) = fs::create_dir_all(archive_dir) {
        return Outcome::Failed(format!("creating {}: {e}", archive_dir.display()));
    }
    let ts = timestamp_now();
    for n in 1..=10u32 {
        let suffix = if n == 1 { String::new() } else { format!("-{n}") };
        let dst = archive_dir.join(format!("{stem}-{ts}{suffix}{ext}"));
        // Check-then-rename because Unix rename(2) silently replaces an
        // existing destination; same-second runs must not clobber archives.
        if dst.exists() {
            continue;
        }
        return match fs::rename(src, &dst) {
            Ok(()) => Outcome::Archived(dst),
            Err(e) => Outcome::Failed(format!(
                "renaming {} to {}: {e}",
                src.display(),
                dst.display()
            )),
        };
    }
    Outcome::Failed(format!("no free archive name for {}", src.display()))
}

/// Current UTC time as `YYYYMMDD-HHMMSS`.
fn timestamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_timestamp(secs)
}

/// Format seconds since the Unix epoch as `YYYYMMDD-HHMMSS` in UTC
/// (Howard Hinnant's civil-from-days algorithm; no chrono dependency).
fn format_timestamp(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let mo = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}{mo:02}{d:02}-{h:02}{m:02}{s:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_anchors() {
        assert_eq!(format_timestamp(0), "19700101-000000");
        assert_eq!(format_timestamp(86_399), "19700101-235959");
        assert_eq!(format_timestamp(86_400), "19700102-000000");
        // 1e9 seconds: 2001-09-09 01:46:40 UTC.
        assert_eq!(format_timestamp(1_000_000_000), "20010909-014640");
        // Leap day: 2024-02-29 12:00:00 UTC.
        assert_eq!(format_timestamp(1_709_208_000), "20240229-120000");
    }

    #[test]
    fn absent_source_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("nope.md");
        assert_eq!(
            rotate(&src, &tmp.path().join(".chug"), "LEDGER", ".md"),
            Outcome::Skipped
        );
    }

    #[test]
    fn rotates_into_timestamped_archive() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("LEDGER.md");
        fs::write(&src, "old state").unwrap();
        let out = rotate(&src, &tmp.path().join(".chug"), "LEDGER", ".md");
        let Outcome::Archived(dst) = out else {
            panic!("expected Archived, got {out:?}");
        };
        let name = dst.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.starts_with("LEDGER-") && name.ends_with(".md"), "{name}");
        assert_eq!(fs::read_to_string(&dst).unwrap(), "old state");
        assert!(!src.exists(), "source moved away");
    }

    #[test]
    fn same_second_rotations_get_distinct_names() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".chug");
        let mut names = Vec::new();
        for content in ["first", "second"] {
            let src = tmp.path().join("LEDGER.md");
            fs::write(&src, content).unwrap();
            let Outcome::Archived(dst) = rotate(&src, &dir, "LEDGER", ".md") else {
                panic!("expected Archived");
            };
            names.push(dst);
        }
        assert_ne!(names[0], names[1], "collision must suffix, not clobber");
        let contents: Vec<String> = names
            .iter()
            .map(|p| fs::read_to_string(p).unwrap())
            .collect();
        assert!(contents.contains(&"first".to_string()));
        assert!(contents.contains(&"second".to_string()));
    }

    #[cfg(unix)]
    #[test]
    fn read_only_archive_dir_reports_failed() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("LEDGER.md");
        fs::write(&src, "state").unwrap();
        let dir = tmp.path().join(".chug");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let out = rotate(&src, &dir, "LEDGER", ".md");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(out, Outcome::Failed(_)), "got {out:?}");
        assert_eq!(fs::read_to_string(&src).unwrap(), "state", "source kept");
    }
}

//! T136: crash-safe whole-file replacement. Live state files (the session
//! transcript, the todo store) are rewritten wholesale on every update; a
//! plain `fs::write` opens the target with O_TRUNC first, so a crash or
//! write failure mid-rewrite destroys the only copy — the exact
//! destroy-on-crash leg the codex review named for transcript.rs's
//! truncating rewrite. [`write_atomic`] writes a same-directory temp file
//! (pid-suffixed: concurrent writers never collide), flushes it, then
//! `fs::rename`s it over the target — a POSIX rename replaces the target
//! atomically, so the target only ever holds whole old or whole new bytes.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

/// Replace `path`'s contents with `bytes` atomically: write a pid-suffixed
/// `.tmp` sibling in the SAME directory (rename stays on one filesystem),
/// `sync_all` it, rename it over the target, and on any failure remove the
/// temp file and leave the target byte-untouched.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp: PathBuf = {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .with_context(|| format!("atomic write: unusable path {}", path.display()))?;
        path.with_file_name(format!("{name}.{}.tmp", std::process::id()))
    };
    let write = || -> anyhow::Result<()> {
        let mut file = File::create(&tmp)
            .with_context(|| format!("opening temp file {}", tmp.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("writing {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("syncing {}", tmp.display()))?;
        drop(file);
        fs::rename(&tmp, path)
            .with_context(|| format!("renaming {} over {}", tmp.display(), path.display()))
    };
    match write() {
        Ok(()) => Ok(()),
        Err(e) => {
            // Best-effort cleanup: never leave a partial temp behind.
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Overwrite replaces the content wholesale; no temp sibling remains.
    #[test]
    fn overwrite_replaces_content_and_leaves_no_temp() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("state.json");
        fs::write(&path, "old bytes").unwrap();
        write_atomic(&path, b"new bytes").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new bytes");
        let entries: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["state.json".to_string()], "{entries:?}");
    }

    /// Create leg: the target may not exist yet.
    #[test]
    fn creates_missing_target() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("fresh.jsonl");
        write_atomic(&path, b"line\n").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"line\n");
    }

    /// Failure leg: when the final rename cannot succeed (target is a
    /// directory), the error propagates, the target is untouched, and the
    /// temp file is cleaned up.
    #[test]
    fn failed_rename_leaves_target_and_cleans_temp() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("target");
        fs::create_dir(&path).unwrap();
        assert!(write_atomic(&path, b"x").is_err());
        assert!(path.is_dir(), "the target was never touched");
        let strays: Vec<String> = fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(strays.is_empty(), "no temp leftover: {strays:?}");
    }
}

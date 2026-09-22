use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::archive;

/// Seed written to `<cwd>/LEDGER.md` when the file does not exist yet.
pub const SEED: &str = "# Ledger\n\n## Done\n- (nothing yet)\n\n## Next\n- Read the spec\n\n## Blockers\n- none\n";

pub fn ledger_path(cwd: &Path) -> PathBuf {
    cwd.join("LEDGER.md")
}

/// Write the seed ledger if none exists yet. Idempotent.
pub fn ensure_seeded(cwd: &Path) -> anyhow::Result<()> {
    let path = ledger_path(cwd);
    if !path.exists() {
        fs::write(&path, SEED)
            .with_context(|| format!("seeding {}", path.display()))?;
    }
    Ok(())
}

/// Read the current ledger; falls back to the seed text if the file is absent.
pub fn read(cwd: &Path) -> anyhow::Result<String> {
    match fs::read_to_string(ledger_path(cwd)) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(SEED.to_string()),
        Err(e) => Err(e).with_context(|| format!("reading {}", ledger_path(cwd).display())),
    }
}

/// Fresh-run housekeeping (T3): a fresh run never inherits a previous
/// session's ledger. Any ledger whose content differs from the pristine seed
/// is rotated to `.chug/LEDGER-<timestamp>.md` (archive — never delete);
/// continuation across a crash is what `--resume` is for, and `--resume`
/// never calls this. Best-effort: rotation failures come back as
/// [`archive::Outcome::Failed`] so the run can warn and continue.
pub fn archive_stale(cwd: &Path) -> archive::Outcome {
    let path = ledger_path(cwd);
    match fs::read_to_string(&path) {
        // Absent or unreadable: nothing (safely) archivable; ensure_seeded /
        // read surface the real error downstream as today.
        Err(_) => archive::Outcome::Skipped,
        Ok(content) if content == SEED => archive::Outcome::Skipped,
        Ok(_) => archive::rotate(&path, &cwd.join(".chug"), "LEDGER", ".md"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        ensure_seeded(tmp.path()).unwrap();
        let content = fs::read_to_string(ledger_path(tmp.path())).unwrap();
        assert!(content.contains("## Next"));
        assert!(content.contains("## Done"));
        assert_eq!(read(tmp.path()).unwrap(), content);
    }

    #[test]
    fn read_returns_seed_without_file() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(read(tmp.path()).unwrap().contains("## Next"));
    }

    #[test]
    fn ensure_seeded_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let path = ledger_path(tmp.path());
        fs::write(&path, "# Ledger\n\n## Done\n- custom\n").unwrap();
        ensure_seeded(tmp.path()).unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("custom"));
    }

    #[test]
    fn archive_stale_rotates_foreign_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(ledger_path(tmp.path()), "# Ledger\n\n## Done\n- OLD GOAL MET\n").unwrap();
        let out = archive_stale(tmp.path());
        let archive::Outcome::Archived(dst) = out else {
            panic!("expected Archived, got {out:?}");
        };
        assert!(fs::read_to_string(&dst).unwrap().contains("OLD GOAL MET"));
        assert!(!ledger_path(tmp.path()).exists(), "ledger moved to archive");
        // After archiving, ensure_seeded installs a pristine seed.
        ensure_seeded(tmp.path()).unwrap();
        assert_eq!(read(tmp.path()).unwrap(), SEED);
    }

    #[test]
    fn archive_stale_skips_seed_and_absent() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(archive_stale(tmp.path()), archive::Outcome::Skipped);
        fs::write(ledger_path(tmp.path()), SEED).unwrap();
        assert_eq!(archive_stale(tmp.path()), archive::Outcome::Skipped);
        assert!(ledger_path(tmp.path()).exists(), "pristine seed left alone");
        assert!(
            !tmp.path().join(".chug").exists(),
            "no archive dir created for pristine state"
        );
    }
}

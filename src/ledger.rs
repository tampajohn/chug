use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;

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
}

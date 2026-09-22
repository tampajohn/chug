use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::api::Message;
use crate::archive;

pub fn transcript_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("transcript.jsonl")
}

/// Fresh-run housekeeping (T7): rotate a non-empty transcript left by a
/// previous session to `.chug/transcript-<timestamp>.jsonl` BEFORE the new
/// run's first append, so a later `--resume` never splices foreign sessions
/// into context. Callers: only the fresh (non-`--resume`) autonomous path —
/// `--resume` loads the file as-is and chat keeps it across sessions.
/// Best-effort: rotation failures come back as [`archive::Outcome::Failed`]
/// so the run can warn and continue appending.
pub fn rotate_fresh(cwd: &Path) -> archive::Outcome {
    let path = transcript_path(cwd);
    match fs::metadata(&path) {
        Ok(meta) if meta.len() > 0 => {
            archive::rotate(&path, &cwd.join(".chug"), "transcript", ".jsonl")
        }
        _ => archive::Outcome::Skipped,
    }
}

/// Append one message as a JSONL line, creating `.chug/` on demand.
pub fn append(cwd: &Path, msg: &Message) -> anyhow::Result<()> {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating transcript dir {}", dir.display()))?;
    let line = serde_json::to_string(msg).context("serializing transcript message")?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(transcript_path(cwd))
        .with_context(|| format!("opening {}", transcript_path(cwd).display()))?;
    writeln!(file, "{line}").context("writing transcript line")?;
    Ok(())
}

/// Load all messages from the transcript. Empty vec when no transcript exists.
pub fn load(cwd: &Path) -> anyhow::Result<Vec<Message>> {
    let path = transcript_path(cwd);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut messages = Vec::new();
    for (lineno, line) in data.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Message = serde_json::from_str(line).with_context(|| {
            format!("parsing {} line {}", path.display(), lineno + 1)
        })?;
        messages.push(msg);
    }
    Ok(messages)
}

/// Rewrite the whole transcript file (used only after trimming).
pub fn rewrite(cwd: &Path, messages: &[Message]) -> anyhow::Result<()> {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating transcript dir {}", dir.display()))?;
    let mut out = String::new();
    for msg in messages {
        out.push_str(&serde_json::to_string(msg).context("serializing transcript message")?);
        out.push('\n');
    }
    fs::write(transcript_path(cwd), out)
        .with_context(|| format!("rewriting {}", transcript_path(cwd).display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ContentBlock, KnownBlock};
    use crate::archive::Outcome;
    use serde_json::json;

    #[test]
    fn append_and_load_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let m1 = Message::user(vec![
            ContentBlock::text_block("kick"),
            ContentBlock::Known(KnownBlock::Thinking {
                thinking: "hmm".into(),
                signature: Some("sig".into()),
            }),
        ]);
        let m2 = Message::assistant(vec![ContentBlock::Other(json!({
            "type": "brand_new_block",
            "payload": [1, 2, 3]
        }))]);
        append(tmp.path(), &m1).unwrap();
        append(tmp.path(), &m2).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), vec![m1.clone(), m2.clone()]);
        rewrite(tmp.path(), &[m1.clone(), m2.clone()]).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), vec![m1, m2]);
    }

    #[test]
    fn load_missing_transcript_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().is_empty());
    }

    #[test]
    fn rotate_fresh_archives_non_empty_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let msg = Message::user(vec![ContentBlock::text_block("Goal: OLD SESSION")]);
        append(tmp.path(), &msg).unwrap();

        let out = rotate_fresh(tmp.path());
        let Outcome::Archived(dst) = out else {
            panic!("expected Archived, got {out:?}");
        };
        let name = dst.file_name().unwrap().to_string_lossy().to_string();
        assert!(
            name.starts_with("transcript-") && name.ends_with(".jsonl"),
            "{name}"
        );
        assert!(
            fs::read_to_string(&dst).unwrap().contains("Goal: OLD SESSION"),
            "archive holds the old session"
        );
        assert!(!transcript_path(tmp.path()).exists(), "transcript moved away");
    }

    #[test]
    fn rotate_fresh_skips_absent_and_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped);

        let path = transcript_path(tmp.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "").unwrap();
        assert_eq!(rotate_fresh(tmp.path()), Outcome::Skipped, "0-byte file");
        assert!(path.exists(), "empty transcript left alone");
        // No archive created next to it.
        let entries: Vec<_> = fs::read_dir(path.parent().unwrap()).unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn rotate_fresh_reports_rename_failure() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let msg = Message::user(vec![ContentBlock::text_block("stuck session")]);
        append(tmp.path(), &msg).unwrap();
        let dir = tmp.path().join(".chug");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();

        let out = rotate_fresh(tmp.path());
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(matches!(out, Outcome::Failed(_)), "got {out:?}");
        assert!(
            load(tmp.path()).unwrap()[0].content[0]
                .text()
                .unwrap()
                .contains("stuck session"),
            "transcript kept as-is"
        );
    }
}

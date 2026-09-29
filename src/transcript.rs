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
///
/// Test-side convenience reader: production resume reads via
/// [`load_with_torn`] (T136 — the torn-tail flag is what lets the resume
/// path truncate the torn bytes). The torn-tail tolerance itself is shared,
/// so every caller sees the same crash-resilient read.
#[cfg(test)]
pub fn load(cwd: &Path) -> anyhow::Result<Vec<Message>> {
    Ok(load_with_torn(cwd)?.0)
}

/// [`load`]'s production shape, plus a flag: `true` when a malformed
/// trailing line was dropped as a torn write (T136 — a crash mid-append
/// tears only the LAST line, so the intact prefix always loads and the
/// transcript stays resumable; real mid-file corruption stays a loud
/// error, because that is data loss, not a torn write). The resume path
/// uses the flag to physically truncate the torn bytes so later appends
/// cannot merge into them.
pub fn load_with_torn(cwd: &Path) -> anyhow::Result<(Vec<Message>, bool)> {
    let path = transcript_path(cwd);
    if !path.exists() {
        return Ok((Vec::new(), false));
    }
    let data =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let mut messages = Vec::new();
    let mut torn_tail = false;
    let lines: Vec<&str> = data.lines().collect();
    let last_content = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|p| p + 1)
        .unwrap_or(0);
    for (lineno, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Message = match serde_json::from_str(line) {
            Ok(msg) => msg,
            Err(_) if lineno + 1 == last_content => {
                // Torn trailing write (T136): the only line a crash can tear
                // is the last one. Drop it; keep everything before it.
                torn_tail = true;
                continue;
            }
            Err(e) => {
                return Err(e).with_context(|| {
                    format!("parsing {} line {}", path.display(), lineno + 1)
                });
            }
        };
        messages.push(msg);
    }
    Ok((messages, torn_tail))
}

/// Rewrite the whole transcript file (used after trimming, and by the resume
/// path when it truncates a torn tail). T136: the write is atomic — a
/// truncating in-place write destroyed the only active transcript when a
/// crash or write failure landed mid-rewrite (temp+rename instead; see
/// [`crate::fsatomic::write_atomic`]).
pub fn rewrite(cwd: &Path, messages: &[Message]) -> anyhow::Result<()> {
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating transcript dir {}", dir.display()))?;
    let mut out = String::new();
    for msg in messages {
        out.push_str(&serde_json::to_string(msg).context("serializing transcript message")?);
        out.push('\n');
    }
    crate::fsatomic::write_atomic(&transcript_path(cwd), out.as_bytes())
        .with_context(|| format!("rewriting {}", transcript_path(cwd).display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ContentBlock, KnownBlock};
    use crate::archive::Outcome;
    use serde_json::json;

    /// T136 torn-tail leg: a crash mid-append leaves a final line that is
    /// not valid JSON. Loading must DROP the torn tail and keep the intact
    /// prefix — aborting made the whole transcript unresumable — while real
    /// corruption BEFORE the last line stays a loud error (that is data
    /// loss, not a torn write; silently skipping it would hide a broken
    /// transcript).
    #[test]
    fn load_drops_torn_trailing_line_but_not_mid_file_corruption() {
        let tmp = tempfile::tempdir().unwrap();
        let path = transcript_path(tmp.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let line = |t: &str| {
            serde_json::to_string(&Message::user(vec![ContentBlock::text_block(t)])).unwrap()
        };
        // Torn LAST line (crash mid-append, no trailing newline): tolerated.
        fs::write(&path, format!("{}\n{}\n{{\"role\":\"user\",", line("one"), line("two")))
            .unwrap();
        let loaded = load(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 2, "torn tail dropped, intact lines kept");
        assert_eq!(loaded[0].content[0].text(), Some("one"));
        assert_eq!(loaded[1].content[0].text(), Some("two"));

        // A torn line that is not last (later lines parse) is REAL
        // corruption: still a loud error.
        fs::write(&path, format!("{}\ngarbage{{\n{}\n", line("one"), line("two"))).unwrap();
        assert!(load(tmp.path()).is_err(), "mid-file corruption stays loud");

        // Whitespace-only trailing lines keep loading (pre-existing rule).
        fs::write(&path, format!("{}\n{}\n  \n", line("one"), line("two"))).unwrap();
        assert_eq!(load(tmp.path()).unwrap().len(), 2);
    }

    /// T136 rewrite destroy leg, deterministically: with RLIMIT_FSIZE capping
    /// writes below the serialized transcript, the rewrite's write phase
    /// fails mid-flight (EFBIG — SIGXFSZ ignored so the failure surfaces as
    /// an error, not process death). The truncating `fs::write` this test
    /// kills opened the LIVE transcript with O_TRUNC before failing — the
    /// only active transcript destroyed (load → empty). The temp+rename
    /// rewrite leaves the previous transcript byte-intact and cleans the
    /// partial temp file.
    #[cfg(unix)]
    #[test]
    fn rewrite_write_failure_keeps_previous_transcript_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let big = "x".repeat(400_000);
        let original = vec![
            Message::user(vec![ContentBlock::text_block("Goal: keep me")]),
            Message::assistant(vec![ContentBlock::text_block(big)]),
        ];
        rewrite(tmp.path(), &original).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), original, "fixture wrote cleanly");

        // Ignore SIGXFSZ so the over-limit write returns EFBIG instead of
        // killing the test process; the signal is ignored BEFORE the limit
        // drops so no concurrent test write can hit the default-terminate
        // window. Both knobs are restored before any assertion runs.
        unsafe {
            libc::signal(libc::SIGXFSZ, libc::SIG_IGN);
        }
        let mut old = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        let rc = unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut old) };
        assert_eq!(rc, 0, "getrlimit");
        let capped = libc::rlimit {
            rlim_cur: 256 * 1024, // below the ~400KB serialized transcript
            rlim_max: old.rlim_max,
        };
        let rc = unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &capped) };
        assert_eq!(rc, 0, "setrlimit");

        let result = rewrite(tmp.path(), &original);

        let rc = unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &old) };
        assert_eq!(rc, 0, "restore rlimit");
        unsafe {
            libc::signal(libc::SIGXFSZ, libc::SIG_DFL);
        }

        assert!(result.is_err(), "the over-limit write must surface an error");
        assert_eq!(
            load(tmp.path()).unwrap(),
            original,
            "the failed rewrite left the previous transcript byte-intact"
        );
        // No partial temp file lingers next to the transcript.
        let dir = transcript_path(tmp.path()).parent().unwrap().to_path_buf();
        let strays: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".tmp"))
            .collect();
        assert!(strays.is_empty(), "no temp leftover: {strays:?}");
        // And the file still appends cleanly (nothing was left truncated
        // mid-structure).
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(transcript_path(tmp.path()))
            .unwrap();
        writeln!(f, "{}", serde_json::to_string(&original[0]).unwrap()).unwrap();
        assert_eq!(load(tmp.path()).unwrap().len(), 3, "append lands cleanly");
    }

    /// T136: the atomic rewrite replaces content wholesale and leaves no
    /// temp sibling behind (mechanism pin for the temp+rename rewrite).
    #[test]
    fn rewrite_replaces_content_and_leaves_no_temp_leftover() {
        let tmp = tempfile::tempdir().unwrap();
        let m1 = Message::user(vec![ContentBlock::text_block("first")]);
        let m2 = Message::user(vec![ContentBlock::text_block("second")]);
        rewrite(tmp.path(), &[m1.clone(), m2.clone()]).unwrap();
        rewrite(tmp.path(), std::slice::from_ref(&m2)).unwrap();
        assert_eq!(load(tmp.path()).unwrap(), vec![m2]);
        let dir = tmp.path().join(".chug");
        let entries: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(entries, vec!["transcript.jsonl".to_string()], "no temp siblings: {entries:?}");
    }

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

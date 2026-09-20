use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::api::Message;

pub fn transcript_path(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("transcript.jsonl")
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
}

//! T105 — F6 phase 1: named session fork slots (`chug fork`).
//!
//! A fork is a named SAVE/RESTORE slot over the two per-cwd session files:
//! `.chug/transcript.jsonl` (T7) and `LEDGER.md` (T3). It gives the serial
//! explore-two-approaches shape (benchmark: unreal-agent forking): run
//! approach A, `chug fork save approach-a`, keep going or restore and try
//! approach B from the same state.
//!
//! Copy semantics only: `save` copies INTO the slot, `restore` copies OUT
//! of it — a slot never mutates on restore, so restoring the same slot
//! twice is a byte-identical outcome. Nothing is lost: restore rotates the
//! live session aside with the existing T7/T3 archive machinery
//! ([`transcript::rotate_fresh`], [`ledger::archive_stale`]) BEFORE any
//! copy, so the live session becomes a timestamped archive exactly like a
//! fresh run leaves behind. If a rotation fails, restore aborts BEFORE
//! copying — the un-archived live state is never overwritten.
//!
//! The T55 interlock: restore refuses while a live chug run holds
//! `.chug/driver.lock`, reusing driver_lock's decision fn and probes
//! verbatim (`holder_status` + `pid_alive` + `argv_names_chug`) — a stale
//! lock degrades to proceed, matching the run path's reclaim semantics.
//!
//! Deliberately outside the run loop: no events, no banner, no ledger
//! writes (the resumed run's own `run_start` records the continuation).
//! Phase 2 deferred (FEATURES.md F6): `--session <name>` concurrent path
//! plumbing and fork-at-iteration-N surgery.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, bail};

use crate::api::Message;
use crate::archive;
use crate::driver_lock::{self, HolderStatus};
use crate::ledger;
use crate::transcript;

/// Fork slots live under `<cwd>/.chug/sessions/<name>/`.
fn sessions_dir(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("sessions")
}

fn slot_dir(cwd: &Path, name: &str) -> PathBuf {
    sessions_dir(cwd).join(name)
}

/// Slot names are single path components: `[A-Za-z0-9._-]+`. `.` and `..`
/// match that charset but are refused — they are directory navigation, not
/// names, and a name must never be able to escape `.chug/sessions/`.
fn validate_name(name: &str) -> anyhow::Result<()> {
    let charset_ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !charset_ok || name == "." || name == ".." {
        bail!(
            "fork: invalid slot name {name:?} — must match [A-Za-z0-9._-]+ \
             (no path separators; \".\" and \"..\" refused)"
        );
    }
    Ok(())
}

/// `chug fork save <name>`: snapshot the live transcript + LEDGER.md
/// (whichever exists) into `.chug/sessions/<name>/`. The transcript is
/// required (absent or empty → error); the ledger is copied when present.
/// Refuses to overwrite an existing slot without `--force`.
pub fn save(cwd: &Path, name: &str, force: bool) -> anyhow::Result<String> {
    validate_name(name)?;
    let src = transcript::transcript_path(cwd);
    let present_and_non_empty = fs::metadata(&src).map(|m| m.len() > 0).unwrap_or(false);
    if !present_and_non_empty {
        bail!(
            "fork: nothing to save — no transcript in {} (run or --resume a session first)",
            cwd.display()
        );
    }
    let slot = slot_dir(cwd, name);
    if slot.exists() {
        if !force {
            bail!(
                "fork: slot {name:?} already exists at {} — pass --force to overwrite",
                slot.display()
            );
        }
        remove_path(&slot)?;
    }
    fs::create_dir_all(&slot)
        .with_context(|| format!("creating fork slot {}", slot.display()))?;
    let transcript_bytes = fs::copy(&src, slot.join("transcript.jsonl"))
        .with_context(|| format!("copying {} into the slot", src.display()))?;
    let ledger_part = match fs::metadata(ledger::ledger_path(cwd)) {
        Ok(_) => {
            let bytes = fs::copy(ledger::ledger_path(cwd), slot.join("LEDGER.md"))
                .with_context(|| "copying LEDGER.md into the slot".to_string())?;
            format!("LEDGER.md {bytes} bytes")
        }
        Err(_) => "no LEDGER.md".to_string(),
    };
    Ok(format!(
        "fork: saved {name:?} (transcript {transcript_bytes} bytes, {ledger_part}) → {}",
        slot.display()
    ))
}

/// `chug fork list`: one line per slot — name, transcript bytes, mtime
/// (UTC `YYYYMMDD-HHMMSS`, the archive timestamp format), and a best-effort
/// ≤80-char preview of the first message text. Empty/absent slots dir →
/// `no forks`.
pub fn list(cwd: &Path) -> anyhow::Result<String> {
    let dir = sessions_dir(cwd);
    let mut names: Vec<String> = match fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter_map(|e| e.file_name().to_str().map(str::to_string))
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("no forks".to_string()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", dir.display())),
    };
    names.sort();
    if names.is_empty() {
        return Ok("no forks".to_string());
    }
    let mut lines = Vec::new();
    for name in names {
        let slot = slot_dir(cwd, &name);
        let transcript = slot.join("transcript.jsonl");
        let (bytes, mtime_src) = match fs::metadata(&transcript) {
            Ok(meta) => (meta.len(), transcript.clone()),
            Err(_) => (0, slot.clone()),
        };
        let mtime = fs::metadata(&mtime_src)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| archive::format_timestamp(d.as_secs()))
            .unwrap_or_else(|| "unknown-time".to_string());
        let preview = first_text_preview(&transcript).unwrap_or_default();
        let mut line = format!("{name}  {bytes}B  {mtime}");
        if !preview.is_empty() {
            line.push_str(&format!("  {preview}"));
        }
        lines.push(line);
    }
    Ok(lines.join("\n"))
}

/// `chug fork restore <name>`: refuse while a live run holds the T55 driver
/// lock, rotate the live transcript + non-seed ledger aside (T7/T3 — nothing
/// is lost), then copy the slot's files into place. Copy semantics only: the
/// slot itself is never mutated, so a second restore is byte-identical. A
/// slot whose LEDGER.md is absent leaves no ledger in place (the next fresh
/// run seeds one per T3).
pub fn restore(cwd: &Path, name: &str) -> anyhow::Result<String> {
    validate_name(name)?;
    // T55 interlock, BEFORE anything moves: a live chug run in this cwd is
    // appending to the very files we are about to rotate and overwrite.
    let lock = fs::read_to_string(driver_lock::lock_path(cwd)).ok();
    if let HolderStatus::Held(pid) = driver_lock::holder_status(
        lock.as_deref(),
        driver_lock::pid_alive,
        driver_lock::argv_names_chug,
    ) {
        bail!(
            "fork: cannot restore — .chug/driver.lock is held by a live chug run \
             (pid {pid}); stop that run or remove .chug/driver.lock"
        );
    }
    let slot = slot_dir(cwd, name);
    if !slot.is_dir() {
        bail!(
            "fork: no slot named {name:?} in {} — see `chug fork list`",
            sessions_dir(cwd).display()
        );
    }
    let slot_transcript = slot.join("transcript.jsonl");
    if !slot_transcript.is_file() {
        bail!(
            "fork: slot {name:?} has no transcript.jsonl — nothing to restore \
             (re-save the slot from a live session)"
        );
    }
    // Rotate the live session aside FIRST (T7/T3 reuse). A Failed rotation
    // aborts the restore: the copy below would overwrite the un-archived
    // live state. Skipped = absent/empty transcript or absent/seed ledger —
    // nothing worth archiving, and overwriting it loses nothing.
    let mut parts: Vec<String> = Vec::new();
    match transcript::rotate_fresh(cwd) {
        archive::Outcome::Archived(dst) => {
            parts.push(format!("archived live transcript → {}", dst.display()))
        }
        archive::Outcome::Skipped => {}
        archive::Outcome::Failed(reason) => {
            bail!("fork: could not archive the live transcript: {reason}")
        }
    }
    match ledger::archive_stale(cwd) {
        archive::Outcome::Archived(dst) => {
            parts.push(format!("archived live LEDGER.md → {}", dst.display()))
        }
        archive::Outcome::Skipped => {}
        archive::Outcome::Failed(reason) => {
            bail!("fork: could not archive the live LEDGER.md: {reason}")
        }
    }
    // Copy the slot into place. fs::copy reads the slot — it never mutates.
    let chug_dir = cwd.join(".chug");
    fs::create_dir_all(&chug_dir)
        .with_context(|| format!("creating {}", chug_dir.display()))?;
    let copied_transcript = fs::copy(&slot_transcript, transcript::transcript_path(cwd))
        .with_context(|| format!("restoring {} ", transcript::transcript_path(cwd).display()))?;
    parts.push(format!(
        "restored .chug/transcript.jsonl ({copied_transcript} bytes)"
    ));
    let slot_ledger = slot.join("LEDGER.md");
    if slot_ledger.is_file() {
        let copied_ledger = fs::copy(&slot_ledger, ledger::ledger_path(cwd))
            .with_context(|| "restoring LEDGER.md".to_string())?;
        parts.push(format!("restored LEDGER.md ({copied_ledger} bytes)"));
    }
    Ok(format!("fork: restored {name:?}: {}", parts.join("; ")))
}

/// Best-effort preview for `fork list`: the first message text in the slot's
/// transcript, whitespace-collapsed, capped at 80 chars (`…` when cut).
/// Unparsable/absent transcript → `None` (the list line just has no preview).
fn first_text_preview(slot_transcript: &Path) -> Option<String> {
    let data = fs::read_to_string(slot_transcript).ok()?;
    for line in data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Message>(line) else {
            continue;
        };
        if let Some(text) = msg.content.iter().find_map(|b| b.text()) {
            let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
            return Some(truncate_preview(&collapsed));
        }
    }
    None
}

const PREVIEW_MAX: usize = 80;

fn truncate_preview(text: &str) -> String {
    if text.chars().count() <= PREVIEW_MAX {
        return text.to_string();
    }
    let head: String = text.chars().take(PREVIEW_MAX - 1).collect();
    format!("{head}…")
}

/// Remove a path that may be a directory (an existing slot) or a file
/// (someone pointed `--force` at an oddity).
fn remove_path(path: &Path) -> anyhow::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
    .with_context(|| format!("removing old fork slot {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::ContentBlock;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    /// A live session worth forking: one user message (the goal) plus an
    /// optional non-seed ledger.
    fn seed_session(cwd: &Path, goal: &str, ledger_text: Option<&str>) {
        transcript::append(cwd, &Message::user(vec![ContentBlock::text_block(goal)])).unwrap();
        if let Some(text) = ledger_text {
            fs::write(ledger::ledger_path(cwd), text).unwrap();
        }
    }

    fn read(path: &Path) -> Vec<u8> {
        fs::read(path).unwrap()
    }

    /// Archive files under `.chug/` whose name starts with `prefix`
    /// (e.g. `transcript-`, `LEDGER-`), sorted.
    fn archives(cwd: &Path, prefix: &str) -> Vec<PathBuf> {
        let mut found: Vec<PathBuf> = fs::read_dir(cwd.join(".chug"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.starts_with(prefix))
            })
            .collect();
        found.sort();
        found
    }

    // ---------------- save ----------------

    #[test]
    fn save_copies_slot_files_byte_identical() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: explore approach A", Some("# Ledger\n\n## Done\n- A\n"));

        let summary = save(cwd, "approach-a", false).unwrap();
        assert!(summary.contains("approach-a"), "{summary}");

        let slot = slot_dir(cwd, "approach-a");
        assert_eq!(
            read(&slot.join("transcript.jsonl")),
            read(&transcript::transcript_path(cwd)),
            "slot transcript is a byte-identical snapshot"
        );
        assert_eq!(
            read(&slot.join("LEDGER.md")),
            read(&ledger::ledger_path(cwd)),
            "slot ledger is a byte-identical snapshot"
        );
        // The live files are untouched by save.
        assert!(transcript::transcript_path(cwd).exists());
        assert!(ledger::ledger_path(cwd).exists());
    }

    #[test]
    fn save_refuses_existing_slot_without_force() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: first pass", None);
        save(cwd, "slot", false).unwrap();
        let slot_before = read(&slot_dir(cwd, "slot").join("transcript.jsonl"));

        // The live session moved on; a silent overwrite would lose the slot.
        transcript::append(cwd, &Message::user(vec![ContentBlock::text_block("iter 2")])).unwrap();
        let err = save(cwd, "slot", false).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--force"), "refusal must name the remedy: {msg}");
        assert!(msg.contains("slot"), "refusal must name the slot: {msg}");
        assert_eq!(
            read(&slot_dir(cwd, "slot").join("transcript.jsonl")),
            slot_before,
            "refused save leaves the slot untouched"
        );
    }

    #[test]
    fn save_force_overwrites() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: first pass", None);
        save(cwd, "slot", false).unwrap();
        transcript::append(cwd, &Message::user(vec![ContentBlock::text_block("iter 2")])).unwrap();

        save(cwd, "slot", true).unwrap();
        let slot_text = fs::read_to_string(slot_dir(cwd, "slot").join("transcript.jsonl")).unwrap();
        assert!(
            slot_text.contains("iter 2") && slot_text.contains("first pass"),
            "--force re-snapshots the current live state: {slot_text}"
        );
    }

    #[test]
    fn save_rejects_invalid_names_naming_the_rule() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: x", None);
        for bad in ["a/b", "..", ".", "", "a b", "a\\b"] {
            let err = save(cwd, bad, false).unwrap_err();
            let msg = err.to_string();
            assert!(
                msg.contains("[A-Za-z0-9._-]"),
                "name {bad:?} refusal must name the rule: {msg}"
            );
        }
        // Validation happens before anything is created.
        let slots = sessions_dir(cwd);
        assert!(
            !slots.exists() || fs::read_dir(&slots).unwrap().next().is_none(),
            "no slot dirs may appear for invalid names"
        );
    }

    #[test]
    fn save_requires_a_transcript() {
        let tmp = tmp();
        let cwd = tmp.path();
        // Absent transcript.
        let err = save(cwd, "slot", false).unwrap_err();
        assert!(
            err.to_string()
                .contains("fork: nothing to save — no transcript in"),
            "{err}"
        );
        // Empty (0-byte) transcript is equally nothing.
        fs::create_dir_all(cwd.join(".chug")).unwrap();
        fs::write(transcript::transcript_path(cwd), "").unwrap();
        let err = save(cwd, "slot", false).unwrap_err();
        assert!(err.to_string().contains("nothing to save"), "{err}");
        assert!(!slot_dir(cwd, "slot").exists());
    }

    #[test]
    fn save_copies_ledger_only_when_present() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: no ledger yet", None);
        let summary = save(cwd, "slot", false).unwrap();
        assert!(
            !slot_dir(cwd, "slot").join("LEDGER.md").exists(),
            "absent live ledger stays absent in the slot"
        );
        assert!(summary.contains("no LEDGER.md"), "{summary}");
    }

    // ---------------- list ----------------

    #[test]
    fn list_says_no_forks_when_empty() {
        let tmp = tmp();
        assert_eq!(list(tmp.path()).unwrap(), "no forks");
        // An existing-but-empty slots dir is the same verdict.
        fs::create_dir_all(sessions_dir(tmp.path())).unwrap();
        assert_eq!(list(tmp.path()).unwrap(), "no forks");
    }

    #[test]
    fn list_shows_names_sizes_and_preview() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: explore approach A with a fairly long goal line", None);
        save(cwd, "approach-a", false).unwrap();
        seed_session(cwd, "Goal: explore approach B", None);
        save(cwd, "approach-b", false).unwrap();

        let out = list(cwd).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2, "one line per slot: {out}");
        assert!(lines[0].starts_with("approach-a  "), "sorted, name first: {out}");
        let bytes_a = fs::metadata(slot_dir(cwd, "approach-a").join("transcript.jsonl"))
            .unwrap()
            .len();
        assert!(
            lines[0].contains(&format!("{bytes_a}B")),
            "transcript bytes on the line: {out}"
        );
        assert!(
            lines[0].contains("explore approach A"),
            "first-message preview on the line: {out}"
        );
        assert!(lines[1].starts_with("approach-b  "), "{out}");
    }

    // ---------------- restore ----------------

    #[test]
    fn restore_archives_live_then_copies_slot() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: approach A", Some("# Ledger\n\n## Done\n- A\n"));
        save(cwd, "a", false).unwrap();
        let slot_transcript_before = read(&slot_dir(cwd, "a").join("transcript.jsonl"));

        // The live session moved on to approach B.
        transcript::append(cwd, &Message::user(vec![ContentBlock::text_block("approach B")]))
            .unwrap();
        fs::write(ledger::ledger_path(cwd), "# Ledger\n\n## Done\n- B\n").unwrap();

        let summary = restore(cwd, "a").unwrap();
        // Live state is back to the slot's byte-identical snapshot.
        assert_eq!(
            read(&transcript::transcript_path(cwd)),
            slot_transcript_before,
            "restored transcript is byte-identical to the slot"
        );
        assert_eq!(
            fs::read_to_string(ledger::ledger_path(cwd)).unwrap(),
            "# Ledger\n\n## Done\n- A\n",
            "restored ledger is byte-identical to the slot"
        );
        let live = transcript::load(cwd).unwrap();
        assert_eq!(live.len(), 1, "exactly the saved session");
        assert!(live[0].content[0].text().unwrap().contains("approach A"));

        // Nothing was lost: B's transcript and ledger landed in timestamped
        // archives (T7/T3 reuse).
        let t_archives = archives(cwd, "transcript-");
        assert_eq!(t_archives.len(), 1, "{summary}");
        assert!(
            fs::read_to_string(&t_archives[0]).unwrap().contains("approach B"),
            "the live transcript is in the archive"
        );
        let l_archives = archives(cwd, "LEDGER-");
        assert_eq!(l_archives.len(), 1, "{summary}");
        assert!(
            fs::read_to_string(&l_archives[0]).unwrap().contains("- B"),
            "the live ledger is in the archive"
        );
        // The summary names what was archived and what was restored.
        let archive_name = t_archives[0].file_name().unwrap().to_string_lossy().to_string();
        assert!(summary.contains(&archive_name), "{summary}");
        assert!(summary.contains("restored .chug/transcript.jsonl"), "{summary}");
        assert!(summary.contains("restored LEDGER.md"), "{summary}");
        // The slot itself never mutated.
        assert_eq!(read(&slot_dir(cwd, "a").join("transcript.jsonl")), slot_transcript_before);
    }

    #[test]
    fn restore_is_idempotent() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: the one state", Some("# Ledger\n\n## Done\n- x\n"));
        save(cwd, "a", false).unwrap();
        let slot_before = read(&slot_dir(cwd, "a").join("transcript.jsonl"));
        fs::write(ledger::ledger_path(cwd), "# Ledger\n\n## Done\n- y\n").unwrap();

        restore(cwd, "a").unwrap();
        let after_first = read(&transcript::transcript_path(cwd));
        restore(cwd, "a").unwrap();
        let after_second = read(&transcript::transcript_path(cwd));

        assert_eq!(after_first, after_second, "second restore is byte-identical");
        assert_eq!(after_second, slot_before, "and equal to the slot");
        assert_eq!(
            read(&slot_dir(cwd, "a").join("transcript.jsonl")),
            slot_before,
            "the slot never mutates on restore"
        );
        // Each restore re-archives the live transcript it found.
        assert_eq!(archives(cwd, "transcript-").len(), 2, "one archive per restore");
    }

    #[test]
    fn restore_missing_slot_names_it() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: x", None);
        let err = restore(cwd, "nope").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no slot named"), "{msg}");
        assert!(msg.contains("nope"), "the error must name the slot: {msg}");
    }

    #[test]
    fn restore_refuses_live_lock_naming_pid() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: pre-lock", None);
        save(cwd, "a", false).unwrap();
        // A lock naming THIS test process: alive, and the test binary's argv
        // (target/.../deps/chug-<hash>) names chug — the T55 double-positive.
        let me = std::process::id();
        fs::create_dir_all(cwd.join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(cwd), format!("{me}\nstart_epoch 0\n")).unwrap();

        let err = restore(cwd, "a").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("driver.lock"), "{msg}");
        assert!(
            msg.contains(&me.to_string()),
            "refusal must name the holding pid {me}: {msg}"
        );
        // Nothing moved: the live transcript is still the pre-restore session.
        let live = transcript::load(cwd).unwrap();
        assert!(live[0].content[0].text().unwrap().contains("pre-lock"));
        assert!(archives(cwd, "transcript-").is_empty(), "no archive on refusal");
    }

    #[test]
    fn restore_proceeds_past_stale_lock() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: stale", None);
        save(cwd, "a", false).unwrap();
        // No live process can hold i32::MAX — the T55 reclaim leg.
        fs::create_dir_all(cwd.join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(cwd), format!("{}\nstart_epoch 0\n", i32::MAX)).unwrap();

        restore(cwd, "a").unwrap();
        let live = transcript::load(cwd).unwrap();
        assert!(live[0].content[0].text().unwrap().contains("stale"));
    }

    #[test]
    fn restore_slot_without_ledger_leaves_no_ledger() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: slot without ledger", None);
        save(cwd, "a", false).unwrap();
        // Live state now has a non-seed ledger that must be archived away.
        fs::write(ledger::ledger_path(cwd), "# Ledger\n\n## Done\n- live\n").unwrap();

        restore(cwd, "a").unwrap();
        assert!(
            !ledger::ledger_path(cwd).exists(),
            "no ledger seeded — the next fresh run seeds per T3"
        );
        let l_archives = archives(cwd, "LEDGER-");
        assert_eq!(l_archives.len(), 1);
        assert!(
            fs::read_to_string(&l_archives[0]).unwrap().contains("- live"),
            "the live ledger was archived, not lost"
        );
    }

    #[test]
    fn restore_slot_without_transcript_errors() {
        let tmp = tmp();
        let cwd = tmp.path();
        seed_session(cwd, "Goal: x", None);
        fs::create_dir_all(slot_dir(cwd, "hollow")).unwrap();
        let err = restore(cwd, "hollow").unwrap_err();
        assert!(
            err.to_string().contains("no transcript.jsonl"),
            "{err}"
        );
    }

    #[test]
    fn preview_is_capped_at_80_chars() {
        let long = "x".repeat(300);
        assert_eq!(truncate_preview(&long).chars().count(), PREVIEW_MAX);
        assert!(truncate_preview(&long).ends_with('…'));
        assert_eq!(truncate_preview("short"), "short");
    }

    #[test]
    fn preview_skips_unparsable_lines_and_takes_first_text() {
        let tmp = tmp();
        let path = tmp.path().join("t.jsonl");
        let msg = Message::user(vec![ContentBlock::text_block("first real goal")]);
        fs::write(
            &path,
            format!("not json\n{}\n", serde_json::to_string(&msg).unwrap()),
        )
        .unwrap();
        assert_eq!(
            first_text_preview(&path).as_deref(),
            Some("first real goal")
        );
        // No parsable message at all → no preview.
        fs::write(&path, "garbage\n").unwrap();
        assert_eq!(first_text_preview(&path), None);
        assert_eq!(first_text_preview(&tmp.path().join("absent.jsonl")), None);
    }
}

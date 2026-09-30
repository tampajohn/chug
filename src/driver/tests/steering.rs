// T104 family: steering — drain_steering/append_steering_notes FIFO wiring. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
#[test]
fn steering_drains_fifo_into_transcript() {
    let tmp = tempfile::tempdir().unwrap();
    let (tx, rx) = mpsc::channel();
    tx.send("note one".to_string()).unwrap();
    tx.send("note two".to_string()).unwrap();
    drop(tx);

    let notes = drain_steering(&rx);
    assert_eq!(notes, vec!["note one".to_string(), "note two".to_string()]);

    let mut messages = vec![Message::user(vec![ContentBlock::text_block("start")])];
    let mut sink = RecordingSink::default();
    append_steering_notes(tmp.path(), &mut messages, &notes, &mut sink).unwrap();

    assert_eq!(messages.len(), 3);
    for (i, expected) in ["[operator] note one", "[operator] note two"]
        .into_iter()
        .enumerate()
    {
        assert_eq!(messages[1 + i].role, "user");
        assert_eq!(messages[1 + i].content[0].text(), Some(expected));
    }
    // transcript file round-trips exactly what the driver holds
    assert_eq!(transcript::load(tmp.path()).unwrap(), messages[1..]);
    // SteeringQueued emitted in FIFO order
    let queued: Vec<String> = sink
        .0
        .iter()
        .filter_map(|e| match e {
            Event::SteeringQueued(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(queued, notes);
}

#[test]
fn steering_drain_ignores_disconnected_channel() {
    let (tx, rx) = mpsc::channel::<String>();
    drop(tx);
    assert!(drain_steering(&rx).is_empty());
}

// ---------- T157: the cross-process steering queue ----------

/// Append one `{"note": …}` line to the cwd's steering queue, the same
/// shape `chug mcp-serve`'s chug_steer writes.
fn queue_note(tmp: &tempfile::TempDir, note: &str) {
    let path = steering_queue_path(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let line = json!({ "note": note, "ts": "t0" });
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(f, "{line}").unwrap();
}

/// The queue is the SECOND transport into the SAME `[operator]`
/// mechanism: a queued note is drained at the boundary, lands as a
/// `[operator] …` user message (transcript included), emits
/// SteeringQueued, and CONSUMES the file (the rename-away drain) so the
/// next boundary does not re-inject it.
#[test]
fn steering_queue_note_lands_at_the_next_iteration_boundary_and_consumes_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    queue_note(&tmp, "queue note one");
    queue_note(&tmp, "queue note two");
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let controls = Controls::detached();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        None,
        &observ::Sink::Noop,
    );
    let mut knobs = knobs_with(5);
    let mut llm = ScriptedLlm::new(vec![tool_use_response(
        "goal_complete",
        json!({"summary": "done with it"}),
    )]);
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
    // Both queued notes landed, FIFO, as [operator] user messages.
    let text: Vec<String> = messages
        .iter()
        .filter_map(|m| m.content[0].text().map(str::to_string))
        .collect();
    assert!(
        text.contains(&"[operator] queue note one".to_string())
            && text.contains(&"[operator] queue note two".to_string()),
        "{text:?}"
    );
    // The transcript round-trips them (the note is run context, not
    // just in-memory decoration).
    let stored = transcript::load(tmp.path()).unwrap();
    assert!(
        stored
            .iter()
            .any(|m| { m.content[0].text() == Some("[operator] queue note one") })
    );
    // SteeringQueued fired once per note.
    let queued: Vec<String> = sink
        .0
        .iter()
        .filter_map(|e| match e {
            Event::SteeringQueued(n) => Some(n.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        queued,
        vec!["queue note one".to_string(), "queue note two".to_string()]
    );
    // The drain CONSUMED the queue: the file is gone (renamed away and
    // the staging copy deleted), so a second run in this cwd does not
    // re-inject the same notes.
    assert!(
        !steering_queue_path(tmp.path()).exists(),
        "a drained queue must not survive the boundary"
    );
    // And a second boundary drains nothing: messages gained no further
    // [operator] lines (the run ended, but the drain helper itself is
    // now empty).
    assert!(drain_steering_queue(tmp.path()).is_empty());
}

/// Channel notes (the live chat dock) drain BEFORE the queue's FIFO —
/// the live session surface outranks the async dropbox; the order is
/// pinned.
#[test]
fn steering_channel_notes_drain_before_the_queue_fifo() {
    let tmp = tempfile::tempdir().unwrap();
    queue_note(&tmp, "queued note");
    let (tx, rx) = mpsc::channel();
    tx.send("channel note".to_string()).unwrap();
    drop(tx);
    let mut notes = drain_steering(&rx);
    notes.extend(drain_steering_queue(tmp.path()));
    assert_eq!(
        notes,
        vec!["channel note".to_string(), "queued note".to_string()],
        "channel first, then the queue FIFO"
    );
}

/// THE m6-orderflip killing test (T157 fix-up): the PRODUCTION
/// composition — `drive_loop` draining the channel and then the queue —
/// is pinned by driving it, not by re-implementing it. The m6 mutant
/// (queue drained BEFORE the channel) passed the whole driver suite
/// because the only channel-vs-queue order pin above re-implements the
/// composition at the helper level, and the boundary e2e test asserts
/// presence (`contains`), not order. This test drives `drive_loop`
/// with BOTH transports live and pins the landed `[operator]`
/// sequence — in the in-memory messages AND the transcript — as
/// channel-first, then the queue's FIFO.
#[test]
fn drive_loop_lands_channel_notes_before_queue_notes() {
    let tmp = tempfile::tempdir().unwrap();
    queue_note(&tmp, "queue note one");
    queue_note(&tmp, "queue note two");
    let (stx, srx) = mpsc::channel();
    stx.send("channel note".to_string()).unwrap();
    let controls = Controls {
        abort: Arc::new(AtomicBool::new(false)),
        steering_rx: srx,
    };
    let (_utx, urx) = mpsc::channel::<SlashUpdate>();
    let ctx = ctx_for(
        &tmp,
        Mode::Autonomous,
        &controls,
        &urx,
        None,
        &observ::Sink::Noop,
    );
    let mut knobs = knobs_with(5);
    let mut llm = ScriptedLlm::new(vec![tool_use_response(
        "goal_complete",
        json!({"summary": "done with it"}),
    )]);
    let mut gate = None;
    let mut messages = Vec::new();
    let mut sink = RecordingSink::default();
    let outcome = drive_loop(
        &ctx,
        &mut knobs,
        &mut llm,
        &mut gate,
        &mut messages,
        Some("check: true".to_string()),
        &mut sink,
        &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
    )
    .unwrap();
    assert!(matches!(outcome, DriveOutcome::RunFinished(0)));
    let landed: Vec<String> = messages
        .iter()
        .filter_map(|m| m.content[0].text().map(str::to_string))
        .filter(|t| t.starts_with("[operator]"))
        .collect();
    assert_eq!(
        landed,
        vec![
            "[operator] channel note".to_string(),
            "[operator] queue note one".to_string(),
            "[operator] queue note two".to_string(),
        ],
        "drive_loop lands channel notes first, then the queue FIFO: {landed:?}"
    );
    // The transcript — the next-iteration context a reader (or a
    // resumed run) sees — carries the SAME sequence.
    let stored: Vec<String> = transcript::load(tmp.path())
        .unwrap()
        .iter()
        .filter_map(|m| m.content[0].text().map(str::to_string))
        .filter(|t| t.starts_with("[operator]"))
        .collect();
    assert_eq!(
        stored, landed,
        "the transcript lands the same [operator] sequence"
    );
}

/// Malformed queue lines and empty notes are dropped, not injected: a
/// junk line must not corrupt the transcript, and a whitespace note
/// must not become an empty `[operator]` message.
#[test]
fn steering_queue_drain_skips_malformed_and_empty_notes() {
    let tmp = tempfile::tempdir().unwrap();
    let path = steering_queue_path(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    use std::io::Write as _;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .unwrap();
    writeln!(f, "not json at all").unwrap();
    writeln!(f, "{{\"note\": \"   \"}}").unwrap();
    writeln!(f, "{{\"other\": 1}}").unwrap();
    writeln!(f, "{{\"note\": \"the real note\"}}").unwrap();
    drop(f);
    assert_eq!(
        drain_steering_queue(tmp.path()),
        vec!["the real note".to_string()]
    );
    // Consumed either way — junk is not left behind to be re-parsed.
    assert!(!steering_queue_path(tmp.path()).exists());
}

/// The queue's notes ride the SAME override semantics as the channel's:
/// the exact `allow destructive` phrase (trimmed, case-insensitive) is
/// what the risk gate disables on — the queue transport cannot mint a
/// wider override vocabulary.
#[test]
fn steering_queue_allow_destructive_matches_the_channel_override_phrase() {
    assert!(is_allow_destructive(" allow destructive "));
    assert!(is_allow_destructive("ALLOW DESTRUCTIVE"));
    assert!(!is_allow_destructive("allow destructive now"));
    assert!(!is_allow_destructive("queued note"));
}

/// An absent queue (the common case) degrades to nothing — the drain
/// is a no-op, not an error.
#[test]
fn steering_queue_drain_without_a_queue_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(drain_steering_queue(tmp.path()).is_empty());
    assert!(drain_steering_queue(&tmp.path().join(".chug")).is_empty());
}

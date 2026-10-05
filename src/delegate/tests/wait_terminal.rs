// T109 family: wait_terminal — the terminal wait mode (T89).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod wait_terminal;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 9;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    // ---- T89: the terminal wait mode ----

    /// The iteration bump + budget-low lines the T89 fixtures append
    /// mid-wait (progress telemetry: significant for the pre-T89 wake set,
    /// deliberately NOT for a terminal wait).
    const T89_ITERATION_BUMP: &str = "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":8}";
    const T89_BUDGET_LOW: &str =
        "{\"type\":\"budget_low\",\"ts\":\"t5\",\"threshold\":0.8}";

    /// T89 req 1 + 3: with `terminal: true`, an ITERATION ADVANCE does not
    /// wake the wait — the wait runs to its (short) deadline — while the
    /// SAME fixture pattern DOES wake a significant-mode wait
    /// (non-vacuousness of both legs at once).
    ///
    /// Timing: the writer lands at ~0.3 s; with `wait_secs: 3` and the 2.5 s
    /// cadence there is exactly one mid-wait poll tick (~2.5 s), where the
    /// bump IS visible — so a terminal wait that (wrongly) woke on iteration
    /// advances returned there (~2.5 s, under the elapsed lower bound), while
    /// the pinned behavior runs to the deadline leg. The bump is still
    /// RENDERED: the deadline leg's final read carries `last_iteration: 8`.
    #[test]
    fn delegate_status_terminal_wait_ignores_iteration_advance_that_wakes_significant_mode() {
        // Leg A: terminal mode — the iteration advance must NOT wake.
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, T89_ITERATION_BUMP);
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 3
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // NOT early: only the deadline leg (>= 3 s) satisfies this — a
        // terminal wait waking on the ~2.5 s iteration tick fails here.
        assert!(
            elapsed >= Duration::from_millis(2900),
            "iteration advance woke a terminal wait early: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "terminal wait hung: {elapsed:?}");
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — terminal wait woke early");
        // The advance is RENDERED by the deadline leg's final read (telemetry
        // the terminal wait declined to wake on, not an entry snapshot).
        assert!(result.content.contains("last_iteration: 8"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);

        // Leg B: the SAME fixture pattern DOES wake significant mode — the
        // pre-T89 contract is untouched (non-vacuousness of leg A).
        let tmp2 = tempfile::tempdir().unwrap();
        write_events_fixture(tmp2.path(), &[T29_RUN_START]);
        let events2 = tmp2.path().join(".chug/events.jsonl");
        let writer2 = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events2, T89_ITERATION_BUMP);
        });
        let started2 = Instant::now();
        let result2 = dispatch(
            &delegate_ctx(tmp2.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp2.path(), "wait_secs": 30}),
        );
        let elapsed2 = started2.elapsed();
        writer2.join().unwrap();
        assert!(!result2.is_error, "{}", result2.content);
        assert!(
            elapsed2 < Duration::from_secs(15),
            "significant-mode wait did not wake on the iteration advance: {elapsed2:?}"
        );
        assert!(result2.content.contains("last_iteration: 8"), "{}", result2.content);
    }

    /// T89 req 1: with `terminal: true`, a `budget_low_seen` flip does NOT
    /// wake the wait (deadline leg) — budget-low is progress telemetry the
    /// orchestrator does not act on mid-child; the flag is still RENDERED by
    /// the deadline leg's final read. Timing as in the iteration pin above:
    /// `wait_secs: 3` gives the flip one visible poll tick (~2.5 s), so a
    /// mutant waking on it returns under the elapsed lower bound.
    #[test]
    fn delegate_status_terminal_wait_ignores_budget_low_flip() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, T89_BUDGET_LOW);
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 3
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed >= Duration::from_millis(2900),
            "budget_low flip woke a terminal wait early: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "terminal wait hung: {elapsed:?}");
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — terminal wait woke early");
        // Rendered at the deadline, never wake-worthy: the T89 defect shape.
        assert!(result.content.contains("budget_low_seen: true"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
    }

    /// T89 req 1(a): a terminal wait wakes EARLY on a `goal_seen` flip —
    /// the child claimed its goal, the first of the four terminal facts.
    ///
    /// NON-VACUOUSNESS: the render pins (`goal_seen: true`, `state: done`)
    /// kill an entry-snapshot mutant, and the elapsed bound kills a
    /// sleeps-to-deadline mutant (the goal is on disk at the ~2.5 s tick;
    /// the 30 s deadline is far).
    #[test]
    fn delegate_status_terminal_wait_wakes_early_on_goal_flag() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"all done\"}",
            );
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 30
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "terminal wait did not wake on the goal flip: {elapsed:?}"
        );
        assert!(result.content.contains("goal_seen: true"), "{}", result.content);
        assert!(result.content.contains("state: done"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
    }

    /// T89 req 1(b): a terminal wait wakes EARLY on an `abort_seen` flip
    /// (the `abort_reason` rides the same `abort` line — covered by the same
    /// wake), carrying the abort verdict.
    #[test]
    fn delegate_status_terminal_wait_wakes_early_on_abort_flag() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\"}",
            );
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 30
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "terminal wait did not wake on the abort flip: {elapsed:?}"
        );
        assert!(result.content.contains("abort_seen: true"), "{}", result.content);
        assert!(
            result.content.contains("abort_reason: iteration budget exceeded"),
            "{}",
            result.content
        );
        assert!(result.content.contains("state: aborted"), "{}", result.content);
    }

    /// T89 req 1(c): a terminal wait wakes EARLY on the observed liveness
    /// flip alive → dead — the real-process leg per the existing liveness
    /// tests (T29 fix-up FINDING 1's pattern): a REAL own child, handle
    /// dropped per the launch contract, SIGKILLed mid-wait; the events
    /// stream never changes, so liveness is the ONLY wake available.
    #[cfg(unix)]
    #[test]
    fn delegate_status_terminal_wait_wakes_when_child_dies_liveness_flip() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        drop(child); // launch contract: detached, never waited by the handle
        let killer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        });
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "pid": pid,
                "terminal": true,
                "wait_secs": 30
            }),
        );
        let elapsed = started.elapsed();
        killer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "terminal wait did not wake on the liveness flip: {elapsed:?}"
        );
        assert!(result.content.contains("alive: false"), "{}", result.content);
        // The events state is unchanged — the wake came from the flip.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
    }

    /// T89 req 1(d): a terminal wait wakes EARLY when the events file is
    /// CREATED mid-wait (missing at entry — the launch→build window).
    #[test]
    fn delegate_status_terminal_wait_wakes_on_events_file_creation() {
        let tmp = tempfile::tempdir().unwrap();
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            fs::create_dir_all(events.parent().unwrap()).unwrap();
            fs::write(&events, format!("{T29_RUN_START}\n")).unwrap();
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 30
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "terminal wait did not wake on events-file creation: {elapsed:?}"
        );
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("last_event: run_start t0"), "{}", result.content);
    }

    /// T89 req 2: the rejection legs. `terminal: true` with `wait_secs`
    /// absent — or explicitly `0` — is a tool error naming that terminal
    /// waits need `wait_secs > 0` (a terminal instant poll is a
    /// contradiction; corrective error, never silent degradation); a
    /// non-boolean `terminal` is a tool error too; and `terminal` on
    /// `launch` or `collect` is the same presence-based rejection the
    /// `wait_secs` arm uses.
    #[test]
    fn delegate_terminal_rejection_legs() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let ctx = delegate_ctx(tmp.path());

        // status + terminal: true, wait_secs absent.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "terminal": true}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("wait_secs > 0"),
            "rejection must name the wait_secs > 0 requirement: {}",
            result.content
        );
        assert!(
            result.content.contains("terminal"),
            "rejection must name the terminal flag: {}",
            result.content
        );

        // status + terminal: true, wait_secs: 0 — the instant leg is still a
        // contradiction for a terminal wait.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "terminal": true, "wait_secs": 0}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("wait_secs > 0"),
            "wait_secs: 0 must be rejected the same way: {}",
            result.content
        );

        // Non-boolean terminal: tool error, never a silent ignore.
        for bad in [json!("true"), json!(1)] {
            let result = dispatch(
                &ctx,
                "delegate",
                &json!({"action": "status", "cwd": tmp.path(), "terminal": bad, "wait_secs": 5}),
            );
            assert!(result.is_error, "{bad}: {}", result.content);
            assert!(
                result.content.contains("boolean"),
                "{bad} must be rejected as non-boolean: {}",
                result.content
            );
        }

        // terminal on launch: rejected on PRESENCE (even `false`), same
        // style as the wait_secs arm.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "launch",
                "cwd": tmp.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "terminal": true
            }),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("status action only"),
            "launch rejection must name that terminal is status-only: {}",
            result.content
        );

        // terminal on collect: rejected the same way.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "terminal": true}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("status action only"),
            "collect rejection must name that terminal is status-only: {}",
            result.content
        );
    }


    // ---- T234: the outcome-resolved terminal wake set ----

    /// T234 req 3 + 6(a): a goal-gate REJECTION already present at entry is
    /// STALE NEWS — the gate spoke before the wait began and the driver
    /// KEEPS RUNNING after a rejection — so a `terminal: true` wait must
    /// block to its deadline instead of instant-returning on the stale
    /// verdict (the pre-T234 outcome-blind latch woke on the goal line's
    /// mere presence, collapsing every later terminal long-poll into
    /// instant polling exactly when the arc got interesting).
    ///
    /// NON-VACUOUSNESS: kills named mutant (a) (the outcome ignored — the
    /// pre-row behavior, `goal_seen` driving the terminal wake → instant
    /// return) and mutant (c)-at-entry (rejection presence waking without
    /// the entry gate): both fail the elapsed lower bound and the
    /// `waited: >= 3` pin.
    #[test]
    fn delegate_status_terminal_wait_ignores_rejection_present_at_entry() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                T29_RUN_START,
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"rejected\",\"reason\":\"check failed\"}",
            ],
        );
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 3
            }),
        );
        let elapsed = started.elapsed();
        assert!(!result.is_error, "{}", result.content);
        // NOT early: only the deadline leg (>= 3 s) satisfies this — the
        // stale rejection must never relatch the terminal wake.
        assert!(
            elapsed >= Duration::from_millis(2900),
            "stale rejection instant-woke a terminal wait: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "terminal wait hung: {elapsed:?}");
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — terminal wait woke early");
        // The deadline render still RESOLVES the verdict for the poller:
        // the compat latch plus the outcome flags (the accepted line absent).
        assert!(result.content.contains("goal_seen: true"), "{}", result.content);
        assert!(result.content.contains("goal_rejected_seen: true"), "{}", result.content);
        assert!(!result.content.contains("goal_accepted_seen"), "{}", result.content);
    }

    /// T234 req 3 + 6(c): a NEW rejection landing mid-wait wakes the
    /// terminal wait ONCE — the orchestrator wants to steer immediately —
    /// and the waking payload becomes the NEXT call's entry: a second
    /// terminal wait over the now-stale rejection blocks normally. The
    /// rejected-wakes-EVERY-poll mutant (no entry gating) passes call 1 but
    /// fails call 2's elapsed lower bound.
    #[test]
    fn delegate_status_terminal_wait_wakes_once_on_new_rejection_then_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"rejected\",\"reason\":\"check failed\"}",
            );
        });
        // Call 1: the NEW rejection mid-wait wakes early.
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 30
            }),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "terminal wait did not wake on the new rejection: {elapsed:?}"
        );
        // The wake carries the resolution: the rejection flag (plus the
        // compat latch and state, byte-compatible).
        assert!(result.content.contains("goal_rejected_seen: true"), "{}", result.content);
        assert!(result.content.contains("goal_seen: true"), "{}", result.content);
        assert!(!result.content.contains("goal_accepted_seen"), "{}", result.content);

        // Call 2: the rejection is now STALE at entry — the wait blocks to
        // its (short) deadline instead of relatching (the payload call 1
        // returned is exactly the entry call 2 sees).
        let started2 = Instant::now();
        let result2 = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "status",
                "cwd": tmp.path(),
                "terminal": true,
                "wait_secs": 3
            }),
        );
        let elapsed2 = started2.elapsed();
        assert!(!result2.is_error, "{}", result2.content);
        assert!(
            elapsed2 >= Duration::from_millis(2900),
            "stale rejection re-woke the second terminal wait: {elapsed2:?}"
        );
        assert!(elapsed2 < Duration::from_secs(30), "terminal wait hung: {elapsed2:?}");
        let waited = waited_secs_of(&result2.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — stale rejection woke the wait early");
    }

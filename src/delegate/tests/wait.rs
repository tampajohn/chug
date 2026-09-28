// T109 family: wait — the wait_secs long-poll wake set (T29/T58).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod wait;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 9;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// T29 test 1 (instant leg byte-identical) + test 6's `0` boundary: on a
    /// fixed fake `.chug/`, `wait_secs` absent and `wait_secs: 0` both render
    /// EXACTLY the pre-T29 payload — pinned as one whole string, so any
    /// render drift, any new field, or any `waited:` line leaking into the
    /// instant leg fails here.
    #[test]
    fn delegate_status_wait_secs_absent_and_zero_are_byte_identical_to_pre_t29() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let ctx = delegate_ctx(tmp.path());
        let absent = dispatch(&ctx, "delegate", &json!({"action": "status", "cwd": tmp.path()}));
        let zero = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 0}),
        );
        let expected = "\
state: running
alive: unknown (no pid given)
max_iters: 50
last_iteration: 7
last_event: iteration t1
budget_low_seen: false
goal_seen: false
abort_seen: false
log_tail: (none)";
        assert_eq!(absent.content, expected, "{}", absent.content);
        assert_eq!(zero.content, absent.content, "wait_secs: 0 must be byte-identical to absent");
        assert!(!absent.content.contains("waited:"), "{}", absent.content);
        assert!(!absent.is_error && !zero.is_error);
    }

    /// T29 test 2 (early return on state change) AND test 7 (non-vacuousness).
    /// A writer thread appends an `iteration` line mid-wait; `wait_secs: 30`
    /// must return well under 30 s carrying the NEW state.
    ///
    /// NON-VACUOUSNESS TECHNIQUE: the assertions are on the returned CONTENT,
    /// not just timing. Gutting the wait loop to a fixed sleep (sleep to the
    /// deadline, then render) passes every timing assertion but returns the
    /// ENTRY state — `last_iteration: none` — so pinning `last_iteration: 7`
    /// and `last_event: iteration t1` kills it. The return-immediately
    /// mutant fails the same pins; a wait-the-full-30s mutant fails the
    /// elapsed bound below.
    #[test]
    fn delegate_status_wait_returns_early_on_state_change_with_new_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, T29_ITERATION);
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // Early: well under the 30 s deadline (CI slack).
        assert!(elapsed < Duration::from_secs(15), "wait did not return early: {elapsed:?}");
        // The NEW state, not the entry snapshot — the non-vacuousness pins.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t1"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        // The wait leg's one extra line, naming the actual elapsed seconds.
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
    }

    /// T29 test 3 (deadline): a static `.chug/` and `wait_secs: 2` return
    /// after ~2 s with the entry state unchanged and the `waited:` line
    /// present (1 ≤ waited ≤ 10 for CI slack).
    #[test]
    fn delegate_status_wait_returns_at_deadline_with_unchanged_state() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        assert!(!result.is_error, "{}", result.content);
        assert!(elapsed >= Duration::from_secs(1), "returned before any wait could elapse: {elapsed:?}");
        assert!(elapsed < Duration::from_secs(30), "overshot the 2s deadline: {elapsed:?}");
        // Unchanged state fields — the same fields the instant leg renders.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t1"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!((1..=10).contains(&waited), "waited: {waited}s");
    }

    /// T29 test 4 (missing events file). Req 2(a) makes the file's CREATION a
    /// wake trigger, so a file missing at entry does not skip the wait — the
    /// two legs pin both halves and reconcile the spec's "returns
    /// immediately" wording with req 2(a):
    /// (a) no writer: the wait is strictly bounded by the deadline, never a
    ///     hang (req 5), and renders the starting/`events: nothing read`
    ///     note leg with no panic;
    /// (b) a creator thread: returns promptly — as soon as the file appears —
    ///     with the new state (the file-creation trigger, non-vacuously).
    #[test]
    fn delegate_status_wait_on_missing_events_file_bounded_then_created() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = delegate_ctx(tmp.path());
        // (a) No writer: bounded by the deadline, starting/note leg, no panic.
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        assert!(!result.is_error, "{}", result.content);
        assert!(elapsed >= Duration::from_secs(1), "no-writer wait returned before its deadline: {elapsed:?}");
        assert!(elapsed < Duration::from_secs(30), "missing-file wait hung: {elapsed:?}");
        assert!(result.content.contains("state: starting"), "{}", result.content);
        assert!(result.content.contains("events: nothing read"), "{}", result.content);
        assert!(result.content.contains("last_event: none"), "{}", result.content);
        assert!(waited_secs_of(&result.content).is_some(), "{}", result.content);

        // (b) Creator thread: the file appearing mid-wait IS the state
        // change, so the wait returns promptly with the new state.
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            fs::create_dir_all(events.parent().unwrap()).unwrap();
            fs::write(&events, format!("{T29_RUN_START}\n")).unwrap();
        });
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        assert!(
            elapsed < Duration::from_secs(15),
            "creation trigger did not wake the wait: {elapsed:?}"
        );
        assert!(result.content.contains("last_event: run_start t0"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
    }

    /// T29 test 6 (boundary pins): `600` accepted, `601` and negative
    /// rejected — the pinned req-4 choice is REJECT, never clamp. (`0` =
    /// instant is pinned byte-identically in the test-1 leg.)
    #[test]
    fn delegate_status_wait_secs_boundaries_reject_out_of_range() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let ctx = delegate_ctx(tmp.path());

        // 600 is accepted. A writer thread cuts the wait short so the test
        // stays fast; acceptance means the parse did not reject the cap.
        let events = tmp.path().join(".chug/events.jsonl");
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(&events, "{\"type\":\"iteration\",\"ts\":\"t9\",\"n\":8}");
        });
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 600}),
        );
        writer.join().unwrap();
        assert!(!result.is_error, "600 must be accepted: {}", result.content);
        assert!(result.content.contains("last_iteration: 8"), "{}", result.content);

        // 601: rejected, naming the 600 cap — not clamped down to it.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 601}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("600"), "{}", result.content);
        // Negative: rejected, naming the >= 0 floor.
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": -1}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("-1"), "{}", result.content);
        // Non-integer: rejected (string and fractional alike).
        for bad in [json!("90"), json!(90.5)] {
            let result = dispatch(
                &ctx,
                "delegate",
                &json!({"action": "status", "cwd": tmp.path(), "wait_secs": bad}),
            );
            assert!(result.is_error, "{}", result.content);
            assert!(result.content.contains("integer"), "{}", result.content);
        }
    }

    /// T29 fix-up, FINDING 1 (spec req 2(b) shipped untested): the
    /// liveness-flip wake leg. A REAL own child (the T28 fixtures' pattern) is
    /// spawned with a static events fixture — so the ONLY thing that changes
    /// during the wait is liveness — and its handle is dropped per the launch
    /// contract, making the waitpid reap inside [`reap_and_alive`] the only
    /// observer of the exit. It is SIGKILLed mid-wait; the wait must wake well
    /// before the deadline with liveness flipped to dead.
    ///
    /// NON-VACUOUSNESS (validator mutant M7: the `liveness_flipped` check
    /// gutted): the events stream never changes, so the gutted wait sleeps to
    /// the full 30 s deadline — the elapsed bound below kills it. A mutant
    /// that skips the waitpid reap keeps answering the zombie's `alive: true`
    /// and fails the `alive: false` pin instead.
    #[cfg(unix)]
    #[test]
    fn delegate_status_wait_wakes_early_when_child_dies_liveness_flip() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        drop(child); // launch contract: detached, never waited by the handle
        // The wait call blocks this thread, so the kill fires from a helper
        // thread: the child dies mid-wait and stays a zombie for the seam.
        let killer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        });
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "pid": pid, "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        killer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // The flip woke the wait: well under the 30 s deadline (CI slack).
        assert!(
            elapsed < Duration::from_secs(15),
            "liveness flip did not wake the wait early: {elapsed:?}"
        );
        // Liveness flipped to dead, with the wait leg's `waited:` line.
        assert!(result.content.contains("alive: false"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
        // The events state is unchanged (`last_iteration: 7`) — the wake came
        // from the liveness flip, not from a state change.
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
    }

    /// T29 fix-up, FINDING 2 (spec req 2's "FIRST of" + "same payload as the
    /// instant leg"): the deadline leg must render the FINAL state, not the
    /// entry snapshot. With the 2.5 s cadence and `wait_secs: 2` there is
    /// exactly ONE poll — at entry; the cadence sleep is capped at the
    /// remaining 2 s, so nothing reads between the entry poll and the
    /// deadline — and a writer landing inside that window is therefore
    /// invisible to every intermediate poll: only a final read at the
    /// deadline can see it.
    ///
    /// NON-VACUOUSNESS: removing the final read (the pre-fix deadline leg,
    /// which rendered the entry snapshot) fails the `last_iteration: 8` pin —
    /// that render carries the entry state, `last_iteration: none`. An
    /// instant-return mutant fails the same pin (the writer has not run yet),
    /// and both pass no timing bound to hide behind.
    #[test]
    fn delegate_status_wait_deadline_renders_final_state_not_entry_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        // Lands ~1 s in: after the entry poll (~0 ms) and before the 2 s
        // deadline wake, with a full second of scheduling slack each side.
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(1000));
            append_events_line(&events, "{\"type\":\"iteration\",\"ts\":\"t8\",\"n\":8}");
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 2}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // The deadline leg ran: at least the entry poll's window elapsed.
        assert!(
            elapsed >= Duration::from_millis(1500),
            "returned before the deadline could elapse: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "overshot the 2s deadline: {elapsed:?}");
        // The deadline render carries the FINAL state, not the entry snapshot.
        assert!(result.content.contains("last_iteration: 8"), "{}", result.content);
        assert!(result.content.contains("last_event: iteration t8"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!((1..=10).contains(&waited), "waited: {waited}s");
    }

    /// T68 (churn pin): a mid-wait `tool_result` append — the `last_event`
    /// churn an active child emits every 2–10 s — must NOT wake the wait.
    /// Timing: the writer lands at ~0.3 s; with `wait_secs: 3` and the 2.5 s
    /// cadence there is exactly one mid-wait poll tick (~2.5 s), where the
    /// churn IS visible — so the pre-T68 any-field wake returned there
    /// (~2.5 s, `waited: 2`) while the significant wake set must run to the
    /// deadline. The payload still carries the NEW last_event: the deadline
    /// leg's final read renders the churn it declined to wake on.
    ///
    /// NON-VACUOUSNESS (validator-recorded): reverting the wake condition to
    /// any-field-diff turns this red — the wake fires at the ~2.5 s tick,
    /// under the elapsed lower bound and with `waited: 2 < 3`. Gutting the
    /// significant comparison so NOTHING wakes keeps this green but turns
    /// T29's iteration-append pin and the goal-append pin below red.
    #[test]
    fn delegate_status_wait_last_event_churn_does_not_wake() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START]);
        let events = tmp.path().join(".chug/events.jsonl");
        // The realistic churn line: name/ok/is_error/duration_ms/preview.
        // It moves ONLY last_event_type/ts — no significant field changes
        // (state stays `running`, no iteration, no verdict flag).
        let writer = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            append_events_line(
                &events,
                "{\"type\":\"tool_result\",\"ts\":\"t2\",\"name\":\"edit\",\"ok\":true,\"is_error\":false,\"duration_ms\":3,\"preview\":\"edited\"}",
            );
        });
        let ctx = delegate_ctx(tmp.path());
        let started = Instant::now();
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 3}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // NOT early: the churn was on disk before the ~2.5 s poll tick, so an
        // any-field wake returns there — under this bound, which only the
        // deadline leg (≥ 3 s) can satisfy.
        assert!(
            elapsed >= Duration::from_millis(2900),
            "last_event churn woke the wait early: {elapsed:?}"
        );
        assert!(elapsed < Duration::from_secs(30), "churn wait hung: {elapsed:?}");
        // `waited:` ≈ the request — the pre-T68 any-field wake reports 2.
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited >= 3, "waited: {waited}s — churn woke the wait early");
        // Churn is RENDERED: the deadline's final read carries the new
        // last_event (not an entry-snapshot render).
        assert!(result.content.contains("last_event: tool_result t2"), "{}", result.content);
        // …while every significant field is unchanged.
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("last_iteration: none"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
    }

    /// T68 (significant-wake pin): a mid-wait `goal` append — a verdict flag
    /// flipping with NO `last_iteration` movement — must still wake the wait
    /// EARLY carrying the flag. T29's early-return pin covers the
    /// `last_iteration` advance; this covers the verdict-flag half of the
    /// significant set, so the T68 narrowing cannot over-correct into
    /// "nothing wakes".
    ///
    /// NON-VACUOUSNESS: the over-correction mutant (significant comparison
    /// gutted so nothing wakes) fails the elapsed bound — the goal is on
    /// disk at the ~2.5 s tick and the 30 s deadline is far; an
    /// entry-snapshot render fails the `goal_seen: true` / `state: done`
    /// pins. (The pre-T68 any-field revert passes this test — it is the
    /// churn pin above that kills it.)
    #[test]
    fn delegate_status_wait_wakes_early_on_goal_flag() {
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
            &json!({"action": "status", "cwd": tmp.path(), "wait_secs": 30}),
        );
        let elapsed = started.elapsed();
        writer.join().unwrap();
        assert!(!result.is_error, "{}", result.content);
        // Early: well under the 30 s deadline (CI slack).
        assert!(
            elapsed < Duration::from_secs(15),
            "goal flag did not wake the wait early: {elapsed:?}"
        );
        // The verdict flag, rendered from the woken state — not the entry
        // snapshot (`goal_seen: false`, `state: running`).
        assert!(result.content.contains("goal_seen: true"), "{}", result.content);
        assert!(result.content.contains("state: done"), "{}", result.content);
        assert!(result.content.contains("last_event: goal t2"), "{}", result.content);
        let waited = waited_secs_of(&result.content).expect("waited: line present");
        assert!(waited < 30, "waited: {waited}s");
    }


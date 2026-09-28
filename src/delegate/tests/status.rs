// T109 family: status — the status action: rendering, reaping, liveness, tail windows (T23/T29/T58/T68).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod status;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 10;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    #[test]
    fn delegate_status_without_chug_dir_is_starting_not_error() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: starting"), "{}", result.content);
        assert!(result.content.contains("alive: unknown"), "{}", result.content);
        assert!(result.content.contains("last_iteration: none"), "{}", result.content);
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
        assert!(result.content.contains("log_tail: (none)"), "{}", result.content);
    }

    #[test]
    fn delegate_status_summarizes_synthetic_events_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(
            tmp.path().join(".chug/events.jsonl"),
            concat!(
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"mode\":\"run\",\"model\":\"m\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}\n",
                "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"input_tokens\":1,\"output_tokens\":1}\n",
                "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":100,\"remaining_tokens\":null}\n",
                "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"m\",\"budget_kind\":\"iterations\",\"budget_max\":50}\n",
            ),
        )
        .unwrap();
        // The test process itself is a live pid for the kill(pid, 0) probe.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "pid": std::process::id()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: aborted"), "{}", result.content);
        assert!(result.content.contains("alive: true"), "{}", result.content);
        assert!(result.content.contains("max_iters: 50"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
        assert!(result.content.contains("last_event: abort"), "{}", result.content);
        assert!(result.content.contains("budget_low_seen: true"), "{}", result.content);
        assert!(result.content.contains("abort_seen: true"), "{}", result.content);
        assert!(
            result.content.contains("abort_reason: iteration budget exceeded"),
            "{}",
            result.content
        );
        assert!(result.content.contains("goal_seen: false"), "{}", result.content);
    }

    // ---- T28: status reaps zombie children ----

    /// T28 test 1. An exited own child is a ZOMBIE until reaped, and the
    /// orchestrator never reaps (launch drops the handle) — `kill(pid, 0)`
    /// answers "alive" for zombies, so `status` reported `alive: true` for
    /// children whose events stream already recorded done (cycle-11 eval O2).
    ///
    /// NON-VACUOUSNESS (T28 spec test item 5): this test FAILS pre-T28 —
    /// with no waitpid leg the zombie keeps answering the probe, so the
    /// bounded loop below times out holding `Some(true)` instead of ever
    /// seeing the `Some(false)` that only the reap produces.
    #[cfg(unix)]
    #[test]
    fn delegate_status_reaps_own_exited_child_and_reports_false_twice() {
        let child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        drop(child); // the launch contract: detached, handle dropped, never waited

        // Wait for the child to exit WITHOUT reaping it ourselves: poll the
        // seam until it reports dead — the reap inside the seam is what turns
        // the zombie into a reaped, truly-gone pid.
        let deadline = Instant::now() + Duration::from_secs(10);
        let first = loop {
            match reap_and_alive(u64::from(pid)) {
                Some(false) => break Some(false),
                other => {
                    if Instant::now() >= deadline {
                        break other;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        };
        assert_eq!(first, Some(false), "exited own child must reap to dead");
        // Second poll: already reaped, so waitpid says ECHILD and the plain
        // probe (ESRCH) still reports dead — no error, and not alive.
        assert_eq!(reap_and_alive(u64::from(pid)), Some(false));
    }

    /// T28 test 2. A still-running own child stays alive: the reap leg must
    /// not misreport it (waitpid WNOHANG returns 0 → plain probe → true).
    /// Cleanup kills and reaps, so the suite leaks no zombie or stray sleeper.
    #[cfg(unix)]
    #[test]
    fn delegate_status_reports_own_running_child_alive_then_cleans_up() {
        let mut child = Command::new("sleep").arg("30").spawn().unwrap();
        let pid = child.id();
        assert_eq!(reap_and_alive(u64::from(pid)), Some(true));
        child.kill().expect("kill the sleep child");
        assert!(!child.wait().expect("reap the sleep child").success());
    }

    /// T28 test 3. A pid that is NOT our child keeps the plain probe's
    /// semantics exactly: waitpid says ECHILD, so the kill(pid, 0)/EPERM
    /// answer is unchanged from pre-T28 — both for an existing foreign pid
    /// and for a provably dead never-our-child pid.
    #[cfg(unix)]
    #[test]
    fn delegate_status_foreign_pid_keeps_probe_semantics() {
        // pid 1 exists on every unix (init/launchd) and is never our child:
        // alive via the probe's ok/EPERM leg, same answer as pre-T28.
        assert_eq!(reap_and_alive(1), Some(true));
        // Provably dead and never an unreaped child of ours: fully reaped via
        // wait() first, so the seam's waitpid says ECHILD and the plain
        // probe's ESRCH reports false.
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        assert!(child.wait().expect("reap the true child").success());
        assert_eq!(reap_and_alive(u64::from(pid)), Some(false));
    }

    /// T28 test 4. Liveness without a pid stays `unknown (no pid given)` —
    /// the reap leg must not leak into the no-pid path.
    #[test]
    fn delegate_status_without_pid_still_reports_alive_unknown() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("alive: unknown (no pid given)"),
            "{}",
            result.content
        );
    }

    #[test]
    fn delegate_log_tail_last_three_nonempty_clipped() {
        let tmp = tempfile::tempdir().unwrap();
        let log = tmp.path().join("delegate.log");
        fs::write(&log, "one\n\n".repeat(500) + "line-4\nline-5\nline-6\nline-7\n").unwrap();
        assert_eq!(read_log_tail(&log), vec!["line-5", "line-6", "line-7"]);
        let long = "y".repeat(500);
        fs::write(&log, format!("{long}\nlast\n")).unwrap();
        assert_eq!(read_log_tail(&log), vec!["y".repeat(200), "last".to_string()]);
        // Unreadable log → empty tail, never an error.
        assert!(read_log_tail(&tmp.path().join("missing.log")).is_empty());
    }

    /// Bound pin: the events log grows unboundedly, so `status` must read only
    /// the last [`DELEGATE_EVENTS_TAIL_BYTES`]. A goal line older than that
    /// window must not be reported — reading the whole file (bound removed)
    /// would see it and fail this test.
    #[test]
    fn delegate_status_reads_only_the_tail_window() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        let mut body = String::from(
            "{\"type\":\"goal\",\"ts\":\"ancient\",\"outcome\":\"accepted\",\"summary\":\"old run\"}\n",
        );
        let filler = format!(
            "{{\"type\":\"iteration\",\"ts\":\"filler\",\"n\":1,\"pad\":\"{}\"}}\n",
            "x".repeat(80)
        );
        while body.len() < DELEGATE_EVENTS_TAIL_BYTES as usize + 4096 {
            body.push_str(&filler);
        }
        body.push_str("{\"type\":\"iteration\",\"ts\":\"recent\",\"n\":9}\n");
        fs::write(tmp.path().join(".chug/events.jsonl"), body).unwrap();

        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            !result.content.contains("goal_seen: true"),
            "pre-bound goal leaked into the tail window: {}",
            result.content
        );
        assert!(result.content.contains("last_iteration: 9"), "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
    }

    /// T58 end-to-end through the status action over a synthetic two-segment
    /// events file (abort in segment 1, `run_start` + iterations in segment
    /// 2): the summary describes the LATEST segment — `state: running`,
    /// `abort_seen: false` — not the latched pre-resume abort.
    #[test]
    fn delegate_status_two_segment_events_reports_latest_segment() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
                "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
                "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
                T58_RESUME_RUN_START,
                "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("abort_seen: false"), "{}", result.content);
        assert!(!result.content.contains("abort_reason"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 1"), "{}", result.content);
    }

    /// T39: `max_tokens` on the `status` action is IGNORED, not rejected —
    /// exactly how `max_iters`/`max_minutes` behave there today (launch-only
    /// inputs that status never reads). The status payload shape is
    /// unchanged.
    #[test]
    fn delegate_status_ignores_max_tokens() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "status", "cwd": tmp.path(), "max_tokens": 250_000}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: running"), "{}", result.content);
        assert!(result.content.contains("last_iteration: 7"), "{}", result.content);
    }


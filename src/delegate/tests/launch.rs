// T109 family: launch — the launch action (T23/T39/T103).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod launch;` line fails the pin's
// reference to this const to compile. (T144 added the launch-scrub
// env leg: 14 → 15.)
pub(super) const TEST_COUNT: usize = 15;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// End-to-end with a stub binary: `CHUG_DELEGATE_BIN` points at a script
    /// that writes a synthetic `run_start`+`iteration` into `$PWD/.chug/` then
    /// sleeps. Launch returns a pid immediately; bounded polling (≤5s) then
    /// sees the summary with `alive: true`.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_stub_then_status_reports_summary_and_liveness() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        use std::os::unix::fs::PermissionsExt;
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();

        let stub = ctx_cwd.path().join("chug-stub.sh");
        fs::write(
            &stub,
            concat!(
                "#!/bin/sh\n",
                "mkdir -p .chug\n",
                "printf '%s\\n' '{\"type\":\"run_start\",\"ts\":\"stub-t0\",\"mode\":\"run\",\"model\":\"stub\",\"max_iters\":40,\"max_minutes\":35,\"max_tokens\":null}' >> .chug/events.jsonl\n",
                "printf '%s\\n' '{\"type\":\"iteration\",\"ts\":\"stub-t1\",\"n\":1,\"input_tokens\":7,\"output_tokens\":3}' >> .chug/events.jsonl\n",
                "echo stub child up\n",
                "sleep 60\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", &stub) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");

        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "stub goal",
                "model": "stub-model",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        assert!(
            launch.content.contains("max_iters: 40 max_minutes: 35"),
            "defaults not applied: {}",
            launch.content
        );
        assert!(
            launch
                .content
                .contains(&format!("log: {}", child_dir.path().join(".chug/delegate.log").display())),
            "{}",
            launch.content
        );
        assert!(
            launch
                .content
                .contains(&format!("events: {}", child_dir.path().join(".chug/events.jsonl").display())),
            "{}",
            launch.content
        );
        let pid: u32 = launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses");

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = None;
        while Instant::now() < deadline {
            let s = dispatch(
                &delegate_ctx(ctx_cwd.path()),
                "delegate",
                &json!({"action": "status", "cwd": child_dir.path(), "pid": pid}),
            );
            assert!(!s.is_error, "{}", s.content);
            if s.content.contains("last_iteration: 1") && s.content.contains("alive: true") {
                seen = Some(s);
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }
        let status = seen.expect("stub summary + liveness within 5s");
        assert!(status.content.contains("state: running"), "{}", status.content);
        assert!(status.content.contains("max_iters: 40"), "{}", status.content);
        assert!(
            status.content.contains("last_event: iteration"),
            "{}",
            status.content
        );
        assert!(
            status.content.contains("stub child up"),
            "console log tail missing: {}",
            status.content
        );

        // The tool detached and dropped the handle, so cleanup only has the pid.
        kill_pid_group(pid);

        // Explicit budgets pass through to the child untouched (the launch
        // line echoes them back; the stub would receive them as argv).
        let launch2 = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "stub goal 2",
                "model": "stub-model",
                "max_iters": 7,
                "max_minutes": 9,
            }),
        );
        assert!(!launch2.is_error, "{}", launch2.content);
        assert!(
            launch2.content.contains("max_iters: 7 max_minutes: 9"),
            "{}",
            launch2.content
        );
        let pid2: u32 = launch2
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses");
        kill_pid_group(pid2);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// Launch failure leg: a binary that does not exist must produce a tool
    /// error naming the path — never a panic, never a driver abort.
    #[test]
    fn delegate_launch_missing_binary_is_tool_error() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", "/nonexistent/chug") };
        // T103: the spec probe must PASS so the leg still tests the binary
        // error, not spec existence (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-spec.md");
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": tmp.path(),
                "spec": "/tmp/chug-spec.md",
                "goal": "g",
                "model": "m",
            }),
        );
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        assert!(result.is_error, "{}", result.content);
        assert!(
            result.content.contains("/nonexistent/chug"),
            "error must name the binary path: {}",
            result.content
        );
    }

    // ---- T103: launch payload existence probes (spec + cwd fail-fast) ----

    /// T103 req 1+2: a nonexistent spec is refused AT THE CALL SITE with the
    /// received path named verbatim (a corrupted payload is visible in the
    /// error) — and no child is spawned, no `.chug/delegate.log` is created
    /// (fail-fast in the caller, not in a child that dies at iteration 1).
    /// NON-VACUOUSNESS: pre-edit this payload reached the launch attempt
    /// (shape-only validation), so the error asserts fail.
    #[test]
    fn delegate_launch_refuses_nonexistent_spec_naming_the_path() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // Absolute (passes the pre-existing shape bail) and guaranteed
        // nonexistent: a name under a real dir that was never created. The
        // seam points at a nonexistent binary so the RED leg can never spawn
        // a real child even if the probe is dropped.
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", "/nonexistent/chug") };
        let bogus_spec = ctx_cwd.path().join("t103-no-such-spec.md");
        let result = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": bogus_spec.display().to_string(),
                "goal": "g",
                "model": "m",
            }),
        );
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        assert!(result.is_error, "{}", result.content);
        assert!(
            result
                .content
                .contains("delegate: spec does not exist or is not a readable file"),
            "refusal must carry the req-1 error: {}",
            result.content
        );
        assert!(
            result.content.contains(bogus_spec.to_str().unwrap()),
            "error must name the received path verbatim: {}",
            result.content
        );
        // Req 2: fail-fast in the caller — the child's fixed log location is
        // never created (which also means no events file, no spawn).
        assert!(
            !child_dir.path().join(".chug").exists(),
            "a refused spec must never create the child's .chug/"
        );
    }

    /// T103: a spec that exists but is a DIRECTORY is refused with the same
    /// error class. `File::open` alone would admit it (open(2) succeeds on a
    /// directory); the probe's is_file leg is what makes "readable file"
    /// honest.
    #[test]
    fn delegate_launch_refuses_a_directory_spec_with_the_same_error() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", "/nonexistent/chug") };
        let result = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": ctx_cwd.path().display().to_string(),
                "goal": "g",
                "model": "m",
            }),
        );
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        assert!(result.is_error, "{}", result.content);
        assert!(
            result
                .content
                .contains("delegate: spec does not exist or is not a readable file"),
            "directory spec must hit the same error class: {}",
            result.content
        );
        assert!(
            result.content.contains(ctx_cwd.path().to_str().unwrap()),
            "error must name the received path verbatim: {}",
            result.content
        );
        assert!(
            !child_dir.path().join(".chug").exists(),
            "a refused spec must never create the child's .chug/"
        );
    }

    /// T103: a nonexistent cwd (absolute, directory-shaped) is refused at the
    /// call site with the received path named verbatim — no spawn, no log.
    /// The existence leg itself pre-dated T103 (`delegate_cwd` already
    /// probed `is_dir`); what T103 changes is the error naming the MISSING
    /// case instead of claiming the path exists but is not a directory —
    /// that reword is the RED leg here.
    #[test]
    fn delegate_launch_refuses_nonexistent_cwd_naming_the_path() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", "/nonexistent/chug") };
        let real_spec = ctx_cwd.path().join("t103-real-spec.md");
        fs::write(&real_spec, "# t103: a real, readable spec\n").unwrap();
        let missing_cwd = ctx_cwd.path().join("t103-no-such-child-dir");
        let result = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": missing_cwd.display().to_string(),
                "spec": real_spec.display().to_string(),
                "goal": "g",
                "model": "m",
            }),
        );
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        assert!(result.is_error, "{}", result.content);
        assert!(
            result
                .content
                .contains("delegate: cwd does not exist or is not a directory"),
            "refusal must carry the req-1 cwd error: {}",
            result.content
        );
        assert!(
            result.content.contains(missing_cwd.to_str().unwrap()),
            "error must name the received path verbatim: {}",
            result.content
        );
        // The probe ran before anything touched the (nonexistent) child dir.
        assert!(
            !missing_cwd.exists(),
            "a refused cwd must never be created by the launch path"
        );
    }

    /// T103: a BOTH-valid payload (real, readable spec file; real cwd) still
    /// launches — the probe passes it through and the child argv carries the
    /// spec path verbatim. Guards against over-rejection of well-formed
    /// payloads. (Regression pin, not a RED leg: it is green both before and
    /// after the probe by construction.)
    #[cfg(unix)]
    #[test]
    fn delegate_launch_with_both_payloads_valid_still_spawns() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        let spec = ctx_cwd.path().join("t103-both-valid-spec.md");
        fs::write(&spec, "# t103: both-valid launch payload\n").unwrap();
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": spec.display().to_string(),
                "goal": "g",
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        assert!(
            launch.content.contains("launched: pid "),
            "both-valid launch must spawn: {}",
            launch.content
        );
        let argv = wait_for_argv_dump(child_dir.path());
        assert!(
            argv.windows(2).any(|w| w[0] == "--spec" && w[1] == spec.to_str().unwrap()),
            "spec must reach the child verbatim: {:?}",
            argv
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39 spec test: launch with `max_tokens: 250000` → the child's argv
    /// contains `--max-tokens 250000`, asserted against the argv seam the
    /// other delegate tests use (`CHUG_DELEGATE_BIN`; the stub dumps its
    /// argv). NON-VACUOUSNESS: dropping the argv append leaves the dump
    /// without the flag and fails here.
    #[test]
    fn delegate_launch_with_max_tokens_appends_flag_to_child_argv() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "max_tokens": 250_000,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "250000", "{}", argv.join(" | "));
        // The return text names the configured token budget alongside the
        // existing budgets (the iters/minutes echo pattern).
        assert!(
            launch
                .content
                .contains("max_iters: 40 max_minutes: 35 max_tokens: 250000"),
            "{}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39 byte-identical-argv control: WITHOUT `max_tokens`, the child argv
    /// carries no `--max-tokens` and is exactly the pre-T39 list — children
    /// keep their current no-token-ceiling behavior unless the orchestrator
    /// opts in.
    #[test]
    fn delegate_launch_without_max_tokens_keeps_argv_byte_identical() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        assert_eq!(
            argv,
            vec![
                "run".to_string(),
                "--spec".to_string(),
                "/tmp/chug-stub-spec.md".to_string(),
                "--goal".to_string(),
                "g".to_string(),
                "--model".to_string(),
                "m".to_string(),
                "--max-iters".to_string(),
                "40".to_string(),
                "--max-minutes".to_string(),
                "35".to_string(),
            ],
            "argv must be byte-identical to pre-T39 (no --max-tokens)"
        );
        // The return text is unchanged too: no token-budget echo when absent.
        assert!(
            !launch.content.contains("max_tokens"),
            "absent max_tokens must not appear in the launch text: {}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39 boundary: `max_tokens: 1` is accepted and reaches the child
    /// verbatim (`--max-tokens 1`) — the floor is inclusive.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_boundary_max_tokens_one_reaches_child() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "max_tokens": 1,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "1", "{}", argv.join(" | "));
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T39: `max_tokens: 0` and `max_tokens: -5` are tool errors naming the
    /// constraint — and no child is spawned (the stub's argv dump never
    /// appears in the child dir). NON-VACUOUSNESS: accepting 0 (e.g. by
    /// parsing with `as_u64` like `max_iters` does) fails the error asserts.
    #[test]
    fn delegate_launch_rejects_zero_and_negative_max_tokens_without_spawning() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");
        for bad in [json!(0), json!(-5)] {
            let result = dispatch(
                &delegate_ctx(ctx_cwd.path()),
                "delegate",
                &json!({
                    "action": "launch",
                    "cwd": child_dir.path(),
                    "spec": "/tmp/chug-stub-spec.md",
                    "goal": "g",
                    "model": "m",
                    "max_tokens": bad,
                }),
            );
            assert!(result.is_error, "{bad}: {}", result.content);
            assert!(
                result.content.contains("at least 1"),
                "{bad} must name the constraint: {}",
                result.content
            );
        }
        // No spawn: the stub never ran, so its argv dump does not exist.
        assert!(
            !child_dir.path().join("argv.txt").exists(),
            "a rejected max_tokens must never spawn the child"
        );
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T58 end-to-end: launch with `resume: true` → the child's argv ends
    /// with `--resume` (after any `--max-tokens` pair), and the return text
    /// names the resume leg (`resume: true`, T39 echo pattern). NON-VACUOUSNESS:
    /// dropping the argv append fails the dump assert; dropping the echo fails
    /// the content assert.
    #[test]
    fn delegate_launch_with_resume_appends_flag_to_child_argv() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        // T103: the spec probe needs a real file (see ensure_spec_file).
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "g",
                "model": "m",
                "max_tokens": 250_000,
                "resume": true,
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let argv = wait_for_argv_dump(child_dir.path());
        assert_eq!(
            argv.last().map(String::as_str),
            Some("--resume"),
            "--resume must be the child argv's last argument: {}",
            argv.join(" | ")
        );
        let pos = argv
            .iter()
            .position(|a| a == "--max-tokens")
            .expect("--max-tokens in the child argv");
        assert_eq!(argv[pos + 1], "250000", "{}", argv.join(" | "));
        // The return text names the resume leg alongside the other budgets.
        assert!(
            launch
                .content
                .contains("max_iters: 40 max_minutes: 35 max_tokens: 250000 resume: true"),
            "{}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }


    // ---- T115: the launch result's goal-integrity echoes ----

    /// T115 long-goal leg: the launch result reports the goal's integrity
    /// surface over the exact goal string passed to the child argv —
    /// `goal_bytes` (UTF-8 byte length), `goal_sha256` (pinned against the
    /// external `shasum -a 256` vector for the identical string), and a
    /// tail-anchored ≤120-char `goal_tail`. A 216-byte goal's tail is the
    /// LAST 120 chars: the marker at the very end is what the preview ends
    /// with, and a head window would drop it entirely — cycle-60's
    /// duplicated-TAIL composition garble lives exactly at the end.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_reports_goal_bytes_sha256_and_tail_anchored_tail() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let goal = format!("{}TAIL-MARKER-t115", "x".repeat(200));
        assert_eq!(goal.len(), 216, "fixture goal is 216 bytes");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": goal,
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        // Byte length over UTF-8 bytes (ASCII fixture: bytes == chars).
        assert!(
            launch.content.contains(&format!("goal_bytes: {}", goal.len())),
            "{}",
            launch.content
        );
        // The sha is pinned against the external vector (`shasum -a 256` of
        // the identical byte string) AND equal to the child-side helper —
        // the two ends the orchestrator compares against each other.
        assert!(
            launch.content.contains(
                "goal_sha256: 39a6464943ca682e911300e68501593ea6259d3c4a736ff3e9ae0c33f70d6de9"
            ),
            "{}",
            launch.content
        );
        assert_eq!(
            crate::eventlog::goal_sha256(&goal),
            "39a6464943ca682e911300e68501593ea6259d3c4a736ff3e9ae0c33f70d6de9"
        );
        // Tail-anchored: the LAST 120 chars (head bytes dropped), ≤120 chars,
        // ending with the goal's final marker — the duplicated-tail class is
        // visible in exactly this window.
        let expected_tail: String = goal.chars().skip(goal.chars().count() - 120).collect();
        assert_eq!(expected_tail.chars().count(), 120);
        assert!(
            launch
                .content
                .contains(&format!("goal_tail: {expected_tail}\n")),
            "{}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T115 short-goal leg: a goal ≤120 chars renders `goal_tail` VERBATIM —
    /// the whole goal, byte-identical (a short goal is its own tail) — with
    /// the byte count and the `shasum -a 256`-pinned sha alongside.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_goal_tail_is_verbatim_for_a_short_goal() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let goal = "t115 short goal";
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": goal,
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        assert!(launch.content.contains("goal_bytes: 15"), "{}", launch.content);
        assert!(
            launch.content.contains(
                "goal_sha256: ee9783704a895564c7bd05d69710bc684a4c288cc650ef2d71a3bfb699377220"
            ),
            "{}",
            launch.content
        );
        assert!(
            launch.content.contains("goal_tail: t115 short goal\n"),
            "short goal's tail must be the whole goal verbatim: {}",
            launch.content
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }

    /// T119 multibyte leg (the cycle-61 surviving byte-slicing mutant):
    /// every pre-T119 fixture is ASCII, so a mutant swapping
    /// `tail_preview`'s chars()-based window for a byte slice
    /// (`&goal[goal.len()-120..]`) was invisible to the suite. This
    /// fixture crosses a multibyte boundary past the 120-char cut: the
    /// last 120 CHARS hold 2-byte (`é`, U+00E9) and 4-byte (`🌍`, U+1F30D)
    /// scalars, and byte offset `len-120` = 81 lands MID-`é` (bytes
    /// 80..82), so the byte-slicing mutant PANICS instead of rendering.
    /// Bytes-vs-chars is pinned in BOTH directions on this one fixture:
    /// `goal_tail` is exactly the last ≤120 CHARS (equality against an
    /// independently chars()-computed window — no panic, no replacement
    /// char, no split sequence) while `goal_bytes` stays the UTF-8 BYTE
    /// length (201 ≠ 191 chars), and `goal_sha256` is the external
    /// `shasum -a 256` vector over the fixture's UTF-8 bytes (a
    /// chars-hashed mutant renders a different digest).
    #[cfg(unix)]
    #[test]
    fn delegate_launch_multibyte_goal_tail_stays_on_char_boundaries() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", write_argv_stub(ctx_cwd.path())) };
        ensure_spec_file("/tmp/chug-stub-spec.md");
        let goal = format!("{}é{}{}", "x".repeat(80), "🌍".repeat(3), "y".repeat(107));
        // The fixture self-pins both counts: 191 chars ≠ 201 UTF-8 bytes,
        // so a bytes-vs-chars swap breaks one assert or the other.
        assert_eq!(goal.chars().count(), 191);
        assert_eq!(goal.len(), 201);
        // The byte cut (len-120 = 81) is NOT a char boundary — it sits
        // inside the é (bytes 80..82) — the byte-slicing mutant's slice
        // panics on exactly this fixture.
        assert!(!goal.is_char_boundary(81), "fixture must split a char at the byte cut");
        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": goal,
                "model": "m",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        // Bytes direction: `goal_bytes` is the UTF-8 BYTE length (201),
        // never the chars count (191).
        assert!(
            launch.content.contains("goal_bytes: 201"),
            "{}",
            launch.content
        );
        // The sha is over the fixture's UTF-8 bytes — the external
        // `shasum -a 256` vector for the identical byte string.
        assert!(
            launch.content.contains(
                "goal_sha256: b6e9b17b107090b84e0f83b36cae73aac95b9eb56b5d5f3d920d088bb9f07c91"
            ),
            "{}",
            launch.content
        );
        assert_eq!(
            crate::eventlog::goal_sha256(&goal),
            "b6e9b17b107090b84e0f83b36cae73aac95b9eb56b5d5f3d920d088bb9f07c91"
        );
        // Chars direction: the tail is EXACTLY the last 120 CHARS — the
        // independently computed window (x…é🌍🌍🌍y…), char-boundary-safe.
        // Byte slicing panics on this fixture (the cut is mid-é); any
        // boundary-safe byte window renders fewer than 120 chars and fails
        // this equality.
        let expected_tail: String = goal.chars().skip(goal.chars().count() - 120).collect();
        assert_eq!(expected_tail.chars().count(), 120);
        let actual_tail = launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("goal_tail: "))
            .expect("goal_tail line");
        assert_eq!(actual_tail, expected_tail, "{}", launch.content);
        assert!(
            !actual_tail.contains('\u{FFFD}'),
            "char-boundary-safe: no replacement char: {actual_tail}"
        );
        let pid = spawn_pid_of(&launch);
        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
    }


    /// T144: the delegate launch scrub. The child chug binary must NOT
    /// inherit `CARGO_TARGET_DIR`/`CARGO_BUILD_TARGET_DIR` from the driver
    /// process env — loopd.sh hands every orchestrator the shared cache as a
    /// per-invocation prefix, and that env is exactly what reaches the child
    /// through this spawn. Without the scrub, a delegate child that forgets
    /// its goal-carried `export CARGO_TARGET_DIR=<role-keyed>` builds into
    /// the shared dir and collides with other checkouts (last-builder-wins;
    /// the same class the check harness gets, one level down). Both
    /// spellings must arrive unset; an in-command `export`/prefix inside the
    /// child's own goal text is unaffected (it is not inherited, it is set
    /// by the child's shell after spawn).
    #[cfg(unix)]
    #[test]
    fn delegate_launch_child_does_not_inherit_target_dir_vars() {
        // T151: hold the shared timing domain FIRST (before the env lock —
        // see crate::testsupport's lock-order rule) across the spawn → poll →
        // cleanup body: this leg's outcome rides a child-spawn deadline.
        let _timing = crate::testsupport::timing_guard();

        use std::os::unix::fs::PermissionsExt;
        let _guard = DELEGATE_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let child_dir = tempfile::tempdir().unwrap();
        let ctx_cwd = tempfile::tempdir().unwrap();

        // The stub records both spellings of the var exactly as its exec'd
        // environment presents them (same `${VAR:-UNSET}` probe shape as the
        // run_shell legs), then sleeps so the pid-group kill cleans it up.
        let stub = ctx_cwd.path().join("chug-env-stub.sh");
        fs::write(
            &stub,
            concat!(
                "#!/bin/sh\n",
                "printf 'ctd=%s\\ncbtd=%s\\n' \"${CARGO_TARGET_DIR:-UNSET}\" \"${CARGO_BUILD_TARGET_DIR:-UNSET}\" > env.txt\n",
                "sleep 60\n",
            ),
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: serialized by DELEGATE_ENV_LOCK; no other test reads this var.
        unsafe { std::env::set_var("CHUG_DELEGATE_BIN", &stub) };
        ensure_spec_file("/tmp/chug-stub-spec.md");
        // Seed BOTH spellings the way loopd.sh's per-invocation prefix does —
        // the driver process env is what the launch inherits from.
        // SAFETY: serialized by DELEGATE_ENV_LOCK; both restored before return.
        let saved_target = std::env::var_os("CARGO_TARGET_DIR");
        let saved_alias = std::env::var_os("CARGO_BUILD_TARGET_DIR");
        unsafe {
            std::env::set_var("CARGO_TARGET_DIR", "/tmp/t144-foreign-shared-target");
            std::env::set_var("CARGO_BUILD_TARGET_DIR", "/tmp/t144-foreign-alias-target");
        }

        let launch = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({
                "action": "launch",
                "cwd": child_dir.path(),
                "spec": "/tmp/chug-stub-spec.md",
                "goal": "t144 stub goal",
                "model": "stub-model",
            }),
        );
        assert!(!launch.is_error, "{}", launch.content);
        let pid: u32 = launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses");

        // The stub's env dump, polled like argv.txt (launch returns at
        // spawn; `cbtd=` guards the tail so a partial write re-polls).
        let env_path = child_dir.path().join("env.txt");
        let deadline = Instant::now() + Duration::from_secs(5);
        let dump = loop {
            if let Ok(text) = fs::read_to_string(&env_path)
                && text.contains("cbtd=")
            {
                break text;
            }
            assert!(
                Instant::now() < deadline,
                "stub never wrote {}",
                env_path.display()
            );
            thread::sleep(Duration::from_millis(25));
        };

        kill_pid_group(pid);
        // SAFETY: serialized by DELEGATE_ENV_LOCK; restore the seam and both
        // process values (a panic above must not leak the seed).
        unsafe { std::env::remove_var("CHUG_DELEGATE_BIN") };
        match saved_target {
            Some(v) => unsafe { std::env::set_var("CARGO_TARGET_DIR", v) },
            None => unsafe { std::env::remove_var("CARGO_TARGET_DIR") },
        }
        match saved_alias {
            Some(v) => unsafe { std::env::set_var("CARGO_BUILD_TARGET_DIR", v) },
            None => unsafe { std::env::remove_var("CARGO_BUILD_TARGET_DIR") },
        }
        assert_eq!(
            dump, "ctd=UNSET\ncbtd=UNSET\n",
            "delegate child must not inherit either target-dir spelling"
        );
    }

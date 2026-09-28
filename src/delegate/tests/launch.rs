// T109 family: launch — the launch action (T23/T39/T103).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod launch;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 11;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// End-to-end with a stub binary: `CHUG_DELEGATE_BIN` points at a script
    /// that writes a synthetic `run_start`+`iteration` into `$PWD/.chug/` then
    /// sleeps. Launch returns a pid immediately; bounded polling (≤5s) then
    /// sees the summary with `alive: true`.
    #[cfg(unix)]
    #[test]
    fn delegate_launch_stub_then_status_reports_summary_and_liveness() {
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


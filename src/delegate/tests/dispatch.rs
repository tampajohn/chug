// T109 family: dispatch — tool-boundary validation: cwd/paths/action/probe semantics (T23).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod dispatch;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 4;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// The deliberate `resolve_safe` exemption: the child worktree lives
    /// outside this process's sandbox (in /tmp by design) and `status` must
    /// work on it anyway.
    #[test]
    fn delegate_paths_may_lie_outside_the_orchestrator_cwd() {
        let ctx_cwd = tempfile::tempdir().unwrap();
        let child_dir = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(ctx_cwd.path()),
            "delegate",
            &json!({"action": "status", "cwd": child_dir.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("state: starting"), "{}", result.content);
    }

    #[test]
    fn delegate_rejects_relative_cwd_and_nonexistent_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = delegate_ctx(tmp.path());
        let result = dispatch(&ctx, "delegate", &json!({"action": "status", "cwd": "relative/child"}));
        assert!(result.is_error);
        assert!(result.content.contains("absolute directory"), "{}", result.content);
        let result = dispatch(
            &ctx,
            "delegate",
            &json!({"action": "status", "cwd": tmp.path().join("nope")}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("not a directory"), "{}", result.content);
    }

    #[test]
    fn delegate_unknown_action_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "harvest", "cwd": tmp.path()}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("unknown action"), "{}", result.content);
        // T69: the error names ALL THREE actions verbatim, so a caller
        // reading the error learns the full surface at the moment of need.
        assert!(result.content.contains("\"launch\""), "{}", result.content);
        assert!(result.content.contains("\"status\""), "{}", result.content);
        assert!(result.content.contains("\"collect\""), "{}", result.content);
    }

    #[test]
    fn delegate_alive_probe_true_for_own_pid_false_for_reaped_exit() {
        assert_eq!(process_alive(u64::from(std::process::id())), Some(true));
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        assert!(child.wait().unwrap().success());
        assert_eq!(process_alive(u64::from(pid)), Some(false));
    }


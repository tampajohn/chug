// T109 family: argv — child argv builder pins (T39).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod argv;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 2;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    // ---- T39: delegate launch optional max_tokens passthrough ----

    /// T39/T58: the pure argv builder — with a token budget the flag pair is
    /// appended at the tail (after `--max-minutes`); without one the argv is
    /// byte-identical to pre-T39 (also pre-T58: `resume` false), pinned as
    /// the whole list.
    #[test]
    fn delegate_child_argv_appends_max_tokens_only_when_present() {
        let spec = PathBuf::from("/tmp/spec.md");
        let base = vec![
            OsString::from("run"),
            OsString::from("--spec"),
            OsString::from("/tmp/spec.md"),
            OsString::from("--goal"),
            OsString::from("g"),
            OsString::from("--model"),
            OsString::from("m"),
            OsString::from("--max-iters"),
            OsString::from("40"),
            OsString::from("--max-minutes"),
            OsString::from("35"),
        ];
        // Absent: byte-identical to pre-T39/pre-T58 — no `--max-tokens`
        // anywhere, no `--resume`.
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, false),
            base,
            "absent max_tokens must not change the child argv"
        );
        // T15 parity: the flag pair lands at the tail, verbatim.
        let mut with = base.clone();
        with.push(OsString::from("--max-tokens"));
        with.push(OsString::from("250000"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(250_000), false),
            with
        );
        // Boundary: 1 is accepted and passes through verbatim.
        let mut one = base;
        one.push(OsString::from("--max-tokens"));
        one.push(OsString::from("1"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(1), false),
            one
        );
    }

    /// T58: the pure argv builder with `resume` — `true` appends the bare
    /// `--resume` flag as the very last argument (after any `--max-tokens`
    /// pair), purely additive: spec/goal/model and the budget flags pass
    /// through exactly as today. Absent and `false` both produce the pre-T58
    /// argv byte-for-byte (whole-list pin, T39 style) — children keep their
    /// fresh-start behavior unless the orchestrator opts in.
    #[test]
    fn delegate_child_argv_resume_appends_flag_last_absent_false_byte_identical() {
        let spec = PathBuf::from("/tmp/spec.md");
        let base = vec![
            OsString::from("run"),
            OsString::from("--spec"),
            OsString::from("/tmp/spec.md"),
            OsString::from("--goal"),
            OsString::from("g"),
            OsString::from("--model"),
            OsString::from("m"),
            OsString::from("--max-iters"),
            OsString::from("40"),
            OsString::from("--max-minutes"),
            OsString::from("35"),
        ];
        // Absent (parsed to false upstream) == false == pre-T58, whole list.
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, false),
            base,
            "resume=false must not change the child argv"
        );
        // With resume: `--resume` is the last argument, nothing else moves.
        let mut resumed = base.clone();
        resumed.push(OsString::from("--resume"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, None, true),
            resumed,
            "--resume must be appended after all other flags"
        );
        // Resume composes with the token budget: still the very tail.
        let mut tokens_then_resume = base;
        tokens_then_resume.push(OsString::from("--max-tokens"));
        tokens_then_resume.push(OsString::from("250000"));
        tokens_then_resume.push(OsString::from("--resume"));
        assert_eq!(
            delegate_child_argv(&spec, "g", "m", 40, 35, Some(250_000), true),
            tokens_then_resume,
            "--resume must stay last even after the --max-tokens pair"
        );
    }


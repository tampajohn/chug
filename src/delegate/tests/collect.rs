// T109 family: collect — the collect action (T69/T103).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod collect;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 18;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// Manual smoke, permanent: `collect` against the REAL checkout the test
    /// runs in — a live git repo with real commits (and, while this loop
    /// itself runs, a live `.chug/events.jsonl`) — returns the verdict plus
    /// the resolved commit-refs block in one bounded non-blocking call (the
    /// build_info T20 integration-pin pattern).
    #[test]
    fn delegate_collect_against_the_real_checkout_resolves_commit_refs() {
        let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
        let result = dispatch(
            &delegate_ctx(&root),
            "delegate",
            &json!({"action": "collect", "cwd": root}),
        );
        assert!(!result.is_error, "{}", result.content);
        // This checkout's live `.chug/events.jsonl` can be in ANY run state
        // when the suite runs — mid-run `running`, a clean checkout with no
        // events `starting`, or a FINISHED run's `goal-accepted` /
        // `goal-rejected` / `aborted` (the t69 child's own goal-accepted
        // stream red-fired a running|starting-only pin at the orchestrator's
        // review gates) — so pin the verdict LINE's shape (one of the five
        // known verdicts), never the state. Either way the git leg resolves
        // the REAL checkout's commits.
        let verdict = result
            .content
            .lines()
            .find_map(|l| l.strip_prefix("verdict: "))
            .expect("verdict line present");
        assert!(
            matches!(
                verdict,
                "goal-accepted" | "goal-rejected" | "aborted" | "running" | "starting"
            ),
            "{}",
            result.content
        );
        assert!(
            result.content.contains("commits (range HEAD, up to 20):"),
            "{}",
            result.content
        );
        // Real commit lines: `<7+ hex sha> <subject>`, newest first.
        let commit_lines: Vec<&str> = result
            .content
            .lines()
            .filter(|l| l.starts_with("  ") && !l.trim().is_empty())
            .collect();
        assert!(!commit_lines.is_empty(), "{}", result.content);
        let first = commit_lines[0].trim();
        let sha = first.split(' ').next().unwrap_or_default();
        assert!(sha.len() >= 7 && sha.chars().all(|c| c.is_ascii_hexdigit()), "{first}");
    }

    // ---- T69: the `collect` action — structured child result ----

    /// A `goal` event line with the given outcome and payload field.
    fn goal_line(outcome: &str, field: &str, text: &str) -> String {
        format!("{{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"{outcome}\",\"{field}\":\"{text}\"}}")
    }

    #[test]
    fn delegate_collect_parse_accepted_yields_verdict_summary_and_cmd() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":4}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
            &goal_line("accepted", "summary", "T69 done: collect shipped, gates green"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(
            s.goal_summary.as_deref(),
            Some("T69 done: collect shipped, gates green")
        );
        assert_eq!(s.check_cmd.as_deref(), Some("cargo test --bin chug delegate"));
        assert_eq!(s.abort_reason, None);
    }

    /// The accepted summary is the FULL text — multi-line summaries survive
    /// verbatim (the child's own account, not a one-line clip).
    #[test]
    fn delegate_collect_parse_keeps_multiline_summary_verbatim() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"line one\\nline two\"}",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(s.goal_summary.as_deref(), Some("line one\nline two"));
    }

    /// A rejected verdict latches `goal-rejected` — and a LATER accepted
    /// verdict in the SAME segment flips it (the child's loop continues
    /// after a rejection).
    #[test]
    fn delegate_collect_parse_rejected_then_later_accepted_flips() {
        let rejected_only = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            &goal_line("rejected", "reason", "check failed: 3 tests red"),
        ];
        let s = summarize_collect(&rejected_only);
        assert_eq!(s.verdict(), "goal-rejected");
        assert_eq!(s.goal_summary, None, "no summary on a rejected verdict");
        assert_eq!(s.abort_reason, None);

        // The flip: acceptance after rejection, same segment.
        let start: &str = "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}";
        let flipped = [
            start,
            rejected_only[1],
            &goal_line("accepted", "summary", "fixed and green"),
        ];
        let s2 = summarize_collect(&flipped);
        assert_eq!(s2.verdict(), "goal-accepted");
        assert_eq!(s2.goal_summary.as_deref(), Some("fixed and green"));
    }

    /// An `abort` line latches `aborted` and carries the reason.
    #[test]
    fn delegate_collect_parse_abort_carries_reason() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "aborted");
        assert_eq!(s.abort_reason.as_deref(), Some("iteration budget exceeded"));
        assert_eq!(s.goal_summary, None);
    }

    /// Events without a verdict → `running`; nothing read at all →
    /// `starting`.
    #[test]
    fn delegate_collect_parse_running_vs_starting() {
        let mid_run = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
        ];
        let s = summarize_collect(&mid_run);
        assert_eq!(s.verdict(), "running");
        assert_eq!(s.check_cmd, None);

        let empty = summarize_collect(&[]);
        assert_eq!(empty.verdict(), "starting");
        // A torn-only stream parses to nothing → still `starting`.
        let torn = ["{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"trunc"];
        assert_eq!(summarize_collect(&torn).verdict(), "starting");
    }

    /// T58's segment reset, collect-side: a pre-resume `abort` must not
    /// outlive the resume's `run_start` — post-resume acceptance reports
    /// `goal-accepted`, and the segment's check cmd is the RESUMED segment's.
    /// The verdict-latch reset has its own tooth (finding-2 pin): a resume
    /// that has NOT reached a verdict yet must render `running`, not leak
    /// segment 1's verdict or summary — a post-resume-accepted leg alone
    /// would overwrite the latch and leave the reset vacuous (the
    /// mutant-kill leg: deleting the `run_start` latch reset turns this red).
    #[test]
    fn delegate_collect_parse_run_start_resets_verdict_and_check() {
        let lines = [
            // Segment 1: gated by check A, then died.
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"check A\"}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
            // Segment 2 (the resume): fresh gate, fresh verdict.
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t4\",\"cmd\":\"check B\"}",
            &goal_line("accepted", "summary", "resumed run passed"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted", "pre-resume abort must not outlive the resume");
        assert_eq!(s.abort_reason, None, "pre-resume abort reason must reset");
        assert_eq!(s.check_cmd.as_deref(), Some("check B"), "the segment's LATEST gate wins");
        assert_eq!(s.goal_summary.as_deref(), Some("resumed run passed"));

        // Post-resume-NO-verdict leg: segment 1 reached an ACCEPTED verdict
        // (with its summary), then the resume's segment holds only ordinary
        // tool events — no goal, no abort. The pre-resume verdict and
        // summary must NOT leak: the verdict latch resets to `running`.
        let resumed_mid_run = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"check A\"}",
            &goal_line("accepted", "summary", "pre-resume summary must not leak"),
            T58_RESUME_RUN_START,
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
        ];
        let s2 = summarize_collect(&resumed_mid_run);
        assert_eq!(
            s2.verdict(),
            "running",
            "a pre-resume verdict must not leak into a verdict-less resume"
        );
        assert_eq!(
            s2.goal_summary, None,
            "a pre-resume summary must not leak either"
        );
        assert_eq!(
            s2.check_cmd, None,
            "a pre-resume check cmd must not leak into a verdict-less resume"
        );
    }

    /// Torn/malformed lines are skipped, never fatal — collecting is safe at
    /// ANY child lifecycle moment (mid-write, mid-run, post-cleanup). A
    /// complete verdict line still counts; a torn NEXT write contributes
    /// nothing and changes no verdict.
    #[test]
    fn delegate_collect_parse_torn_lines_skipped_not_fatal() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            &goal_line("accepted", "summary", "done"),
            // Torn final write (partial JSON) and non-JSON noise: skipped.
            "{\"type\":\"iteration\",\"ts\":\"t3\",\"n\":8,\"trunc",
            "not json at all",
            "",
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.verdict(), "goal-accepted");
        assert_eq!(s.goal_summary.as_deref(), Some("done"));
        // A torn VERDICT line must not produce a phantom verdict either —
        // the segment stays `running` on its earlier complete lines.
        let torn_verdict = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"do",
        ];
        let s2 = summarize_collect(&torn_verdict);
        assert_eq!(s2.verdict(), "running");
        assert_eq!(s2.goal_summary, None);
    }

    /// Multiple `verifying` lines in one segment: the LATEST cmd wins (the
    /// last one is the gate that actually decided the verdict).
    #[test]
    fn delegate_collect_parse_latest_verifying_cmd_wins() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"first gate\"}",
            "{\"type\":\"verifying\",\"ts\":\"t2\",\"cmd\":\"second gate\"}",
            "{\"type\":\"verifying\",\"ts\":\"t3\",\"cmd\":\"final gate\"}",
            &goal_line("accepted", "summary", "s"),
        ];
        let s = summarize_collect(&lines);
        assert_eq!(s.check_cmd.as_deref(), Some("final gate"));
    }

    /// The full dispatch render over a synthetic accepted child: the
    /// four-field result (verdict + summary + check cmd + commits block) plus
    /// the pid liveness line, in one bounded non-blocking call.
    #[test]
    fn delegate_collect_dispatch_renders_full_accepted_result() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
                "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
                "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"T69: collect shipped\"}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({
                "action": "collect",
                "cwd": tmp.path(),
                "pid": std::process::id(),
            }),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: goal-accepted"), "{}", result.content);
        assert!(result.content.contains("alive: true"), "{}", result.content);
        assert!(result.content.contains("summary: T69: collect shipped"), "{}", result.content);
        assert!(
            result.content.contains("check_cmd: cargo test --bin chug delegate"),
            "{}",
            result.content
        );
        // Not-a-repo tempdir: the git leg DEGRADES to a note, never an error.
        assert!(
            result.content.contains("commits: (unavailable:"),
            "{}",
            result.content
        );
        assert!(!result.content.contains("abort_reason"), "{}", result.content);
    }

    /// No pid → NO liveness line at all (collect's contract, unlike
    /// status's always-rendered `alive: unknown`).
    #[test]
    fn delegate_collect_without_pid_renders_no_liveness_claim() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: starting"), "{}", result.content);
        assert!(
            result.content.contains("events: nothing read"),
            "a missing events log degrades to the note: {}",
            result.content
        );
        assert!(!result.content.contains("alive:"), "{}", result.content);
        assert!(
            result.content.contains("commits: (unavailable:"),
            "not-a-repo git leg degrades to a note: {}",
            result.content
        );
    }

    /// The abort render path END-TO-END at dispatch level (finding-1 pin):
    /// a stream whose latest segment ends in an `abort` line carrying a
    /// reason renders BOTH the `aborted` verdict AND the `abort_reason:`
    /// line — the bail story is the failure surface the caller greps, and
    /// without the render leg the parse pin alone leaves the render block
    /// dead per the suite (the mutant-kill leg: deleting the render block
    /// turns this red).
    #[test]
    fn delegate_collect_dispatch_renders_aborted_verdict_and_reason() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(
            tmp.path(),
            &[
                T29_RUN_START,
                T29_ITERATION,
                "{\"type\":\"verifying\",\"ts\":\"t1\",\"cmd\":\"cargo test --bin chug delegate\"}",
                "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":50}",
            ],
        );
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: aborted"), "{}", result.content);
        assert!(
            result.content.contains("abort_reason: iteration budget exceeded"),
            "{}",
            result.content
        );
        // The gate that ran before the bail is still the segment's check cmd.
        assert!(
            result.content.contains("check_cmd: cargo test --bin chug delegate"),
            "{}",
            result.content
        );
        // An aborted segment carries no accepted-goal summary.
        assert!(!result.content.contains("summary:"), "{}", result.content);
    }

    /// Mid-run child: events exist without a verdict → `running`, without
    /// blocking (the call returns immediately; a verdict would say so).
    #[test]
    fn delegate_collect_mid_run_reports_running() {
        let tmp = tempfile::tempdir().unwrap();
        write_events_fixture(tmp.path(), &[T29_RUN_START, T29_ITERATION]);
        let started = Instant::now();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path()}),
        );
        // Non-blocking by contract: collect has no wait path — it reads the
        // events file and one best-effort git ref and returns. The cap is
        // deliberately 30s, not 5s: a blocking collect (a wait_secs-style
        // poll) would exceed it by far, while a loaded runner — full-suite
        // parallelism plus shared-target-dir build contention — can stall a
        // process spawn past 5s without any blocking (observed 5.63s in the
        // T183 gate run; the assertion caught a scheduler stall, not a
        // contract break). The contract is "returns without waiting on the
        // child", not "returns in <5s on an idle machine".
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "{:?}",
            started.elapsed()
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("verdict: running"), "{}", result.content);
        assert!(!result.content.contains("summary:"), "{}", result.content);
    }

    /// Req 4: `wait_secs` with `collect` is a tool error naming that the wait
    /// knob is status-only (launch-leg parity — naming beats silently
    /// ignoring).
    #[test]
    fn delegate_collect_rejects_wait_secs_as_status_only() {
        let tmp = tempfile::tempdir().unwrap();
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "wait_secs": 5}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("status action only"), "{}", result.content);
        assert!(result.content.contains("collect"), "{}", result.content);
    }

    /// Req 3: a non-string `base` is a tool error naming the constraint —
    /// never a silent ignore (T39/T58 parse precedent).
    #[test]
    fn delegate_collect_rejects_non_string_base() {
        let tmp = tempfile::tempdir().unwrap();
        for bad in [json!(7), json!(true), json!(["origin/main"])] {
            let result = dispatch(
                &delegate_ctx(tmp.path()),
                "delegate",
                &json!({"action": "collect", "cwd": tmp.path(), "base": bad}),
            );
            assert!(result.is_error, "{bad}: {}", result.content);
            assert!(result.content.contains("must be a string git ref"), "{bad}: {}", result.content);
        }
    }

    /// A real temp repo: commit refs render, the `base` range is honored,
    /// an unresolvable ref degrades to a note, and an empty self-range says
    /// so — every leg `is_error: false`.
    #[test]
    fn delegate_collect_git_legs_refs_base_and_degrades() {
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for the integration pin");
            assert!(
                out.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "first"]);
        let first = git(&["rev-parse", "--short", "HEAD"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "second"]);
        let second = git(&["rev-parse", "--short", "HEAD"]);
        assert_ne!(first, second, "two distinct commits");

        // Default range: BOTH commits, newest first, each as `sha msg`.
        let refs = collect_git_commits(tmp.path(), None).expect("refs resolve in a real repo");
        assert_eq!(refs.len(), 2, "{refs:?}");
        assert!(refs[0].starts_with(&second) && refs[0].contains("second"), "{refs:?}");
        assert!(refs[1].starts_with(&first) && refs[1].contains("first"), "{refs:?}");

        // `base` scopes the range: first..HEAD holds ONLY the second commit.
        let scoped = collect_git_commits(tmp.path(), Some(&first)).expect("base range resolves");
        assert_eq!(scoped.len(), 1, "{scoped:?}");
        assert!(scoped[0].starts_with(&second), "{scoped:?}");

        // A self-range is empty: git exits 0 with no commits in range.
        let none = collect_git_commits(tmp.path(), Some("HEAD")).expect("self-range is empty");
        assert!(none.is_empty(), "{none:?}");

        // Unresolvable base ref: degrade note, never an error.
        let err = collect_git_commits(tmp.path(), Some("definitely-not-a-ref-xyz"))
            .expect_err("bad ref degrades");
        assert!(err.contains("definitely-not-a-ref-xyz"), "{err}");

        // Not a repo: degrade note, never an error, never a panic.
        let bare = tempfile::tempdir().unwrap();
        let err = collect_git_commits(bare.path(), None).expect_err("not a repo degrades");
        assert!(err.contains("not a git repository"), "{err}");
    }

    /// The dispatch-level git legs end-to-end: `base` scopes the rendered
    /// block, the range header names the queried range, and every degrade
    /// keeps `is_error: false`.
    #[test]
    fn delegate_collect_dispatch_renders_commit_refs_with_base() {
        let tmp = tempfile::tempdir().unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(tmp.path())
                .output()
                .expect("git available for the integration pin");
            assert!(out.status.success(), "git {args:?} failed");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        git(&["init", "-q"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "first"]);
        let first = git(&["rev-parse", "--short", "HEAD"]);
        git(&["-c", "user.email=t@t", "-c", "user.name=t", "commit", "--allow-empty", "-qm", "second"]);
        let second = git(&["rev-parse", "--short", "HEAD"]);

        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": first}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains(&format!("commits (range {first}..HEAD, up to 20):")),
            "{}",
            result.content
        );
        // Commit LINES carry the two-space indent — pin on those, so the
        // base sha's appearance in the range HEADER is not confused with a
        // listed commit.
        assert!(result.content.contains(&format!("\n  {second} second")), "{}", result.content);
        assert!(
            !result.content.contains(&format!("\n  {first} first")),
            "the base commit itself must be outside the range: {}",
            result.content
        );

        // An empty range renders the one-line note, still not an error.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": "HEAD"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("commits: (none in range HEAD..HEAD)"),
            "{}",
            result.content
        );

        // An unresolvable ref renders the degrade note, still not an error.
        let result = dispatch(
            &delegate_ctx(tmp.path()),
            "delegate",
            &json!({"action": "collect", "cwd": tmp.path(), "base": "definitely-not-a-ref-xyz"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("commits: (unavailable:"),
            "{}",
            result.content
        );
    }

    /// T147 (T128-M4 kill): BOTH `render_collect` liveness arms are pinned at
    /// the unit level — a live child renders the distinct `alive: true` line,
    /// a dead child the distinct `alive: false` line, no pid renders neither,
    /// and flipping the liveness bool flips the output. The alive-true-flip
    /// mutant (the `Some(true)` arm rendering the dead text) turns the first
    /// pair RED; the reverse flip turns the second pair RED.
    #[test]
    fn delegate_collect_render_collect_pins_both_liveness_arms() {
        let summary = summarize_collect(&[]);
        let no_commits: Result<Vec<String>, String> = Err("not a repo".to_string());
        let live = render_collect(&summary, Some(true), None, &no_commits, None);
        let dead = render_collect(&summary, Some(false), None, &no_commits, None);
        let unasked = render_collect(&summary, None, None, &no_commits, None);
        // The live arm's distinctive text — and ONLY the live arm renders it.
        assert!(live.contains("\nalive: true"), "{live}");
        assert!(!live.contains("alive: false"), "{live}");
        // The dead arm's distinctive text — and ONLY the dead arm renders it.
        assert!(dead.contains("\nalive: false"), "{dead}");
        assert!(!dead.contains("alive: true"), "{dead}");
        // No pid → NO liveness line at all (collect's contract, unlike
        // status's always-rendered `alive: unknown`).
        assert!(!unasked.contains("alive:"), "{unasked}");
        // Flipping the liveness bool flips the rendered output.
        assert_ne!(live, dead, "the liveness bool must flip the rendering");
    }

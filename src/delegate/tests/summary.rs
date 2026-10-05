// T109 family: summary — the events-file summary state machine (T23/T58/T64 pins).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod summary;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 18;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    #[test]
    fn delegate_summary_empty_is_starting() {
        let s = summarize_events(&[]);
        assert_eq!(s.state(), "starting");
        assert_eq!(s.max_iters, None);
        assert_eq!(s.last_iteration, None);
        assert_eq!(s.last_event_type, None);
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_run_start_only_is_running_with_budget() {
        let lines = ["{\"type\":\"run_start\",\"ts\":\"2026-09-25T18:09:35.505Z\",\"mode\":\"run\",\"model\":\"kimi\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}"];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.max_iters, Some(50));
        assert_eq!(s.last_iteration, None);
        assert_eq!(s.last_event_type.as_deref(), Some("run_start"));
        assert_eq!(s.last_event_ts.as_deref(), Some("2026-09-25T18:09:35.505Z"));
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_mid_run_reports_iteration_and_last_event() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":3,\"input_tokens\":10,\"output_tokens\":5}",
            "{\"type\":\"tool_result\",\"ts\":\"t2\",\"name\":\"bash\",\"ok\":true,\"is_error\":false,\"duration_ms\":12,\"preview\":\"hi\"}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(3));
        assert_eq!(s.last_event_type.as_deref(), Some("tool_result"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t2"));
        assert!(!s.budget_low_seen && !s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_budget_low_sets_flag() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":33}",
            "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":1785,\"remaining_tokens\":null}",
            "{\"type\":\"iteration\",\"ts\":\"t3\",\"n\":34}",
        ];
        let s = summarize_events(&lines);
        assert!(s.budget_low_seen);
        assert_eq!(s.state(), "running");
        assert_eq!(s.last_iteration, Some(34));
        assert!(!s.goal_seen && !s.abort_seen);
    }

    #[test]
    fn delegate_summary_goal_sets_flag_and_done_state() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":4}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"VERDICT PASS\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.goal_seen);
        assert!(!s.abort_seen && !s.budget_low_seen);
        assert_eq!(s.state(), "done");
        assert_eq!(s.last_event_type.as_deref(), Some("goal"));
    }

    #[test]
    fn delegate_summary_abort_sets_flag_reason_and_state() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
        ];
        let s = summarize_events(&lines);
        assert!(s.abort_seen);
        assert_eq!(s.abort_reason.as_deref(), Some("iteration budget exceeded"));
        assert_eq!(s.state(), "aborted");
        assert!(!s.goal_seen);
    }

    /// T58 (a): a resumed child appends a NEW `run_start` + iterations to the
    /// same stream after its first segment aborted — the summary must describe
    /// the LATEST segment (`running`, `abort_seen` reset), not keep the
    /// pre-resume abort latched (the cycle-18 bite that had the orchestrator
    /// fall back to `ps`). `last_iteration` keeps last-seen values: here the
    /// new segment's own iteration 2.
    #[test]
    fn delegate_summary_two_segment_abort_then_run_start_reports_running() {
        let lines = [
            // Segment 1: ran, then died mid-arc.
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
            // Segment 2: the resume — fresh run_start, fresh iterations.
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":2}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running", "resumed child mid-run is running, not aborted");
        assert!(!s.abort_seen, "pre-resume abort must not stay latched");
        assert_eq!(s.abort_reason, None, "pre-resume abort reason must reset");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(2), "iteration takes the last-seen value");
        assert_eq!(s.last_event_type.as_deref(), Some("iteration"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t5"));
        assert!(!s.goal_seen && !s.budget_low_seen);
    }

    /// T58 (b): goal in the second segment after an abort in the first —
    /// `done`, not `aborted`.
    #[test]
    fn delegate_summary_goal_in_second_segment_after_abort_reports_done() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"model stream cut\",\"model\":\"kimi\"}",
            "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t4\",\"n\":1}",
            "{\"type\":\"goal\",\"ts\":\"t5\",\"outcome\":\"accepted\",\"summary\":\"VERDICT PASS\"}",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "done");
        assert!(s.goal_seen);
        assert!(!s.abort_seen, "segment-1 abort must not outlive the resume");
        assert_eq!(s.abort_reason, None);
        assert_eq!(s.last_event_type.as_deref(), Some("goal"));
    }

    /// T58 (c): `budget_low` only in the first segment — a resumed child that
    /// has not gone budget-low again must not report the stale latch.
    #[test]
    fn delegate_summary_budget_low_in_first_segment_only_resets_on_resume() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":33}",
            "{\"type\":\"budget_low\",\"ts\":\"t2\",\"remaining_iters\":8,\"remaining_secs\":100}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\"}",
            // The resume: fresh segment, budget_low never fires again.
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":1}",
        ];
        let s = summarize_events(&lines);
        assert!(!s.budget_low_seen, "segment-1 budget_low must reset at the new run_start");
        assert!(!s.abort_seen && !s.goal_seen);
        assert_eq!(s.state(), "running");
    }

    /// T64 pin (b) — the t58-validate over-reset survivor. The T58
    /// two-segment tests above all place segment 2's iterations AFTER its
    /// `run_start`, so a mutant that over-resets `max_iters`/`last_iteration`
    /// to `None` at `run_start` survives: the new segment's own lines refill
    /// both fields before any assertion looks. T58 spec req 4 sentence 2
    /// pins the other half — the fields "already take the last-seen values
    /// and keep doing so" — so in the GAP window (after segment 2's
    /// `run_start`, before its first iteration) the summary must still
    /// report meaningful numbers, which is exactly what a resumed child's
    /// `status` shows while the relaunched process spins up.
    #[test]
    fn delegate_summary_run_start_gap_keeps_last_seen_max_iters_and_iteration() {
        // Segment 1: ran to its iteration-budget abort (max_iters 40, last
        // iteration 40). Segment 2: the resume — a fresh `run_start`
        // carrying its own max_iters (deliberately 50, so the assertions
        // prove WHICH line each field came from), and NOTHING else yet:
        // the gap window.
        let gap = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":39}",
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":50}",
        ];
        let s = summarize_events(&gap);
        // The verdict latches describe segment 2: the pre-resume abort is
        // gone, and a stream ending in a fresh `run_start` is `running`.
        assert_eq!(
            s.state(),
            "running",
            "segment-2's run_start resets the abort latch — the gap reports \
             running, not aborted"
        );
        assert!(
            !s.abort_seen,
            "pre-resume abort must not outlive the new run_start"
        );
        assert_eq!(
            s.abort_reason, None,
            "pre-resume abort reason must reset at the new run_start"
        );
        assert!(!s.goal_seen && !s.budget_low_seen);
        assert_eq!(s.last_event_type.as_deref(), Some("run_start"));
        // The stream-scope fields KEEP LAST-SEEN across the boundary — NOT
        // null/0: `max_iters` from the latest `run_start` seen (segment 2's
        // 50), `last_iteration` still segment 1's final iteration (40) until
        // segment 2 writes its first.
        assert_eq!(
            s.max_iters,
            Some(50),
            "max_iters keeps last-seen across the run_start boundary (the \
             latest run_start's value) — an over-reset to null is the \
             t58-validate survivor mutant"
        );
        assert_eq!(
            s.last_iteration,
            Some(40),
            "last_iteration keeps last-seen across the run_start boundary \
             (segment 1's final iteration) — an over-reset to null/0 is the \
             t58-validate survivor mutant"
        );

        // After segment 2's first iteration event the fields reflect
        // segment 2: its own iteration takes over, its max_iters stands.
        let after = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":39}",
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":40}",
            "{\"type\":\"abort\",\"ts\":\"t3\",\"reason\":\"iteration budget exceeded\",\"model\":\"glm\",\"budget_kind\":\"iterations\",\"budget_max\":40}",
            "{\"type\":\"run_start\",\"ts\":\"t4\",\"max_iters\":50}",
            "{\"type\":\"iteration\",\"ts\":\"t5\",\"n\":1}",
        ];
        let s2 = summarize_events(&after);
        assert_eq!(s2.state(), "running");
        assert_eq!(
            s2.max_iters,
            Some(50),
            "segment 2's run_start max_iters wins once seen"
        );
        assert_eq!(
            s2.last_iteration,
            Some(1),
            "segment 2's own iteration takes over from segment 1's last-seen 40"
        );
        assert_eq!(s2.last_event_type.as_deref(), Some("iteration"));
        assert!(!s2.abort_seen && !s2.goal_seen && !s2.budget_low_seen);
    }

    /// T58 (d) regression pin: a SINGLE-segment stream — the only kind before
    /// resume existed — summarizes exactly as before T58. This is the same
    /// event sequence as the two-segment test minus the second `run_start`:
    /// the abort stays latched and the state is `aborted`.
    #[test]
    fn delegate_summary_single_segment_stream_unchanged_by_t58() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":12}",
            "{\"type\":\"abort\",\"ts\":\"t2\",\"reason\":\"llm request failed\",\"model\":\"kimi\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.abort_seen);
        assert_eq!(s.abort_reason.as_deref(), Some("llm request failed"));
        assert_eq!(s.state(), "aborted");
        assert_eq!(s.max_iters, Some(40));
        assert_eq!(s.last_iteration, Some(12));
        assert!(!s.goal_seen && !s.budget_low_seen);
    }

    #[test]
    fn delegate_summary_malformed_lines_skipped_not_fatal() {
        let lines = [
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7}",
            // A torn final write (partial JSON) and a non-JSON line must be
            // skipped; the summary keeps standing on the complete lines.
            "{\"type\":\"iteration\",\"ts\":\"t2\",\"n\":8,\"trunc",
            "not json at all",
            "",
        ];
        let s = summarize_events(&lines);
        assert_eq!(s.state(), "running");
        assert_eq!(s.last_iteration, Some(7));
        assert_eq!(s.last_event_type.as_deref(), Some("iteration"));
        assert_eq!(s.last_event_ts.as_deref(), Some("t1"));
    }

    /// T68: `significant_ne` is EXACTLY the eight-field wake set (T234: the
    /// two goal-outcome flags joined the original six). A diff
    /// confined to `last_event_type`/`last_event_ts` (the per-tool-call
    /// churn an active child emits every 2–10 s) is NOT significant; a diff
    /// in each of the eight significant fields IS. Pinned field-by-field so a
    /// field cannot silently migrate between the wake set and the churn set.
    #[test]
    fn delegate_summary_significant_ne_is_exactly_the_eight_field_wake_set() {
        let base = DelegateSummary {
            max_iters: Some(50),
            last_iteration: Some(7),
            last_event_type: Some("iteration".to_string()),
            last_event_ts: Some("t1".to_string()),
            budget_low_seen: true,
            goal_seen: false,
            goal_accepted_seen: false,
            goal_rejected_seen: false,
            abort_seen: false,
            abort_reason: Some("iteration budget exceeded".to_string()),
        };
        // Churn only (new last_event): never significant — the T68 defect
        // was exactly this diff waking the wait at the first poll tick.
        let churn = DelegateSummary {
            last_event_type: Some("tool_result".to_string()),
            last_event_ts: Some("t9".to_string()),
            ..base.clone()
        };
        assert!(!base.significant_ne(&churn), "last_event churn must not be significant");
        assert!(!churn.significant_ne(&base), "significance must be symmetric");
        // Identical: not significant.
        assert!(!base.significant_ne(&base.clone()));
        // Each significant field alone IS significant.
        let significant = [
            DelegateSummary { max_iters: None, ..base.clone() },
            DelegateSummary { last_iteration: Some(8), ..base.clone() },
            DelegateSummary { budget_low_seen: false, ..base.clone() },
            DelegateSummary { goal_seen: true, ..base.clone() },
            DelegateSummary { goal_accepted_seen: true, ..base.clone() },
            DelegateSummary { goal_rejected_seen: true, ..base.clone() },
            DelegateSummary { abort_seen: true, ..base.clone() },
            DelegateSummary { abort_reason: None, ..base.clone() },
        ];
        for sig in &significant {
            assert!(
                base.significant_ne(sig),
                "a diff in a significant field must wake: {base:?} vs {sig:?}"
            );
        }
    }


    // ---- T234: the goal verdict resolution (outcome-aware latches) ----

    /// T234 req 1: a goal line with `outcome:"rejected"` latches
    /// `goal_rejected_seen` — while `goal_seen` keeps its any-goal latch and
    /// `state()` stays byte-compatible (`done` on any goal line, the T157
    /// pinned mcp_serve compat): a rejected-only stream reads exactly as
    /// pre-T234 except for the new resolution flags.
    #[test]
    fn delegate_summary_rejected_goal_latches_rejected_flag_and_keeps_goal_seen() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":5}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"rejected\",\"reason\":\"check failed\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.goal_seen, "goal_seen keeps its any-goal latch (pinned compat)");
        assert!(s.goal_rejected_seen, "the rejection must resolve into its flag");
        assert!(!s.goal_accepted_seen);
        assert_eq!(
            s.state(),
            "done",
            "state() stays byte-compatible: any goal line latches done"
        );
        assert_eq!(s.last_event_type.as_deref(), Some("goal"));
    }

    /// T234 req 1: an accepted goal line latches `goal_accepted_seen` — and
    /// only that flag (all goal flags consistent: `goal_seen` +
    /// `goal_accepted_seen`, no rejection).
    #[test]
    fn delegate_summary_accepted_goal_latches_accepted_flag_only() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"accepted\",\"summary\":\"VERDICT PASS\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.goal_seen);
        assert!(s.goal_accepted_seen);
        assert!(!s.goal_rejected_seen);
        assert_eq!(s.state(), "done");
    }

    /// T234 req 1 (fail-safe leg): a goal line with a MISSING or unparseable
    /// `outcome` sets `goal_seen` only — exactly the pre-T234 behavior; the
    /// resolution flags stay false rather than guessing.
    #[test]
    fn delegate_summary_goal_line_without_outcome_sets_goal_seen_only() {
        // Missing outcome field entirely.
        let missing = ["{\"type\":\"goal\",\"ts\":\"t2\",\"summary\":\"no outcome field\"}"];
        let s = summarize_events(&missing);
        assert!(s.goal_seen);
        assert!(!s.goal_accepted_seen && !s.goal_rejected_seen, "missing outcome must not guess");
        // Unparseable outcome (present but not a string).
        let non_string = ["{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":7}"];
        let s = summarize_events(&non_string);
        assert!(s.goal_seen);
        assert!(!s.goal_accepted_seen && !s.goal_rejected_seen, "non-string outcome must not guess");
    }

    /// T234: the seen-flags are ADDITIVE latches, not a latch-overwrite like
    /// `summarize_collect`'s verdict — a segment rejected and LATER accepted
    /// reports both (the orchestrator sees the full gate history: a rejection
    /// was seen AND an acceptance was seen; the collect verdict still says
    /// the LATEST verdict is accepted).
    #[test]
    fn delegate_summary_rejected_then_accepted_latches_both_flags() {
        let lines = [
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t2\",\"outcome\":\"rejected\",\"reason\":\"check failed\"}",
            "{\"type\":\"goal\",\"ts\":\"t3\",\"outcome\":\"accepted\",\"summary\":\"fixed and verified\"}",
        ];
        let s = summarize_events(&lines);
        assert!(s.goal_seen);
        assert!(s.goal_rejected_seen && s.goal_accepted_seen);
        assert_eq!(s.state(), "done");
    }

    /// T234 req 2: the T58 segment reset covers the new flags — a pre-resume
    /// rejection must not bleed into the resumed segment's summary (the
    /// resume case: the gate spoke in segment 1, the child kept running in
    /// segment 2).
    #[test]
    fn delegate_summary_run_start_resets_goal_outcome_flags() {
        let lines = [
            // Segment 1: ran, gate rejected.
            "{\"type\":\"run_start\",\"ts\":\"t0\",\"max_iters\":40}",
            "{\"type\":\"goal\",\"ts\":\"t1\",\"outcome\":\"rejected\",\"reason\":\"check failed\"}",
            // Segment 2: the resume — fresh run_start, fresh iterations.
            "{\"type\":\"run_start\",\"ts\":\"t2\",\"max_iters\":40}",
            "{\"type\":\"iteration\",\"ts\":\"t3\",\"n\":1}",
        ];
        let s = summarize_events(&lines);
        assert!(!s.goal_seen, "the pre-resume goal latch resets with the segment");
        assert!(
            !s.goal_rejected_seen && !s.goal_accepted_seen,
            "a pre-resume rejection must not bleed into the resumed segment"
        );
        assert_eq!(s.state(), "running");
    }

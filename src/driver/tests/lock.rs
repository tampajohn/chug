// T104 family: lock — .chug/driver.lock run-path acquire/release/reclaim (T55). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T55: .chug/driver.lock — run-path acquire/release ----------

    /// Test transport feeding scripted response bodies through the real
    /// Client, so tests can run the FULL `run_loop` (lock acquire at startup,
    /// release at exit) to goal-acceptance without network.
    struct ScriptedTransport(std::sync::Mutex<std::collections::VecDeque<Value>>);

    impl crate::api::Transport for ScriptedTransport {
        fn send(
            &self,
            _url: &str,
            _headers: &[(String, String)],
            _body: &str,
        ) -> Result<crate::api::RawResponse, crate::api::TransportError> {
            let body = self
                .0
                .lock()
                .unwrap()
                .pop_front()
                .expect("scripted transport exhausted");
            Ok(crate::api::RawResponse {
                status: 200,
                headers: Vec::new(),
                body: body.to_string(),
            })
        }
    }

    fn scripted_client(responses: Vec<Value>) -> Client {
        Client::with_transport_for_tests(
            Arc::new(ScriptedTransport(std::sync::Mutex::new(responses.into()))),
            "test-model",
        )
    }

    fn scripted_accepting_run_config(tmp: &tempfile::TempDir, spec: &Path, resume: bool) -> RunConfig {
        // A run that would accept immediately: the LLM answers goal_complete
        // on its first (and only) call; the spec's `check: true` accepts it.
        let (stx, srx) = mpsc::channel();
        drop(stx);
        RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec.to_path_buf(),
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0,
            resume,
            controls: Controls {
                abort: Arc::new(AtomicBool::new(false)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            mcp_off: true,
        }
    }

    /// Release leg (T55 req 5): a scripted run to goal-acceptance leaves NO
    /// lock behind, and a second scripted run in the same cwd acquires
    /// cleanly — the same-cwd successor is never blocked.
    #[test]
    fn run_releases_lock_on_goal_acceptance_and_successor_acquires() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let responses = || {
            vec![tool_use_response("goal_complete", json!({"summary": "wrapped up"}))]
        };

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(responses()),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "goal accepted");
        assert!(
            !driver_lock::lock_path(tmp.path()).exists(),
            "the lock is released on the goal-acceptance exit path"
        );

        // Second scripted run, same cwd: acquires cleanly (nothing stale).
        let mut sink2 = RecordingSink::default();
        let code2 = run_loop(
            scripted_accepting_run_config(&tmp, &spec, true),
            scripted_client(responses()),
            None,
            &mut sink2,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code2, 0);
        assert!(!driver_lock::lock_path(tmp.path()).exists());
    }

    /// Run-path integration pin (T55): a run that finds a lock held by a
    /// LIVE non-chug process (a real `sleep`, argv lacking chug) RECLAIMS it
    /// and proceeds — its events/transcript writes land — proving the check
    /// sits before the appends without blocking them. A stale (dead-holder)
    /// lock never blocks the next run either.
    #[test]
    fn run_reclaims_lock_held_by_live_non_chug_process_and_writes_proceed() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let mut holder = std::process::Command::new("sleep")
            .arg("37")
            .spawn()
            .expect("spawning sleep");
        // Hand-write the lock as if the sleep held it (alive, argv ≠ chug).
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", holder.id()))
            .expect("seeding the foreign lock");

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(vec![tool_use_response(
                "goal_complete",
                json!({"summary": "wrapped up"}),
            )]),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "the live-but-not-chug holder never blocks a run");

        // The run's own writes proceeded: the transcript holds this run's
        // goal message (the reclaim happened before any append, and nothing
        // was blocked by the pre-existing file).
        let messages = transcript::load(tmp.path()).unwrap();
        assert!(
            messages
                .iter()
                .any(|m| m.content[0].text().unwrap_or_default().starts_with("Goal: x\n")),
            "run wrote its transcript despite the pre-existing lock"
        );
        let events = fs::read_to_string(tmp.path().join(".chug/events.jsonl")).unwrap();
        assert!(events.contains("\"run_start\""), "events log written");
        assert!(!driver_lock::lock_path(tmp.path()).exists(), "released at exit");

        holder.kill().unwrap();
        holder.wait().unwrap();
    }

    /// A SIGKILLed holder leaves a stale lock BY DESIGN; the next run must
    /// reclaim it transparently (T55 req 4/5, acceptance row 2).
    #[test]
    fn run_reclaims_stale_lock_left_by_killed_holder() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let mut holder = std::process::Command::new("sleep")
            .arg("37")
            .spawn()
            .expect("spawning sleep");
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", holder.id()))
            .expect("seeding the doomed lock");
        // SIGKILL-class death: no destructor runs, the lock file survives.
        holder.kill().unwrap();
        holder.wait().unwrap();
        assert!(driver_lock::lock_path(tmp.path()).exists(), "stale by design");

        let mut sink = RecordingSink::default();
        let code = run_loop(
            scripted_accepting_run_config(&tmp, &spec, false),
            scripted_client(vec![tool_use_response(
                "goal_complete",
                json!({"summary": "wrapped up"}),
            )]),
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 0, "a stale lock never blocks the next run");
        assert!(!driver_lock::lock_path(tmp.path()).exists());
    }

    /// Chat exemption (T55 req 6): chat's turn path drives the SAME
    /// drive_loop but never the run startup path, so a chat turn in a cwd
    /// leaves any lock file exactly as it found it — never created, never
    /// removed. drive_loop is the shared iteration loop; run_loop is the
    /// only acquire site, and run_chat never calls it.
    #[test]
    fn chat_turn_never_creates_or_removes_the_driver_lock() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".chug")).unwrap();
        // A pre-existing lock file (as a live run would leave it) that the
        // chat turn must neither consume nor delete.
        fs::write(driver_lock::lock_path(tmp.path()), format!("{}\n", i32::MAX)).unwrap();

        let (_utx, urx) = mpsc::channel::<SlashUpdate>();
        let controls = Controls::detached();
        let ctx = ctx_for(&tmp, Mode::Chat, &controls, &urx, None, &observ::Sink::Noop);
        let mut knobs = knobs_with(3);
        let mut llm = ScriptedLlm::new(vec![text_only_response("done, idle")]);
        let mut gate = None;
        let mut messages = Vec::new();
        let outcome = drive_loop(
            &ctx,
            &mut knobs,
            &mut llm,
            &mut gate,
            &mut messages,
            None,
            &mut RecordingSink::default(),
            &mut McpRegistry::new(tmp.path(), true, None).unwrap(),
        )
        .unwrap();
        assert!(matches!(outcome, DriveOutcome::TurnEnded(_)));
        let text = fs::read_to_string(driver_lock::lock_path(tmp.path())).unwrap();
        assert_eq!(
            text.lines().next().unwrap().trim(),
            i32::MAX.to_string(),
            "chat neither acquired (rewrote) nor removed the existing lock"
        );
        // And no run started here, so no lock was created either.
        assert_eq!(fs::read_to_string(driver_lock::lock_path(tmp.path())).unwrap(), text);
    }


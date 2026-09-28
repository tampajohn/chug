// T104 family: resume — fresh-run ledger archiving + transcript rotation (T3/T7). Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    // ---------- T3: fresh-run ledger archiving ----------

    /// A RunConfig that aborts at the first iteration boundary: the whole
    /// startup path (seeding, archiving, first-message append) runs, but no
    /// LLM call is ever made.
    fn aborted_run_config(tmp: &tempfile::TempDir, spec: &Path, resume: bool) -> RunConfig {
        let (stx, srx) = mpsc::channel();
        drop(stx);
        RunConfig {
            cwd: tmp.path().to_path_buf(),
            spec_path: spec.to_path_buf(),
            goal: "x".to_string(),
            model: "test-model".to_string(),
            max_iters: 5,
            max_minutes: 10,
            max_tokens: 0, // no token budget: pre-T15 behavior
            resume,
            controls: Controls {
                abort: Arc::new(AtomicBool::new(true)),
                steering_rx: srx,
            },
            risk_gate: false,
            bash_timeout: Duration::from_secs(tools::BASH_TIMEOUT_SECS),
            mcp_config: None,
            mcp_off: true,
            // T117: a literal test goal — no pack expansion.
            goal_pack: None,
        }
    }

    fn ledger_archives(tmp: &tempfile::TempDir) -> Vec<PathBuf> {
        let dir = tmp.path().join(".chug");
        if !dir.exists() {
            return Vec::new();
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let path = e.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                (name.starts_with("LEDGER-") && name.ends_with(".md")).then_some(path)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn fresh_run_archives_foreign_ledger_and_reseeds() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        std::fs::write(
            tmp.path().join("LEDGER.md"),
            "# Ledger\n\n## Done\n- OLD PROJECT goal met\n",
        )
        .unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        let code = run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();
        assert_eq!(code, 1, "aborted at the first boundary");

        let archives = ledger_archives(&tmp);
        assert_eq!(archives.len(), 1, "exactly one ledger archive: {archives:?}");
        assert!(
            std::fs::read_to_string(&archives[0])
                .unwrap()
                .contains("OLD PROJECT goal met")
        );
        assert_eq!(
            ledger::read(tmp.path()).unwrap(),
            ledger::SEED,
            "fresh run starts on the pristine seed"
        );
    }

    #[test]
    fn fresh_run_leaves_pristine_seed_ledger_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        std::fs::write(tmp.path().join("LEDGER.md"), ledger::SEED).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        assert!(ledger_archives(&tmp).is_empty(), "no archive for the seed");
        assert_eq!(ledger::read(tmp.path()).unwrap(), ledger::SEED);
    }

    #[test]
    fn resume_run_never_archives_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let foreign = "# Ledger\n\n## Done\n- previous run state\n";
        std::fs::write(tmp.path().join("LEDGER.md"), foreign).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, true),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        assert!(ledger_archives(&tmp).is_empty(), "--resume never archives");
        assert_eq!(ledger::read(tmp.path()).unwrap(), foreign, "ledger kept");
    }

    #[cfg(unix)]
    #[test]
    fn fresh_run_warns_and_proceeds_when_ledger_archive_fails() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let foreign = "# Ledger\n\n## Done\n- stuck foreign ledger\n";
        std::fs::write(tmp.path().join("LEDGER.md"), foreign).unwrap();
        // The rename needs write permission on the source dir (cwd); with cwd
        // read-only but .chug/ writable the archive fails while the run can
        // still proceed and append its transcript.
        std::fs::create_dir(tmp.path().join(".chug")).unwrap();
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555)).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        let result = run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        );
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(result.unwrap(), 1, "run proceeds despite the failed archive");
        assert_eq!(ledger::read(tmp.path()).unwrap(), foreign, "ledger kept as-is");
    }

    // ---------- T7: fresh-run transcript rotation ----------

    fn transcript_archives(tmp: &tempfile::TempDir) -> Vec<PathBuf> {
        let dir = tmp.path().join(".chug");
        if !dir.exists() {
            return Vec::new();
        }
        let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| {
                let path = e.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                (name.starts_with("transcript-") && name.ends_with(".jsonl")).then_some(path)
            })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn fresh_run_rotates_previous_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let old = Message::user(vec![ContentBlock::text_block("Goal: OLD SESSION")]);
        transcript::append(tmp.path(), &old).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, false),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        // The old session was archived with its content intact.
        let archives = transcript_archives(&tmp);
        assert_eq!(archives.len(), 1, "exactly one transcript archive: {archives:?}");
        assert!(
            std::fs::read_to_string(&archives[0])
                .unwrap()
                .contains("Goal: OLD SESSION")
        );
        // The new transcript begins with this run's goal message and nothing
        // else (the abort fires before any LLM call).
        let messages = transcript::load(tmp.path()).unwrap();
        assert_eq!(messages.len(), 1);
        assert!(
            messages[0].content[0]
                .text()
                .unwrap()
                .starts_with("Goal: x\n"),
            "{:?}",
            messages[0].content[0].text()
        );
    }

    #[test]
    fn resume_run_loads_transcript_without_rotating() {
        let tmp = tempfile::tempdir().unwrap();
        let spec = write_spec(&tmp);
        let old = Message::user(vec![ContentBlock::text_block("Goal: OLD SESSION")]);
        transcript::append(tmp.path(), &old).unwrap();

        let client = Client::new_without_credentials("test-model").unwrap();
        let mut sink = RecordingSink::default();
        run_loop(
            aborted_run_config(&tmp, &spec, true),
            client,
            None,
            &mut sink,
            &observ::Sink::Noop,
        )
        .unwrap();

        assert!(transcript_archives(&tmp).is_empty(), "--resume never archives");
        let messages = transcript::load(tmp.path()).unwrap();
        assert_eq!(
            messages,
            vec![old],
            "transcript untouched: the old session loads as-is"
        );
    }


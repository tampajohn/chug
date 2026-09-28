// T109: the body of delegate.rs's `#[cfg(test)] mod tests` (3,129 lines,
// 83 #[test] fns at the split, 86 after T115) lives in this file+directory
// module pair;
// delegate.rs keeps the one-line declaration. THIS FILE HOLDS THE SHARED
// HARNESS (T84's one rule): the spec-named helper family (delegate_ctx,
// DELEGATE_ENV_LOCK, write_argv_stub, ensure_spec_file, wait_for_argv_dump,
// spawn_pid_of) plus the shared fixtures (T29/T58 consts,
// write_events_fixture, append_events_line, waited_secs_of). Per T84's
// single-caller rule, goal_line moved with collect.rs and the T89 consts
// moved with wait_terminal.rs (their only callers). Moved bytes are
// byte-identical to the pre-split module body and keep their in-module
// 4-space indent (a move, not a rewrite).
    use super::*;
    use std::io::Write as _;
    use std::sync::Mutex;
    use serde_json::json;
    use crate::tools::{BASH_TIMEOUT_SECS, dispatch, kill_pid_group, tool_schemas};

    // ---- T23: delegate ----

    fn delegate_ctx(cwd: &Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(BASH_TIMEOUT_SECS),
        }
    }

    /// The tests that mutate `CHUG_DELEGATE_BIN` take this: the env is
    /// process-global and cargo runs test threads in parallel.
    static DELEGATE_ENV_LOCK: Mutex<()> = Mutex::new(());

    // ---- T29: status wait_secs long-poll ----

    /// Synthetic events lines shared by the T29 fixtures (no trailing
    /// newline; the fixture writer adds one per line).
    const T29_RUN_START: &str = "{\"type\":\"run_start\",\"ts\":\"t0\",\"mode\":\"run\",\"model\":\"m\",\"max_iters\":50,\"max_minutes\":35,\"max_tokens\":null}";
    const T29_ITERATION: &str =
        "{\"type\":\"iteration\",\"ts\":\"t1\",\"n\":7,\"input_tokens\":1,\"output_tokens\":1}";
    /// T58: the resumed segment's fresh `run_start` — same shape, later ts.
    const T58_RESUME_RUN_START: &str =
        "{\"type\":\"run_start\",\"ts\":\"t3\",\"max_iters\":40}";

    /// A fixed fake `.chug/` with the given events lines.
    fn write_events_fixture(cwd: &Path, lines: &[&str]) {
        fs::create_dir_all(cwd.join(".chug")).unwrap();
        let body: String = lines.iter().map(|l| format!("{l}\n")).collect();
        fs::write(cwd.join(".chug/events.jsonl"), body).unwrap();
    }

    /// Append one raw line to the fixture's events file (the mid-wait writer
    /// threads use this).
    fn append_events_line(path: &Path, line: &str) {
        let mut f = fs::OpenOptions::new().append(true).open(path).unwrap();
        f.write_all(line.as_bytes()).unwrap();
        f.write_all(b"\n").unwrap();
    }

    /// The `waited: <n>s` line's seconds — the wait leg's one extra line.
    fn waited_secs_of(content: &str) -> Option<u64> {
        content
            .lines()
            .find_map(|l| l.strip_prefix("waited: ")?.strip_suffix('s')?.parse().ok())
    }

    /// A stub child that records the argv it was invoked with, one argument
    /// per line, into `argv.txt` in its cwd (the child dir), then sleeps so
    /// the pid-group kill cleans it up.
    #[cfg(unix)]
    fn write_argv_stub(dir: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        // T82 check-fix: publish the dump ATOMICALLY (write argv.tmp, then
        // rename). The old stub truncated argv.txt in place, so a poller
        // could read the file in the truncate→printf window and take an
        // EMPTY dump for the final one — observed as a flaky red in
        // `cargo test` at default parallelism (the spec check) while
        // `--test-threads=4` and nextest's process isolation stayed green.
        let stub = dir.join("chug-argv-stub.sh");
        fs::write(
            &stub,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > argv.tmp && mv argv.tmp argv.txt\nsleep 60\n",
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        stub
    }

    /// T103: the launch spec probe requires a REAL, readable spec file. The
    /// stub/argv launch tests always passed the fictional
    /// `/tmp/chug-stub-spec.md` (the missing-binary leg: `/tmp/chug-spec.md`)
    /// — the stub child never reads the spec, only its argv, so the file
    /// never existed and the probe now correctly refuses the payload. Ensure
    /// it exists so those legs keep testing what they tested (asserts and
    /// pinned path strings untouched). Content is never read (the probe
    /// stats+opens; the stub dumps argv); deliberately NOT cleaned up — a
    /// parallel test may still be probing it.
    fn ensure_spec_file(path: &str) {
        if Path::new(path).is_file() {
            return;
        }
        fs::write(
            path,
            "t103: launch-payload probe needs a real spec file; content is unread\n",
        )
        .expect("ensure stub spec file exists");
    }

    /// The stub's argv dump, polled for (launch returns at spawn; the stub
    /// writes the dump within milliseconds of exec).
    #[cfg(unix)]
    fn wait_for_argv_dump(child_dir: &Path) -> Vec<String> {
        let path = child_dir.join("argv.txt");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Ok(text) = fs::read_to_string(&path) {
                // The stub renames argv.tmp into place (atomic publish), so
                // a successful read is complete; the non-empty guard is
                // belt-and-braces for any future in-place writer — an empty
                // dump must never satisfy the poll (T82 check-fix).
                let argv: Vec<String> = text.lines().map(str::to_string).collect();
                if !argv.is_empty() {
                    return argv;
                }
            }
            assert!(
                Instant::now() < deadline,
                "stub never wrote argv.txt in {}",
                child_dir.display()
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    #[cfg(unix)]
    fn spawn_pid_of(launch: &ToolResult) -> u32 {
        launch
            .content
            .lines()
            .find_map(|l| l.strip_prefix("launched: pid "))
            .expect("pid in launch output")
            .trim()
            .parse()
            .expect("pid parses")
    }


    // One `mod` line per family file; each family's tests live in exactly one
    // file.
    mod summary;
    mod status;
    mod dispatch;
    mod wait;
    mod wait_terminal;
    mod schema;
    mod launch;
    mod argv;
    mod parse;
    mod collect;

    /// T109 req 4 — the count pin (T104's M1 mutant lesson: without a pin, a
    /// dropped `mod <family>;` line silently shrinks the suite). Two legs:
    /// (1) each family's `pub(super) const TEST_COUNT` is referenced here —
    ///     a dropped `mod <family>;` line leaves the path unresolved and the
    ///     module fails to COMPILE;
    /// (2) each pinned count is checked against the family file's actual
    ///     `#[test]` fns (include_str!), and the total is pinned at 86 — so
    ///     an edited, deleted, or added moved test trips the assertion.
    #[test]
    fn delegate_test_module_count_pin() {
        let families: &[(&str, &str, usize)] = &[
            ("summary.rs", include_str!("summary.rs"), summary::TEST_COUNT),
            ("status.rs", include_str!("status.rs"), status::TEST_COUNT),
            ("dispatch.rs", include_str!("dispatch.rs"), dispatch::TEST_COUNT),
            ("wait.rs", include_str!("wait.rs"), wait::TEST_COUNT),
            (
                "wait_terminal.rs",
                include_str!("wait_terminal.rs"),
                wait_terminal::TEST_COUNT,
            ),
            ("schema.rs", include_str!("schema.rs"), schema::TEST_COUNT),
            ("launch.rs", include_str!("launch.rs"), launch::TEST_COUNT),
            ("argv.rs", include_str!("argv.rs"), argv::TEST_COUNT),
            ("parse.rs", include_str!("parse.rs"), parse::TEST_COUNT),
            (
                "collect.rs",
                include_str!("collect.rs"),
                collect::TEST_COUNT,
            ),
        ];
        let mut total = 0usize;
        for (name, src, pinned) in families {
            let actual = src.lines().filter(|l| *l == "    #[test]").count();
            assert_eq!(
                actual, *pinned,
                "{name}: pinned TEST_COUNT drifted from its #[test] fns"
            );
            total += actual;
        }
        assert_eq!(
            total, 86,
            "delegate test count drifted — recount and update the count pin"
        );
    }

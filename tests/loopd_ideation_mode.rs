//! T326 — ideation mode: the dormant loop proposes outward-facing features
//! to a `## Proposed` lane; the operator flips, the loop never self-flips.
//!
//! The spec's four test bullets, each as a fixture leg over the REAL
//! scripts/ideate.py and the REAL loopd.sh (the T325 battery's harness
//! shape — fixture repos, stubbed host probes, no network):
//!
//!   1. `the_canned_packet_files_gated_stubs_idempotently` — a canned
//!      decisions.jsonl produces ≤3 stubs with the correct file + row
//!      shapes, and a re-run files nothing (idempotent by slug).
//!   2. `the_noul_gate_rejects_meta_and_passes_outward_fail_closed` — a
//!      meta-class stub ("add wrap_assert leg") is rejected; an outward
//!      stub ("chug.sh cost dashboard") passes (mocked noul); the daemon
//!      unavailable fails CLOSED to rejection.
//!   3. `dormant_and_due_launches_the_ideation_child_on_glm` /
//!      `not_dormant_launches_no_ideation` /
//!      `dormant_but_not_due_or_unarmed_launches_no_ideation` /
//!      `an_ideation_child_failure_is_never_stamped` — the loopd fixture:
//!      dormant + timer due → ONE glm launch; not dormant → no ideation;
//!      not due / unarmed → no ideation; a crashed child is never stamped.
//!   4. `a_p_only_todo_row_count_is_zero_and_dormancy_holds` — an eval
//!      cycle over a TODO.md whose only rows are `proposed` files nothing
//!      and treats open count as 0 (dormancy holds; even an auto-ratified
//!      `open` P-row never wakes the valve).
//!
//! Plus the amendment leg (`the_ratify_sweep_flips_only_eligible_rows` —
//! 7d-old `proposed` rows auto-flip unless never-dispatch or vetoed) and
//! the static pins (`the_ideation_wiring_is_pinned_in_loopd_sh`).

#[path = "common/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

fn repo_root() -> PathBuf {
    // T48: resolve the repo root at RUNTIME — cargo runs test binaries with
    // cwd = the package root; the compile-time macro false-redded the
    // cycle-21 post-merge gates under the T47 shared cache.
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel)).expect("read repo file")
}

fn flat(s: &str) -> String {
    s.replace("\\\n", "").replace('\n', " ")
}

fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let got = haystack.matches(needle).count();
    assert_eq!(
        got, expected,
        "{what}: expected exactly {expected} occurrence(s) of {needle:?}, probed {got}"
    );
}

const FRESH_EPOCH: u64 = 1_790_510_400;
const FRESH_DAY: &str = "2026-09-27";

/// The full provider-routed glm id — the ideation child's model (the spec's
/// call: glm, never kimi).
const GLM_ID: &str = "anthropic-system.ai.glm-5-3-flash";

const TODO_ONE_DONE: &str = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T1 | title | specs/T1-slug.md | 2 | done | notes |\n";

const TODO_ONE_OPEN: &str = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T1 | title | specs/T1-slug.md | 2 | open | notes |\n";

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t326", "-c", "user.email=t326@example.test"])
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn write_exec(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, body).expect("write fixture stub");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fixture stub");
}

// --- the mocked noul daemon (the gate's test surface) --------------------------

/// The captured request bodies, keyed by socket — the question-shape pin.
static NOUL_CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Serve ONE canned /judge answer on a unix socket until the test process
/// ends (the T259 serve_judge shape: HTTP/1.1 over AF_UNIX, the body read
/// to its Content-Length, a canned JSON answer back). `answer` is the raw
/// JSON body; `"yes"` marks the stub meta-class, `"no"` passes it.
fn serve_noul(sock_path: &Path, answer: &str) {
    serve_noul_status(sock_path, 200, answer)
}

/// The status-carrying variant (the F3 legs: a non-200 with a VALID body
/// must still reject — the gate's fail-CLOSED is not status-blind).
fn serve_noul_status(sock_path: &Path, status: u16, answer: &str) {
    use std::io::{Read, Write};
    let _ = std::fs::remove_file(sock_path);
    let listener = std::os::unix::net::UnixListener::bind(sock_path).expect("bind mock noul");
    let answer = answer.to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = Vec::new();
            loop {
                let mut chunk = [0u8; 16384];
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&chunk[..n]);
                        let text = String::from_utf8_lossy(&buf);
                        let Some(head_end) = text.find("\r\n\r\n") else {
                            continue;
                        };
                        let want = text
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().ok())
                            })
                            .flatten()
                            .unwrap_or(0);
                        if buf.len() >= head_end + 4 + want {
                            break;
                        }
                    }
                }
            }
            let req = String::from_utf8_lossy(&buf).to_string();
            if req.starts_with("POST /judge")
                && let Some(body) = req.split("\r\n\r\n").nth(1)
            {
                NOUL_CAPTURED
                    .lock()
                    .expect("captured map")
                    .push(body.to_string());
            }
            let reason = if status == 200 { "OK" } else { "Service Unavailable" };
            let resp = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                answer.len(),
                answer
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });
}

fn noul_answer(choice: &str) -> String {
    format!(
        "{{\"model\":\"mock-noul\",\"answers\":{{\"meta_class\":{{\"type\":\"choice\",\"choice\":\"{choice}\",\"confidence\":0.93}}}},\"usage\":{{}}}}"
    )
}

// --- the ideate fixture (a repo-shaped dir: TODO lane, FEATURES, corpus) -------

/// The ideate fixture's canned corpus: the eval-triage census input —
/// laya-triage x3 (the top friction class), validation-routing x2, one
/// outcome (excluded from the census — it is a backfill, not a decision).
const CANNED_CORPUS: &str = concat!(
    "{\"id\":\"d1\",\"ts\":1,\"class\":\"laya-triage\",\"subject\":\"s1\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"skip\",\"confidence\":0.9}\n",
    "{\"id\":\"d2\",\"ts\":2,\"class\":\"laya-triage\",\"subject\":\"s2\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"glm\",\"confidence\":0.9}\n",
    "{\"id\":\"d3\",\"ts\":3,\"class\":\"laya-triage\",\"subject\":\"s3\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"kimi\",\"confidence\":0.5}\n",
    "{\"id\":\"d4\",\"ts\":4,\"class\":\"validation-routing\",\"subject\":\"s4\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"kimi\",\"confidence\":0.9}\n",
    "{\"id\":\"d5\",\"ts\":5,\"class\":\"validation-routing\",\"subject\":\"s5\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"glm\",\"confidence\":0.9}\n",
    "{\"id\":\"d6\",\"ts\":6,\"class\":\"outcome\",\"subject\":\"d1\",\"inputs\":\"i\",\"options\":\"o\",\"choice\":\"landed-clean\",\"confidence\":1}\n",
);

/// The ideate fixture's TODO.md: the shipped lane shape — a table header,
/// two operator-approved rows (P001 hand-named + P002 hand-named — the F1
/// class: the shipped lane's operator-filed rows carry NO `pNNN-` filename
/// prefix, so the dedupe must take its keys from the row spec cells and the
/// proposed dir AS THEY ARE, prefix or not), one never-dispatch internal row
/// (the P005-P007 marker), one OLD machine row (the amendment's flip input).
const IDEATE_TODO: &str = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T1 | title | specs/T1-slug.md | 2 | done | notes |\n\
\n\
## Proposed\n\
\n\
Machine-maintained lane (T326): proposals land here with status `proposed`.\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| P001 | sandbox-deploy go-live | specs/proposed/sandbox-deploy-go-live.md | 3 | open | operator-approved 2026-11-06 (blanket approval) |\n\
| P002 | F16 unpark decision | specs/proposed/f16-unpark-decision.md | 4 | open | operator-approved 2026-11-06 (blanket approval) — keep/kill memo |\n\
| P005 | Pilot-team onboarding kit (internal) | specs/proposed/p005-pilot-onboarding-kit.md | 2 | proposed | APPROVED 2026-11-06 — owner fb011b00 (internal artifact, NOT loop work; do not dispatch): 2026-11-06 — Q4 deliverable |\n\
| P011 | old machine row | specs/proposed/p011-old-machine-row.md | 2 | proposed | machine-filed 2020-01-01 (T326 cycle 2026-W01) — awaiting operator ratification |\n";

const IDEATE_FEATURES: &str = "# Features\n\
\n\
## Landed\n\
\n\
- F10 — something landed\n\
\n\
## Parked\n\
\n\
- **F16 — Alternate-runtime judge serving (the kev DeltaNet port)** (parked\n\
  2026-10-05, operator call): the bake-off verdict removes the consumer.\n\
\n\
## Working rules\n\
\n\
- rule\n";

const IDEATE_LEDGER: &str = "# Ledger\n\n- deferred: the sandbox-deploy go-live needs a rootless runtime decision\n";

fn run_ideate(root: &Path, args: &[&str], sock: Option<&Path>) -> (String, i32) {
    let mut cmd = Command::new("python3");
    cmd.arg(repo_root().join("scripts/ideate.py"))
        .args(args)
        .arg(root)
        .current_dir(root)
        .env("HOME", root) // the chug-watch fact reads the fixture's HOME (no checkout → unknown)
        .env("LC_ALL", "C");
    if let Some(sock) = sock {
        cmd.env("CHUG_NOUL_SOCK", sock);
    } else {
        cmd.env_remove("CHUG_NOUL_SOCK").env_remove("CHUG_DAEMON_SOCK");
    }
    // T332: the decision-provider selection is per-leg explicit — no host
    // environment may flip a daemon-path fixture onto the proxy (auto with
    // no configured proxy is today's shape byte for byte).
    cmd.env_remove("CHUG_DECISION_PROVIDER")
        .env_remove("CHUG_DECISION_PROXY_URL")
        .env_remove("CHUG_DECISION_PROXY_TOKEN");
    let out = cmd.output().expect("spawn ideate.py");
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn ideate_fixture() -> tempdir_holder::Temp {
    let root = tempdir_holder::make();
    std::fs::create_dir_all(root.path().join("docs")).expect("fixture docs dir");
    std::fs::create_dir_all(root.path().join("specs/proposed")).expect("fixture proposed dir");
    std::fs::create_dir_all(root.path().join(".chug")).expect("fixture .chug dir");
    std::fs::write(root.path().join("TODO.md"), IDEATE_TODO).expect("fixture TODO.md");
    std::fs::write(root.path().join("FEATURES.md"), IDEATE_FEATURES).expect("fixture FEATURES.md");
    std::fs::write(root.path().join("LEDGER.md"), IDEATE_LEDGER).expect("fixture LEDGER.md");
    std::fs::write(root.path().join(".chug/decisions.jsonl"), CANNED_CORPUS).expect("fixture corpus");
    for (name, body) in [
        ("sandbox-deploy-go-live.md", "# P001 — sandbox-deploy go-live\n"),
        ("f16-unpark-decision.md", "# P002 — F16 unpark decision\n"),
        ("p005-pilot-onboarding-kit.md", "# P005 — Pilot-team onboarding kit (internal)\n"),
        ("p011-old-machine-row.md", "# P011 — old machine row\n"),
    ] {
        std::fs::write(root.path().join("specs/proposed").join(name), body)
            .expect("fixture proposed stub");
    }
    std::fs::write(root.path().join("docs/note.md"), "# notes\n").expect("fixture doc");
    git(root.path(), &["init", "-q"]);
    git(
        root.path(),
        &["add", "-A"],
    );
    git(
        root.path(),
        &["commit", "-q", "-m", "eval: t326 ideate fixture baseline"],
    );
    root
}

/// A tiny non-generic tempdir holder (tempfile::TempDir's Drop cleans up;
/// wrapped so the fixture fns read like the dormant battery's).
mod tempdir_holder {
    use std::path::{Path, PathBuf};

    pub struct Temp(tempfile::TempDir);
    pub fn make() -> Temp {
        Temp(tempfile::tempdir().expect("tempdir"))
    }
    impl Temp {
        pub fn path(&self) -> &Path {
            self.0.path()
        }
        pub fn join(&self, rel: &str) -> PathBuf {
            self.0.path().join(rel)
        }
    }
}

// --- the loopd fixture (the T325 battery's harness, verbatim shape) ------------

struct Fixture(tempfile::TempDir);

fn fixture() -> Fixture {
    let tmp = tempfile::tempdir().expect("tempdir");
    // T213/T330: loopd.sh sources scripts/*.sh fragments relative to its own
    // path — the fixture copies loopd.sh plus the DERIVED fragment set
    // (common::copy_loopd_fixture), so a new fragment never re-breaks this
    // fixture silently (the routing fixture's lesson, generalized).
    common::copy_loopd_fixture(tmp.path());
    std::fs::write(tmp.path().join(".gitignore"), "/.chug/\n/target-shared/\n/bin/\n")
        .expect("write fixture .gitignore");
    git(tmp.path(), &["init", "-q"]);
    let f = Fixture(tmp);
    f.commit_tree(
        "eval: cycle-100 fresh eval — the baseline (T326 fixture)",
        Some(TODO_ONE_DONE),
        Some("# eval\n"),
    );
    f.pin_eval_mtime();
    f.log_line("2026-09-27T12:00:00Z loopd start (pid 4242) — T326 fixture baseline record");
    f
}

impl Fixture {
    fn path(&self) -> &Path {
        self.0.path()
    }

    fn commit_tree(&self, subject: &str, todo: Option<&str>, eval: Option<&str>) {
        if let Some(todo) = todo {
            std::fs::write(self.path().join("TODO.md"), todo).expect("write fixture TODO.md");
        }
        if let Some(eval) = eval {
            std::fs::write(self.path().join("EVALUATION.md"), eval)
                .expect("write fixture EVALUATION.md");
        }
        git(self.path(), &["add", "-A"]);
        let status = Command::new("git")
            .args(["-c", "user.name=t326", "-c", "user.email=t326@example.test"])
            .arg("-C")
            .arg(self.path())
            .args(["commit", "-q", "--allow-empty", "-m", subject])
            .output()
            .expect("spawn git commit");
        assert!(
            status.status.success(),
            "fixture commit failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn commit_empty(&self, subject: &str) {
        self.commit_tree(subject, None, None);
    }

    fn log_line(&self, line: &str) {
        let log = self.path().join(".chug/loopd/loopd.log");
        std::fs::create_dir_all(log.parent().unwrap()).expect("fixture log dir");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .expect("open fixture loopd.log");
        writeln!(f, "{line}").expect("append fixture log line");
    }

    fn pin_eval_mtime(&self) {
        use std::fs::FileTimes;
        let path = self.path().join("EVALUATION.md");
        if !path.exists() {
            std::fs::write(&path, "# eval\n").expect("write fixture EVALUATION.md");
        }
        let f = std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open fixture EVALUATION.md for mtime pinning");
        f.set_times(FileTimes::new().set_modified(
            std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(FRESH_EPOCH),
        ))
        .expect("pin fixture mtime");
    }

    fn wraps(&self, n: usize) {
        for i in 0..n {
            self.commit_empty(&format!(
                "eval: cycle-{} wrap notes (empty-delta disposition, {}st consecutive no-op)",
                101 + i,
                i + 1
            ));
        }
    }

    fn backdate_anchor(&self, age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        let when = format!("@{} +0000", now - age_secs);
        let out = Command::new("git")
            .args(["-c", "user.name=t326", "-c", "user.email=t326@example.test"])
            .arg("-C")
            .arg(self.path())
            .env("GIT_COMMITTER_DATE", &when)
            .args(["commit", "--amend", "--no-edit", "-q"])
            .output()
            .expect("spawn git amend");
        assert!(
            out.status.success(),
            "anchor amend failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn latch_raw(&self, body: &str) {
        let dir = self.path().join(".chug/loopd");
        std::fs::create_dir_all(&dir).expect("fixture state dir");
        std::fs::write(dir.join("dormant-state.json"), body).expect("write fixture latch");
    }

    fn latch_ideation(&self, age_secs: u64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        let eval = now.saturating_sub(2 * 86_400);
        let ige = now - age_secs;
        self.latch_raw(&format!(
            "{{\"last_eval_ts\": {eval}, \"last_ideation_ts\": {ige}}}\n"
        ));
    }

    /// The latch's recorded last_ideation_ts, if present and readable.
    fn latch_ideation_ts(&self) -> Option<i64> {
        let text = std::fs::read_to_string(self.path().join(".chug/loopd/dormant-state.json"))
            .ok()?;
        let at = text.find("\"last_ideation_ts\"")?;
        let rest = &text[at + "\"last_ideation_ts\"".len()..];
        let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).collect();
        digits.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    }

    /// The latch's recorded last_eval_ts (the preservation pin).
    fn latch_eval_ts(&self) -> Option<i64> {
        let text = std::fs::read_to_string(self.path().join(".chug/loopd/dormant-state.json"))
            .ok()?;
        let at = text.find("\"last_eval_ts\"")?;
        let rest = &text[at + "\"last_eval_ts\"".len()..];
        let digits: String = rest.chars().skip_while(|c| !c.is_ascii_digit()).collect();
        digits.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
    }

    fn fixture_log(&self) -> String {
        std::fs::read_to_string(self.path().join(".chug/loopd/loopd.log"))
            .expect("read the fixture loopd.log")
    }

    /// Seed the Proposed lane (the arming condition): a specs/proposed stub
    /// file, committed — the ideation timer's git anchor.
    fn seed_lane(&self, age_secs: u64) {
        let dir = self.path().join("specs/proposed");
        std::fs::create_dir_all(&dir).expect("fixture lane dir");
        std::fs::write(dir.join("p001-sandbox-deploy-go-live.md"), "# P001 — sandbox-deploy go-live\n")
            .expect("write fixture lane stub");
        std::fs::write(self.path().join("TODO.md"), TODO_ONE_DONE).expect("write fixture TODO.md");
        git(self.path(), &["add", "-A"]);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        let when = format!("@{} +0000", now - age_secs);
        let status = Command::new("git")
            .args(["-c", "user.name=t326", "-c", "user.email=t326@example.test"])
            .arg("-C")
            .arg(self.path())
            .env("GIT_COMMITTER_DATE", &when)
            .args(["commit", "-q", "--allow-empty", "-m", "todo: T326 Proposed lane seed"])
            .output()
            .expect("spawn git lane commit");
        assert!(
            status.status.success(),
            "lane commit failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// The specs/proposed git anchor's epoch (the ideation timer's git half).
    fn lane_anchor_ts(&self) -> Option<i64> {
        let probe = Command::new("git")
            .arg("-C")
            .arg(self.path())
            .args(["log", "-1", "--format=%ct", "--", "specs/proposed"])
            .output()
            .expect("spawn git anchor probe");
        if !probe.status.success() {
            return None;
        }
        String::from_utf8_lossy(&probe.stdout).trim().parse().ok()
    }

    /// Install the run-mode stubs (the T325 shape) with a cycle child that
    /// APPENDS its full argv to child-argv.txt — the ideation leg's
    /// observable (model id, spec path, budgets). Each call restarts the
    /// recording.
    fn install_run_stubs(&self, chug_exits: i32) -> PathBuf {
        let bin = self.path().join("bin");
        std::fs::create_dir_all(&bin).expect("fixture stub bin dir");
        write_exec(&bin.join("ps"), "#!/bin/sh\necho \"fake ps: no drivers\"\n");
        write_exec(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
        let chug_dir = self.path().join("target/release");
        std::fs::create_dir_all(&chug_dir).expect("fixture target dir");
        write_exec(
            &chug_dir.join("chug"),
            &format!(
                "#!/bin/sh\n\
                 {{\n  \
                   echo '--- invocation ---'\n  \
                   printf '%s\\n' \"$@\"\n\
                 }} >> \"$PWD/child-argv.txt\"\n\
                 printf 'chug: goal complete\\nsummary: t326 stub child ran\\n'\n\
                 exit {chug_exits}\n"
            ),
        );
        let _ = std::fs::remove_file(self.path().join("child-argv.txt"));
        self.path().join("child-argv.txt")
    }

    fn child_argv(&self) -> String {
        std::fs::read_to_string(self.path().join("child-argv.txt"))
            .expect("read the stub child's argv recording")
    }

    fn run_one_pass_with(&self, chug_exits: i32) {
        self.install_run_stubs(chug_exits);
        let mut cmd = Command::new("bash");
        cmd.arg(self.path().join("loopd.sh"))
            .arg("run")
            .current_dir(self.path())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_string())
                ),
            )
            .env("HOME", self.path())
            .env("CHUG_ROUTINE_TODAY", FRESH_DAY)
            .env("CHUG_DAEMON_SOCK", self.path().join(".chug/no-daemon.sock"))
            .env("CHUG_SITE_DIR", self.path().join("NO-SUCH-SITE"))
            .env("CHUG_SITE_SYNC_NO_PUSH", "1")
            .env("LOOPD_SLEEP_OK", "1")
            .env("LOOPD_SLEEP_FAIL", "1")
            .env("LOOPD_MAX_LOOPS", "1")
            .env("LOOP_DAEMON_ENSURE", "0")
            .env("GIT_AUTHOR_NAME", "t326-fixture")
            .env("GIT_AUTHOR_EMAIL", "t326@example.test")
            .env("GIT_COMMITTER_NAME", "t326-fixture")
            .env("GIT_COMMITTER_EMAIL", "t326@example.test")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null");
        // T346: pin the T213/T330 loader files to ABSENT fixture paths — an
        // operator-staged ~/.chug/profile.env must never reach the stub
        // cycle (the T332 hermeticity contract).
        common::pin_loopd_env_files(&mut cmd, self.path());
        let status = cmd
            .output()
            .expect("spawn loopd.sh run");
        assert!(
            status.status.success(),
            "run pass exited {:?}: {}",
            status.status.code(),
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn run_one_pass(&self) {
        self.run_one_pass_with(0);
    }
}

const DAY: u64 = 86_400;

// --- spec bullet 1: the canned packet, the stub shapes, the idempotence --------

#[test]
fn the_canned_packet_files_gated_stubs_idempotently() {
    let f = ideate_fixture();
    let sock = f.join("noul.sock");
    serve_noul(&sock, &noul_answer("no")); // the mocked noul: outward, file it
    let (out, code) = run_ideate(f.path(), &[], Some(&sock));
    assert_eq!(code, 0, "ideate.py cycle exited {code}: {out}");

    // The packet line names every source, deterministic over the fixture.
    assert!(
        out.contains(
            "ideation packet (T326): parked=F16; blocked=1; \
             census=laya-triage(3),validation-routing(2); release=unknown; \
             chug-watch=unknown (no local checkout)"
        ),
        "packet line missing: {out}"
    );
    // The amendment ran BEFORE the filing: the old machine row flipped, the
    // never-dispatch row did not.
    assert!(
        out.contains("ideation auto-ratified (T326): P011"),
        "auto-ratify line missing: {out}"
    );
    // One gated filing (≤3): the census candidate. The PARKED F16 candidate's
    // slug (`f16-unpark-decision`) is ALREADY TAKEN — by the operator's
    // hand-named P002 row + stub file, neither carrying a `pNNN-` prefix —
    // so the cycle must NOT re-file it (the F1 duplicate class: the shipped
    // lane's operator-filed rows P001-P003 use hand-named files, and a
    // prefix-anchored dedupe re-files their proposals as duplicates, which
    // the 7d sweep then auto-ratifies with zero operator involvement).
    let filed: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("ideation filed (T326): "))
        .map(|l| &l["ideation filed (T326): ".len()..])
        .collect();
    assert_eq!(filed.len(), 1, "expected 1 filing (F16 deduped), got {filed:?}: {out}");
    assert!(
        !filed.iter().any(|n| n.contains("f16-unpark-decision")),
        "the F1 duplicate: the cycle re-filed the operator's hand-named P002 proposal: {filed:?}"
    );

    // The stub FILES exist with the shape (5 content lines — title + 4
    // bullets — plus the blank separator, the seeded lane's exact shape).
    for name in &filed {
        let text = std::fs::read_to_string(f.join("specs/proposed").join(name))
            .unwrap_or_else(|e| panic!("read filed stub {name}: {e}"));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines.len(),
            6,
            "{name}: probed {} lines (need 6 = title + blank + 4 bullets): {text:?}",
            lines.len()
        );
        assert!(lines[0].starts_with("# P0"), "{name}: title line: {}", lines[0]);
        assert_eq!(lines[1], "", "{name}: the shape's blank separator");
        assert!(
            lines[2].starts_with("- **User-visible value**: "),
            "{name}: {}",
            lines[2]
        );
        assert!(lines[3].starts_with("- **First slice**: "), "{name}: {}", lines[3]);
        assert!(lines[4].starts_with("- **Estimate class**: "), "{name}: {}", lines[4]);
    }

    // The ROWS: 6 cells, status proposed, machine-filed notes, the spec cell
    // names an existing file, and the id numbering continued past the lane.
    let todo = std::fs::read_to_string(f.path().join("TODO.md")).expect("read TODO.md");
    let rows: Vec<Vec<String>> = todo
        .lines()
        .filter(|l| l.trim_start().starts_with("| P"))
        .map(|l| {
            // The row's cells WITHOUT the leading/trailing empties the outer
            // pipes produce (a 6-cell row splits into 8 raw fields).
            l.trim()
                .trim_start_matches('|')
                .trim_end_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 5, "5 P-rows expected (4 seeded + 1 filed): {todo}");
    for (i, cells) in rows.iter().enumerate() {
        assert_eq!(cells.len(), 6, "row {i}: 6 cells, got {}", cells.len());
    }
    assert_eq!(rows[4][0], "P012", "the filed ids continue the lane's numbering");
    assert_eq!(rows[4][4], "proposed", "machine rows land as proposed");
    assert!(
        rows[4][5].starts_with("machine-filed ") && rows[4][5].contains("(T326 cycle "),
        "machine notes: {}", rows[4][5]
    );
    assert!(
        f.join(&rows[4][2]).is_file(),
        "the spec cell names an existing file: {}",
        rows[4][2]
    );

    // The hand-named P002 (the F1 dedupe key holder) is UNTOUCHED and the
    // lane still holds exactly ONE f16-unpark-decision row — no duplicate
    // filing, no duplicate id.
    assert_eq!(rows[1][0], "P002");
    assert_eq!(rows[1][4], "open", "the operator-approved P002 is never touched");
    assert_eq!(rows[1][2], "specs/proposed/f16-unpark-decision.md");
    assert_eq!(
        todo.matches("f16-unpark-decision").count(),
        1,
        "the lane holds exactly ONE f16-unpark-decision row (no F1 duplicate): {todo}"
    );

    // The never-dispatch row is untouched, and the auto-ratified row reads open.
    assert_eq!(rows[2][0], "P005");
    assert_eq!(rows[2][4], "proposed", "the internal-artifact row is NEVER flipped");
    assert_eq!(rows[3][0], "P011");
    assert_eq!(rows[3][4], "open", "the 7d-old machine row auto-ratified");
    assert!(
        rows[3][5].contains("auto-ratified") && rows[3][5].contains("(T326)"),
        "the flip is logged on the row: {}", rows[3][5]
    );

    // --- idempotence: the same week's re-run files nothing, flips nothing ----
    let (out2, code2) = run_ideate(f.path(), &[], Some(&sock));
    assert_eq!(code2, 0, "re-run exited {code2}: {out2}");
    assert!(
        !out2.contains("ideation filed (T326):"),
        "the re-run filed again (slug dedupe broken): {out2}"
    );
    assert!(
        !out2.contains("ideation auto-ratified (T326):"),
        "the re-run re-flipped (the sweep is not idempotent): {out2}"
    );
    let todo2 = std::fs::read_to_string(f.path().join("TODO.md")).expect("read TODO.md again");
    assert_eq!(todo, todo2, "the re-run rewrote TODO.md");
    // --check passes over the post-cycle lane.
    let (check_out, check_code) = run_ideate(f.path(), &["--check"], Some(&sock));
    assert_eq!(check_code, 0, "--check failed: {check_out}");
    assert!(check_out.contains("lane consistent"), "{check_out}");
}

// --- the F6 pin: an EMPTY lane files INSIDE the lane, never the next section ----

/// The empty-lane TODO.md: the `## Proposed` lane exists with its table
/// skeleton (header + separator) but ZERO P-rows, and a `## Open` section
/// with its own table follows. The pre-fix insertion scan skipped the
/// skeleton and landed the new row inside the NEXT section's table
/// (immediately before `| T2 |`) — the F6 latent bug: cross-section
/// bookkeeping corrupted the moment the lane ever empties.
const TODO_EMPTY_LANE: &str = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T1 | title | specs/T1-slug.md | 2 | done | notes |\n\
\n\
## Proposed\n\
\n\
Machine-maintained lane (T326): proposals land here with status `proposed`.\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
\n\
## Open\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T2 | an open row | specs/T2-open.md | 2 | open | notes |\n";

#[test]
fn an_empty_lane_files_inside_the_lane_not_the_next_section() {
    let f = ideate_fixture();
    std::fs::write(f.path().join("TODO.md"), TODO_EMPTY_LANE).expect("write empty-lane TODO");
    let sock = f.join("noul.sock");
    serve_noul(&sock, &noul_answer("no"));
    let (out, code) = run_ideate(f.path(), &[], Some(&sock));
    assert_eq!(code, 0, "ideate.py cycle exited {code}: {out}");
    assert!(
        out.contains("ideation filed (T326): "),
        "the cycle filed at least one proposal: {out}"
    );

    let todo = std::fs::read_to_string(f.path().join("TODO.md")).expect("read TODO.md");
    let lines: Vec<&str> = todo.lines().collect();

    // The lane's own span: from the `## Proposed` heading, past its table
    // separator, up to the next `## ` heading.
    let lane_at = lines
        .iter()
        .position(|l| l.trim() == "## Proposed")
        .expect("the lane heading survives");
    let sep_at = lines
        .iter()
        .enumerate()
        .skip_while(|(i, _)| *i <= lane_at)
        .find(|(_, l)| l.trim_start().starts_with("|--"))
        .map(|(i, _)| i)
        .expect("the lane's table separator survives");
    let open_at = lines
        .iter()
        .position(|l| l.trim() == "## Open")
        .expect("the next section heading survives");

    // EVERY P-row sits INSIDE the lane span — directly after the lane's own
    // separator, never past the `## Open` boundary (the pre-fix bug landed
    // the row before `| T2 |`, inside the Open table).
    let prows: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with("| P"))
        .map(|(i, _)| i)
        .collect();
    assert!(
        !prows.is_empty(),
        "the cycle filed a row: {out}\n{todo}"
    );
    for i in &prows {
        assert!(
            *i > sep_at && *i < open_at,
            "the filed row must land inside the `## Proposed` lane (after its \
             separator at {sep_at}, before `## Open` at {open_at}) — probed row at \
             line {}: {}\n{todo}",
            i + 1,
            lines[*i]
        );
    }
    assert_eq!(
        lines[sep_at + 1],
        lines[*prows.first().expect("probed")],
        "the first filed row sits DIRECTLY after the lane's separator"
    );

    // The `## Open` section is byte-untouched — its table still ends with
    // the T2 row, unshifted, uncorrupted.
    let open_tail = "## Open\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| T2 | an open row | specs/T2-open.md | 2 | open | notes |\n";
    assert!(
        todo.contains(open_tail),
        "the `## Open` section must be byte-identical after the filing: {todo}"
    );

    // And the post-cycle lane passes --check.
    let (check_out, check_code) = run_ideate(f.path(), &["--check"], Some(&sock));
    assert_eq!(check_code, 0, "--check failed over the empty-lane filing: {check_out}");
    assert!(check_out.contains("lane consistent"), "{check_out}");
}

// --- spec bullet 2: the noul gate — meta rejected, outward passed, fail CLOSED --

#[test]
fn the_noul_gate_rejects_meta_and_passes_outward_fail_closed() {
    let f = ideate_fixture();
    let stub = |name: &str, title: &str| {
        let path = f.join(name);
        std::fs::write(
            &path,
            format!(
                "# P900 — {title}\n\n\
                 - **User-visible value**: v\n\
                 - **First slice**: s\n\
                 - **Estimate class**: S.\n\
                 - **Source fact**: f\n"
            ),
        )
        .expect("write gate stub");
        path
    };
    let meta = stub("meta-stub.md", "add wrap_assert leg");
    let outward = stub("outward-stub.md", "chug.sh cost dashboard");

    // The mocked noul classifies the stub META → the gate rejects (exit 2),
    // with the spec's exact log line.
    let yes_sock = f.join("noul-yes.sock");
    serve_noul(&yes_sock, &noul_answer("yes"));
    let (out, code) = run_ideate_gate(f.path(), &meta, &yes_sock);
    assert_eq!(code, 2, "the meta stub must be REJECTED (exit 2): {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class add wrap_assert leg"),
        "the reject line: {out}"
    );

    // The mocked noul classifies the stub OUTWARD → the gate passes (exit 0).
    let no_sock = f.join("noul-no.sock");
    serve_noul(&no_sock, &noul_answer("no"));
    let (out, code) = run_ideate_gate(f.path(), &outward, &no_sock);
    assert_eq!(code, 0, "the outward stub must PASS (exit 0): {out}");
    assert!(
        out.contains("ideation gate (T326): PASS chug.sh cost dashboard"),
        "the pass line: {out}"
    );

    // The gate's question shape reached the daemon (System One classification).
    let captured = NOUL_CAPTURED.lock().expect("captured map").clone();
    assert!(
        captured.iter().any(|b| b.contains("meta_class") && b.contains("noul")),
        "the gate never asked a meta_class question: {captured:?}"
    );

    // FAIL CLOSED: the daemon unavailable → rejection (exit 2), even for the
    // outward stub — better silent than meta.
    let (out, code) = run_ideate_gate(f.path(), &outward, &f.join("no-such-daemon.sock"));
    assert_eq!(code, 2, "an unavailable daemon must fail CLOSED: {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class chug.sh cost dashboard"),
        "the fail-closed reject line: {out}"
    );
}

/// The F3 legs: the gate's fail-CLOSED is pinned on EVERY bad-daemon shape,
/// not just daemon-ABSENT. Only an explicit confident "no" on a 200 passes;
/// everything else — non-200, unparsable body, missing choice key, a hedge
/// choice — rejects. Each leg names the mutant it kills.
#[test]
fn the_noul_gate_fails_closed_on_bad_daemon_answers() {
    let f = ideate_fixture();
    let outward = f.join("outward-stub.md");
    std::fs::write(
        &outward,
        "# P901 — chug.sh cost dashboard\n\n\
         - **User-visible value**: v\n\
         - **First slice**: s\n\
         - **Estimate class**: S.\n\
         - **Source fact**: f\n",
    )
    .expect("write outward stub");

    // Leg 1 (kills M7, the non-200 check dropped): a 503 carrying a VALID
    // outward "no" body must STILL reject — the status alone fails closed.
    // Without the status check the valid body would parse and PASS.
    let s = f.join("noul-503.sock");
    serve_noul_status(&s, 503, &noul_answer("no"));
    let (out, code) = run_ideate_gate(f.path(), &outward, &s);
    assert_eq!(code, 2, "a non-200 answer must fail CLOSED even with a valid body: {out}");

    // Leg 2 (kills M2's wider family — the except-path): a 200 with an
    // UNPARSABLE body rejects (json.loads raises → the gate's failure arm).
    let s = f.join("noul-garbage.sock");
    serve_noul(&s, "not-json-at-all");
    let (out, code) = run_ideate_gate(f.path(), &outward, &s);
    assert_eq!(code, 2, "a garbage body must fail CLOSED: {out}");

    // Leg 3 (kills M3, the choice check loosened to != "yes"): a 200 with
    // valid JSON whose meta_class answer carries NO choice key rejects.
    // Under `!= "yes"` the missing key (None) would pass the gate.
    let s = f.join("noul-nochoice.sock");
    serve_noul(
        &s,
        "{\"model\":\"mock-noul\",\"answers\":{\"meta_class\":{\"type\":\"choice\",\"confidence\":0.93}},\"usage\":{}}",
    );
    let (out, code) = run_ideate_gate(f.path(), &outward, &s);
    assert_eq!(code, 2, "a missing-choice body must fail CLOSED: {out}");

    // Leg 4 (also kills M3): a 200 with choice "maybe" — neither the
    // outward "no" nor the meta "yes" — rejects. Only an explicit
    // confident "no" passes; a hedge is not an approval.
    let s = f.join("noul-maybe.sock");
    serve_noul(&s, &noul_answer("maybe"));
    let (out, code) = run_ideate_gate(f.path(), &outward, &s);
    assert_eq!(code, 2, "a hedged 'maybe' choice must fail CLOSED: {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class chug.sh cost dashboard"),
        "every fail-closed leg logs the spec's reject line: {out}"
    );
}

/// `--gate-only` takes the stub file as its OWN argument (not the root).
fn run_ideate_gate(root: &Path, stub: &Path, sock: &Path) -> (String, i32) {
    run_ideate_gate_env(root, stub, sock, &[])
}

/// The gate runner with explicit extra env (the T332 provider legs set the
/// selection keys; every UNLISTED decision var is stripped, so a leg tests
/// exactly the env it names).
fn run_ideate_gate_env(
    root: &Path,
    stub: &Path,
    sock: &Path,
    envs: &[(&str, &str)],
) -> (String, i32) {
    let mut cmd = Command::new("python3");
    cmd.arg(repo_root().join("scripts/ideate.py"))
        .arg("--gate-only")
        .arg(stub)
        .current_dir(root)
        .env("HOME", root)
        .env("LC_ALL", "C")
        .env("CHUG_NOUL_SOCK", sock)
        .env_remove("CHUG_DECISION_PROVIDER")
        .env_remove("CHUG_DECISION_PROXY_URL")
        .env_remove("CHUG_DECISION_PROXY_TOKEN");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let out = cmd.output().expect("spawn ideate.py --gate-only");
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

// --- T332: the noul gate's PROXY provider (the /v1/systemone contract) --------

use std::net::TcpListener;
use std::sync::Arc;

/// A LOCAL mock of the proxy's /v1/systemone surface: a TCP server on an
/// ephemeral 127.0.0.1 port serving ONE canned response to every POST and
/// capturing every raw request so the pins can assert the path, the
/// Authorization header, and the model field. NEVER a live network call —
/// the listener is loopback-only and dies with the test (the
/// loopd_laya_triage MockProxy shape, sized for this battery).
struct GateProxyMock {
    port: u16,
    captured: Arc<Mutex<Vec<String>>>,
}

impl GateProxyMock {
    fn serve(status: u16, body: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the mock gate proxy");
        let port = listener.local_addr().expect("local addr").port();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let capt = captured.clone();
        let body = body.to_string();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut buf = Vec::new();
                let mut chunk = [0u8; 16384];
                // Read to the end of the head AND the declared body (the
                // serve_noul shape — a single read may split the stream).
                loop {
                    match stream.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                    let text = String::from_utf8_lossy(&buf).to_string();
                    let Some(head_end) = text.find("\r\n\r\n") else {
                        continue;
                    };
                    let want = text
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse::<usize>().ok())
                        })
                        .flatten()
                        .unwrap_or(0);
                    if buf.len() >= head_end + 4 + want {
                        break;
                    }
                }
                capt.lock().expect("captured requests").push(
                    String::from_utf8_lossy(&buf).to_string(),
                );
                let reason = if status == 200 { "OK" } else { "Service Unavailable" };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(resp.as_bytes());
                let _ = stream.flush();
            }
        });
        Self { port, captured }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn requests(&self) -> Vec<String> {
        self.captured.lock().expect("captured requests").clone()
    }
}

/// The canned /v1/systemone envelope (the recorded contract's answers
/// shape — identical to the daemon's). Fixture values only: the real
/// endpoint and any real token NEVER appear in a tracked file.
fn systemone_answer(choice: &str) -> String {
    format!(
        "{{\"model\":\"typesafe/jev-fixture\",\"answers\":{{\"meta_class\":{{\"type\":\"choice\",\"choice\":\"{choice}\",\"confidence\":0.93}}}},\"usage\":{{\"input_tokens\":11,\"output_tokens\":3,\"cost\":0.0002}},\"id\":\"gen-dec-fixture\",\"provider\":\"TypeSafe\"}}"
    )
}

const GATE_FIXTURE_TOKEN: &str = "decision-GATE-FIXTURE-TOKEN-42";

/// An outward-facing gate stub (the T326 stub shape, three cells + source).
fn gate_stub(f: &Path, name: &str, title: &str) -> PathBuf {
    let path = f.join(name);
    std::fs::write(
        &path,
        format!(
            "# P902 — {title}\n\n\
             - **User-visible value**: v\n\
             - **First slice**: s\n\
             - **Estimate class**: S.\n\
             - **Source fact**: f\n"
        ),
    )
    .expect("write gate stub");
    path
}

/// T332 leg 1 — the proxy provider's verdicts parse exactly like the
/// daemon's: an OUTWARD classification passes (exit 0), a META
/// classification rejects (exit 2), and the wire carried the contract's
/// required shape (POST /v1/systemone, Bearer token, the model field, the
/// same meta_class question + proposal state).
#[test]
fn the_noul_gate_consults_the_proxy_with_the_daemons_verdict_semantics() {
    let f = ideate_fixture();
    let outward = gate_stub(f.path(), "outward-stub.md", "chug.sh cost dashboard");
    let meta = gate_stub(f.path(), "meta-stub.md", "add wrap_assert leg");

    // OUTWARD through the proxy → PASS (exit 0), the daemon's exact pass line.
    let proxy = GateProxyMock::serve(200, &systemone_answer("no"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &[
            ("CHUG_DECISION_PROVIDER", "proxy"),
            ("CHUG_DECISION_PROXY_URL", &proxy.url()),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 0, "an outward stub must PASS through the proxy: {out}");
    assert!(
        out.contains("ideation gate (T326): PASS chug.sh cost dashboard"),
        "the pass line is provider-blind: {out}"
    );

    // META through the proxy → REJECT (exit 2), the daemon's exact reject line.
    let proxy_meta = GateProxyMock::serve(200, &systemone_answer("yes"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &meta,
        &f.join("no-such-daemon.sock"),
        &[
            ("CHUG_DECISION_PROVIDER", "proxy"),
            ("CHUG_DECISION_PROXY_URL", &proxy_meta.url()),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 2, "a meta stub must REJECT through the proxy: {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class add wrap_assert leg"),
        "the reject line is provider-blind: {out}"
    );
    assert_eq!(proxy_meta.requests().len(), 1, "one proxy call for the meta leg");

    // the wire: POST /v1/systemone, Bearer token, the model field, and the
    // SAME question/state body the daemon leg builds
    let reqs = proxy.requests();
    assert_eq!(reqs.len(), 1, "exactly one proxy call per gate: {reqs:?}");
    let req = &reqs[0];
    assert!(
        req.starts_with("POST /v1/systemone"),
        "the recorded path: {}",
        req.lines().next().unwrap_or_default()
    );
    assert!(req.contains("Authorization: Bearer "), "the token rides as Bearer: {req}");
    assert!(req.contains("\"model\": \"jev\""), "the contract's required model field: {req}");
    assert!(
        req.contains("\"meta_class\"") && req.contains("proposal_stub"),
        "the same gate body rides (question + state): {req}"
    );
}

/// The provider=proxy env triple (one URL argument — a free fn so the
/// elided lifetimes unify across the static and dynamic members).
fn proxy_env(url: &str) -> [(&str, &str); 3] {
    [
        ("CHUG_DECISION_PROVIDER", "proxy"),
        ("CHUG_DECISION_PROXY_URL", url),
        ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
    ]
}

/// T332 leg 2 — the fail-CLOSED degrade is transport-blind: a proxy that is
/// DOWN, a non-200, an UNPARSABLE body, and a proxy configured but with NO
/// endpoint URL all reject the outward stub (exit 2), byte-for-byte the
/// daemon-down arms. A hedge choice through the proxy rejects too — only an
/// explicit confident 'no' passes, whichever transport answered.
#[test]
fn the_noul_gate_fails_closed_when_the_proxy_degrades() {
    let f = ideate_fixture();
    let outward = gate_stub(f.path(), "outward-stub.md", "chug.sh cost dashboard");

    // Leg 1 (the dead endpoint): an unroutable loopback port — nothing listens.
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind the dead port");
    let dead_port = listener.local_addr().expect("local addr").port();
    drop(listener);
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &proxy_env(&format!("http://127.0.0.1:{dead_port}")),
    );
    assert_eq!(code, 2, "a dead proxy must fail CLOSED: {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class chug.sh cost dashboard"),
        "the fail-closed reject line is transport-blind: {out}"
    );

    // Leg 2 (the non-200): a 503 carrying a VALID outward body still rejects.
    let s503 = GateProxyMock::serve(503, &systemone_answer("no"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &proxy_env(&s503.url()),
    );
    assert_eq!(code, 2, "a non-200 proxy answer must fail CLOSED: {out}");

    // Leg 3 (the unparsable body): a 200 with garbage rejects.
    let garbage = GateProxyMock::serve(200, "not-json-at-all");
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &proxy_env(&garbage.url()),
    );
    assert_eq!(code, 2, "an unparsable proxy body must fail CLOSED: {out}");

    // Leg 4 (the hedge): a 200 with a 'maybe' choice rejects — the
    // daemon's only-explicit-no rule, unchanged by the transport.
    let hedge = GateProxyMock::serve(200, &systemone_answer("maybe"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &proxy_env(&hedge.url()),
    );
    assert_eq!(code, 2, "a hedged choice must fail CLOSED: {out}");

    // Leg 5 (the unconfigured endpoint): provider=proxy with NO URL — the
    // named fix, then the same reject.
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &[
            ("CHUG_DECISION_PROVIDER", "proxy"),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 2, "an unconfigured proxy must fail CLOSED: {out}");
}

/// T332 leg 3 — the auto matrix, ideate edition: with a proxy configured,
/// a WARM daemon socket wins (the proxy is configured but never called —
/// $0 stays $0), and a DEAD socket falls through to the proxy.
#[test]
fn the_auto_matrix_prefers_a_warm_socket_and_falls_through_to_the_proxy() {
    let f = ideate_fixture();
    let outward = gate_stub(f.path(), "outward-stub.md", "chug.sh cost dashboard");

    // Warm socket: the mocked daemon answers the gate → PASS with the proxy
    // NEVER consulted (the proxy's mock serves a 'yes' that would reject —
    // a pass here proves the DAEMON answered).
    let sock = f.join("noul.sock");
    serve_noul(&sock, &noul_answer("no"));
    let proxy = GateProxyMock::serve(200, &systemone_answer("yes"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &sock,
        &[
            ("CHUG_DECISION_PROVIDER", "auto"),
            ("CHUG_DECISION_PROXY_URL", &proxy.url()),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 0, "the warm daemon answers through auto: {out}");
    assert!(
        out.contains("ideation gate (T326): PASS chug.sh cost dashboard"),
        "the pass line: {out}"
    );
    assert!(
        proxy.requests().is_empty(),
        "the configured proxy was NEVER called when the socket is warm: {:?}",
        proxy.requests()
    );

    // Dead socket: the same auto config falls through to the proxy — the
    // mock's 'no' passes, proving the proxy answered (a dead daemon cannot).
    let proxy = GateProxyMock::serve(200, &systemone_answer("no"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &f.join("no-such-daemon.sock"),
        &[
            ("CHUG_DECISION_PROVIDER", "auto"),
            ("CHUG_DECISION_PROXY_URL", &proxy.url()),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 0, "the proxy fallback answers through auto: {out}");
    assert_eq!(proxy.requests().len(), 1, "the proxy was consulted exactly once");
}

/// T332 leg 4 — an unknown CHUG_DECISION_PROVIDER mode refuses into the
/// fail-CLOSED even with a HEALTHY daemon: the choice must never be silent,
/// and it must never map to a provider the operator did not ask for.
#[test]
fn an_unknown_provider_mode_fails_closed_without_consulting_anything() {
    let f = ideate_fixture();
    let outward = gate_stub(f.path(), "outward-stub.md", "chug.sh cost dashboard");
    let sock = f.join("noul.sock");
    serve_noul(&sock, &noul_answer("no"));
    let proxy = GateProxyMock::serve(200, &systemone_answer("no"));
    let (out, code) = run_ideate_gate_env(
        f.path(),
        &outward,
        &sock,
        &[
            ("CHUG_DECISION_PROVIDER", "bogus"),
            ("CHUG_DECISION_PROXY_URL", &proxy.url()),
            ("CHUG_DECISION_PROXY_TOKEN", GATE_FIXTURE_TOKEN),
        ],
    );
    assert_eq!(code, 2, "an unknown mode must fail CLOSED: {out}");
    assert!(
        out.contains("ideation reject (T326): meta-class chug.sh cost dashboard"),
        "the refuse arm reads as any other reject: {out}"
    );
    assert!(
        proxy.requests().is_empty(),
        "no provider was consulted on a refused mode: {:?}",
        proxy.requests()
    );
}

// --- the amendment: auto-ratification flips only eligible rows ------------------

#[test]
fn the_ratify_sweep_flips_only_eligible_rows() {
    let f = ideate_fixture();
    let todo = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
## Proposed\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| P011 | old machine row | specs/proposed/p011-old-machine-row.md | 2 | proposed | machine-filed 2020-01-01 (T326 cycle 2026-W01) — awaiting operator ratification |\n\
| P012 | future machine row | specs/proposed/p012-future.md | 2 | proposed | machine-filed 2099-01-01 (T326 cycle 2099-W01) — awaiting operator ratification |\n\
| P013 | internal row | specs/proposed/p013-internal.md | 2 | proposed | APPROVED 2026-11-06 — owner fb011b00 (internal artifact, NOT loop work; do not dispatch): old |\n\
| P014 | undated row | specs/proposed/p014-undated.md | 2 | proposed | hand-filed, no date on record |\n\
| P015 | vetoed row | specs/proposed/p015-vetoed.md | 2 | vetoed | machine-filed 2020-01-01 — operator vetoed this one |\n\
| P016 | second old row | specs/proposed/p016-second.md | 2 | proposed | machine-filed 2020-06-01 (T326 cycle 2026-W23) — awaiting operator ratification |\n\
| P017 | aged never-dispatch row | specs/proposed/p017-aged-never.md | 2 | proposed | APPROVED 2020-01-01 — owner fb011b00 (internal artifact, NOT loop work; do not dispatch): aged past any boundary |\n\
| P018 | aged operator-owned row | specs/proposed/p018-aged-owned.md | 2 | proposed | machine-filed 2020-02-02 (T326 cycle 2020-W05) — operator-owned: the operator executes this out-of-band |\n";
    std::fs::write(f.path().join("TODO.md"), todo).expect("write ratify TODO");
    for (n, slug) in [
        ("p011-old-machine-row.md", "old"),
        ("p012-future.md", "future"),
        ("p013-internal.md", "internal"),
        ("p014-undated.md", "undated"),
        ("p015-vetoed.md", "vetoed"),
        ("p016-second.md", "second"),
        ("p017-aged-never.md", "aged never"),
        ("p018-aged-owned.md", "aged owned"),
    ] {
        std::fs::write(f.join("specs/proposed").join(n), format!("# {slug}\n"))
            .expect("write ratify stub");
    }
    let (out, code) = run_ideate(f.path(), &["--ratify-only"], None);
    assert_eq!(code, 0, "ratify-only exited {code}: {out}");
    assert!(
        out.contains("ideation auto-ratified (T326): P011, P016"),
        "the disposition line names exactly the eligible rows: {out}"
    );

    let after = std::fs::read_to_string(f.path().join("TODO.md")).expect("read TODO.md");
    for (id, want, what) in [
        ("P011", "open", "old machine row flips"),
        ("P016", "open", "second old row flips"),
        ("P012", "proposed", "future-dated row stays"),
        ("P013", "proposed", "never-dispatch row stays"),
        ("P014", "proposed", "undated row stays"),
        ("P015", "vetoed", "vetoed row stays vetoed"),
        // The F2 legs: AGED marker rows — the marker skip, not the age
        // check, is what keeps these proposed (both are years past the 7d
        // boundary; removing the marker skip flips them). P017 carries the
        // P005-P007 `do not dispatch` marker, P018 the `operator-owned`
        // marker — each NEVER_FLIP_MARKERS entry gets its own aged row.
        ("P017", "proposed", "aged do-not-dispatch row NEVER flips (the marker, not the age)"),
        ("P018", "proposed", "aged operator-owned row NEVER flips (the marker, not the age)"),
    ] {
        let line = after
            .lines()
            .find(|l| l.starts_with(&format!("| {id} ")))
            .unwrap_or_else(|| panic!("{id} row present"));
        let cells: Vec<&str> = line.split('|').skip(1).map(|c| c.trim()).collect();
        assert_eq!(cells[4], want, "{what}: {line}");
    }
    // The flip note is logged on the row (the amendment's visibility rule).
    let p011 = after.lines().find(|l| l.starts_with("| P011 ")).expect("P011");
    assert!(
        p011.contains("auto-ratified") && p011.contains("(T326)"),
        "the flip note: {p011}"
    );
    // Idempotent: a second sweep flips nothing (the rows read open now).
    let (out2, code2) = run_ideate(f.path(), &["--ratify-only"], None);
    assert_eq!(code2, 0, "second sweep exited {code2}: {out2}");
    assert!(
        !out2.contains("ideation auto-ratified (T326):"),
        "the second sweep re-flipped: {out2}"
    );
}

// --- the amendment's boundary: exactly 7 days, pinned statically + dynamically -

/// Today's UTC date and its -7d/-6d offsets as ISO strings (probed via
/// python3 — the same clock ideate.py's sweep reads; no chrono dependency).
fn today_utc_offsets() -> (String, String, String) {
    let out = Command::new("python3")
        .args([
            "-c",
            "import datetime; t=datetime.datetime.now(datetime.timezone.utc).date(); \
             print(t.isoformat()); \
             print((t-datetime.timedelta(days=7)).isoformat()); \
             print((t-datetime.timedelta(days=6)).isoformat())",
        ])
        .output()
        .expect("spawn python3 date probe");
    assert!(
        out.status.success(),
        "date probe failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|s| s.trim().to_string())
        .collect();
    assert_eq!(lines.len(), 3, "date probe shape: {:?}", lines);
    (lines[0].clone(), lines[1].clone(), lines[2].clone())
}

/// The F4 pin: the amendment's ratification boundary is EXACTLY 7 days —
/// a row filed 7d ago flips (age >= 7), a row filed 6d ago stays. The 6d
/// row IS the finding's 6d23h wall-age case: the sweep compares calendar
/// dates, so a row filed yesterday-at-23:59 (wall age up to ~1d short of
/// the boundary at probe time) reads `6` and must stay proposed. Mutants:
/// `<=` (flips at 6) fails the P021 leg; RATIFY_AFTER_DAYS=8 fails both;
/// both are also caught by the wiring test's static pins.
#[test]
fn the_ratify_boundary_is_exactly_seven_days() {
    let f = ideate_fixture();
    let (_today, d7, d6) = today_utc_offsets();
    let todo = format!(
        "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
## Proposed\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| P020 | boundary row | specs/proposed/p020-boundary.md | 2 | proposed | machine-filed {d7} (T326 cycle) — awaiting operator ratification |\n\
| P021 | inside row | specs/proposed/p021-inside.md | 2 | proposed | machine-filed {d6}T23:59 (T326 cycle) — awaiting operator ratification |\n"
    );
    std::fs::write(f.path().join("TODO.md"), todo).expect("write boundary TODO");
    for (n, body) in [
        ("p020-boundary.md", "# P020 — boundary row\n"),
        ("p021-inside.md", "# P021 — inside row\n"),
    ] {
        std::fs::write(f.join("specs/proposed").join(n), body).expect("write boundary stub");
    }
    let (out, code) = run_ideate(f.path(), &["--ratify-only"], None);
    assert_eq!(code, 0, "ratify-only exited {code}: {out}");
    assert!(
        out.contains("ideation auto-ratified (T326): P020")
            && !out.contains("P021"),
        "exactly the 7d-old row flips; the 6d(-23h) row stays: {out}"
    );
    let after = std::fs::read_to_string(f.path().join("TODO.md")).expect("read TODO.md");
    for (id, want, what) in [
        ("P020", "open", "the exactly-7d-old row flips"),
        ("P021", "proposed", "the 6d23h row stays proposed"),
    ] {
        let line = after
            .lines()
            .find(|l| l.starts_with(&format!("| {id} ")))
            .unwrap_or_else(|| panic!("{id} row present"));
        let cells: Vec<&str> = line.split('|').skip(1).map(|c| c.trim()).collect();
        assert_eq!(cells[4], want, "{what}: {line}");
    }
}

// --- spec bullet 3: the loopd fixture legs --------------------------------------

#[test]
fn dormant_and_due_launches_the_ideation_child_on_glm() {
    let f = fixture();
    // The EVALUATION audit is NOT due (fresh anchor, fresh eval half); the
    // queue is empty and the valve streak is tripped → the suspended-valve
    // skip is the ONLY launch path this cycle — exactly where ideation rides.
    f.backdate_anchor(2 * DAY);
    f.wraps(4);
    f.seed_lane(9 * DAY); // the lane anchor is 9d old → the timer is DUE
    f.latch_ideation(9 * DAY); // the record half agrees (the newer decides)
    // The arming condition holds: the lane exists in git (the anchor is real).
    let now0 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let lane = f.lane_anchor_ts().expect("the lane anchor exists (armed)");
    assert!(
        (lane - (now0 - 9 * DAY as i64)).abs() < 120,
        "the lane anchor is the 9d-old seed commit: {lane}"
    );
    f.run_one_pass();

    // The stub records one argv token per line — flatten for the shape pins.
    let argv = flat(&f.child_argv());
    assert!(
        argv.contains("--spec specs/ideation-cycle.md"),
        "the ideation launch carries the ideation spec: {argv}"
    );
    assert!(
        argv.contains(&format!("--model {GLM_ID}")),
        "the ideation launch runs on glm (the full provider-routed id): {argv}"
    );
    assert!(
        !argv.contains("--model anthropic-system.ai.kimi-k3"),
        "the ideation launch must never be kimi: {argv}"
    );
    assert!(argv.contains("--max-iters 16"), "the bounded child budget: {argv}");
    assert!(argv.contains("--max-minutes 15"), "the bounded child budget: {argv}");

    let log = f.fixture_log();
    assert!(
        log.contains("ideation cycle (T326): timer due — launching specs/ideation-cycle.md"),
        "the launch line: {log}"
    );
    assert!(
        log.contains("ideation cycle (T326): child ok — timer stamped"),
        "the stamp line: {log}"
    );
    // The stamp: the ideation half is NOW, the eval half is PRESERVED.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let ige = f.latch_ideation_ts().expect("the stamped ideation half");
    assert!(ige > now - 120, "the ideation stamp is fresh: {ige} vs now {now}");
    let lev = f.latch_eval_ts().expect("the preserved eval half");
    let want = now - 2 * DAY as i64;
    assert!(
        (lev - want).abs() < 120,
        "the eval half is preserved from the record (probed {lev}, want ≈{want})"
    );
    // The cycle still wrote its T258 disposition (the leg is an addition, not
    // a replacement).
    assert!(
        log.contains("cheap-exit disposition (T258):"),
        "the disposition line: {log}"
    );
}

#[test]
fn not_dormant_launches_no_ideation() {
    let f = fixture();
    f.seed_lane(30 * DAY); // the timer WOULD be due — the queue is the guard
    f.latch_ideation(30 * DAY);
    f.commit_tree("eval: cycle-101 open row (T326 fixture)", Some(TODO_ONE_OPEN), None);
    f.run_one_pass();

    let argv = f.child_argv();
    assert!(
        !argv.contains("ideation-cycle.md"),
        "an awake cycle must never launch the ideation leg: {argv}"
    );
    assert!(
        !f.fixture_log().contains("ideation cycle (T326): timer due"),
        "no ideation launch line on an awake cycle: {}",
        f.fixture_log()
    );
    // The wake DISARMS the latch entirely (the T325 rule: any open row removes
    // the state file) — so no ideation timer can survive a wake either: the
    // next dormancy stretch re-arms both halves from scratch (the eval anchor
    // at entry; the ideation anchor from the lane's git history).
    assert!(
        !f.path().join(".chug/loopd/dormant-state.json").exists(),
        "an open row disarms the latch (the T325 wake) — no ideation half survives it"
    );
}

#[test]
fn dormant_but_not_due_or_unarmed_launches_no_ideation() {
    // (a) dormant, lane armed, timer NOT due (stamped last cycle).
    let f = fixture();
    f.backdate_anchor(2 * DAY);
    f.wraps(4);
    f.seed_lane(2 * DAY);
    f.latch_ideation(2 * DAY);
    f.run_one_pass();
    assert!(
        !f.path().join("child-argv.txt").exists(),
        "a not-due timer must not launch ANY child (the dormant skip stays childless)"
    );
    assert!(
        !f.fixture_log().contains("ideation cycle (T326): timer due"),
        "no ideation launch line on a not-due cycle: {}",
        f.fixture_log()
    );

    // (b) dormant, NO lane, NO record half → UNARMED — never due (ideation
    // needs its lane to exist; this is also the T325 compatibility pin: the
    // dormant-mode battery's fixtures have no lane, and their stub child
    // must never be launched by the ideation leg).
    let f2 = fixture();
    f2.backdate_anchor(2 * DAY);
    f2.wraps(4);
    f2.latch_raw("{\"last_eval_ts\": 0}\n");
    f2.run_one_pass();
    assert!(
        !f2.path().join("child-argv.txt").exists(),
        "the unarmed timer must not launch ANY child (the T325 dormant skip stays childless)"
    );
}

#[test]
fn an_ideation_child_failure_is_never_stamped() {
    let f = fixture();
    f.backdate_anchor(2 * DAY);
    f.wraps(4);
    f.seed_lane(9 * DAY);
    f.latch_ideation(9 * DAY);
    // The child CRASHES (exit 1): the loop pass itself still succeeds (the
    // leg is best-effort), but the timer is NOT stamped.
    f.run_one_pass_with(1);
    let log = f.fixture_log();
    assert!(
        log.contains("ideation cycle (T326): child FAILED (rc=1) — timer NOT stamped, re-fires next cycle"),
        "the failure line: {log}"
    );
    let ige = f.latch_ideation_ts().expect("the ideation half");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    assert!(
        ige <= now - 9 * DAY as i64 + 5,
        "the crashed child's timer is NOT stamped (re-fires next cycle): {ige}"
    );
}

// --- the T325-F1 anchor-max shape: the ideation timer takes the NEWER half ------

/// The F5 pin: the ideation timer's anchor is the NEWER of the record half
/// (last_ideation_ts) and the git half (newest specs/proposed commit) — the
/// T325-F1 max shape. Every prior ideation leg had the two halves EQUAL, so
/// a max->min mutation (M5) stayed green. Each leg here makes the halves
/// DIFFER, and in both directions the newer half must decide:
///   (a) stale record + fresh git → the landed-commit corrective wins → the
///       timer is NOT due (mirrors the dormant battery's
///       a_landed_audit_stops_the_duplicate_the_next_cycle_despite_the_stale_latch);
///   (b) fresh record + stale git → the stamp wins → NOT due.
/// Under max->min BOTH legs take the STALE half → due → launch → RED.
#[test]
fn the_ideation_timer_takes_the_newer_half_despite_the_stale_other() {
    // (a) the stale-record leg: the stamp was lost/never landed (30d old),
    // but a lane commit landed 1d ago — the git anchor stops the duplicate.
    let f = fixture();
    f.backdate_anchor(2 * DAY);
    f.wraps(4);
    f.seed_lane(DAY); // the git half: FRESH (1d)
    f.latch_ideation(30 * DAY); // the record half: STALE (30d)
    f.run_one_pass();
    assert!(
        !f.path().join("child-argv.txt").exists(),
        "a fresh git anchor must stop the timer despite the stale record half \
         (max->min takes the stale half and launches): {}",
        f.fixture_log()
    );
    assert!(
        !f.fixture_log().contains("ideation cycle (T326): timer due"),
        "no ideation launch line when the newer (git) half is fresh: {}",
        f.fixture_log()
    );

    // (b) the mirror leg: the record half is fresh (1d — a stamped timer),
    // the git half is stale (30d). The newer (record) half decides → NOT due.
    let f2 = fixture();
    f2.backdate_anchor(2 * DAY);
    f2.wraps(4);
    f2.seed_lane(30 * DAY); // the git half: STALE (30d)
    f2.latch_ideation(DAY); // the record half: FRESH (1d)
    f2.run_one_pass();
    assert!(
        !f2.path().join("child-argv.txt").exists(),
        "a fresh record half must stop the timer despite the stale git half: {}",
        f2.fixture_log()
    );
}

// --- spec bullet 4: the proposed-lane safety -------------------------------------

#[test]
fn a_p_only_todo_row_count_is_zero_and_dormancy_holds() {
    let f = fixture();
    // A TODO.md whose ONLY data rows are P-rows: one `proposed` machine row,
    // one auto-ratified `open` row (the amendment's shape), one
    // never-dispatch internal row. The queue is EMPTY as the gate reads it.
    let todo = "| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
## Proposed\n\
\n\
Machine-maintained lane (T326).\n\
\n\
| id | title | spec | pri | status | notes |\n\
|----|-------|------|-----|--------|-------|\n\
| P001 | machine proposal | specs/proposed/p001-machine-proposal.md | 2 | proposed | machine-filed 2026-11-06 (T326 cycle 2026-W45) — awaiting operator ratification |\n\
| P002 | ratified proposal | specs/proposed/p002-ratified-proposal.md | 2 | open | machine-filed 2026-10-30 (T326 cycle 2026-W44) — awaiting operator ratification — auto-ratified 2026-11-06 (T326) |\n\
| P003 | internal proposal | specs/proposed/p003-internal-proposal.md | 2 | proposed | APPROVED 2026-11-06 — owner fb011b00 (internal artifact, NOT loop work; do not dispatch) |\n";
    std::fs::write(f.path().join("TODO.md"), todo).expect("write P-only TODO.md");
    f.pin_eval_mtime();
    f.latch_raw("{\"last_eval_ts\": 0}\n");
    f.commit_tree("eval: t326 P-only fixture baseline", None, Some("# eval\n"));
    let gate = run_predicate(f.path(), &[("CHUG_ROUTINE_TODAY", FRESH_DAY)]);
    assert_eq!(verb_of(&gate), "skip", "a P-only queue is an empty queue: {gate}");
    assert_eq!(field_of(&gate, "rows="), "0", "P-rows count as 0: {gate}");
    assert_eq!(
        field_of(&gate, "dormant="),
        "suspended-valve",
        "dormancy holds over a P-only lane: {gate}"
    );
    // Even the auto-ratified `open` P-row never wakes the valve — the loop
    // NEVER dispatches on its own proposals (the P002 open row proves the
    // wake condition reads T-rows only: new=0 over a lane holding an open
    // proposal).
    assert!(
        !gate
            .split_whitespace()
            .any(|fld| fld == "new=1"),
        "no P-row may wake the loop: {gate}"
    );
}

// --- the static pins -------------------------------------------------------------

#[test]
fn the_ideation_wiring_is_pinned_in_loopd_sh() {
    let loopd = read("loopd.sh");

    // The one knob: 7d, once.
    count_eq(&loopd, "IDEATION_SECS=604800", 1, "the ideation timer knob");
    // The timer is built on the SAME latch (the T325 state file), one extra key.
    count_eq(&loopd, "\"last_ideation_ts\"", 3, "the ideation half's latch writes + awk reader");
    count_eq(&loopd, "last_ideation_ts", 10, "the ideation half's readers");
    // The timer anchor: the NEWER of the record and the specs/proposed git anchor.
    count_eq(&flat(&loopd), "git log -1 --format=%ct -- specs/proposed", 1, "the lane anchor");
    count_eq(&loopd, "ideation_timer_ts()", 1, "the timer's max-shape helper, defined once");

    // The launch: ONE glm call, the full provider-routed id, the bounded budgets.
    count_eq(&flat(&loopd), "--model \"$LOOP_IDEATION_MODEL\"", 1, "the glm launch");
    count_eq(&flat(&loopd), "--max-iters 16 --max-minutes 15", 1, "the child budgets");
    count_eq(
        &loopd,
        "LOOP_IDEATION_MODEL=\"${LOOP_IDEATION_MODEL:-anthropic-system.ai.glm-5-3-flash}\"",
        1,
        "the ideation model knob",
    );
    count_eq(&loopd, "IDEATION_GOAL=", 1, "the child goal, set once");
    // The ideation leg's function body never names kimi (the spec's call:
    // glm, never kimi).
    let leg = loopd
        .split("ideation_cycle() {")
        .nth(1)
        .and_then(|s| s.split("\n}").next())
        .unwrap_or("");
    assert!(!leg.contains("kimi"), "the ideation leg must never name kimi: {leg}");

    // The leg is an ADDITION to the suspended-valve skip, never a replacement:
    // the ideation_cycle call sits inside the valve branch, between the T325
    // cheap-exit echo and the T258 disposition echo.
    let cheap_exit = loopd
        .find("dormant cheap-exit (T325)")
        .expect("the T325 cheap-exit echo");
    let disposition = loopd
        .find("cheap-exit disposition (T258):")
        .expect("the T258 disposition echo");
    let call = loopd.find("\n        ideation_cycle\n").expect("the ideation call site");
    assert!(
        cheap_exit < call && call < disposition,
        "the ideation call must sit inside the suspended-valve skip, after the \
         T325 echo and before the disposition write"
    );
    let valve_at = loopd[..call]
        .rfind("= \"suspended-valve\" ]; then")
        .expect("the ideation call rides the suspended-valve branch");
    assert!(
        valve_at < call,
        "the branch condition must precede the call (the leg fires only on suspended-valve)"
    );

    // The stamp lands AFTER the child returns (the T325-F1 shape): inside
    // ideation_cycle, the state write follows the chug run invocation.
    let run_at = leg.find("./target/release/chug run").expect("the ideation launch");
    let stamp_at = leg.find("ideation_state_write").expect("the ideation stamp");
    assert!(
        stamp_at > run_at,
        "the stamp must land AFTER the child returns (a crashed child is never stamped)"
    );

    // dormant_state_write reads the ideation half BEFORE its write (the
    // preservation order) — never clobbers a recorded last_ideation_ts.
    let dsw = loopd
        .split("dormant_state_write() {")
        .nth(1)
        .and_then(|s| s.split("\n}").next())
        .unwrap_or("");
    let read_at = dsw.find("ige=$(ideation_state_ts)").expect("the read-before-write");
    let write_at = dsw.find("printf '{\"last_eval_ts\"").expect("the latch write");
    assert!(
        read_at < write_at,
        "the ideation half must be read BEFORE the eval write clobbers the latch"
    );

    // ideate.py: the gate fails CLOSED, the never-flip markers, the ≤3 cap.
    let ideate = read("scripts/ideate.py");
    // The fail-closed gate: the noul_gate body's OWN except arm, sliced —
    // the T332 healthz probe shares the file (its warm-miss arm reads the
    // same), and this pin is about THE GATE.
    let gate = ideate
        .split("def noul_gate(")
        .nth(1)
        .and_then(|s| s.split("\ndef ").next())
        .unwrap_or("");
    count_eq(gate, "except Exception:\n        return False", 1, "the fail-closed gate");
    // T332: the provider consult is wired — the third copy of the one
    // selection table, the contract's model field, the refusal arm.
    count_eq(&ideate, "def noul_provider(", 1, "the T332 provider selection, defined once");
    count_eq(&ideate, "wire[\"model\"] = \"jev\"", 1, "the proxy contract's required model field");
    count_eq(
        gate,
        "provider = noul_provider()",
        1,
        "the gate consults the selection before any transport",
    );
    count_eq(&ideate, "MAX_PROPOSALS = 3", 1, "the filing cap");
    count_eq(&ideate, "NEVER_FLIP_MARKERS = (\"do not dispatch\", \"operator-owned\")", 1, "the never-flip markers");
    count_eq(&ideate, "ideation reject (T326): meta-class", 2, "the reject log line (both the filing path and --gate-only)");
    count_eq(&ideate, "ideation auto-ratified (T326): ", 1, "the ratify log line");
    // The amendment's boundary is EXACTLY 7d (the F4 pins): the knob's value
    // and the comparison shape are BOTH pinned — a `<=` mutant flips at 6d.
    count_eq(
        &ideate,
        "RATIFY_AFTER_DAYS = 7",
        1,
        "the amendment's 7d ratify boundary (the sweep's only age knob)",
    );
    count_eq(
        &ideate,
        "if (today - filed).days < RATIFY_AFTER_DAYS:",
        1,
        "the boundary comparison (age >= 7d flips; a <= mutant flips at 6d)",
    );
    // The empty-lane insert anchor is its own named helper, defined once (F6).
    count_eq(&ideate, "def _lane_insert_at(", 1, "the F6 empty-lane insert anchor");

    // The child's spec exists and carries the check line.
    let spec = read("specs/ideation-cycle.md");
    assert!(
        spec.contains("check: python3 scripts/ideate.py --check"),
        "the ideation spec's check line"
    );
    assert!(
        spec.contains("NEVER flip a\n   `proposed` row yourself"),
        "the child spec carries the never-self-flip rule"
    );
}

// --- the predicate probe (the T325 battery's shape) -----------------------------

fn run_predicate(root: &Path, envs: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("bash");
    cmd.arg(root.join("loopd.sh"))
        .arg("predicate")
        .current_dir(root)
        .env_remove("CHUG_ROUTINE_TODAY")
        .env_remove("LOOP_ORCH_MODEL")
        .env_remove("LOOP_ROUTINE_MODEL")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.env_remove("CHUG_DAEMON_SOCK").env_remove("CHUG_HOME");
    cmd.env("HOME", root);
    // T329: the T330 profile loader resolves CHUG_PROFILE_ENV against the
    // loopd env-loader contract, not HOME alone — pin both loader files to
    // empty fixture paths (a host ~/.chug/profile.env must never leak in).
    common::pin_loopd_env_files(&mut cmd, root);
    let out = cmd.output().expect("spawn loopd.sh predicate");
    assert!(
        out.status.success(),
        "predicate probe exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn verb_of(gate: &str) -> &str {
    gate.split_whitespace().next().expect("gate line has a verb")
}

fn field_of(gate: &str, key: &str) -> String {
    gate.split_whitespace()
        .find(|f| f.starts_with(key))
        .map(|f| f[key.len()..].to_string())
        .unwrap_or_default()
}

//! T204 phase 1 (F15) — static pin: the feature-off build graph carries
//! ZERO inference deps (candle-*, hf-hub, tokenizers).
//!
//! The T204 doctrine (Cargo.toml's `daemon` feature comment) is that every
//! `chug run` keeps its lean 9-crate-compile hot path with no candle in it:
//! the ~650MB judge model loads ONCE per host in the `chug daemon` process,
//! never in a per-run process. Nothing enforces "off by default" at the
//! dependency level — an accidental non-optional candle dep or a feature
//! leak would compile silently. This test runs `cargo tree` over the
//! default (feature-off) graph and fails the moment any inference crate
//! appears, so the regression forces a deliberate re-review instead of a
//! slow hot-path compile landing unnoticed.
//!
//! Runs the real cargo (no reimplementation: the tests guard the manifest,
//! they do not duplicate cargo's resolution). `cargo test` executes test
//! binaries with the package root as cwd (the T48 doctrine), so the plain
//! invocation resolves this checkout; `--offline` keeps it read-only and
//! deterministic once the registry cache is warm (it is by test time — the
//! parent `cargo test` just built against it).

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BANNED: [&str; 5] = ["candle-core", "candle-nn", "candle-transformers", "hf-hub", "tokenizers"];

#[test]
fn feature_off_tree_has_zero_inference_deps() {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let out = Command::new(&cargo)
        .args(["tree", "--offline", "-e", "normal", "--no-default-features"])
        .output()
        .unwrap_or_else(|e| panic!("spawning {cargo} tree: {e}"));
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    for dep in BANNED {
        assert!(
            !tree.contains(dep),
            "feature-off dependency graph contains {dep} — the daemon feature \
             must keep candle/hf-hub/tokenizers out of the default build\n{tree}"
        );
    }
}

// --- T226 — the feature-off build's daemon surfaces --------------------------
//
// Two pins close the T215 LOW findings' unpinned halves, both against the
// REAL feature-off binary (CARGO_BIN_EXE_chug — the default test build IS
// the feature-lean client build):
// - the loopd (b)-leg probe blindness (`daemon --help` exit 0 in BOTH
//   builds) is anchored to the real bytes, so the sandbox stubs in
//   tests/loopd_daemon_ensure.rs mimic reality;
// - the client-side wrong-binary shape (daemon_binary() -> current_exe,
//   the K7 root cause) fails open with the fast-fail flavor and leaves the
//   refusal trail, exactly as the T215 verdict verified — pinned, not
//   changed.

/// Deadline for a `chug daemon` CLI verb to exit. MUST exceed the ensure's
/// own internal SPAWN_WAIT_BUDGET (60s — a mutant that loses the fast-fail
/// waits the full budget) plus headroom for the macOS first-exec stall
/// (a freshly rebuilt binary's first execs can sit in _dyld_start under
/// Gatekeeper/syspolicyd assessment for tens of seconds — observed live at
/// 48s): a wedged ensure fails the test bounded, never hangs the suite
/// (the T6 stub-harness rule).
const T226_CLI_DEADLINE: Duration = Duration::from_secs(150);

/// Bounded wait for a spawned CLI child: exit status + captured stderr, or
/// kill + panic at the deadline. The daemon_lifecycle.rs shape, local to
/// this family (no shared harness between integration binaries).
struct T226Output {
    status: std::process::ExitStatus,
    stderr: String,
}

fn wait_with_deadline(mut child: Child, deadline: Duration) -> T226Output {
    let started = Instant::now();
    loop {
        if child.try_wait().expect("poll the ensure child").is_some() {
            let output = child.wait_with_output().expect("reap the ensure child");
            return T226Output {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            };
        }
        if started.elapsed() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the chug child did not exit within {deadline:?}");
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// T226 (T215 LOW-(a) anchor) — the REAL feature-off build answers
/// `daemon --help` with clap's exit 0: the subcommand and its help derive
/// from the same ungated doc comments in both builds, so the loopd (b)-leg
/// probe (`chug daemon --help` exit 0) cannot discriminate this binary from
/// a feature-on release. The blindness is ACCEPTED (install.sh ships
/// release.yml's feature-on tarballs, so the carrier is safe today) and
/// pinned as accepted in tests/loopd_daemon_ensure.rs's both-shapes sandbox;
/// THIS test anchors that sandbox's shape-A stub to the real bytes: if the
/// help surface ever becomes feature-gated (a nonzero --help here), the
/// stub stops mimicking reality and both the probe's behavior and that pin
/// must be re-derived deliberately.
#[test]
fn feature_off_daemon_help_exits_zero_probe_blindness_is_real() {
    let out = Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("daemon")
        .arg("--help")
        .output()
        .expect("spawn CARGO_BIN_EXE_chug daemon --help");
    assert_eq!(
        out.status.code(),
        Some(0),
        "the feature-off build must answer `daemon --help` with clap's exit 0 \
         (the (b)-leg probe reads exactly this):\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.stdout.is_empty(),
        "clap rendered the daemon help on stdout"
    );
}

/// T226 (T215 LOW-(b)) — the client-side wrong-binary shape, pinned as the
/// verdict verified it (fail-open; NOT a behavior change). The daemon
/// client resolves its spawn target with `daemon_binary()` — current_exe
/// here, the exact K7 shape: a feature-lean build resolving ITSELF — spawns
/// it in serve mode, and the feature-off stub refuses with the compiled
/// literal. The ensure must then:
///   - FAIL FAST on the dead child: the error carries the fast-fail flavor
///     "exited before serving" (the latch:true permanent flavor), NEVER the
///     60s budget-expiry flavor ("gave up waiting" — a different message,
///     so this assertion discriminates deterministically, no wall clock);
///   - exit 1 (best-effort by contract: a nonzero ensure never blocks the
///     caller — never a panic's 101, never 0);
///   - leave the refusal trail in the daemon log exactly ONCE (one spawn
///     attempt — the ensure never retries within a call);
///   - bind no socket.
///
/// The latch SET itself (the failure stored so later judge() calls replay
/// it without re-spawning) is per-PROCESS client state, pinned at unit
/// level by src/daemon.rs's judge-path tests — which inject the binary
/// directly; THIS pin covers the leg those tests bypass: the RESOLUTION
/// (daemon_binary() -> current_exe) against the real feature-off bytes,
/// through the one-shot CLI (`daemon --ensure`) that shares the client's
/// ensure core.
#[test]
fn feature_off_wrong_binary_client_fails_open_and_leaves_the_refusal_trail() {
    let home = tempfile::tempdir().expect("daemon home tempdir");
    let child = Command::new(env!("CARGO_BIN_EXE_chug"))
        .arg("daemon")
        .arg("--ensure")
        // Host-scoped daemon home: lock, socket, and log all live INSIDE the
        // tempdir — the run must touch nothing real (the daemon_lifecycle
        // rule). Env is scoped per child via Command::env — never the test
        // process's global env (the mcp_serve.rs rule).
        .env("CHUG_HOME", home.path())
        .env_remove("CHUG_DAEMON_SOCK") // the daemon paths resolve via CHUG_HOME
        .env_remove("CHUG_DAEMON_STUB") // the REAL refusal path, never the stub backend
        .env_remove("CHUG_DELEGATE_BIN") // resolve current_exe — the K7 wrong-binary shape
        .env_remove("CHUG_LAYA_CHECKPOINT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CARGO_BIN_EXE_chug daemon --ensure");
    let out = wait_with_deadline(child, T226_CLI_DEADLINE);
    assert_eq!(
        out.status.code(),
        Some(1),
        "best-effort fail-open: exit 1, never a panic's 101, never 0\nstderr:\n{}",
        out.stderr
    );
    assert!(
        out.stderr.contains("judge daemon ensure failed"),
        "the ensure names its failure:\n{}",
        out.stderr
    );
    assert!(
        out.stderr.contains("exited before serving"),
        "the FAST-FAIL flavor: the client noticed the spawned daemon die (the \
         latch:true permanent flavor) instead of waiting out the 60s budget, \
         whose expiry flavor reads \"gave up waiting\":\n{}",
        out.stderr
    );
    let log = std::fs::read_to_string(home.path().join("daemon.log"))
        .expect("the daemon log exists (the spawn wrote it)");
    assert_eq!(
        log.matches("built without the judge daemon").count(),
        1,
        "the refusal trail, exactly once — one spawn attempt, no retry\n{log}"
    );
    assert!(
        !home.path().join("daemon.sock").exists(),
        "no socket was ever bound — the stub refused before serving"
    );
}

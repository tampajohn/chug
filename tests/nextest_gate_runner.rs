//! T82 — nextest-first gate runner (doctrine pins).
//!
//! The bounded-gates rule (META-SPEC T6; LOOP-SPEC step 3's runner rule,
//! step 5, Phase-3 wrap) prefers `cargo nextest run --release` when
//! `cargo nextest` is on PATH (mechanically: `command -v cargo-nextest`)
//! and falls back UNCONDITIONALLY to the T78 form
//! `cargo test --release -- --test-threads=4`. The spec's Tests section
//! makes the review checks explicit: the fallback is unconditional, NO
//! template hard-requires nextest, and the both-runners wall-time
//! measurement is in the acceptance path. cargo-nextest is a HOST tool —
//! `cargo install cargo-nextest` or the get.nexte.st tarball — NOT a crate
//! dependency, so Cargo.toml must stay clean of it; loopd.sh installs
//! nothing and only logs which runner cycles use (one loopd.log line at
//! startup). Every pin here is exact-count so a mutation (drop the
//! fallback clause, make a template nextest-only, add a dependency) is
//! independently observable.
//!
//! T48 doctrine: every pin resolves the docs from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via a compile-time path.
//!
//! T195 — the gate source-touch guard: every gate template aimed at a
//! shared role-keyed target dir from a NON-MAIN checkout rebinds the dir's
//! artifacts to the local checkout first (`touch src/*.rs tests/*.rs;`
//! immediately before the env-prefixed cargo invocation) — cargo's
//! mtime-only freshness check otherwise reads a foreign checkout's
//! artifacts as fresh against this checkout's older sources. The
//! `shared_dir_gate_lines_carry_the_touch_guard` pin below is the
//! structural walk; the touched-form carriers are pinned in
//! shared_target_dir.rs.

use std::path::Path;
use std::process::Command;

fn read(p: &str) -> String {
    std::fs::read_to_string(Path::new(".").join(p))
        .unwrap_or_else(|e| panic!("{p} readable from cwd: {e}"))
}

fn count_eq(hay: &str, needle: &str, want: usize, what: &str) {
    assert_eq!(
        hay.matches(needle).count(),
        want,
        "{what}: needle {needle:?} must occur exactly {want} time(s)"
    );
}

/// The nextest form must never stand alone: in every prose block of each
/// template carrier that names `cargo nextest run --release`, the fallback
/// stem `cargo test --release` appears in the SAME block — the pairing IS
/// the "never a hard dependency" guarantee, checked mechanically.
fn every_nextest_block_carries_the_fallback(text: &str, file: &str) {
    for block in text.split("\n\n") {
        if block.contains("cargo nextest run --release") {
            assert!(
                block.contains("cargo test --release"),
                "{file}: a block names the nextest runner without the \
                 fallback in the same block — that template hard-requires \
                 nextest (T82 req 1); block starts: {:?}",
                block.lines().next().unwrap_or("")
            );
        }
    }
}

/// (a) cargo-nextest is a HOST tool: nothing in Cargo.toml may depend on
/// it, and the doctrine says so.
#[test]
fn nextest_is_a_host_tool_not_a_crate_dependency() {
    let cargo_toml = read("Cargo.toml");
    assert!(
        !cargo_toml.contains("nextest"),
        "Cargo.toml must not reference nextest — cargo-nextest is a host \
         tool, NOT a crate dependency (T82 repo context)"
    );
    let loop_spec = read("LOOP-SPEC.md");
    count_eq(&loop_spec, "HOST tool", 1, "LOOP-SPEC names nextest's status");
    let readme = read("README.md");
    count_eq(
        &readme,
        "host tool, not a crate dependency",
        1,
        "README names nextest's status",
    );
}

/// (b) The fallback is unconditional: both specs, the README, and loopd.sh
/// state it, and both specs carry the T78 fallback command at their
/// carriers (counts kept in step with shared_target_dir.rs).
#[test]
fn fallback_is_unconditional_in_every_carrier() {
    let loop_spec = read("LOOP-SPEC.md");
    let meta = read("META-SPEC.md");
    let readme = read("README.md");
    let loopd = read("loopd.sh");
    // The "never a hard dependency" clause: LOOP-SPEC's runner rule (1),
    // META-SPEC §6 goal text + T6 rule (2), README (1), loopd.sh's comment
    // (1). Zero in any of these means the unconditional-fallback statement
    // was dropped from that carrier.
    count_eq(&loop_spec, "never a hard dependency", 1, "LOOP-SPEC");
    count_eq(&meta, "never a hard dependency", 2, "META-SPEC");
    count_eq(&readme, "never a hard dependency", 1, "README");
    count_eq(&loopd, "never a hard dependency", 1, "loopd.sh");
    // The unconditional wording itself, once per spec.
    count_eq(
        &loop_spec,
        "is unconditional: nextest is",
        1,
        "LOOP-SPEC runner rule states the fallback is unconditional",
    );
    count_eq(
        &meta,
        "the fallback is UNCONDITIONAL",
        1,
        "META-SPEC T6 rule states the fallback is unconditional",
    );
    // The fallback command at its carriers (see shared_target_dir.rs for
    // the per-carrier story).
    count_eq(
        &loop_spec,
        "cargo test --release -- --test-threads=4",
        3,
        "LOOP-SPEC T78-fallback carriers: step-3 template, step-3 runner \
         rule, step-5",
    );
    count_eq(
        &meta,
        "cargo test --release -- --test-threads=4",
        5,
        "META-SPEC T78-fallback carriers: §5, §6, §7, T6 rule + its \
         fallback-cap clause",
    );
}

/// (c) No template hard-requires nextest: every block that names the
/// nextest runner also names the fallback (LOOP-SPEC, META-SPEC, README),
/// and loopd.sh's if/else pairs the two branches mechanically.
#[test]
fn no_template_hard_requires_nextest() {
    for (file, text) in [
        ("LOOP-SPEC.md", read("LOOP-SPEC.md")),
        ("META-SPEC.md", read("META-SPEC.md")),
        ("README.md", read("README.md")),
    ] {
        every_nextest_block_carries_the_fallback(&text, file);
    }
    let loopd = read("loopd.sh");
    // The if-branch (nextest) and else-branch (fallback) of the startup
    // log — one `command -v cargo-nextest` check, both runners named.
    count_eq(&loopd, "command -v cargo-nextest", 1, "loopd.sh check");
    count_eq(
        &loopd,
        "gate runner: cargo nextest run --release (cargo-nextest on PATH)",
        1,
        "loopd.sh nextest-branch log line",
    );
    count_eq(
        &loopd,
        "gate runner: cargo test --release -- --test-threads=4 (fallback — cargo-nextest absent)",
        1,
        "loopd.sh fallback-branch log line",
    );
}

/// (d) loopd.sh installs nothing, checks at STARTUP (before the cycle
/// loop — the T50 re-exec re-runs it per re-exec, which is still a
/// startup), and stays syntactically valid.
#[test]
fn loopd_checks_runner_at_startup_installs_nothing() {
    let loopd = read("loopd.sh");
    assert!(
        !loopd.contains("cargo install"),
        "loopd.sh must install nothing — the runner check is a probe, not \
         an installer (T82 req 2)"
    );
    let check = loopd
        .find("command -v cargo-nextest")
        .expect("loopd.sh runs the T82 runner check");
    let cycle_loop = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh cycle loop marker");
    assert!(
        check < cycle_loop,
        "the runner check must run at startup, BEFORE the cycle loop (T82 \
         req 2: one loopd.log line at startup)"
    );
    // One log line per branch, both to $LOG, right after the loopd-start
    // line.
    let start_line = loopd.find("loopd start (pid $$)").expect("start line");
    assert!(
        check > start_line,
        "the runner check follows the startup log line (T82 req 2)"
    );
    let status = Command::new("bash")
        .arg("-n")
        .arg("loopd.sh")
        .status()
        .expect("spawn bash -n");
    assert!(
        status.success(),
        "loopd.sh must stay syntactically valid (bash -n)"
    );
}

/// (e) The both-runners measurement is in the acceptance path: the first
/// cycle after the switch runs BOTH runners once in main and records both
/// wall times in Outcomes — stated in LOOP-SPEC step 3's runner rule and
/// in META-SPEC's T6 rule (the spec's Tests section makes this a review
/// check, so it is pinned, not prose).
#[test]
fn both_runners_measurement_is_in_the_acceptance_path() {
    let loop_spec = read("LOOP-SPEC.md");
    let step3_start = loop_spec
        .find("3. **Review.**")
        .expect("LOOP-SPEC step-3 heading");
    let step4_start = loop_spec[step3_start..]
        .find("4. **Adversarial validation")
        .expect("LOOP-SPEC step-4 heading");
    let step3 = &loop_spec[step3_start..step3_start + step4_start];
    for (needle, what) in [
        ("run BOTH runners once in main", "the both-runners first cycle"),
        (
            "record both wall times in that cycle's Outcomes",
            "the Outcomes wall-time record",
        ),
    ] {
        assert!(
            step3.contains(needle),
            "LOOP-SPEC step 3's runner rule must keep {what}: {needle:?}"
        );
    }
    let meta = read("META-SPEC.md");
    for (needle, what) in [
        ("BOTH runners", "the both-runners first cycle"),
        ("both wall times in Outcomes", "the Outcomes wall-time record"),
    ] {
        assert!(
            meta.contains(needle),
            "META-SPEC's T6 rule must keep {what}: {needle:?}"
        );
    }
}

/// (f) The bounded caps wrap the nextest form: LOOP-SPEC step 3's template
/// and META-SPEC T6's examples carry the nextest command under the same
/// perl/timeout caps the fallback uses (the T78 bounded-cap rule applies
/// to the new runner unchanged). T195: LOOP-SPEC's gate template is
/// touch-guarded, so the pin follows the touched carrier.
#[test]
fn bounded_caps_wrap_the_nextest_form() {
    let loop_spec = read("LOOP-SPEC.md");
    count_eq(
        &loop_spec,
        "touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared perl -e 'alarm 280; exec @ARGV' cargo nextest run --release",
        1,
        "LOOP-SPEC step-3 template caps the nextest form under the T195 touch \
         guard (T178 re-keyed the alarm below the CHUG_BASH_TIMEOUT=300 bash \
         cap; T195 prefixed the touch)",
    );
    let meta = read("META-SPEC.md");
    count_eq(
        &meta,
        "perl -e 'alarm 280; exec @ARGV' cargo nextest run --release",
        1,
        "META-SPEC T6 macOS example caps the nextest form (T187 re-keyed the \
         alarm below the bash cap, matching T178's LOOP-SPEC templates; the \
         T6 illustration names no shared dir, so it carries no touch — T195)",
    );
    count_eq(
        &meta,
        "timeout 280 cargo nextest run --release",
        1,
        "META-SPEC T6 Linux example caps the nextest form (T187)",
    );
}

/// (g) The docs-only guard floor is explicitly exempt from the runner rule
/// — it runs one targeted test binary, where nextest's suite-wide
/// scheduling buys nothing. Pinning the exemption stops a later editor
/// from "fixing" the floor inconsistently with the runner rule.
#[test]
fn guard_floor_is_exempt_from_the_runner_rule() {
    let loop_spec = read("LOOP-SPEC.md");
    count_eq(
        &loop_spec,
        "The docs-only guard floor below is exempt",
        1,
        "LOOP-SPEC states the guard-floor exemption",
    );
}

/// (h) T195 — the gate source-touch guard, structural: every `perl -e
/// 'alarm 280` gate line whose env prefix names a `target-shared` dir
/// OTHER than `target-shared-main` is immediately preceded by the touch
/// guard (`touch src/*.rs tests/*.rs;` directly before the env-prefixed
/// cargo invocation). Mechanism: cargo's freshness check is mtime-only and
/// its artifact filename excludes the checkout path, so a shared role dir
/// can hold a foreign checkout's mtime-fresh artifacts against this
/// checkout's older sources — the touch rebinds them to the local checkout
/// at gate time (`;` not `&&`, so a touch hiccup never blocks the gate).
/// Exempt: `target-shared-main` env prefixes (T57 — every builder in that
/// dir is a main checkout and git refreshes mtimes on merge/checkout) and
/// alarm-280 lines with NO env prefix on the line (META-SPEC T6's cap
/// illustration, not a gate template). Exact walked-line counts per file
/// keep carrier drift observable: a new shared-dir gate line without the
/// touch dies on the prefix assert; a removed or re-keyed template dies on
/// the count.
#[test]
fn shared_dir_gate_lines_carry_the_touch_guard() {
    const TOUCH_PREFIX: &str = "touch src/*.rs tests/*.rs; CARGO_TARGET_DIR=";
    for (file, want) in [("LOOP-SPEC.md", 2usize), ("META-SPEC.md", 0)] {
        let text = read(file);
        let mut walked = 0usize;
        for line in text.lines() {
            if !line.contains("perl -e 'alarm 280") {
                continue;
            }
            // The env prefix must sit on the same line to key the gate to a
            // shared dir; a line without one is a cap illustration.
            let dir = match line.split("CARGO_TARGET_DIR=").nth(1) {
                Some(rest) => rest.split_whitespace().next().unwrap_or(""),
                None => continue,
            };
            if dir.contains("target-shared-main") {
                continue; // T57: every builder in that dir is a main checkout
            }
            assert!(
                dir.contains("target-shared"),
                "{file}: an alarm-280 gate line's env prefix does not name a \
                 target-shared dir — unexpected carrier form: {line}"
            );
            let prefix = line.split("perl -e 'alarm 280").next().unwrap_or("");
            assert!(
                prefix.contains(TOUCH_PREFIX),
                "{file}: a shared-dir gate line lacks the T195 touch guard \
                 immediately before its cargo invocation: {line}"
            );
            walked += 1;
        }
        assert_eq!(
            walked, want,
            "{file}: the T195 structural pin must walk exactly {want} \
             shared-dir alarm-280 gate line(s) — a drift means a carrier \
             was added, removed, or re-keyed without amending this pin"
        );
    }
}

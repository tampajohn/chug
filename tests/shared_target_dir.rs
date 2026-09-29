//! T47 — shared CARGO_TARGET_DIR for worktree builds.
//!
//! Doctrine pins, not behavior: every round's worktree build (impl child,
//! validator's mutate→test→revert→re-test, orchestrator gates) must land in
//! the shared `target-shared/` cache — a NEW dir, never the repo's own
//! `target/` (the operator's build cache stays separate so a wedge can't
//! poison daily builds). These tests pin the templates and supervisor wiring
//! so a later edit cannot silently drop a carrier, blur the separation, or
//! reintroduce the two defects the T47 fix-up round killed: a bare `export`
//! inside loopd's while loop (stale `./target/debug/chug` from cycle 2 on)
//! and weak `>= N` counts that survived multi-carrier mutants.
//!
//! T52 — role-keyed dirs: T44's overlap puts TWO cargo consumers from
//! DIFFERENT checkouts on one target dir, and cargo's artifact metadata hash
//! excludes the checkout path — identical package+profile+features yield the
//! SAME artifact filename, so last-builder-wins and a gate can execute
//! another checkout's binary (observed cycle 22: the t49 gates ran a
//! validator's leftover mutant). Validators therefore build into
//! `target-shared-validate/` ALWAYS (serial role → one dir suffices);
//! overlap-window gates into `target-shared-gates/`; impl children keep
//! `target-shared/`. These pins guard the new carriers the same way.
//!
//! T57 — the main-dedicated gates dir: the SEQUENTIAL residue of that class.
//! At T55's post-merge gates, step 3's worktree gates had compiled the
//! worktree's PRE-T53 `tests/loopd_reexec.rs` into `target-shared`; the
//! merge didn't touch that file, so its main-checkout mtime stayed OLDER
//! than the artifact and cargo ran the stale binary as fresh (3/4 false
//! red; the false-GREEN leg — a stale passing binary masking a real main
//! failure — is silent). Post-merge (step 5) and final main (Phase 3)
//! gates therefore build into `target-shared-main/` ALWAYS — a dir whose
//! builders are ALWAYS main checkouts, so its artifacts are main content
//! by construction. These pins guard the new carriers (step 5's re-run,
//! Phase 3's final gates, the .gitignore line, the README clause) and that
//! the dir never leaks into META-SPEC.md or loopd.sh.
//!
//! T78 — release builds for the loop: the supervisor builds and launches the
//! RELEASE binary (`cargo build --release` → `./target/release/chug`), the
//! META-SPEC nohup templates launch `target/release/chug` (delegate children
//! re-launch the orchestrator's own executable — no binary path is needed in
//! LOOP-SPEC's delegate template), and the bounded review/merge/validation
//! gates run the release profile — T82: the nextest-first gate runner
//! (`cargo nextest run --release` when `cargo nextest` is on PATH, else the
//! fallback `cargo test --release -- --test-threads=4`). These pins also
//! enforce the sweep side:
//! no `target/debug/chug` launch path may survive in loopd.sh or either
//! template. The spec `check:` convention stays plain `cargo test` (debug) —
//! release is for gates only.

use std::path::PathBuf;

const SHARED: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared";
/// T52: the validator's role-keyed dir — ALWAYS, not conditionally.
const VALIDATE: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate";
/// T52: the orchestrator's gate dir for the T44 overlap window (incl.
/// N+1's impl during N's post-merge gates).
const GATES: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-gates";
/// T57: the main-dedicated gates dir — step 5's post-merge re-run and
/// Phase 3's final gates build here ALWAYS, never conditionally on
/// overlap (a dir whose builders are ALWAYS main checkouts keeps
/// post-merge artifacts identical to main content by construction).
const MAIN: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-main";
/// The repo's OWN target dir — no template may ever point CARGO_TARGET_DIR at
/// it (that is the separation the T47 review is required to check).
const REPO_TARGET: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target";
/// T79: the per-mutant-leg role-keyed target dir — LOOP-SPEC step 4 spells
/// it in full; META-SPEC §6's goal text carries the repo-root-agnostic
/// `...` elision (the T79 spec's own spelling).
const MUT_DIR: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-mut-<k>";
/// T79: one throwaway worktree per mutant leg.
const MUT_WT: &str = "/tmp/chug-mut-<item>-<k>";

// T48: cargo runs test binaries with cwd = the package root; the compile-time env! path is wrong under the T47 shared cache (cycle-21) — resolve at runtime.
fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

fn assert_contains(haystack: &str, needle: &str, what: &str) {
    assert!(
        haystack.contains(needle),
        "{what} must contain {needle:?} (T47)"
    );
}

/// Per-carrier pin: `needle` must occur EXACTLY `expected` times. A bare
/// `>= N` count over N carriers is what let T47's mutant M6 (both nohup
/// prefixes dropped) survive; an exact per-carrier count dies on a drop of
/// any one carrier — and names the carrier in the failure message.
fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let found = haystack.matches(needle).count();
    assert_eq!(
        found, expected,
        "{what}: expected carrier {needle:?} exactly {expected}×, found \
         {found}× — a dropped (or duplicated) carrier breaks the T47 wiring \
         this pin guards"
    );
}

#[test]
fn gitignore_ignores_target_shared() {
    let gitignore = read(".gitignore");
    assert!(
        gitignore.lines().any(|l| l.trim() == "target-shared/"),
        ".gitignore must gain a `target-shared/` line (T47); got:\n{gitignore}"
    );
}

#[test]
fn gitignore_ignores_the_t52_role_keyed_sibling_dirs() {
    let gitignore = read(".gitignore");
    // Both T52 siblings are ignored like target-shared/ itself (the caches
    // are machine-local build output, never committed).
    let shared = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared/")
        .expect(".gitignore must keep the T47 `target-shared/` line");
    let mut ats = vec![shared];
    for dir in ["target-shared-validate/", "target-shared-gates/"] {
        ats.push(
            gitignore
                .lines()
                .position(|l| l.trim() == dir)
                .unwrap_or_else(|| {
                    panic!(".gitignore must gain a `{dir}` line (T52); got:\n{gitignore}")
                }),
        );
    }
    // NEXT TO the T47 entry — one contiguous block, not appended at some
    // distant end of the file (req 2's "next to target-shared/").
    ats.sort_unstable();
    assert_eq!(
        ats[2] - ats[0],
        2,
        "the three target-shared* cache lines must form one contiguous block \
         in .gitignore (T52); got:\n{gitignore}"
    );
}

#[test]
fn loopd_creates_target_shared_at_supervisor_start() {
    let loopd = read("loopd.sh");
    let mkdir = loopd
        .find("mkdir -p target-shared")
        .expect("loopd.sh must `mkdir -p target-shared` (T47)");
    // At supervisor start: before the cycle loop and before the first build,
    // so the shared cache exists for every cycle including the first.
    let first_build = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target\" cargo build --release >> \"$LOG\" 2>&1 || build_rc=$?")
        .expect("loopd.sh builds its own binary");
    let loop_start = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    assert!(
        mkdir < first_build && mkdir < loop_start,
        "loopd.sh must create target-shared at supervisor start — before the \
         cycle loop and the first `cargo build --release` (T47)"
    );
}

#[test]
fn loopd_prefixes_the_chug_invocation_never_the_supervisor_env() {
    let loopd = read("loopd.sh");
    // Regression (T47 fix-up finding 1, proven by a 2-cycle simulation): the
    // old bare `export CARGO_TARGET_DIR=` sat INSIDE the while loop, so it
    // persisted in the supervisor's environment across iterations — cycle 1
    // built ./target first, but cycle 2+'s `cargo build` ran with the var
    // set, went into target-shared, and ./target/debug/chug was never
    // rebuilt (stale supervisor binary). The export form is banned outright.
    assert!(
        !loopd.contains("export CARGO_TARGET_DIR"),
        "loopd.sh must NOT `export CARGO_TARGET_DIR` — inside the while loop \
         the export persists across iterations and redirects the \
         supervisor's own `cargo build` into target-shared from cycle 2 on, \
         leaving ./target/debug/chug stale (T47 finding 1)"
    );
    // The cycle's orchestrator instead gets the var as a per-invocation env
    // prefix on the chug call: only that process — and the delegate children
    // that inherit its launch env — sees the shared cache.
    let invocation = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target-shared\" ./target/release/chug run")
        .expect(
            "loopd.sh must env-prefix the chug invocation itself with \
             CARGO_TARGET_DIR=\"$ROOT/target-shared\" so only the cycle's \
             orchestrator (and its delegate children) builds into the \
             shared cache (T47)",
        );
    // Ordering pin: the supervisor builds its own binary FIRST — into
    // ./target, which is what `./target/release/chug` resolves to — and the
    // shared-cache prefix applies only to the cycle invocation after it.
    let build = loopd
        .find("CARGO_TARGET_DIR=\"$ROOT/target\" cargo build --release >> \"$LOG\" 2>&1 || build_rc=$?")
        .expect("loopd.sh builds its own binary first");
    assert!(
        build < invocation,
        "loopd.sh's own `cargo build --release` (pinned to ./target, its rc \
         latched — T137) must precede the env-prefixed chug invocation — the \
         supervisor's binary cache stays separate from the shared children's \
         cache (T47)"
    );
}

#[test]
fn loop_spec_templates_export_the_shared_dir() {
    let spec = read("LOOP-SPEC.md");
    assert_contains(&spec, SHARED, "LOOP-SPEC.md");
    // Per-carrier pins — each of the three carriers is asserted by name and
    // location, so dropping any ONE of them must fail this test (delegate
    // has no env parameter and bash calls don't share env, so no carrier is
    // redundant).
    // (1) step-1 worktree build line.
    count_eq(
        &spec,
        &format!("{SHARED}` then"),
        1,
        "LOOP-SPEC step-1 worktree build line (T47)",
    );
    // (2) step-2 delegate goal text — carries the export to the child.
    count_eq(
        &spec,
        &format!("{SHARED} before"),
        1,
        "LOOP-SPEC step-2 delegate goal text (T47)",
    );
    // (3) step-3 review-gate env prefix — both legs of the T82 runner
    //     (nextest leg and fallback leg) carry the T47 prefix.
    count_eq(
        &spec,
        &format!("{SHARED} perl -e 'alarm 600; exec @ARGV' cargo nextest run --release"),
        1,
        "LOOP-SPEC step-3 review-gate prefix, nextest leg (T47+T82)",
    );
    count_eq(
        &spec,
        &format!("{SHARED} perl -e 'alarm 600; exec @ARGV' cargo test --release"),
        1,
        "LOOP-SPEC step-3 review-gate prefix, fallback leg (T47+T82)",
    );
    // Tradeoff note: recovery is operator-owned and cheap.
    assert_contains(&spec, "rm -rf target-shared", "LOOP-SPEC.md");
}

#[test]
fn loop_spec_validator_exports_the_validate_dir_always() {
    let spec = read("LOOP-SPEC.md");
    // T52: the validator's goal export names the role-keyed dir — ALWAYS,
    // never conditionally (a conditional rule is a future
    // mis-application; T44's cap — one validator in flight — makes one
    // validator dir sufficient because validators are serial). Per-carrier
    // pins, T47 pattern:
    // (1) step 4's §6-goal export sentence carries the full export line.
    count_eq(
        &spec,
        &format!("export {VALIDATE}"),
        1,
        "LOOP-SPEC step-4 validator goal export (T52)",
    );
    // (2) the spec-test's grep -c "target-shared-validate" ≥ 2, encoded as
    //     the exact count per the T47 pin pattern (a bare `>= 2` would
    //     survive dropping either carrier): the launch paragraph names the
    //     dir AND the export sentence carries it.
    count_eq(
        &spec,
        "target-shared-validate",
        2,
        "LOOP-SPEC target-shared-validate carriers: step-4 launch paragraph + \
         goal-export sentence (T52)",
    );
    // (3) the rule is stated in its unconditional form.
    assert_contains(
        &spec,
        "ALWAYS, never conditionally",
        "LOOP-SPEC step-4 validator-dir rule (T52)",
    );
    // (4) impl children are UNCHANGED — the split is by role, so step 2's
    //     goal text still exports the T47 shared dir.
    count_eq(
        &spec,
        &format!("{SHARED} before"),
        1,
        "LOOP-SPEC step-2 impl goal export keeps target-shared (T52)",
    );
}

#[test]
fn loop_spec_gate_dir_is_role_keyed_for_the_overlap_window() {
    let spec = read("LOOP-SPEC.md");
    // (1) step 3's gate template keeps the T47 shared dir as its base (the
    //     no-child-in-flight arm) — the T47 carrier pin survives T52 and
    //     T82 (both legs of the runner rule carry the prefix; the counts
    //     live in loop_spec_templates_export_the_shared_dir).
    count_eq(
        &spec,
        &format!("{SHARED} perl -e 'alarm 600; exec @ARGV' cargo nextest run --release"),
        1,
        "LOOP-SPEC step-3 review-gate base prefix, nextest leg (T47+T82)",
    );
    // (2) the conditional arm carries the gates dir exactly once.
    count_eq(
        &spec,
        GATES,
        1,
        "LOOP-SPEC step-3 conditional gates-dir prefix (T52)",
    );
    // (3) the conditional is spelled: shared when NO child in flight, gates
    //     whenever an impl child may be concurrently building — the T44
    //     overlap window, explicitly including N+1's impl during N's
    //     post-merge gates.
    assert_contains(
        &spec,
        "when NO child is in flight",
        "LOOP-SPEC step-3 no-child arm (T52)",
    );
    assert_contains(
        &spec,
        "whenever an impl child may be concurrently building",
        "LOOP-SPEC step-3 conditional arm (T52)",
    );
    assert_contains(
        &spec,
        "post-merge gates",
        "LOOP-SPEC step-3 names the post-merge overlap (T52)",
    );
    // (4) the mechanism sentence: metadata hash excludes the checkout path →
    //     same artifact filename → last builder wins the slot.
    assert_contains(
        &spec,
        "metadata hash excludes the checkout path",
        "LOOP-SPEC step-3 mechanism sentence (T52)",
    );
    assert_contains(
        &spec,
        "last-builder-wins",
        "LOOP-SPEC step-3 last-builder-wins mechanism (T52)",
    );
}

#[test]
fn meta_spec_templates_export_the_shared_dir() {
    let spec = read("META-SPEC.md");
    assert_contains(&spec, SHARED, "META-SPEC.md");
    // Per-carrier pins (T47 fix-up finding 2: the old bare `>= 3` count let
    // mutant M6 — dropping BOTH nohup env-prefixes — survive, because 3 of
    // the 5 carriers were still standing). All carriers are now pinned
    // individually by name and location; dropping any ONE must fail.
    // T82: §5's review gate grew a second leg (nextest + fallback), so the
    // total is 6.
    count_eq(&spec, SHARED, 6, "META-SPEC total carrier count (T47+T82)");
    // (1) Why bullet — the child-binary bullet quotes the export verbatim.
    count_eq(
        &spec,
        &format!("`{SHARED}`"),
        1,
        "META-SPEC Why-bullet backtick-quoted export (T47)",
    );
    // (2) impl nohup launch template (§4).
    count_eq(
        &spec,
        &format!(
            "{SHARED} nohup /Users/jadams/workspace/chug/target/release/chug \
             run \\\n     --spec <your feature spec file>"
        ),
        1,
        "META-SPEC §4 impl nohup launch template (T47)",
    );
    // (3) review gate — the orchestrator's own gate run is env-prefixed on
    //     BOTH legs of the T82 runner.
    count_eq(
        &spec,
        &format!("{SHARED} cargo nextest run --release"),
        1,
        "META-SPEC §5 review-gate prefix, nextest leg (T47+T82)",
    );
    count_eq(
        &spec,
        &format!("{SHARED} cargo test --release -- --test-threads=4"),
        1,
        "META-SPEC §5 review-gate prefix, fallback leg (T47+T82)",
    );
    // (4) validator nohup launch template (§6).
    count_eq(
        &spec,
        &format!(
            "{SHARED} nohup /Users/jadams/workspace/chug/target/release/chug \
             run \\\n     --spec <the round's feature spec"
        ),
        1,
        "META-SPEC §6 validator nohup launch template (T47)",
    );
    // (5) §6's goal text — the export spelled out for the validator child
    // (the delegate path has no env parameter).
    count_eq(
        &spec,
        &format!("export {SHARED}"),
        1,
        "META-SPEC §6 goal-text export (T47)",
    );
    // Both nohup prefixes together: dropping one — or both at once (the M6
    // mutant) — must fail, independent of the §4/§6 context pins above.
    count_eq(
        &spec,
        &format!("{SHARED} nohup"),
        2,
        "META-SPEC BOTH nohup launch templates carry the env prefix (T47 \
         mutant M6)",
    );
    // Tradeoff note: recovery is operator-owned and cheap.
    assert_contains(&spec, "rm -rf target-shared", "META-SPEC.md");
}

#[test]
fn no_template_points_cargo_target_dir_at_the_repo_own_target() {
    for file in ["LOOP-SPEC.md", "META-SPEC.md"] {
        let spec = read(file);
        // Every `CARGO_TARGET_DIR=<repo>/target…` occurrence must continue
        // with `-shared` — pointing the var at the repo's own target/ is the
        // separation T47 forbids. (Never a bare contains(): the shared path
        // itself has the repo-target path as a prefix.)
        let offenders: Vec<usize> = spec
            .match_indices(REPO_TARGET)
            .filter(|(i, _)| !spec[*i + REPO_TARGET.len()..].starts_with("-shared"))
            .map(|(i, _)| i)
            .collect();
        assert!(
            offenders.is_empty(),
            "{file} must never point CARGO_TARGET_DIR at the repo's own \
             target/ (offsets {offenders:?}) — the shared dir is \
             target-shared (T47 separation)"
        );
    }
}

#[test]
fn gitignore_ignores_the_t57_main_dedicated_dir() {
    let gitignore = read(".gitignore");
    // (4a) Exact-count, not existence (T47/T52 carrier doctrine): the line
    // must occur EXACTLY once.
    count_eq(
        &gitignore,
        "target-shared-main/",
        1,
        ".gitignore target-shared-main line (T57)",
    );
    // Contiguous with the target-shared* family (req 2) — one four-line
    // block, not appended at a distant end of the file.
    let mut ats: Vec<usize> = Vec::new();
    for dir in [
        "target-shared/",
        "target-shared-validate/",
        "target-shared-gates/",
        "target-shared-main/",
    ] {
        ats.push(gitignore.lines().position(|l| l.trim() == dir).unwrap_or_else(
            || panic!(".gitignore must keep a `{dir}` line (T47/T52/T57); got:\n{gitignore}"),
        ));
    }
    ats.sort_unstable();
    assert_eq!(
        ats[3] - ats[0],
        3,
        "the four target-shared* cache lines must form one contiguous block \
         in .gitignore (T57); got:\n{gitignore}"
    );
}

#[test]
fn loop_spec_post_merge_gates_name_the_main_dedicated_dir() {
    let spec = read("LOOP-SPEC.md");
    // (4b) Strictly stronger than "named at least once": the full env
    // prefix occurs EXACTLY twice — step 5's post-merge re-run + Phase 3's
    // final gates. Dropping either carrier (or duplicating one) fails.
    count_eq(
        &spec,
        MAIN,
        2,
        "LOOP-SPEC target-shared-main env-prefix carriers: step-5 post-merge \
         re-run + Phase-3 final gates (T57)",
    );
    // The post-merge gate instruction itself names the dir: everything
    // between `re-run gates in main` and the row-flip clause carries the
    // full prefix and the unconditional ALWAYS form.
    let at = spec
        .find("re-run gates in main")
        .expect("LOOP-SPEC step 5 keeps the `re-run gates in main` instruction (T57)");
    let end = at
        + spec[at..]
            .find("the TODO row to `done`")
            .expect("LOOP-SPEC step 5 keeps the row-flip clause (T57)");
    let window = &spec[at..end];
    assert!(
        window.contains(MAIN),
        "the step-5 post-merge gate instruction (`re-run gates in main`) must \
         name {MAIN} (T57); got:\n{window}"
    );
    assert!(
        window.contains("ALWAYS"),
        "the step-5 post-merge rule must be stated in its unconditional ALWAYS \
         form — never conditionally on the T44 overlap (T57); got:\n{window}"
    );
    // Phase 3's final gates carry the same rule.
    let p3 = spec
        .find("Final gates green in main")
        .expect("LOOP-SPEC Phase 3 keeps the final-gates bullet (T57)");
    let w3 = &spec[p3..(p3 + 500).min(spec.len())];
    assert!(
        w3.contains(MAIN),
        "Phase 3's final-gates bullet must name {MAIN} (T57); got:\n{w3}"
    );
    // Mechanism sentence, T52 paragraph style: artifact filename excludes
    // the checkout path → last-builder-wins → only a main-builders-only dir
    // keeps post-merge artifacts identical to main content.
    assert!(
        spec.contains("artifact filename excludes the checkout path"),
        "LOOP-SPEC must carry the T57 mechanism sentence (cargo's artifact \
         filename excludes the checkout path)"
    );
    assert!(
        spec.matches("last-builder-wins").count() >= 2,
        "LOOP-SPEC must keep the T52 step-3 mechanism AND gain the T57 step-5 \
         one (last-builder-wins twice)"
    );
    // The T55 false-red receipt, named in the parenthetical: the stale
    // pre-T53 worktree binary executed as fresh.
    assert!(
        spec.contains("T55") && spec.contains("loopd_reexec"),
        "the T57 mechanism parenthetical must name the T55 false-red receipt \
         (the stale pre-T53 tests/loopd_reexec.rs binary)"
    );
    // The old crossover text is gone: step 3's role-keyed rule no longer
    // claims to govern post-merge gates — they are ALWAYS main-dedicated.
    assert!(
        !spec.contains("applies here too"),
        "step 5 must no longer defer post-merge gates to step 3's role-keyed \
         rule (`applies here too`) — they use target-shared-main ALWAYS (T57)"
    );
}

/// T64 pin (a) — the t57-validate M5 survivor. The T57 step-5 window pin
/// above asserts `contains(MAIN)` + `contains("ALWAYS")`, which the
/// mechanism sentence's `ALWAYS main checkouts` satisfies even when the
/// ALWAYS-form rule sentence itself is reworded or dropped — the M5 mutant
/// survived on exactly that. This pin scopes the WHOLE step 5 (heading to
/// heading) and pins the ALWAYS language in its exact form, exact-count per
/// the T47-carrier doctrine: the ALWAYS-form carrier for the main-gates dir
/// occurs exactly once, so dropping, rewording, or duplicating it goes RED
/// while `ALWAYS main checkouts` can never stand in for it.
#[test]
fn loop_spec_step5_window_carries_the_always_form_exactly_once() {
    let spec = read("LOOP-SPEC.md");
    let start = spec
        .find("5. **Harvest")
        .expect("LOOP-SPEC step-5 heading (`5. **Harvest`) (T64)");
    let end = start
        + spec[start..]
            .find("6. **Budget check")
            .expect("LOOP-SPEC step-6 heading (`6. **Budget check`) (T64)");
    let window = &spec[start..end];
    // (1) The main-gates dir occurs EXACTLY once in step 5 — its other
    //     spec-wide carrier (Phase 3's final gates) sits outside the window
    //     and is counted by the T57 spec-wide pin above.
    count_eq(
        window,
        MAIN,
        1,
        "LOOP-SPEC step-5 window target-shared-main carrier (T64)",
    );
    // (2) The ALWAYS-form rule language occurs EXACTLY once in the window,
    //     in its exact wording — the mechanism sentence's `ALWAYS main
    //     checkouts` does not contain this phrase and cannot satisfy the
    //     count (the M5 survivor's escape hatch, now closed).
    count_eq(
        window,
        "ALWAYS, never conditionally",
        1,
        "LOOP-SPEC step-5 window ALWAYS-form rule language (T64, t57-validate \
         M5 survivor)",
    );
    // (3) The two are ONE carrier: the ALWAYS-form sentence is the one
    //     carrying the main-gates dir (byte-exact adjacency, T47 pattern —
    //     the env prefix ends `target-shared-main`,` and the next line
    //     opens with the ALWAYS form).
    count_eq(
        window,
        &format!("{MAIN}`,\n   ALWAYS, never conditionally"),
        1,
        "LOOP-SPEC step-5 combined carrier: main-gates dir + ALWAYS-form in \
         one rule sentence (T64)",
    );
    // (4) The step-5 rule names what it is NOT: step 3's conditional
    //     role-keyed split — the disambiguation that makes the ALWAYS-form
    //     meaningful (an M5 reword that keeps the phrase but drops the
    //     contrast still dies here).
    assert!(
        window.contains("NOT step 3's"),
        "the step-5 ALWAYS-form must disambiguate itself from step 3's \
         role-keyed rule (`this is NOT step 3's ...`) (T64); got:\n{window}"
    );
}

#[test]
fn readme_target_cache_clause_names_the_main_dedicated_dir() {
    let readme = read("README.md");
    count_eq(
        &readme,
        "target-shared-main/",
        1,
        "README target-cache clause (T57)",
    );
    // Integrated into the existing continuous-mode paragraph (req 3), not a
    // new bullet: the dir appears in the SAME paragraph as the T52 sibling
    // caches, with its role spelled out.
    let at = readme
        .find("target-shared-validate/")
        .expect("README keeps the T52 sibling-cache clause");
    let start = readme[..at].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let end = at + readme[at..].find("\n\n").unwrap_or(readme.len() - at);
    let para = &readme[start..end];
    assert!(
        para.contains("target-shared-main/"),
        "README's target-cache clause must name target-shared-main/ in the \
         same paragraph as the T52 sibling caches (T57); got:\n{para}"
    );
    assert!(
        para.contains("post-merge"),
        "the README clause must say the dir serves the post-merge (and final \
         main) gates (T57); got:\n{para}"
    );
}

#[test]
fn t57_dir_stays_scoped_to_the_orchestrator_surfaces() {
    // Req 5: META-SPEC.md and loopd.sh are untouched — the main-dedicated
    // dir is a LOOP-SPEC orchestrator-gates carrier only (loopd.sh spawns
    // the orchestrator with `target-shared`; it never gates in main, and
    // the child-launch doctrine is unchanged).
    for file in ["META-SPEC.md", "loopd.sh"] {
        let text = read(file);
        assert!(
            !text.contains("target-shared-main"),
            "{file} must NOT name the T57 main-dedicated dir — it is scoped \
             to LOOP-SPEC.md (+ .gitignore/README) (T57 req 5)"
        );
    }
}

/// T78 — the supervisor binary: loopd builds the RELEASE profile and the
/// cycle invocation launches it. Exact-count pins per the T47 carrier
/// doctrine, plus the sweep side: a `target/debug/chug` launch path must
/// not survive anywhere in loopd.sh. (Historical receipts that QUOTE the
/// old debug path — this file's T47 finding comments, specs/ of past items —
/// are not launch paths and stay untouched.)
#[test]
fn loopd_builds_and_launches_the_release_binary() {
    let loopd = read("loopd.sh");
    // (1) The supervisor's own build is the release form, exactly once —
    //     gated (rc latched) and pinned to ./target (T137: the launched
    //     path must be the path just built).
    count_eq(
        &loopd,
        "CARGO_TARGET_DIR=\"$ROOT/target\" cargo build --release >> \"$LOG\" 2>&1 || build_rc=$?",
        1,
        "loopd.sh gated release build line (T78 + T137)",
    );
    // (2) The sweep: no debug-binary launch path survives in loopd.sh.
    assert!(
        !loopd.contains("./target/debug/chug"),
        "loopd.sh must not reference ./target/debug/chug anywhere — the \
         supervisor builds and launches the release binary (T78)"
    );
    // (3) The cycle invocation launches the release binary, env-prefixed,
    //     exactly once (the T47 prefix pin, restated at the release path).
    count_eq(
        &loopd,
        "CARGO_TARGET_DIR=\"$ROOT/target-shared\" ./target/release/chug run",
        1,
        "loopd.sh env-prefixed release invocation (T78)",
    );
}

/// T78 — the templates: every launch path and gate is the release profile.
/// The two META-SPEC nohup templates (the M6-mutant pair) must BOTH launch
/// `target/release/chug`; LOOP-SPEC carries no binary path at all (delegate
/// children re-launch the orchestrator's own executable) — pin the absence
/// so a future editor cannot hardcode a debug path back in; the review,
/// merge, and validator gate commands carry the T82 runner (nextest when on
/// PATH) with the T78 fallback `cargo test --release -- --test-threads=4`
/// named at every carrier; and the T78 tradeoff (first release build
/// slower, shared target dirs amortize) is named in both templates. The
/// spec `check:` convention stays debug — pinned via the T6 clause.
#[test]
fn launch_paths_and_gates_are_the_release_profile() {
    let meta = read("META-SPEC.md");
    let loop_spec = read("LOOP-SPEC.md");
    // (1) Both nohup launch templates carry the release path (the T47 M6
    //     pin, restated at the T78 path).
    count_eq(
        &meta,
        &format!("{SHARED} nohup /Users/jadams/workspace/chug/target/release/chug run"),
        2,
        "META-SPEC both nohup templates launch target/release/chug (T78)",
    );
    // (2) The sweep: no debug launch path survives in either template.
    assert!(
        !meta.contains("target/debug/chug"),
        "META-SPEC.md must not reference target/debug/chug — child and \
         validator launches use target/release/chug (T78)"
    );
    assert!(
        !loop_spec.contains("target/debug/chug"),
        "LOOP-SPEC.md must not reference target/debug/chug — delegate \
         children re-launch the orchestrator's own executable, so no \
         binary path belongs in the spec (T78)"
    );
    // (3) The T78 fallback gate command at all five carriers: §5 review
    //     gate, §6 validator goal text, §7 merge gate, T6 rule's canonical
    //     statement, T6 rule's fallback-cap clause (T82 moved the cap
    //     examples to the nextest form — the fallback cap is stated in
    //     prose there — so the fallback command itself is the carrier).
    count_eq(
        &meta,
        "cargo test --release -- --test-threads=4",
        5,
        "META-SPEC T78-fallback gate command carriers: §5 review, §6 \
         validator goal, §7 merge gate, T6 rule + its fallback-cap clause \
         (T82)",
    );
    // (4) LOOP-SPEC's executable review-gate template is the release form —
    //     now the T82 fallback leg (the nextest leg carries the same cap,
    //     pinned in nextest_gate_runner.rs).
    count_eq(
        &loop_spec,
        "perl -e 'alarm 600; exec @ARGV' cargo test --release",
        1,
        "LOOP-SPEC step-3 review-gate template carries the T82 fallback \
         leg under the same bounded cap (T78+T82)",
    );
    // (5) The tradeoff is NAMED in both templates (first release build
    //     slower, shared target dirs amortize).
    for (name, text) in [("META-SPEC.md", &meta), ("LOOP-SPEC.md", &loop_spec)] {
        assert!(
            text.contains("first release build into a cold cache is slower"),
            "{name} must name the T78 build-time tradeoff: the first release \
             build into a cold cache is slower (compile time) (T78)"
        );
        assert!(
            text.contains("amortize"),
            "{name} must name the T78 tradeoff's other half: the shared \
             target dirs amortize it (T78)"
        );
    }
    // (6) The spec `check:` convention stays debug — the T6 rule says so
    //     explicitly, so a future editor cannot "fix" spec check lines to
    //     --release.
    assert!(
        meta.contains("stays plain `cargo test`"),
        "META-SPEC's gates rule must state that the spec `check:` convention \
         stays plain `cargo test` (debug) — only review/merge/validation \
         gates run the release form (T78 req 4)"
    );
}

/// T78 sweep — kimi round-1 verdict: FAIL (weak tests, not correctness).
/// Four mutants survived because each reverted ONE `--release` carrier back
/// to the debug form and nothing counted it: M8 (LOOP-SPEC step-1 worktree
/// build), M9 (META-SPEC why-bullet build), M10 (LOOP-SPEC step-5 gate
/// form), M11 (LOOP-SPEC Phase-3 wrap). These tests close the class: EVERY
/// `cargo build --release` / `cargo test --release` carrier the T78 diff
/// (a74769f) introduced is exact-count-pinned, granularly — one pin per
/// carrier, so each carrier's revert is independently observable —
/// including carriers the earlier single-line needles cannot see
/// (META-SPEC T6's two line-wrapped examples, README's line-wrapped gate
/// form, loopd.sh's comment restatements).
#[test]
fn loop_spec_release_carriers_are_pinned_per_carrier() {
    let spec = read("LOOP-SPEC.md");
    // (M8) step 1: the worktree build-warm command is the release form.
    count_eq(
        &spec,
        "cargo build --release",
        1,
        "LOOP-SPEC step-1 worktree build is the release form (T78 req 2; \
         kimi R1 mutant M8)",
    );
    // step 1's why-prose states the gates' profile — a carrier too. T82
    // rewrote it to name the nextest-first runner (the T78 release form is
    // the fallback leg of step 3's rule).
    count_eq(
        &spec,
        "gates below run the\n   release-profile gate runner",
        1,
        "LOOP-SPEC step-1 why-prose names the T82 gate runner (T78+T82)",
    );
    // (M10) step 5: the post-merge gate command is step 3's T82 runner, its
    // fallback leg still the T78 release form under the bounded cap.
    count_eq(
        &spec,
        "`cargo test --release -- --test-threads=4` under the bounded cap",
        1,
        "LOOP-SPEC step-5 gate command keeps the T78 fallback form under \
         the bounded cap (T78 req 3 + T82; kimi R1 mutant M10)",
    );
    // (M11) Phase-3 wrap: the final gates name the T82 runner.
    count_eq(
        &spec,
        "clippy + step 5's T82 gate runner",
        1,
        "LOOP-SPEC Phase-3 wrap pins the final gates to step 5's T82 gate \
         runner (T78 req 3 + T82; kimi R1 mutant M11)",
    );
}

#[test]
fn meta_spec_release_carriers_are_pinned_per_carrier() {
    let meta = read("META-SPEC.md");
    // (M9) the why-bullet's per-worktree build is the release form.
    count_eq(
        &meta,
        "cargo build --release",
        1,
        "META-SPEC why-bullet worktree build is the release form (T78 req 2; \
         kimi R1 mutant M9)",
    );
    // The T6 rule's two cap examples are now nextest-form carriers the
    // single-line fallback needle (count 5 above) cannot see — the T82 cap
    // examples show the nextest form; the fallback leg is stated in prose
    // right after them ("the same caps wrap the fallback ... instead").
    count_eq(
        &meta,
        "perl -e 'alarm 600; exec @ARGV' cargo nextest run --release",
        1,
        "META-SPEC T6 macOS example wraps the T82 nextest form (T82)",
    );
    count_eq(
        &meta,
        "timeout 600 cargo nextest run --release",
        1,
        "META-SPEC T6 Linux example wraps the T82 nextest form (T82)",
    );
}

#[test]
fn loopd_and_readme_release_carriers_are_pinned_per_carrier() {
    let loopd = read("loopd.sh");
    let readme = read("README.md");
    // Both loopd.sh `cargo build --release` carriers: the build line
    // (full-line-pinned above) and the T47 stale-binary comment's
    // restatement of it — T78 rewrote that comment too.
    count_eq(
        &loopd,
        "cargo build --release",
        2,
        "loopd.sh release-build carriers: the build line + the T47 comment \
         (T78 req 1)",
    );
    // The T82 fallback leg is named by the else-branch startup log line
    // (the T78 comment's restatement wraps mid-command, so the log line is
    // the countable carrier; nextest_gate_runner.rs pins both branches).
    count_eq(
        &loopd,
        "cargo test --release -- --test-threads=4",
        1,
        "loopd.sh names the T82 fallback gate form (T78 req 3 + T82 req 2)",
    );
    // README's continuous-mode paragraph documents both carriers.
    count_eq(
        &readme,
        "cargo build --release",
        1,
        "README names the supervisor's release build (T78)",
    );
    // ... and the gate form — line-wrapped mid-command in the prose, so
    // match it wrap-insensitively.
    let flat_readme = readme.replace('\n', " ");
    count_eq(
        &flat_readme,
        "cargo test --release",
        1,
        "README names the release gate form (wrap-insensitive — the clause \
         line-wraps between `cargo test` and `--release`) (T78)",
    );
}

/// T79 — parallel mutation legs: after the clean-tree gates pass, the
/// validator MAY run its mutation legs in parallel — one throwaway worktree
/// per mutant (`/tmp/chug-mut-<item>-<k>`), each with its OWN role-keyed
/// target dir (the T52 lesson one level down: a mutant's binaries must
/// never share a target dir with another checkout's builds — the exact
/// cross-checkout artifact race T52 fixed), capped at 3 legs in flight,
/// serial the default when mutants touch overlapping files (the overlap
/// judgment declared in the verdict notes), tree-restored semantics
/// unchanged (main worktree byte-clean before the verdict; throwaway
/// worktrees removed after results are collected), findings referencing
/// mutant names — not leg dirs. LOOP-SPEC's step-4 clause spells the
/// repo-root path in full (it resolves §6's `...` elision for the loop's
/// delegates); the wrap points are chosen so every carrier sits on ONE
/// line, so a rewrap that splits a carrier goes red (the T63/T72
/// byte-identity stance).
#[test]
fn loop_spec_parallel_mutant_legs_are_pinned() {
    let spec = read("LOOP-SPEC.md");
    // Needle self-check (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        MUT_DIR.contains("-mut-<k>") && MUT_WT.contains("<item>"),
        "the needles must carry the per-leg dir and per-mutant worktree shapes"
    );
    // Every carrier occurs EXACTLY once spec-wide — zero means the clause
    // was dropped, more than one means it is stated twice (the T67
    // self-match lesson).
    for (needle, what) in [
        ("Parallel mutants (T79,", "the T79 knob's name carrier"),
        (MUT_WT, "the per-mutant throwaway worktree path"),
        (MUT_DIR, "the per-leg role-keyed target dir (full path)"),
        ("MAY run its mutation legs", "the validator-discretion MAY"),
        ("cap legs in flight at 3", "the 3-leg parallelism cap"),
        (
            "declares that overlap judgment in its verdict notes",
            "the overlapping-files serial-default clause",
        ),
        ("byte-clean before the verdict", "the tree-restored byte-clean leg"),
        (
            "throwaway worktrees removed after results are collected",
            "the post-collection worktree cleanup leg",
        ),
        ("mutant names, not leg dirs", "the VERDICT-format preservation leg"),
    ] {
        count_eq(&spec, needle, 1, &format!("LOOP-SPEC T79 carrier: {what}"));
    }
    // Scope: the whole clause lives INSIDE step 4 (between step 4's and
    // step 5's headings) — the T72 window pattern.
    let start = spec
        .find("4. **Adversarial validation")
        .expect("LOOP-SPEC step-4 heading present (T79)");
    let end = start
        + spec[start..]
            .find("5. **Harvest")
            .expect("LOOP-SPEC step-5 heading after step 4 (T79)");
    let window = &spec[start..end];
    assert!(
        window.contains("Parallel mutants (T79,") && window.contains(MUT_WT),
        "the T79 parallel-mutant clause must live inside step 4; got:\n{window}"
    );
}

/// T79 in META-SPEC §6: the validator GOAL TEXT carries the same mandate —
/// this is the string children actually execute (LOOP-SPEC §2 step 4
/// inherits §6's goal verbatim except its export line). The goal's line
/// wraps are free (it is a quoted nohup/delegate argument, rewrapped at
/// every doctrine edit), so the prose needles are pinned WRAP-INSENSITIVELY
/// (the T78 README pattern): flattened newlines, exact wording — a dropped
/// or reworded clause still goes red. The two path carriers stay on one
/// line by construction and are pinned raw.
#[test]
fn meta_spec_validator_goal_carries_the_parallel_mutant_mandate() {
    let meta = read("META-SPEC.md");
    // The §6 goal-text window: from the VALIDATION ONLY opener to the model
    // flag that ends the goal argument.
    let start = meta
        .find("--goal \"VALIDATION ONLY")
        .expect("META-SPEC §6 keeps the validator goal template (T79)");
    let end = start
        + meta[start..]
            .find("--model anthropic-system.ai.kimi-k3")
            .expect("META-SPEC §6 goal template ends at the model flag (T79)");
    let goal = &meta[start..end];
    // split_whitespace join (the T78 flat idiom): the goal text wraps with a
    // 13-space continuation indent, so a bare '\n'→' ' replace leaves runs
    // of spaces that break mid-phrase needles.
    let flat = goal.split_whitespace().collect::<Vec<_>>().join(" ");
    // The elided-path carrier (the T79 spec's own spelling — §6 stays
    // repo-root-agnostic; LOOP-SPEC step 4 resolves it): exactly one, raw.
    count_eq(
        &meta,
        "CARGO_TARGET_DIR=.../target-shared-mut-<k>",
        1,
        "META-SPEC §6 goal-text per-leg target dir carrier, elided form (T79)",
    );
    count_eq(
        &meta,
        "target-shared-mut-",
        1,
        "META-SPEC carries the mut-leg dir family exactly once — a second \
         statement would fork the rule (T79)",
    );
    for (needle, what) in [
        (MUT_WT, "the per-mutant throwaway worktree path"),
        ("MAY run the mutation legs in PARALLEL", "the parallel MAY"),
        ("cap legs in flight at 3", "the 3-leg parallelism cap"),
        (
            "declare that overlap judgment in your verdict notes",
            "the overlapping-files serial-default clause",
        ),
        ("byte-clean before the verdict", "the tree-restored byte-clean leg"),
        (
            "remove the throwaway worktrees after the results are collected",
            "the post-collection worktree cleanup leg",
        ),
        ("findings reference mutant names, not leg dirs", "the VERDICT-format leg"),
    ] {
        count_eq(
            &flat,
            needle,
            1,
            &format!("META-SPEC §6 validator goal (flat): {what} (T79)"),
        );
    }
}

/// T79 — the per-leg caches are gitignored BY GLOB (k varies per leg), and
/// the glob line sits in the target-shared* block right after the T57
/// four-dir run (the T57 contiguity pin above still holds; this pin adds
/// the fifth line's exact position).
#[test]
fn gitignore_ignores_the_t79_mut_leg_cache_family() {
    let gitignore = read(".gitignore");
    count_eq(
        &gitignore,
        "target-shared-mut-*/",
        1,
        ".gitignore target-shared-mut-* glob line (T79)",
    );
    let main_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-main/")
        .expect(".gitignore keeps the T57 `target-shared-main/` line");
    let mut_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-mut-*/")
        .expect(".gitignore keeps the T79 `target-shared-mut-*/` line");
    assert_eq!(
        mut_at,
        main_at + 1,
        "the T79 mut-leg glob must sit directly after the T57 four-dir block \
         in .gitignore (one contiguous cache family); got:\n{gitignore}"
    );
}

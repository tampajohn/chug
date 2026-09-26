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
        .find("  cargo build >> \"$LOG\" 2>&1")
        .expect("loopd.sh builds its own binary");
    let loop_start = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    assert!(
        mkdir < first_build && mkdir < loop_start,
        "loopd.sh must create target-shared at supervisor start — before the \
         cycle loop and the first `cargo build` (T47)"
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
        .find("CARGO_TARGET_DIR=\"$ROOT/target-shared\" ./target/debug/chug run")
        .expect(
            "loopd.sh must env-prefix the chug invocation itself with \
             CARGO_TARGET_DIR=\"$ROOT/target-shared\" so only the cycle's \
             orchestrator (and its delegate children) builds into the \
             shared cache (T47)",
        );
    // Ordering pin: the supervisor builds its own binary FIRST — into
    // ./target, which is what `./target/debug/chug` resolves to — and the
    // shared-cache prefix applies only to the cycle invocation after it.
    let build = loopd
        .find("  cargo build >> \"$LOG\" 2>&1")
        .expect("loopd.sh builds its own binary first");
    assert!(
        build < invocation,
        "loopd.sh's own `cargo build` (into ./target) must precede the \
         env-prefixed chug invocation — the supervisor's binary cache stays \
         separate from the shared children's cache (T47)"
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
    // (3) step-3 review-gate env prefix.
    count_eq(
        &spec,
        &format!("{SHARED}\n   perl -e"),
        1,
        "LOOP-SPEC step-3 review-gate prefix (T47)",
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
    //     no-child-in-flight arm) — the T47 carrier pin survives T52.
    count_eq(
        &spec,
        &format!("{SHARED}\n   perl -e"),
        1,
        "LOOP-SPEC step-3 review-gate base prefix (T47, unchanged by T52)",
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
    // the 5 carriers were still standing). All five carriers are now pinned
    // individually by name and location; dropping any ONE must fail.
    count_eq(&spec, SHARED, 5, "META-SPEC total carrier count (T47)");
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
            "{SHARED} nohup /Users/jadams/workspace/chug/target/debug/chug \
             run \\\n     --spec <your feature spec file>"
        ),
        1,
        "META-SPEC §4 impl nohup launch template (T47)",
    );
    // (3) review gate — the orchestrator's own `cargo test` is env-prefixed.
    count_eq(
        &spec,
        &format!("{SHARED} cargo test"),
        1,
        "META-SPEC §5 review-gate cargo test prefix (T47)",
    );
    // (4) validator nohup launch template (§6).
    count_eq(
        &spec,
        &format!(
            "{SHARED} nohup /Users/jadams/workspace/chug/target/debug/chug \
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

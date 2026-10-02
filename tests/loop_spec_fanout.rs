//! T194 — fan-out phase 2: the 3-child fleet, parallel validators, and the
//! resource governor.
//!
//! Doctrine pins, not behavior. T161's cap (≤2 children in flight, ≤1
//! validator — "never 3+, never 2 validators") bound every cycle with 3+
//! queue rows while the orchestrator sat idle on long-poll; T194 amends it:
//! at most 3 children in flight, of which at most 2 validators —
//! - a 3rd impl child only when all three items' spec-named target-file
//!   lists are PAIRWISE disjoint (a 3rd impl whose files overlap ANY
//!   in-flight impl's files is blocked — the predicate pinned below);
//! - a 2nd validator only when two items are simultaneously past gates —
//!   never two validators on the same item (correlated verdicts add
//!   nothing); family independence unchanged (validators are always kimi);
//! - validators slot-keyed like impls: `target-shared-validate-a` (the solo
//!   default) / `target-shared-validate-b` (the pattern-(iv) second
//!   validator) — two validators sharing one dir would serialize on cargo's
//!   build lock and defeat the change, and the T47 invariant holds: no two
//!   cargo processes share a target dir. The 3-impl fleet likewise needed a
//!   third impl slot (`target-shared-impl-c`) so the widened cap can never
//!   collide two impls on one dir;
//! - a resource governor: at most 4 cargo-heavy children total (impls +
//!   validators); when memory pressure forces a choice, validators win (a
//!   verdict unblocks a merge and the queue behind it) and the orchestrator
//!   records the degradation via decision_log.
//!
//! Every pin is a per-carrier exact count (the T47 pattern): dropping any
//! ONE carrier goes red, and so does duplicating one. The old-cap deletion
//! guard is a zero-count pin: reintroducing "never 3+" or "never 2
//! validators" anywhere in LOOP-SPEC.md goes red.

use std::path::PathBuf;

/// T194: validator slot a — the solo default (full env-prefix carrier).
const VALIDATE_A: &str =
    "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-a";
/// T194: validator slot b — the pattern-(iv) second validator's dir.
const VALIDATE_B: &str =
    "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-validate-b";
/// T194: the third impl slot for the 3-impl fleet.
const IMPL_C: &str = "CARGO_TARGET_DIR=/Users/jadams/workspace/chug/target-shared-impl-c";

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(repo_root().join(rel))
        .unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

/// Prose needles wrap freely at doctrine edits, so count them
/// wrap-insensitively (the T78 flat idiom: whitespace runs collapse).
fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Per-carrier pin: `needle` must occur EXACTLY `expected` times (the T47
/// pattern — a bare `>= N` survives dropping all but one carrier).
fn count_eq(haystack: &str, needle: &str, expected: usize, what: &str) {
    let found = haystack.matches(needle).count();
    assert_eq!(
        found, expected,
        "{what}: expected carrier {needle:?} exactly {expected}×, found \
         {found}× — a dropped (or duplicated) T194 carrier breaks the \
         fan-out wiring this pin guards"
    );
}

/// Needle self-check (the T48 idiom): a mangled needle must not let a pin
/// pass silently — every path carrier must carry its slot suffix.
#[test]
fn t194_needles_carry_their_slot_shapes() {
    assert!(VALIDATE_A.ends_with("target-shared-validate-a"));
    assert!(VALIDATE_B.ends_with("target-shared-validate-b"));
    assert!(IMPL_C.ends_with("target-shared-impl-c"));
    assert!(VALIDATE_A != VALIDATE_B && IMPL_C != VALIDATE_A);
}

/// The amended cap at the carriers the T161 pin guarded: §2 intro +
/// Pipeline hard cap + Hard rules for the "in flight" form; §2 intro +
/// Trivial-row bundling cross-ref + Pipeline + Hard rules for the flat
/// validator form (the same 4-carrier set the old ≤1-validator pin
/// counted). The old cap text must be GONE — zero hits, LOOP-SPEC-wide.
#[test]
fn t194_new_caps_present_and_old_caps_gone() {
    let spec = read("LOOP-SPEC.md");
    count_eq(
        &flat(&spec),
        "at most 3 children in flight",
        3,
        "the 3-children cap: §2 intro + pipeline hard cap + hard rules \
         (flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "of which at most 2 validators",
        4,
        "the ≤2-validators cap: §2 intro + bundling cross-ref + pipeline + \
         hard rules (flat) (T194)",
    );
    // Deletion guard: the old caps are gone everywhere in LOOP-SPEC.md.
    assert!(
        !spec.contains("never 3+"),
        "`never 3+` must have ZERO hits in LOOP-SPEC.md — T194 allows three \
         children in flight (of which at most 2 validators)"
    );
    assert!(
        !spec.contains("never 2 validators"),
        "`never 2 validators` must have ZERO hits in LOOP-SPEC.md — T194 \
         allows a 2nd validator when two items are simultaneously past gates"
    );
    // The amendment is ATTRIBUTED where the cap is restated (§2 intro), so
    // a reader of the T161-era prose finds the amendment.
    count_eq(
        &flat(&spec),
        "T194 amends T161's 2-child cap",
        1,
        "the amendment attribution in the §2 intro (flat) (T194)",
    );
}

/// The 3rd-impl slot's PAIRWISE gate: the launch condition, the
/// all-three-lists predicate, and the blocked-overlap form of the same
/// predicate (a 3rd impl whose spec-named files overlap ANY in-flight
/// impl's files is blocked). Carriers: Pipeline pattern (iii) + the
/// pattern-gate preamble + the hard cap + the Hard rules restatement.
#[test]
fn t194_pairwise_disjointness_gate_is_stated() {
    let spec = read("LOOP-SPEC.md");
    // The pattern exists and is named.
    count_eq(
        &flat(&spec),
        "(iii) T194's 3rd impl slot",
        1,
        "the pattern-(iii) naming (pipeline) (T194)",
    );
    // The launch condition: N+2 may launch while BOTH N and N+1 fly, iff
    // the PAIRWISE gate passes.
    count_eq(
        &flat(&spec),
        "the PAIRWISE gate passes",
        1,
        "the 3rd-impl launch condition (pipeline, flat) (T194)",
    );
    // The predicate, twice: pattern (iii) spells it for dispatch, the Hard
    // rules restate it as the cap's condition.
    count_eq(
        &flat(&spec),
        "no file appears on any two of the three lists",
        2,
        "the all-three-lists pairwise predicate: pipeline pattern (iii) + \
         hard rules (flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "all three items' spec-named target-file lists are PAIRWISE \
         disjoint",
        1,
        "the hard-rules 3-impl permission sentence (flat) (T194)",
    );
    // The blocked form: a 3rd impl whose files overlap ANY in-flight impl's
    // files is blocked — the negative arm of the predicate.
    count_eq(
        &flat(&spec),
        "overlap ANY in-flight impl's files is blocked",
        1,
        "the blocked-overlap arm of the pairwise gate (pipeline, flat) \
         (T194)",
    );
}

/// The 2nd validator: launches ONLY when two items are simultaneously past
/// gates, never two validators on the SAME item; family independence
/// unchanged. Carriers: §2 intro + Pipeline pattern (iv) + hard cap + step
/// 4 + Hard rules for the launch condition; pipeline + step 4 + hard rules
/// for the same-item ban.
#[test]
fn t194_second_validator_launch_rule_is_stated() {
    let spec = read("LOOP-SPEC.md");
    count_eq(
        &flat(&spec),
        "(iv) T194's 2nd validator",
        1,
        "the pattern-(iv) naming (pipeline) (T194)",
    );
    count_eq(
        &flat(&spec),
        "two items are simultaneously past gates",
        4,
        "the 2nd-validator launch condition: §2 intro + pattern (iv) + hard \
         cap + hard rules (flat) (T194)",
    );
    // The same-item ban in both spellings (pipeline capitalizes SAME).
    count_eq(
        &flat(&spec),
        "never two validators on the same item",
        2,
        "the same-item ban: step 4 + hard rules (flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "never two validators on the SAME item",
        1,
        "the same-item ban, pipeline spelling (flat) (T194)",
    );
    // Family independence is restated at the new slot: validators are
    // always kimi (the T81 rule, unchanged by the wider fleet).
    count_eq(
        &flat(&spec),
        "family independence unchanged — validators are always kimi",
        1,
        "the family-independence restatement (pattern iv, flat) (T194)",
    );
}

/// Validator slot dirs: slot a is the solo default, slot b the second
/// validator's — full env-prefix carriers in step 4 (export + the slot
/// rule) and the Pipeline paragraph, the Hard-rules restatement, the
/// cargo-lock rationale, and the wrap-gates carve-out (wrap gates keep
/// target-shared-gates; post-merge keeps target-shared-main).
#[test]
fn t194_validator_slot_dirs_are_stated() {
    let spec = read("LOOP-SPEC.md");
    // Full env-prefix carriers (step-4 export sentence + pipeline
    // paragraph, once each) and bare-name carriers (step-4 slot rule +
    // env JSON value + hard rules, per exact count).
    count_eq(
        &spec,
        VALIDATE_A,
        2,
        "target-shared-validate-a full env-prefix carriers: step-4 slot-a \
         export + pipeline (T194)",
    );
    count_eq(
        &spec,
        VALIDATE_B,
        2,
        "target-shared-validate-b full env-prefix carriers: step-4 slot-b \
         export + pipeline (T194)",
    );
    count_eq(
        &spec,
        "target-shared-validate-a",
        5,
        "target-shared-validate-a carriers: step-4 export + step-4 slot \
         rule + step-4 env JSON + pipeline + hard rules (T194)",
    );
    count_eq(
        &spec,
        "target-shared-validate-b",
        4,
        "target-shared-validate-b carriers: step-4 export + step-4 env \
         JSON + pipeline + hard rules (T194)",
    );
    // The slot keying is stated: slot a is the solo default, slot b is the
    // second validator's.
    count_eq(
        &flat(&spec),
        "validator slot a, the solo default",
        1,
        "the slot-a solo-default rule (step 4, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "is slot b and its export becomes",
        1,
        "the slot-b assignment rule (step 4, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "keyed by validator slot",
        1,
        "the hard-rules slot keying (flat) (T194)",
    );
    // WHY slots: sharing one dir serializes on cargo's build lock — the
    // rationale at every carrier that names the split (step 4 + pipeline +
    // hard rules).
    count_eq(
        &flat(&spec),
        "two validators sharing one dir would serialize on cargo's build \
         lock",
        2,
        "the serialization rationale: step 4 + pipeline (flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "serialize on cargo's build lock",
        3,
        "the lock-serialization naming: step 4 + pipeline + hard rules \
         (flat) (T194)",
    );
    // The T47 invariant is restated at the widened fleet.
    count_eq(
        &flat(&spec),
        "no two cargo processes share a target dir",
        2,
        "the T47 invariant: step 4 + pipeline (flat) (T194)",
    );
    // Wrap gates keep target-gates; post-merge keeps target-main — the
    // slot split is validator-only.
    count_eq(
        &flat(&spec),
        "Wrap gates keep `target-shared-gates`",
        1,
        "the wrap-gates carve-out (step 4, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "post-merge gates keep `target-shared-main`",
        1,
        "the post-merge carve-out (step 4, flat) (T194)",
    );
}

/// The 3-impl fleet's third build slot: the T47/T52 role-keying extended —
/// without impl-c, a 3rd impl launched after the solo impl merged would
/// collide with an in-flight impl on impl-a or impl-b (both held). Full
/// env-prefix carriers in the step-2 dispatch rule + the Pipeline
/// paragraph, the Hard-rules slot list, and the "always a slot no in-flight
/// impl holds" invariant sentence.
#[test]
fn t194_third_impl_slot_is_stated() {
    let spec = read("LOOP-SPEC.md");
    count_eq(
        &spec,
        IMPL_C,
        2,
        "target-shared-impl-c full-path carriers: step-2 dispatch rule + \
         pipeline overlap paragraph (T194)",
    );
    count_eq(
        &flat(&spec),
        "always a slot no in-flight impl holds",
        1,
        "the slot-collision invariant sentence (pipeline, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "when impl-a AND impl-b are both held",
        2,
        "the impl-c assignment condition: step 2 + pipeline (flat) (T194)",
    );
    // The hard-rules slot list names all three impl slots.
    count_eq(
        &flat(&spec),
        "target-shared-impl-a / target-shared-impl-b / target-shared-impl-c",
        1,
        "the hard-rules three-slot list (flat) (T194)",
    );
}

/// The resource governor: at most 4 cargo-heavy children total (impls +
/// validators) as a Hard rule; under memory pressure validators win (a
/// verdict unblocks a merge and the queue behind it); the degradation is
/// recorded via decision_log. The Pipeline hard cap carries the pointer.
#[test]
fn t194_resource_governor_is_stated() {
    let spec = read("LOOP-SPEC.md");
    count_eq(
        &flat(&spec),
        "Resource governor (T194)",
        1,
        "the governor rule's naming (hard rules, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "at most 4 cargo-heavy children",
        1,
        "the 4-cargo-heavy-children ceiling (hard rules, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "the resource governor (Hard rules)",
        1,
        "the pipeline hard-cap pointer to the governor (flat) (T194)",
    );
    // Validators win under memory pressure — stated at both carriers.
    count_eq(
        &flat(&spec),
        "validators win",
        2,
        "the validators-win priority: pipeline pointer + hard rules (flat) \
         (T194)",
    );
    count_eq(
        &flat(&spec),
        "records the degradation via decision_log",
        1,
        "the decision_log degradation record (hard rules, flat) (T194)",
    );
    count_eq(
        &flat(&spec),
        "a validator verdict unblocks a merge and the queue behind it",
        1,
        "the WHY of validators-win (hard rules, flat) (T194)",
    );
}

/// .gitignore: the three new cache dirs join the target-shared* family at
/// its tail, one contiguous block — impl-c directly after the T161 pair,
/// then the two validator slots (the T57/T79/T161 contiguity pattern).
#[test]
fn t194_cache_dirs_are_gitignored_contiguously() {
    let gitignore = read(".gitignore");
    for dir in [
        "target-shared-impl-c/",
        "target-shared-validate-a/",
        "target-shared-validate-b/",
    ] {
        count_eq(
            &gitignore,
            dir,
            1,
            ".gitignore cache-dir line (T194)",
        );
    }
    let b_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-impl-b/")
        .expect(".gitignore keeps the T161 `target-shared-impl-b/` line");
    let c_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-impl-c/")
        .expect(".gitignore must gain a `target-shared-impl-c/` line (T194)");
    let va_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-validate-a/")
        .expect(".gitignore must gain a `target-shared-validate-a/` line (T194)");
    let vb_at = gitignore
        .lines()
        .position(|l| l.trim() == "target-shared-validate-b/")
        .expect(".gitignore must gain a `target-shared-validate-b/` line (T194)");
    assert_eq!(
        (c_at, va_at, vb_at),
        (b_at + 1, b_at + 2, b_at + 3),
        "the T194 cache lines must extend the target-shared* family at its \
         tail in .gitignore (impl-c, validate-a, validate-b directly after \
         impl-b); got:\n{gitignore}"
    );
}

/// README documents the widened fleet in the existing continuous-mode cache
/// paragraph (the T57/T161 pattern): the validator slot dirs, the third
/// impl slot, and the two-past-gates rule for the 2nd validator. The README
/// does NOT restate the child-count caps themselves — LOOP-SPEC is their
/// carrier (T194 touches README's cache docs only because the slot dirs
/// replaced the single validate cache).
#[test]
fn t194_readme_documents_the_widened_fleet() {
    let readme = read("README.md");
    count_eq(
        &readme,
        "target-shared-validate-a/",
        1,
        "README validator slot-a carrier (T194)",
    );
    count_eq(
        &readme,
        "target-shared-validate-b/",
        2,
        "README validator slot-b carriers: cache list + 2nd-validator rule \
         (T194)",
    );
    count_eq(
        &readme,
        "target-shared-impl-c/",
        1,
        "README impl-c carrier (T194)",
    );
    // Integrated: the slot carriers sit in the SAME paragraph as the T52
    // sibling caches and the T161 impl slots.
    let at = readme
        .find("target-shared-validate-a/")
        .expect("README keeps the validator slot-cache clause (T194)");
    let start = readme[..at].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let end = at + readme[at..].find("\n\n").unwrap_or(readme.len() - at);
    let para = flat(&readme[start..end]);
    assert!(
        para.contains("target-shared-impl-a/") && para.contains("target-shared-main/"),
        "the README T194 clause must sit in the same paragraph as the T52 \
         sibling caches and the T161 impl slots; got:\n{para}"
    );
    assert!(
        para.contains("T194") && para.contains("serialize on cargo's build lock"),
        "the README clause must name T194 and the serialization rationale; \
         got:\n{para}"
    );
    assert!(
        para.contains("two items are simultaneously past gates"),
        "the README clause must carry the 2nd-validator launch rule; \
         got:\n{para}"
    );
}

/// loopd.sh records the slot-dir convention where the T47 shared-cache
/// discipline lives — a comment only (loopd never sets a slot dir itself:
/// children get theirs via the delegate goal export + env map, LOOP-SPEC
/// step 4; the T47 ban on `export CARGO_TARGET_DIR` in loopd.sh holds).
#[test]
fn t194_loopd_records_the_validator_slot_convention() {
    let loopd = read("loopd.sh");
    count_eq(
        &loopd,
        "target-shared-validate-a",
        1,
        "loopd.sh names validator slot a (T194 comment)",
    );
    count_eq(
        &loopd,
        "target-shared-validate-b",
        1,
        "loopd.sh names validator slot b (T194 comment)",
    );
    // The comment sits in the T47 block, before the supervisor loop — the
    // convention is recorded where the shared cache is created.
    let comment_at = loopd
        .find("validator children are SLOT-keyed")
        .expect("loopd.sh carries the T194 slot-keying comment");
    let loop_start = loopd
        .find("while [ ! -f \"$STOP\" ]")
        .expect("loopd.sh has a supervisor loop");
    assert!(
        comment_at < loop_start,
        "the T194 slot-keying comment must sit at supervisor start (the T47 \
         block), before the cycle loop"
    );
    // The T47 ban holds: loopd.sh still never exports CARGO_TARGET_DIR.
    assert!(
        !loopd.contains("export CARGO_TARGET_DIR"),
        "loopd.sh must NOT `export CARGO_TARGET_DIR` — the T194 comment is a \
         comment; slot dirs reach children via their delegate goal export + \
         env map (T47 finding 1)"
    );
}

//! T269 — release-count method pins: LOOP-SPEC's Phase-3 release-trigger
//! bullet (T100) names the MECHANICAL count of items landed since the
//! newest `v*` tag.
//!
//! The trigger fires at "≥3 items landed since the newest `v*` tag", but
//! at filing the doctrine never said HOW the count is computed, and the
//! practiced count (in-cycle memory plus merge-commit subject
//! enumeration against the canonical `Merge branch 'loop-t<N>'` shape)
//! undercounts when a landing's commit shape varies. Live fire, verified
//! at filing (the cycle-393 eval §2.2): `v0.17.11..HEAD` held FOUR
//! flipped rows — T265/T266/T267/T268 — while the wrap records said "1
//! item" (true 2), "1 item" (true 3 — the trigger tripped there and the
//! release was suppressed), "2 items" (true 4), and then quoted "2
//! items" verbatim across thirteen zero-row wraps. T266's landing is a
//! custom-subject merge commit (topologically a merge, subject
//! `T266: …`, no `Merge branch` token), so a `--grep='Merge branch'`
//! enumeration returns exactly 2 — the frozen recorded count. The remedy
//! is ONE clause in the release-trigger bullet naming the mechanical
//! count authority: the count of DISTINCT flipped TODO row-ids via the
//! flip-commit subjects (`^todo:` — Phase 2 step 5 mandates one per
//! landed row, and a bundled flip names every row in one commit), with
//! the exact command shape, recomputed mechanically at EVERY wrap (never
//! quoted forward from a previous wrap's notes), and the explicit ban —
//! merge-subject enumeration and in-cycle memory are never the count
//! authority.
//!
//! These pins keep the clause from silently drifting out in a future
//! doctrine edit: (a) the count-method sentence sits in the Phase-3
//! window exactly once, flat-matched (deletion-proof — delete the
//! sentence, the count goes 0, RED; a reword goes 0, RED; a duplicated
//! copy goes 2, RED), (b) the mechanical command shape and its two
//! greppable tokens are present file-wide raw (presence-only so a future
//! doctrine edit may name a token a second time without spurious red —
//! leg (a) carries the positional and duplication guards), (c) `distinct`
//! is named case-insensitively (the acceptance grep is `grep -ci`), and
//! (d) the ban sentence names BOTH banned count authorities —
//! merge-subject enumeration and in-cycle memory — so the ban cannot
//! lose half its scope while staying green.
//!
//! T48 doctrine: the pin resolves LOOP-SPEC.md from the checkout the
//! binary RUNS against (`std::env::current_dir()`; cargo runs test
//! binaries with cwd = the package root), never via the compile-time
//! manifest-dir macro — under the T47 shared cache a compile-time path
//! can point at a since-removed worktree. T78 idiom: multi-word needles
//! are matched against whitespace-collapsed text because the doctrine
//! prose wraps mid-phrase. Deletion-proof convention: every count leg
//! doubles as the deletion check. No production code changes — docs+pin
//! only.

fn loop_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("LOOP-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading LOOP-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 flat idiom): the doctrine prose wraps
/// mid-phrase, so a multi-word needle must match whitespace-collapsed text
/// or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- windows (the T64 loose-heading pattern) ----

const PHASE3_HEADING: &str = "## Phase 3";
const HARD_RULES_HEADING: &str = "## Hard rules";

fn window<'a>(spec: &'a str, start_marker: &str, end_marker: &str, what: &str) -> &'a str {
    let start = spec
        .find(start_marker)
        .unwrap_or_else(|| panic!("the {what} window's start marker {start_marker:?} is present"));
    let end = start
        + spec[start..]
            .find(end_marker)
            .unwrap_or_else(|| panic!("the {what} window's end marker {end_marker:?} follows its start"));
    &spec[start..end]
}

// ---- the count-method clause (T269 req 1) ----

/// The count-method sentence as the spec writes it: the authority
/// (DISTINCT flipped TODO row-ids via the flip-commit subjects), the
/// exact command shape, and the every-wrap recompute rule, in one
/// sentence. The command shape carries the T276 leading id-list anchor
/// (the `sed` step reduces each flip subject to its LEADING row-id list
/// before the `-oE` extraction).
const COUNT_METHOD_CLAUSE: &str = "COUNT METHOD (T269): the items-since-tag count is the count of \
DISTINCT flipped TODO row-ids in the range — the flip-commit subjects \
(`^todo:`) name every flipped row including bundled rows (Phase 2 step 5), \
so the mechanical count is `git log <tag>..HEAD --format='%s' | grep -E \
'^todo:' | sed -E 's/^todo: (file |fix )?(T[0-9]+( *[+,] \
*T[0-9]+)*).*/\\2/' | grep -oE 'T[0-9]+' | sort -u | wc -l` (or an \
equivalent distinct-id method) against the ancestor-sanitized tag above, \
recomputed mechanically at EVERY wrap — never quoted forward from a \
previous wrap's notes.";

/// The exact command shape — the mechanical count itself, greppable
/// end-to-end (the acceptance grep pins `sort -u` out of this shape).
/// The `sed` step is the T276 leading id-list anchor: it extracts the
/// flip subject's leading row-id list before the `-oE` pass, so stray
/// trailing-prose T-tokens (the d1791632020-16 third sighting) cannot
/// inflate the distinct-id recount, while bundled `T<a> + T<b>` shapes
/// survive (every bundled id sits in the leading list).
const COMMAND_SHAPE: &str = "git log <tag>..HEAD --format='%s' | grep -E '^todo:' | sed -E 's/^todo: (file |fix )?(T[0-9]+( *[+,] *T[0-9]+)*).*/\\2/' | grep -oE 'T[0-9]+' | sort -u | wc -l";

// ---- the ban sentence (T269 req 1's explicit ban) ----

/// The ban sentence as the spec writes it — BOTH drift shapes named as
/// the banned authorities, with the ban phrase verbatim.
const BAN_SENTENCE: &str = "Merge-subject enumeration (`Merge branch` greps) and in-cycle memory \
are never the count authority";

/// (a) The count-method sentence lives in the Phase-3 window (the
/// release-trigger bullet's section) EXACTLY once, flat-matched. Zero
/// means the T269 clause was deleted, rewrapped, or reworded out of shape
/// (the undercount class re-opens — merge-subject greps and in-cycle
/// memory creep back as the de-facto count); more than one means the
/// method was stated twice.
#[test]
fn count_method_clause_sits_in_phase3_release_trigger_exactly_once() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    let needle = flat(COUNT_METHOD_CLAUSE);
    assert!(
        needle.contains(COMMAND_SHAPE),
        "the clause needle must carry the exact mechanical command shape \
         verbatim ({COMMAND_SHAPE:?})"
    );
    assert!(
        needle.contains("DISTINCT"),
        "the clause needle must name the count as DISTINCT row-ids"
    );
    assert!(
        needle.contains("recomputed mechanically at EVERY wrap"),
        "the clause needle must carry the every-wrap recompute rule — \
         quoted-forward counts are the 13-wrap drift shape"
    );
    let spec = flat(&loop_spec());
    let p3 = window(
        &spec,
        &flat(PHASE3_HEADING),
        &flat(HARD_RULES_HEADING),
        "Phase-3",
    );
    assert_eq!(
        p3.matches(&needle).count(),
        1,
        "LOOP-SPEC Phase 3's release trigger must carry the T269 count-method \
         sentence exactly once ({needle:?}) — zero means the clause was \
         deleted, rewrapped, or reworded, more than one means it is stated \
         twice"
    );
}

/// (b) The mechanical command shape and its greppable tokens are present
/// file-wide, raw text unflattened — the acceptance grep's direct pin.
/// Presence-only: a future doctrine edit that names a token a second time
/// must not spuriously red this leg; leg (a) carries the positional and
/// duplication guards.
#[test]
fn mechanical_command_shape_and_tokens_present_file_wide() {
    let spec = loop_spec();
    assert!(
        spec.contains(COMMAND_SHAPE),
        "LOOP-SPEC.md must contain the mechanical count command shape \
         {COMMAND_SHAPE:?} — the T269 count method drifted out of doctrine"
    );
    assert!(
        spec.contains("^todo:"),
        "LOOP-SPEC.md must contain the literal `^todo:` flip-subject token — \
         the T269 count method drifted out of doctrine"
    );
    assert!(
        spec.contains("sort -u"),
        "LOOP-SPEC.md must contain the literal `sort -u` distinct-id \
         deduplication — the T269 count method drifted out of doctrine"
    );
}

/// (c) `distinct` is named case-insensitively (the acceptance grep is
/// `grep -ci 'distinct'`): the count is DISTINCT ROW-IDS, not commits —
/// a bundled flip names every row in one commit, so the row-id count is
/// the one that satisfies the ≥3 trigger.
#[test]
fn distinct_is_named_case_insensitively() {
    let spec = loop_spec();
    assert!(
        spec.to_lowercase().contains("distinct"),
        "LOOP-SPEC.md must name the count as distinct (any case) flipped \
         row-ids — the T269 count method drifted out of doctrine"
    );
    // The authority word sits inside the pinned clause itself.
    assert!(
        flat(COUNT_METHOD_CLAUSE).contains("DISTINCT"),
        "the clause needle must itself carry the DISTINCT authority word — \
         a mangled needle must not let this pin pass silently"
    );
}

/// (d) The ban sentence sits in the Phase-3 window EXACTLY once,
/// flat-matched, and names BOTH banned count authorities — merge-subject
/// enumeration (`Merge branch` greps) and in-cycle memory. Zero means the
/// ban was deleted or reworded (one drift shape re-opens while the other
/// stays banned); more than one means it is stated twice.
#[test]
fn ban_sentence_names_both_banned_authorities_exactly_once() {
    // Needle self-checks (T48 idiom).
    let needle = flat(BAN_SENTENCE);
    assert!(
        needle.contains("`Merge branch` greps"),
        "the ban needle must name merge-subject enumeration as banned"
    );
    assert!(
        needle.contains("in-cycle memory"),
        "the ban needle must name in-cycle memory as banned"
    );
    assert!(
        needle.contains("never the count authority"),
        "the ban needle must carry the ban phrase verbatim"
    );
    let spec = flat(&loop_spec());
    let p3 = window(
        &spec,
        &flat(PHASE3_HEADING),
        &flat(HARD_RULES_HEADING),
        "Phase-3",
    );
    assert_eq!(
        p3.matches(&needle).count(),
        1,
        "LOOP-SPEC Phase 3 must ban merge-subject enumeration and in-cycle \
         memory as count authorities in one sentence exactly once \
         ({needle:?}) — zero means the ban was deleted or reworded, more \
         than one means it is stated twice"
    );
}

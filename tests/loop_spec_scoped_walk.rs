//! T293 — the scoped-walk rule pin: orchestrator probes never walk the
//! repo root; prune-first when a walk is unavoidable.
//!
//! The repo-root recursive-walk 300s kill census hit its fifth fire at the
//! cycle-673 wrap (record d1791821960-7): a compose-probe batch ran
//! `find . -name '*.plist' -not -path './target*'` from the repo root — a
//! full recursive walk whose `-not -path` clause filters OUTPUT lines but
//! never prunes the TRAVERSAL, so the walk still descended every directory
//! including `target/` and the bash cap killed the process group at 300s
//! mid-walk. Zero casualty (the batch's earlier legs' output was preserved;
//! the plist locate was re-probed SCOPED and hit in under a second), but
//! the burn is the class the census paces: five fires of the same shape.
//! The census bullet's discipline — "probes scoped to .chug/, tests/*.rs,
//! named files" — existed as hygiene but carried no load-bearing doctrine
//! sentence and no pin: nothing mechanically greeted a cold orchestrator
//! composing a locating probe. T293 executes the bullet's armed letter:
//! ONE paragraph in META-META-SPEC's probe-discipline area (beside the
//! T284 probe-hygiene rule it extends) carrying the ban, the SCOPED shapes
//! a locating probe takes first, the prune-first shape for the genuinely
//! unavoidable walk, the census + record evidence, and the class letter —
//! the counted census + the doctrine sentence IS the guard; a mechanical
//! walk-detector over probe text is the brittle-awk class and stays
//! rejected (a 6th fire re-weighs the declined `scripts/scoped-find.sh`
//! helper).
//!
//! These pins assert that paragraph exists, sits where the spec requires,
//! and carries its load-bearing sentences. No production code changes;
//! doctrine-only. The pin file greps META-META-SPEC.md only — never a
//! spec prose copy (the T67 self-match lesson).
//!
//! T48 doctrine: every pin resolves its file from the checkout the binary
//! RUNS against (`std::env::current_dir()`; cargo runs test binaries with
//! cwd = the package root), never via the compile-time manifest-dir
//! macro — under the T47 shared cache a compile-time path can point at a
//! since-removed worktree. T78 idiom: multi-word needles are matched
//! against whitespace-collapsed text because the doctrine prose wraps
//! mid-phrase. Deletion-proof convention: every count leg doubles as the
//! deletion check (delete the paragraph → count 0 → RED). RED-proof: the
//! orchestrator RED-proved this pin pre-merge by mutating the doctrine
//! paragraph in the worktree (deleting the paragraph, and separately
//! rewording the mechanism sentence to the fire's broken `-not -path`
//! filter shape) and watching these legs go red.

fn meta_meta_spec() -> String {
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    std::fs::read_to_string(root.join("META-META-SPEC.md"))
        .unwrap_or_else(|e| panic!("reading META-META-SPEC.md from the runtime checkout: {e}"))
}

/// Wrap-insensitive copy (the T78 flat idiom): the doctrine prose wraps
/// mid-phrase, so a multi-word needle must match whitespace-collapsed text
/// or the pin goes red on the REAL doctrine.
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- windows (the T64 loose-heading pattern) ----

/// The probe-discipline paragraph the scoped-walk rule EXTENDS — the T284
/// probe-hygiene lead opens the window.
const PROBE_HYGIENE_LEAD: &str = "**Probe hygiene (T284";
/// The window closes where the Read-first corpus list ends — the next
/// `##` section heading.
const WRITE_EVAL_HEADING: &str = "## Write `EVALUATION.md`";

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

// ---- the paragraph's load-bearing needles ----

/// The paragraph's bold lead — the rule's name + T-number + the required
/// `prune-first` token.
const SCOPED_WALK_LEAD: &str = "Scoped-walk rule (T293, prune-first / scoped-probe)";

/// The ban sentence's core: never a repo-root recursive walk INLINE.
const BAN: &str = "NEVER run a recursive walk from the repo root inline";

/// The ban's walk family — the kin named in the spec.
const WALK_KIN: &str = "`grep -r .`, `du .` and kin";

/// The census, NAMED (the spec's required token `recursive-walk 300s
/// kill` sits inside it).
const CENSUS: &str = "repo-root recursive-walk 300s kill";

/// The fifth fire's record id — the paragraph's evidence.
const RECORD: &str = "d1791821960-7";

/// The fifth fire's exact command, quoted.
const FIFTH_FIRE: &str = "`find . -name '*.plist' -not -path './target*'`";

/// The zero-casualty note (no figure was ever quoted from the killed leg).
const ZERO_CASUALTY: &str = "no figure was quoted from the killed leg";

/// The ban's mechanism sentence — WHY a filter is not a prune. The
/// cycle-673 fire's exact mechanism.
const MECHANISM: &str = "a `-not -path` filter does NOT prune the traversal";

/// The prune-first shape — the `-prune` STOPS the descent.
const PRUNE_CMD: &str = "`find . -path ./target -prune -o ... -print`";

/// The class letter: the counted census + the doctrine sentence IS the
/// guard.
const GUARD: &str = "The counted census + this doctrine sentence IS the guard";

/// The rejected alternative — the brittle-awk class letter.
const WALK_DETECTOR_REJECT: &str =
    "a mechanical walk-detector over probe text is the brittle-awk class";

/// The weighed-and-DECLINED helper (requirement 3: no helper script; a 6th
/// fire re-weighs it).
const HELPER_DECLINED: &str = "`scripts/scoped-find.sh`";

// ---- the SCOPED shapes a locating probe takes first ----

const LS_CHUG: &str = "`ls .chug/`";
const LS_TESTS: &str = "`ls tests/`";
const LADDER: &str = "`[ -f path ]` ladder";
const LS_FILES: &str = "`git ls-files`";
const BOUNDED_FIND: &str = "`find <named-dir> -maxdepth N`";

/// (a) The scoped-walk paragraph lives in the probe-discipline area —
/// INSIDE the window opened by the T284 probe-hygiene lead it extends and
/// closed by the next `##` heading — and carries the ban sentence, the
/// census name, the fifth fire's record id, the quoted fifth-fire command,
/// and the zero-casualty note, each EXACTLY once (flat-matched). Deleting
/// the paragraph drops every count to 0; moving it out of the
/// probe-discipline area does too; duplicating it doubles them.
#[test]
fn scoped_walk_paragraph_lives_beside_probe_hygiene_with_its_evidence() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        SCOPED_WALK_LEAD.starts_with("Scoped-walk rule (T293")
            && SCOPED_WALK_LEAD.contains("prune-first")
            && SCOPED_WALK_LEAD.ends_with("scoped-probe)"),
        "the lead needle must name the rule, its T-number, and the \
         prune-first token verbatim"
    );
    assert!(
        BAN.starts_with("NEVER") && BAN.ends_with("inline"),
        "the ban needle must carry the never-inline prohibition verbatim"
    );
    assert!(
        CENSUS.contains("recursive-walk 300s kill"),
        "the census needle must carry the spec's required census token"
    );
    assert!(
        RECORD.starts_with("d179") && RECORD.len() == 13,
        "the record needle must be the census's fifth-fire id"
    );
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    let probe_area = window(
        &spec,
        &flat(PROBE_HYGIENE_LEAD),
        &flat(WRITE_EVAL_HEADING),
        "probe-discipline",
    );
    for (needle, what) in [
        (SCOPED_WALK_LEAD, "the scoped-walk rule's lead"),
        (BAN, "the never-walk-inline ban"),
        (WALK_KIN, "the walk family (`find .`/`grep -r .`/`du .` kin)"),
        (CENSUS, "the census name"),
        (RECORD, "the fifth fire's record id"),
        (FIFTH_FIRE, "the quoted fifth-fire command"),
        (ZERO_CASUALTY, "the zero-casualty note"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            probe_area.matches(&needle).count(),
            1,
            "META-META-SPEC's probe-discipline area must state {what} \
             exactly once — zero means the T293 scoped-walk paragraph was \
             deleted, reworded, or moved out of the area beside the T284 \
             probe-hygiene rule it extends, more than one means it is \
             stated twice"
        );
    }
    // Placement: the paragraph sits AFTER the probe-hygiene lead it
    // extends, inside the same window.
    let hygiene_at = probe_area
        .find(&flat(PROBE_HYGIENE_LEAD))
        .expect("the probe-hygiene lead opens the window");
    let scoped_at = probe_area
        .find(&flat(SCOPED_WALK_LEAD))
        .expect("the scoped-walk lead is in the window (counted above)");
    assert!(
        scoped_at > hygiene_at,
        "the scoped-walk paragraph must FOLLOW the T284 probe-hygiene \
         paragraph it extends ({scoped_at} vs {hygiene_at}) — moved before \
         it, the rule no longer reads as the walk-shaped member of the \
         probe-discipline family"
    );
}

/// (b) The SCOPED shapes the paragraph mandates for locating probes —
/// named directories, the `[ -f path ]` ladder, `git ls-files`
/// (index-resident, no walk), the bounded `find` — and the prune-first
/// shape for the genuinely unavoidable walk, each EXACTLY once inside the
/// probe-discipline window. The lowercase `prune-first` token occurs
/// EXACTLY once (the lead) and the PRUNE-FIRST emphasis once — the spec's
/// check line greps the token, so zero occurrences is an instant RED.
#[test]
fn scoped_shapes_and_the_prune_first_shape_are_pinned() {
    assert!(
        PRUNE_CMD.contains("-prune") && PRUNE_CMD.contains("-print"),
        "the prune-first needle must carry the -prune/-o/-print shape"
    );
    assert!(
        LADDER.starts_with("`[ -f") && LADDER.ends_with("ladder"),
        "the ladder needle must be the [ -f path ] ladder shape verbatim"
    );
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    let probe_area = window(
        &spec,
        &flat(PROBE_HYGIENE_LEAD),
        &flat(WRITE_EVAL_HEADING),
        "probe-discipline",
    );
    for (needle, what) in [
        (LS_CHUG, "the named-directory shape for .chug/"),
        (LS_TESTS, "the named-directory shape for tests/"),
        (LADDER, "the [ -f path ] ladder shape"),
        (LS_FILES, "the index-resident git ls-files shape"),
        (BOUNDED_FIND, "the bounded named-dir find shape"),
        (PRUNE_CMD, "the prune-first command shape"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            probe_area.matches(&needle).count(),
            1,
            "the probe-discipline window must state {what} exactly once — \
             zero means a SCOPED shape (or the prune-first shape) was \
             dropped from the T293 paragraph, more than one means it is \
             stated twice"
        );
    }
    // The required literal token, lowercase, exactly once in the window —
    // the spec's own check line greps `prune-first` in META-META-SPEC.md.
    assert_eq!(
        probe_area.matches("prune-first").count(),
        1,
        "the probe-discipline window must carry the `prune-first` token \
         exactly once — zero means the token (and likely the paragraph) \
         was deleted; more than one means a duplicate statement drifted in"
    );
    // The uppercase emphasis is the paragraph's second mention of the
    // doctrine's name — exactly one, so the rule reads once as a name and
    // once as the imperative.
    assert_eq!(
        probe_area.matches("PRUNE-FIRST").count(),
        1,
        "the probe-discipline window must carry the PRUNE-FIRST emphasis \
         exactly once — zero means the unavoidable-walk imperative was \
         reworded away, more than one means it is stated twice"
    );
}

/// (c) The ban's mechanism sentence and the class letter: the paragraph
/// must say WHY the fifth fire fired (a `-not -path` filter does not prune
/// the traversal) and must close with the census-as-guard letter that
/// keeps a mechanical walk-detector rejected. Plus the negative leg: the
/// `-not -path` token occurs EXACTLY TWICE in the window — once inside the
/// quoted fifth-fire command (the evidence) and once inside the mechanism
/// sentence (the diagnosis). A THIRD occurrence means someone drifted in
/// another `-not -path`-as-filter recommendation — the exact broken shape
/// the paragraph exists to ban.
#[test]
fn mechanism_sentence_and_the_census_as_guard_are_pinned() {
    assert!(
        MECHANISM.starts_with("a `-not -path` filter")
            && MECHANISM.ends_with("prune the traversal"),
        "the mechanism needle must carry the does-not-prune diagnosis \
         verbatim"
    );
    assert!(
        GUARD.starts_with("The counted census") && GUARD.ends_with("IS the guard"),
        "the guard needle must be the census-as-guard letter verbatim"
    );
    assert!(
        WALK_DETECTOR_REJECT.contains("walk-detector")
            && WALK_DETECTOR_REJECT.ends_with("brittle-awk class"),
        "the walk-detector needle must be the rejected-alternative letter \
         verbatim"
    );
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    let probe_area = window(
        &spec,
        &flat(PROBE_HYGIENE_LEAD),
        &flat(WRITE_EVAL_HEADING),
        "probe-discipline",
    );
    for (needle, what) in [
        (MECHANISM, "the does-not-prune mechanism sentence"),
        (GUARD, "the census-as-guard class letter"),
        (WALK_DETECTOR_REJECT, "the brittle-awk walk-detector rejection"),
        (HELPER_DECLINED, "the declined scoped-find helper's re-arm note"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            probe_area.matches(&needle).count(),
            1,
            "the probe-discipline window must state {what} exactly once — \
             zero means the T293 paragraph's diagnosis or class letter was \
             deleted, more than one means it is stated twice"
        );
    }
    // Negative leg: `-not -path` appears exactly twice — the quoted fire
    // and the mechanism's naming of it. A third is a drifted-in
    // filter-shaped recommendation (the ban's own target shape).
    assert_eq!(
        probe_area.matches("-not -path").count(),
        2,
        "the probe-discipline window must carry `-not -path` exactly twice \
         (the quoted fifth-fire command + the mechanism sentence naming it) \
         — a third occurrence means another -not -path-as-filter \
         recommendation drifted into doctrine, the exact broken shape the \
         scoped-walk rule bans"
    );
    // The record id is unique FILE-WIDE: the census's fifth fire is named
    // as the paragraph's evidence exactly once, so re-quoting it elsewhere
    // (or losing it to a reword) goes red.
    assert_eq!(
        spec.matches(RECORD).count(),
        1,
        "META-META-SPEC must name the fifth fire's record id {RECORD:?} \
         exactly once file-wide — zero means the paragraph's evidence was \
         dropped, more than one means it is quoted twice"
    );
}

// ---- the T297 python accessor-shape extension ----

/// The python variant's census name — the spec's required literal token
/// (the T297 check line greps it in META-META-SPEC.md).
const PYTHON_PROBE_SHAPE: &str = "python-probe-shape";

/// The python noisy failure's core phrase — the spec's required literal
/// phrase (the TypeError's stem, grepped by the T297 check line).
const LIST_INDICES: &str = "list indices must be integers";

/// The root the python text extends — its bold opening needle.
const ACCESSOR_ROOT: &str = "**The accessor-shape root:**";

/// The paragraph's closing letter — the python extension sits BEFORE it,
/// so the letter still reads as covering both tools.
const BOTH_CLASSES: &str = "Both classes are a probe composed from habit";

/// The jq legs preserved verbatim (an EXTENSION, never a rewrite): the
/// jq-probe-slip family name and the jq noisy array-index failure quote.
const JQ_FAMILY: &str = "jq-probe-slip family";
const JQ_NOISY: &str = "Cannot index array with string";

/// (d) The T284 probe-hygiene paragraph's accessor-shape root names the
/// PYTHON variant beside jq: the `python-probe-shape` census token and
/// the `list indices must be integers` TypeError phrase each occur
/// exactly once (in the probe-discipline window AND file-wide), the
/// extension sits INSIDE the accessor-shape root (after its opening
/// needle, before the paragraph's closing both-classes letter), the T284
/// lead still opens the paragraph, and the jq legs it extends are intact
/// (the jq-probe-slip family + the jq noisy quote, each exactly once).
/// RED-proof: the orchestrator proved this pin pre-merge by mutating the
/// doctrine paragraph in the worktree — deleting the python extension
/// (both token counts → 0) and separately reverting it to jq-only text —
/// and watching these legs go red.
#[test]
fn probe_hygiene_accessor_root_names_the_python_probe_shape() {
    // Needle self-checks (T48 idiom): a mangled needle must not let this
    // pin pass silently.
    assert!(
        PYTHON_PROBE_SHAPE.starts_with("python-") && PYTHON_PROBE_SHAPE.ends_with("-shape"),
        "the census needle must be the python-probe-shape token verbatim"
    );
    assert!(
        LIST_INDICES.starts_with("list indices") && LIST_INDICES.ends_with("integers"),
        "the TypeError needle must be the list-indices phrase verbatim"
    );
    let spec_text = meta_meta_spec();
    let spec = flat(&spec_text);
    let probe_area = window(
        &spec,
        &flat(PROBE_HYGIENE_LEAD),
        &flat(WRITE_EVAL_HEADING),
        "probe-discipline",
    );
    // The required tokens, EXACTLY once in the window and once file-wide —
    // zero means the python extension was deleted or reworded (the
    // T297 check line's greps go red with it), more than one means a
    // duplicate statement drifted in.
    for (needle, what) in [
        (PYTHON_PROBE_SHAPE, "the python-probe-shape census token"),
        (LIST_INDICES, "the list-indices TypeError phrase"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            probe_area.matches(&needle).count(),
            1,
            "the probe-discipline window must state {what} exactly once — \
             zero means the T297 python extension was deleted, reworded, \
             or moved out of the T284 paragraph, more than one means it \
             is stated twice"
        );
        assert_eq!(
            spec.matches(&needle).count(),
            1,
            "META-META-SPEC must carry {what} exactly once file-wide"
        );
    }
    // The T284 lead still opens the paragraph — the extension never
    // reworded the paragraph's existing opening.
    assert_eq!(
        spec.matches(&flat(PROBE_HYGIENE_LEAD)).count(),
        1,
        "META-META-SPEC must still lead the probe-hygiene paragraph with \
         the T284 needle exactly once"
    );
    // Placement: the python text sits INSIDE the accessor-shape root —
    // after its opening needle, before the paragraph's closing letter —
    // so the root reads as one root with a jq arm and a python arm.
    let root_at = probe_area
        .find(&flat(ACCESSOR_ROOT))
        .expect("the accessor-shape root opens inside the window");
    let python_at = probe_area
        .find(&flat(PYTHON_PROBE_SHAPE))
        .expect("the python census is in the window (counted above)");
    let both_at = probe_area
        .find(&flat(BOTH_CLASSES))
        .expect("the closing both-classes letter is in the window");
    assert!(
        root_at < python_at && python_at < both_at,
        "the python extension must sit INSIDE the accessor-shape root \
         (after its opening, before the closing both-classes letter) — \
         moved outside it, the paragraph no longer reads as one root \
         with a jq arm and a python arm"
    );
    // The jq legs are preserved (an EXTENSION, never a rewrite): the
    // jq-probe-slip family and the jq noisy-failure quote still occur
    // exactly once each in the window.
    for (needle, what) in [
        (JQ_FAMILY, "the jq-probe-slip family name"),
        (JQ_NOISY, "the jq noisy array-index failure quote"),
    ] {
        let needle = flat(needle);
        assert_eq!(
            probe_area.matches(&needle).count(),
            1,
            "the probe-discipline window must still state {what} exactly \
             once — zero means the python extension REWROTE the jq legs \
             instead of extending them"
        );
    }
}

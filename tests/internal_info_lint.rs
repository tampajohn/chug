//! T249 — the internal-info lint: the repo can never re-accumulate org details.
//!
//! Operator history: the public repo once carried org-identifying literals
//! (the org name, its fleet abbreviation, internal monorepo codenames, an
//! internal proxy hostname, internal DNS hosts). The operator's redaction
//! pass landed at 47cd0d1 (amended from a local pass whose commit message
//! named the literals — that earlier object is local-only dangling history).
//! This test is the org-info sibling of `no_secret_spill.rs` (T205 req 5):
//! it walks every TRACKED file and fails on any line matching the pattern
//! list, so the class cannot regrow through loop-authored content (evals
//! quote operator context; specs get written with internal names).
//!
//! The pattern list below is the SINGLE SOURCE of the exact literals (req 1
//! of specs/t249-internal-info-lint.md — the spec names classes and match
//! rules only, and is itself a tracked file this lint scans, so it must
//! stay redacted). Each entry's `reason` names its spec class and its
//! derivation: the before-lines of `git show 47cd0d1` plus the pre-amend
//! message inventory. Report file:line, NEVER the line content.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Word char for boundary purposes (the `\b` class): ASCII alphanumeric or
/// `_`. A multi-byte char's bytes never land in this class, so non-ASCII
/// prose is a boundary — the correct reading for a word-bounded needle.
fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// `needle` occurs in `line` with BOTH ends word-bounded. `ci` lowercases
/// both sides first (ASCII-only transform: byte offsets stay valid).
/// Word-bounding is why hex shas and embedded prose never trip: an embedded
/// occurrence has word chars on at least one side and does not match.
fn word_bounded(line: &str, needle: &str, ci: bool) -> bool {
    let hay = if ci {
        line.to_ascii_lowercase()
    } else {
        line.to_string()
    };
    let needle = if ci {
        needle.to_ascii_lowercase()
    } else {
        needle.to_string()
    };
    let bytes = hay.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = hay[from..].find(needle.as_str()) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok = start == 0 || !is_word_char(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_word_char(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        // needles start with an ASCII byte, so start+1 is a char boundary
        from = start + 1;
    }
    false
}

/// Spec class 8, verbatim: `[a-z0-9][a-z0-9.-]*\.internal\.com\b` — a
/// host-shaped run of DNS chars directly under the `.internal.com` TLD.
/// The trailing `\b` is what lets `x.internal.company` pass (the char after
/// `com` is a word char, so no boundary). The leading class needs at least
/// one `[a-z0-9]` before the dot-run, so a bare `.internal.com` never
/// matches. (Hosts of the derivation's own shape — `…org-internal.com` —
/// have no dot before `internal`, so class 1 catches them instead; the two
/// classes layer by design.)
fn internal_host(line: &str) -> bool {
    const TLD: &str = ".internal.com";
    let bytes = line.as_bytes();
    let mut from = 0usize;
    while let Some(pos) = line[from..].find(TLD) {
        let dot = from + pos;
        let end = dot + TLD.len();
        let trailing_ok = end == bytes.len() || !is_word_char(bytes[end]);
        // consume [a-z0-9.-]* leftward from the TLD's dot; the regex matches
        // iff some consumed char is [a-z0-9] (the match may start there)
        let mut left = dot;
        while left > 0 {
            let b = bytes[left - 1];
            if b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-' {
                left -= 1;
            } else {
                break;
            }
        }
        let has_leading = bytes[left..dot]
            .iter()
            .any(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if trailing_ok && has_leading {
            return true;
        }
        from = dot + 1; // the byte at `dot` is '.', so +1 is a char boundary
    }
    false
}

struct Pattern {
    name: &'static str,
    reason: &'static str,
    hits: fn(&str) -> bool,
}

/// The pattern list — the single source of the exact literals (req 1).
/// Derivation record for every entry: the before-lines of
/// `git show 47cd0d1` + the pre-amend message inventory.
const PATTERNS: &[Pattern] = &[
    Pattern {
        name: "org-name",
        reason: "spec class 1: the org NAME, case-insensitive word — \
                 `VideoAmp` in the 47cd0d1 before-lines (\"a VideoAmp (VA) \
                 machine fleet\", \"the videoamp HF org\", \
                 `videoamp/laya-base`); allow-listed in LICENSE only (MIT \
                 attribution)",
        hits: |line| word_bounded(line, "videoamp", true),
    },
    Pattern {
        name: "fleet-abbrev",
        reason: "spec class 2: the org's two-letter fleet abbreviation, \
                 case-sensitive word-bounded — `VA` in the before-lines \
                 (\"VA-private\", \"VA fleet\", \"VA hosts\"); word-boundary \
                 so hex shas and prose (VAULT, VALIDATE) never trip",
        hits: |line| word_bounded(line, "VA", false),
    },
    Pattern {
        name: "monorepo-codename",
        reason: "spec class 3: the internal monorepo codename, one word \
                 case-insensitive — `vampflake` in the before-lines (\"the \
                 vampflake pattern\", \"vampflake/central\", \
                 `VAMPFLAKE-CHUG-FEASIBILITY.md`)",
        hits: |line| word_bounded(line, "vampflake", true),
    },
    Pattern {
        name: "monorepo-intermediate",
        reason: "spec class 4: the monorepo's intermediate name, \
                 case-insensitive — `central` (\"vampflake/central\", the \
                 pre-amend message's `vampflake-central`); word-bounded so \
                 `centralized` never trips",
        hits: |line| word_bounded(line, "central", true),
    },
    Pattern {
        name: "proxy-hostname",
        reason: "spec class 6: the internal proxy hostname, exact literal — \
                 `internal-llm-proxy` in the 47cd0d1 before-lines (src/api.rs \
                 comments, specs/t112; the pre-amend pass first generalized \
                 the raw host to this form, the landed pass to prose)",
        hits: |line| word_bounded(line, "internal-llm-proxy", true),
    },
    Pattern {
        name: "monorepo-repo-caps",
        reason: "spec class 7: the internal monorepo repo name, CAPS form \
                 only — `INTERNAL-MONOREPO` (the landed diff's t191 \
                 before-line `INTERNAL-MONOREPO-FEASIBILITY.md`); the \
                 lowercase after-form `internal-monorepo` is the operator's \
                 generalized vocabulary and is legal everywhere",
        hits: |line| word_bounded(line, "INTERNAL-MONOREPO", false),
    },
    Pattern {
        name: "internal-host",
        reason: "spec class 8: internal DNS hosts \
                 `[a-z0-9][a-z0-9.-]*\\.internal\\.com\\b` — the filing's \
                 `*.internal.com hosts` inventory; trailing word-boundary so \
                 `x.internal.company` never trips",
        hits: internal_host,
    },
];

// Spec class 5 (internal machine codename, letter+digits, case-sensitive,
// word-bounded) has NO pattern entry, deliberately. Its only derivation
// candidates are `K7` and `F94` (DEPENDENCIES.md's "operator session, F94
// checkout" and FEATURES.md's "On K7 — where the loop lives" before-lines).
// Both are a KEPT class, not a pattern: the operator's own landed sweep
// redacted them in two prose spots and kept ~27 mentions across TODO.md,
// loopd.sh, scripts/, ten specs, and src/daemon.rs — operator-personal
// infra (the `tampajohn` category, not org info), and this arc may not
// touch TODO.md/loopd.sh, so a K7/F94 pattern could never reach the spec's
// zero-hit-at-HEAD gate. Excluded per the spec's own over-broad/refine rule
// (specs/t249-internal-info-lint.md), dashd-style; the kept-class shapes
// are pinned passing below so the exclusion stays load-bearing.

/// Per-FILE allow-list (req 2): exact repo-relative paths never scanned.
/// Today the only entry is LICENSE — the org name appears there as the MIT
/// attribution ("Copyright (c) 2026 VideoAmp, Inc.") and is intentional.
const FILE_ALLOWLIST: &[&str] = &["LICENSE"];

/// Per-LINE allow-list (req 2): exact (path, 1-based line) pairs exempted
/// where a pattern legally appears. No substring-broad exemptions — an
/// entry names one line in one file. Empty today (LICENSE is per-file).
const LINE_ALLOWLIST: &[(&str, usize)] = &[];

/// This lint's own source — its pattern literals ARE the point, so the walk
/// skips it by exact filename (the no_secret_spill.rs self-exclusion).
const SELF_FILE: &str = "internal_info_lint.rs";

fn is_file_allowed(rel: &str) -> bool {
    FILE_ALLOWLIST.contains(&rel)
}

fn is_line_allowed(rel: &str, line_no: usize) -> bool {
    LINE_ALLOWLIST.contains(&(rel, line_no))
}

/// Matched patterns for one line, as (pattern name) — the caller pairs it
/// with file:line. Never the line content.
fn scan_line(line: &str) -> Vec<&'static str> {
    PATTERNS
        .iter()
        .filter(|p| (p.hits)(line))
        .map(|p| p.name)
        .collect()
}

/// One hit: repo-relative path, 1-based line, pattern name.
struct Hit {
    file: String,
    line: usize,
    pattern: &'static str,
}

impl Hit {
    fn label(&self) -> String {
        format!("{}:{} ({})", self.file, self.line, self.pattern)
    }
}

/// Walk TRACKED files (`git ls-files -z` run from `root` = the test cwd =
/// the package root; never CARGO_MANIFEST_DIR per the T48 pin), skip
/// `*.lock` and the lint's own source, apply the allow-lists, and scan
/// line-by-line. UTF-8-unreadable files are skipped (they cannot be prose
/// spills). Returns the scanned relative paths (for scope pins) and the
/// hits. A git failure aborts the walk loudly — the lint must never pass
/// because it silently scanned nothing.
fn scan_tracked_tree(root: &Path) -> (Vec<PathBuf>, Vec<Hit>) {
    let out = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(root)
        .output()
        .expect("git ls-files runs (the lint walks tracked files)");
    assert!(
        out.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut scanned = Vec::new();
    let mut hits = Vec::new();
    for part in out.stdout.split(|b| *b == 0u8) {
        if part.is_empty() {
            continue;
        }
        #[cfg(unix)]
        let rel = {
            use std::os::unix::ffi::OsStrExt;
            PathBuf::from(std::ffi::OsStr::from_bytes(part))
        };
        #[cfg(not(unix))]
        let rel = PathBuf::from(String::from_utf8_lossy(part).into_owned());
        let rel_str = rel.to_str().unwrap_or("").to_string();
        let name = rel.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == SELF_FILE {
            continue; // this file — the matcher itself, no org content of its own
        }
        if name.ends_with(".lock") {
            continue; // Cargo.lock et al — lockfile hashes, not prose
        }
        if is_file_allowed(&rel_str) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&rel)) else {
            continue; // UTF-8-unreadable — skipped, cannot be prose
        };
        scanned.push(rel);
        for (i, line) in text.lines().enumerate() {
            let line_no = i + 1;
            if is_line_allowed(&rel_str, line_no) {
                continue;
            }
            for pattern in scan_line(line) {
                hits.push(Hit {
                    file: rel_str.clone(),
                    line: line_no,
                    pattern,
                });
            }
        }
    }
    (scanned, hits)
}

#[test]
fn tracked_files_carry_zero_org_details() {
    // runtime root, never the compile-time macro (the T48 pin bans
    // CARGO_MANIFEST_DIR in Rust source: cargo runs test binaries with
    // cwd = the package root)
    let root = std::env::current_dir().expect("cargo sets the test cwd to the package root");
    let (scanned, hits) = scan_tracked_tree(&root);
    assert!(
        scanned.len() > 50,
        "the walk found only {} tracked files — the lint must cover the real tree",
        scanned.len()
    );
    let names: Vec<String> = scanned.iter().map(|p| p.display().to_string()).collect();
    // the named redaction targets and the spec itself are IN scope …
    for must_scan in [
        "specs/t249-internal-info-lint.md",
        "specs/t205-hf-org-laya-hosting.md",
        "runbooks/laya-hf-hosting.md",
    ] {
        assert!(
            names.iter().any(|n| n == must_scan),
            "{must_scan} must be walked — it is a redaction target / the spec itself"
        );
    }
    // … and the skip rules really skip
    for must_skip in ["Cargo.lock", "tests/internal_info_lint.rs", "LICENSE"] {
        assert!(
            !names.iter().any(|n| n == must_skip),
            "{must_skip} must not be walked (skip rule / allow-list)"
        );
    }
    assert!(
        hits.is_empty(),
        "internal org details committed at {} — the repo is public; \
         see specs/t249-internal-info-lint.md and the derivation record \
         (git show 47cd0d1 before-lines)",
        hits.iter()
            .map(|h| h.label())
            .collect::<Vec<_>>()
            .join(", ")
    );
}

#[test]
fn planted_org_name_literal_fails() {
    // the derivation's own before-shape, planted (in-test synthetic strings
    // and tempdir fixtures ONLY — never a tracked fixture file, which would
    // trip the lint it tests)
    let planted = scan_line("Any laya fine-tune destined for the videoamp HF org is gated");
    assert_eq!(planted, vec!["org-name"]);
    // the same via the file path a tracked spill would take
    let dir = tempfile::tempdir().expect("tempdir");
    let file = dir.path().join("planted.md");
    std::fs::write(&file, "fine line one\nthe videoamp/laya-base mirror\n")
        .expect("planted fixture writes");
    let text = std::fs::read_to_string(&file).expect("planted fixture reads");
    let hits: Vec<(usize, &'static str)> = text
        .lines()
        .enumerate()
        .flat_map(|(i, l)| scan_line(l).into_iter().map(move |p| (i + 1, p)))
        .collect();
    assert_eq!(hits, vec![(2, "org-name")], "file:line, never content");
}

#[test]
fn every_pattern_entry_documents_its_derivation() {
    // req 1: the pattern list is the single source of the exact literals,
    // each entry carrying a reason that names its spec class and its
    // derivation record — this pin keeps the field load-bearing
    assert_eq!(PATTERNS.len(), 7, "class 5 is a documented kept-class, not a pattern");
    for p in PATTERNS {
        assert!(
            !p.name.is_empty() && !p.reason.is_empty(),
            "pattern entry {} is incomplete",
            p.name
        );
        assert!(
            p.reason.contains("spec class"),
            "{}'s reason must name its spec class",
            p.name
        );
    }
    let names: Vec<&str> = PATTERNS.iter().map(|p| p.name).collect();
    assert_eq!(
        names,
        [
            "org-name",
            "fleet-abbrev",
            "monorepo-codename",
            "monorepo-intermediate",
            "proxy-hostname",
            "monorepo-repo-caps",
            "internal-host",
        ],
        "the pattern table is the derivation's literal source — reorder only with cause"
    );
}

#[test]
fn license_passes_only_via_the_allowlist() {
    // the allow-list is load-bearing: raw scanning of LICENSE HITS (the MIT
    // attribution carries the org name), and the tree walk exempts it by
    // exact file path — not by substring
    let license = std::fs::read_to_string("LICENSE").expect("LICENSE reads");
    let any_hit = license.lines().any(|l| !scan_line(l).is_empty());
    assert!(
        any_hit,
        "LICENSE must raw-hit the org-name pattern — the allow-list is what makes it pass"
    );
    assert!(is_file_allowed("LICENSE"));
    assert!(!is_file_allowed("src/api.rs"));
    assert!(!is_file_allowed("LICENSE.target")); // exact-path, no prefix slop
    // the per-line mechanism exists and is exact; empty today
    assert!(!is_line_allowed("runbooks/x.md", 7));
    assert!(LINE_ALLOWLIST.is_empty(), "no per-line exemptions today");
}

#[test]
fn redacted_forms_pass() {
    // the operator's generalized vocabulary is legal everywhere (spec:
    // the replacement vocabulary must NOT be patterns)
    for legal in [
        // the already-redacted `org/…` form (req 4's named pin)
        "export CHUG_LAYA_CHECKPOINT=\"org/laya-judge@9c6af39cdce45b570f0b7f8fad2b311c96019804\"",
        // the replacement vocabulary in prose
        "F13 fine-tunes go org-private from day one (internal-monorepo decision logs)",
        "One loop per repo is the repo-level lever (the dashboard/internal-monorepo pattern):",
        "against internal monorepos the logs encode internal structure/doctrine: org-private from day one",
        // the org-gated form and the internal machine fleet phrasing
        "org-membership-gated, machines pull via a read-scoped HF_TOKEN",
        "fine-tune for an internal machine fleet — the daemon's risk-gate model",
    ] {
        assert!(
            scan_line(legal).is_empty(),
            "legal redacted form must pass: {legal}"
        );
    }
}

#[test]
fn word_boundary_pins() {
    // a sha-LIKE token with the fleet-abbreviation letters EMBEDDED passes
    // (word-boundary: word chars on at least one side) …
    assert!(scan_line("rev 9c6af39VAd2b311c96019804 pinned").is_empty());
    // … and a real 40-char lowercase sha passes trivially (case: VA is
    // uppercase-only; shas are lowercase hex)
    assert!(scan_line("org/laya-stop-judge@9c6af39cdce45b570f0b7f8fad2b311c96019804").is_empty());
    // machine-codename letters mid-token pass (kept class — see the class-5
    // note above the pattern table)
    assert!(scan_line("rev d0e6f94a1b2c3d4e5f60718293a4b5c6d7e8f90 is pinned").is_empty());
    // standalone forms fail
    assert_eq!(scan_line("migrating it to the VA org"), vec!["fleet-abbrev"]);
    assert_eq!(
        scan_line("the vampflake pattern"),
        vec!["monorepo-codename"]
    );
    assert_eq!(scan_line("use INTERNAL-MONOREPO here"), vec!["monorepo-repo-caps"]);
    assert_eq!(
        scan_line("on the production endpoint (internal-llm-proxy)"),
        vec!["proxy-hostname"]
    );
    // embedded-but-word-char-flanked forms never trip (prose safety)
    assert!(scan_line("the VAULT token and VALIDATE_A flag stay legal").is_empty());
    assert!(scan_line("centralized plumbing is common prose").is_empty());
    // the kept-class shapes pass — the class-5 exclusion is load-bearing
    assert!(scan_line("layad is F94-only (venv + launchd + an S1-quarantine history)").is_empty());
    assert!(scan_line("K7 has 18 cores so 9.68/18=0.54 reads QUIET").is_empty());
}

#[test]
fn internal_host_trailing_boundary_pin() {
    // a host-shaped string under the internal TLD fails …
    assert_eq!(
        scan_line("on the production endpoint (`tools-proxy.internal.com`), streamed"),
        vec!["internal-host"]
    );
    assert_eq!(
        scan_line("point HF_ENDPOINT at proxy.internal.com:8443"),
        vec!["internal-host"]
    );
    // … the same name under a longer public TLD passes (the trailing \b)
    assert!(scan_line("reach the docs at https://x.internal.company/docs").is_empty());
    // the verbatim leading class: a bare dot-prefixed TLD never matches
    assert!(scan_line("the guard watches for .internal.com suffixes").is_empty());
    assert!(scan_line("bare internal.com with no label does not match").is_empty());
    // the derivation's own host form has no dot before `internal`, so class 1
    // catches it (org name) while class 8 stays quiet — layering by design
    assert_eq!(
        scan_line("**Bug**: on the production endpoint (`tools-proxy.videoamp-internal.com`),"),
        vec!["org-name"]
    );
}

#[test]
fn the_derivation_before_shapes_all_fail() {
    // one line echoing each derived class's 47cd0d1 before-shape — the lint
    // must catch exactly the forms the redaction removed
    assert_eq!(
        scan_line("fine-tune for a VideoAmp (VA) machine fleet — the daemon's risk-gate"),
        vec!["org-name", "fleet-abbrev"]
    );
    assert_eq!(
        scan_line("Existing worked examples to distill: vampflake feasibility eval"),
        vec!["monorepo-codename"]
    );
    assert_eq!(
        scan_line("(VAMPFLAKE-CHUG-FEASIBILITY.md arc), codex adversarial review arc,"),
        vec!["monorepo-codename"]
    );
    assert_eq!(
        scan_line("against vampflake/central the logs encode internal structure"),
        vec!["monorepo-codename", "monorepo-intermediate"]
    );
    assert_eq!(
        scan_line("  (INTERNAL-MONOREPO-FEASIBILITY.md arc), codex adversarial review arc,"),
        vec!["monorepo-repo-caps"]
    );
    assert_eq!(
        scan_line("// internal-llm-proxy (cycle-61 probe):"),
        vec!["proxy-hostname"]
    );
    assert_eq!(
        scan_line("-VA-org migration is the operator's IP call, flagged), and F13"),
        vec!["fleet-abbrev"]
    );
}

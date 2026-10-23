//! T100 — release-scripts guard: tag/version divergence + mechanical notes.
//!
//! Two of the release doctrine's load-bearing pieces are scripts, and a
//! script is only as good as its fixtures:
//!
//! - `scripts/check-tag-version.sh` (req 3): a release tag whose version
//!   disagrees with Cargo.toml ships a binary that lies about itself, and a
//!   published tag is immutable history — the check must catch the
//!   divergence (and bad tag shapes, and a missing version) while both
//!   sides are still free to fix.
//! - `scripts/release-notes.sh` (req 2): the GitHub Release body and the
//!   annotated tag message are BOTH generated mechanically from the
//!   semantic commit subjects between the previous tag and the release —
//!   a deterministic script (T46 digest pattern), not a model: the group
//!   classifier, the previous-tag default, the first-release fallback and
//!   byte-identical reruns are each pinned here.
//!
//! Fixtures are throwaway `git init` repos under a tempdir (T48 doctrine:
//! the scripts under test are resolved from the RUNTIME checkout via
//! `current_dir()` — the T47 shared cache hands cached test binaries to
//! other checkouts, so a compile-time manifest-dir path is forbidden).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn script(name: &str) -> PathBuf {
    let p = repo_root().join("scripts").join(name);
    assert!(
        p.is_file(),
        "{} must exist at the repo root's scripts/ — the release scripts were dropped",
        p.display()
    );
    p
}

/// Run one of the release scripts under POSIX sh, in `cwd`, with `args`.
fn run_sh(script: &Path, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new("sh");
    cmd.arg(script).args(args).current_dir(cwd);
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("spawning sh {}: {e}", script.display()))
}

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "chug-t100-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("creating fixture dir");
        let f = Fixture { dir };
        f.git(&["init", "--quiet"]);
        f.git(&["config", "user.name", "t100-fixture"]);
        f.git(&["config", "user.email", "t100@fixture.invalid"]);
        f.git(&["config", "commit.gpgsign", "false"]);
        f.git(&["config", "tag.gpgSign", "false"]);
        f
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("creating fixture subdir");
        }
        std::fs::write(path, contents).expect("writing fixture file");
    }

    fn commit(&self, msg: &str) -> String {
        // rev-list fails on the unborn HEAD (first commit); count from 0 there.
        let n = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(&self.dir)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().parse::<u32>().unwrap_or(0))
            .unwrap_or(0);
        self.write(&format!("f{n}.txt"), msg);
        self.git(&["add", "."]);
        self.git(&["commit", "--quiet", "-m", msg]);
        self.git(&["rev-parse", "--short", "HEAD"])
    }

    /// An empty commit: the cheap way to build a long history (the T303
    /// over-cap fixture needs ~600 commits to blow past the 120,000-byte
    /// body budget — writing+adding a file per commit would double the
    /// subprocess cost for nothing).
    fn commit_empty(&self, msg: &str) {
        self.git(&["commit", "--quiet", "--allow-empty", "-m", msg]);
    }

    fn tag(&self, name: &str, msg: &str) {
        self.git(&["tag", "-a", name, "-m", msg]);
    }

    fn cargo_toml(&self, version: &str) {
        self.write(
            "Cargo.toml",
            &format!(
                "[package]\nname = \"chug\"\nversion = \"{version}\"\n"
            ),
        );
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// --- check-tag-version.sh ------------------------------------------------------

/// The happy pairing: tag v0.2.1 against Cargo.toml 0.2.1 passes with an
/// explicit "matches" line (CI logs should say WHY they passed).
#[test]
fn tag_matching_cargo_version_passes() {
    let f = Fixture::new("ctv-ok");
    f.cargo_toml("0.2.1");
    f.commit("chore: bootstrap");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v0.2.1"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert!(
        stdout(&out).contains("matches"),
        "the ok line must say it matches: {}", stdout(&out)
    );
}

/// The divergence leg (req 3): tag 0.3.0 vs manifest 0.2.1 fails, names BOTH
/// versions, and names the fix including the immutable-tag rule.
#[test]
fn divergent_tag_fails_naming_both_versions_and_the_fix() {
    let f = Fixture::new("ctv-diverge");
    f.cargo_toml("0.2.1");
    f.commit("chore: bootstrap");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v0.3.0"], &[]);
    assert_eq!(
        out.status.code(),
        Some(1),
        "divergence must exit 1 (usage/env errors are 2): stderr {}",
        stderr(&out)
    );
    let err = stderr(&out);
    assert!(err.contains("v0.3.0") && err.contains("0.2.1"), "must name both sides: {err}");
    assert!(
        err.contains("never moved") || err.contains("NEW tag"),
        "the fix message must carry the immutable-tag doctrine: {err}"
    );
}

/// Bad shapes fail before any comparison: no `v`, missing the patch segment,
/// or a stray suffix — each names the required v<major>.<minor>.<patch> shape.
#[test]
fn malformed_tag_shapes_fail() {
    let f = Fixture::new("ctv-shape");
    f.cargo_toml("0.2.1");
    f.commit("chore: bootstrap");
    for bad in ["0.2.1", "v0.2", "v0.2.1.1", "vX.Y.Z", "release-0.2.1"] {
        let out = run_sh(&script("check-tag-version.sh"), f.path(), &[bad], &[]);
        assert_ne!(
            out.status.code(),
            Some(0),
            "tag {bad:?} must not pass the shape check"
        );
        let err = stderr(&out);
        assert!(
            err.contains("v<major>.<minor>.<patch>"),
            "tag {bad:?} must name the required shape: {err}"
        );
    }
}

/// With no argv and no GITHUB_REF the script falls back to the tag actually
/// ON HEAD (`git describe --exact-match`): works on a tagged HEAD, and exits
/// 2 (environment, not divergence) when HEAD carries no tag.
#[test]
fn github_ref_and_describe_fallbacks() {
    let f = Fixture::new("ctv-ref");
    f.cargo_toml("1.2.3");
    f.commit("chore: bootstrap");
    let out = run_sh(
        &script("check-tag-version.sh"),
        f.path(),
        &[],
        &[("GITHUB_REF", "refs/tags/v1.2.3")],
    );
    assert_eq!(out.status.code(), Some(0), "GITHUB_REF is honored: {}", stderr(&out));
    // A non-tag GITHUB_REF must NOT be mistaken for a tag.
    let out = run_sh(
        &script("check-tag-version.sh"),
        f.path(),
        &[],
        &[("GITHUB_REF", "refs/heads/main")],
    );
    assert_ne!(out.status.code(), Some(0), "a branch ref is not a tag");

    // describe fallback: tag HEAD then check with no args at all.
    f.tag("v1.2.3", "release notes");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &[], &[]);
    assert_eq!(out.status.code(), Some(0), "describe fallback: {}", stderr(&out));

    // Untagged HEAD (and no GITHUB_REF) → exit 2 with the usage hint.
    f.commit("chore: second");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &[], &[]);
    assert_eq!(out.status.code(), Some(2), "no resolvable tag is a usage error, not a divergence");
    assert!(
        stderr(&out).contains("no tag to check"),
        "the usage error must say what to pass: {}",
        stderr(&out)
    );
}

/// The version is read from the [package] section ONLY: a version key in a
/// later section ([dev-dependencies]-style) must not satisfy the check.
#[test]
fn version_is_read_from_the_package_section_only() {
    let f = Fixture::new("ctv-section");
    f.write(
        "Cargo.toml",
        "[package]\nname = \"chug\"\nversion = \"0.2.1\"\n\n[dev-dependencies]\ntempfile = \"3\"\nversion = \"9.9.9\"\n",
    );
    f.commit("chore: bootstrap");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v0.2.1"], &[]);
    assert_eq!(out.status.code(), Some(0), "the [package] version wins: {}", stderr(&out));
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v9.9.9"], &[]);
    assert_ne!(out.status.code(), Some(0), "a stray version key in another section must not satisfy v9.9.9");
}

/// Environment errors are exit 2 and self-describing: missing Cargo.toml, and
/// a manifest with no [package] version.
#[test]
fn environment_failures_exit_two_and_name_the_problem() {
    let f = Fixture::new("ctv-env");
    f.commit("chore: no manifest");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v0.1.0"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("Cargo.toml not found"),
        "must name the missing manifest: {}",
        stderr(&out)
    );

    let f = Fixture::new("ctv-noversion");
    f.write("Cargo.toml", "[package]\nname = \"chug\"\n");
    f.commit("chore: no version");
    let out = run_sh(&script("check-tag-version.sh"), f.path(), &["v0.1.0"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stderr(&out).contains("no [package] version"),
        "must name the missing version: {}",
        stderr(&out)
    );
}

// --- release-notes.sh ----------------------------------------------------------

/// Shared fixture: bootstrap commit → v0.1.0 → one commit per semantic
/// family → v0.2.0. Returns (fixture, short-sha map by commit subject stem).
fn notes_fixture(name: &str) -> Fixture {
    let f = Fixture::new(name);
    f.cargo_toml("0.1.0");
    f.commit("chore: bootstrap");
    f.tag("v0.1.0", "first");
    f.cargo_toml("0.2.0");
    f.commit("t42: add the thing"); // features
    f.commit("feat: sparkle"); // conventional feature
    f.commit("fix: crash on empty input"); // fixes
    f.commit("docs: readme install section"); // docs
    f.commit("spec: wrap doctrine for t42"); // docs
    f.commit("todo: t42 done"); // chore (bookkeeping)
    f.tag("v0.2.0", "second");
    f
}

/// The generated notes group by family, cite the short sha, exclude the
/// previous tag's commits, and order the headings Features/Fixes/Docs/Chore.
#[test]
fn notes_group_families_order_and_range() {
    let f = notes_fixture("notes-groups");
    let out = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let notes = stdout(&out);

    assert!(notes.contains("## Features"), "Features heading: {notes}");
    assert!(notes.contains("t42: add the thing"), "feature impl commits are features: {notes}");
    assert!(notes.contains("feat: sparkle"), "conventional feat commits are features: {notes}");
    assert!(notes.contains("## Fixes"), "Fixes heading: {notes}");
    assert!(notes.contains("fix: crash on empty input"), "fix commits are fixes: {notes}");
    assert!(notes.contains("## Docs"), "Docs heading: {notes}");
    assert!(notes.contains("docs: readme install section"), "docs: is docs: {notes}");
    assert!(notes.contains("spec: wrap doctrine for t42"), "spec: is docs (doctrine): {notes}");
    assert!(notes.contains("## Chore"), "Chore heading: {notes}");
    assert!(notes.contains("todo: t42 done"), "bookkeeping is chore: {notes}");

    // The range is right: the bootstrap commit (pre-v0.1.0) never appears.
    assert!(
        !notes.contains("chore: bootstrap"),
        "notes cover SINCE the previous tag — the bootstrap commit predates it: {notes}"
    );

    // Fixed heading order + each commit carries its short sha in backticks.
    let idx = |h: &str| notes.find(h).expect(h);
    assert!(idx("## Features") < idx("## Fixes"));
    assert!(idx("## Fixes") < idx("## Docs"));
    assert!(idx("## Docs") < idx("## Chore"));
    for line in notes.lines().filter(|l| l.starts_with("- ")) {
        assert!(
            line.contains("(`") && line.ends_with("`)"),
            "every bullet cites the commit's short sha: {line}"
        );
    }
}

/// Uppercase `T<N>:` prefixes classify the same as lowercase `t<N>:` — the
/// repo's history carries both spellings.
#[test]
fn uppercase_item_prefixes_classify_as_features() {
    let f = Fixture::new("notes-upper");
    f.cargo_toml("0.1.0");
    f.commit("chore: bootstrap");
    f.commit("T42: the upper-case variant");
    let out = run_sh(&script("release-notes.sh"), f.path(), &["HEAD"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let notes = stdout(&out);
    let feats = notes
        .split("## Fixes")
        .next()
        .expect("Features section precedes Fixes");
    assert!(
        feats.contains("T42: the upper-case variant"),
        "T42: must land in Features: {notes}"
    );
}

/// The default FROM is the previous tag: with TO=HEAD on the tagged release
/// commit, `release-notes.sh HEAD` equals `release-notes.sh HEAD v0.1.0` —
/// the loop's wrap call needs no second argument.
#[test]
fn default_from_is_the_previous_tag() {
    let f = notes_fixture("notes-default-from");
    let default_out = run_sh(&script("release-notes.sh"), f.path(), &["HEAD"], &[]);
    let explicit_out = run_sh(
        &script("release-notes.sh"),
        f.path(),
        &["HEAD", "v0.1.0"],
        &[],
    );
    assert_eq!(default_out.status.code(), Some(0));
    assert_eq!(explicit_out.status.code(), Some(0));
    assert_eq!(
        stdout(&default_out),
        stdout(&explicit_out),
        "the default FROM (previous tag via describe) must equal the explicit \
         one — the wrap call and the workflow call must agree byte for byte"
    );
}

/// First release (no tags at all): the fallback range covers the whole
/// history, including the root commit.
#[test]
fn first_release_covers_whole_history_when_no_tag_exists() {
    let f = Fixture::new("notes-first");
    f.cargo_toml("0.1.0");
    f.commit("chore: bootstrap");
    f.commit("t1: the only feature");
    let out = run_sh(&script("release-notes.sh"), f.path(), &["HEAD"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let notes = stdout(&out);
    assert!(notes.contains("chore: bootstrap"), "no previous tag → the whole history is in scope: {notes}");
    assert!(notes.contains("t1: the only feature"), "{notes}");
}

/// Determinism (T46 pattern): identical inputs → byte-identical output, and
/// an empty range prints nothing (exit 0) — there is nothing to lie about.
#[test]
fn reruns_are_byte_identical_and_empty_range_is_silent() {
    let f = notes_fixture("notes-determinism");
    let a = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    let b = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(stdout(&a), stdout(&b), "notes must be deterministic — the tag message and the release body are generated in different processes");
    assert!(!stdout(&a).is_empty(), "non-vacuity: the fixture range must produce notes");

    let empty = run_sh(&script("release-notes.sh"), f.path(), &["HEAD", "HEAD"], &[]);
    assert_eq!(empty.status.code(), Some(0), "empty range is not an error");
    assert!(
        stdout(&empty).trim().is_empty(),
        "empty range prints nothing: {:?}",
        stdout(&empty)
    );
}

/// A non-git cwd and an unresolvable TO_REF are environment errors (exit 2),
/// never silent empty notes.
#[test]
fn environment_failures_are_loud() {
    let dir = std::env::temp_dir().join(format!("chug-t100-nogit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = run_sh(&script("release-notes.sh"), &dir, &["HEAD"], &[]);
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out.status.code(), Some(2), "a non-git cwd must fail loudly: {}", stderr(&out));

    let f = Fixture::new("notes-bad-to");
    f.commit("chore: only");
    let out = run_sh(&script("release-notes.sh"), f.path(), &["refs/heads/nope"], &[]);
    assert_eq!(out.status.code(), Some(2), "an unresolvable TO_REF must fail loudly: {}", stderr(&out));
}

// --- T303: the GitHub 125,000-char release-body cap -----------------------------
//
// Four consecutive tags (v0.17.19–v0.17.22) failed `gh release create` with
// "body is too long (maximum is 125000 characters)": the loop's eval:/wrap
// subjects run multi-KB each, and one 1,122-commit window generated a
// 1,070,737-byte body. The two deterministic guards pinned below: subjects
// truncate at 200 bytes with a "..." marker, and an over-120,000-byte body
// is cut at a line boundary under one footer naming the omission.

/// Leg 1: a multi-KB subject is truncated to the byte budget carrying the
/// "..." marker, byte-exact; an ordinary subject passes through untouched;
/// the truncation is deterministic.
#[test]
fn overlong_subjects_are_truncated_with_an_ellipsis_marker() {
    let f = Fixture::new("notes-longsubj");
    f.cargo_toml("0.1.0");
    f.commit("chore: bootstrap");
    f.tag("v0.1.0", "first");
    let long_subject = format!("eval: {}", "x".repeat(3000));
    let long_sha = f.commit(&long_subject);
    f.commit("fix: small one");
    f.tag("v0.2.0", "second");

    let out = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let notes = stdout(&out);

    // Byte-exact: the first 200 bytes of the subject, then the marker.
    let expected = format!("- {}... (`{}`)", &long_subject[..200], long_sha);
    assert!(notes.contains(&expected), "the truncated bullet line: {notes}");
    assert!(
        !notes.contains(&"x".repeat(201)),
        "no run longer than the budget survives the truncation"
    );
    let line = notes
        .lines()
        .find(|l| l.starts_with("- eval:"))
        .expect("the eval bullet is emitted");
    assert_eq!(
        line.len(),
        2 + 200 + 3 + 3 + long_sha.len() + 2,
        "- + 200-byte subject + ... + sha cite: {line:?}"
    );
    assert!(
        notes.contains("- fix: small one (`"),
        "ordinary subjects are never touched: {notes}"
    );

    let again = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(stdout(&again), notes, "truncation must stay deterministic");
}

/// Leg 2: a range whose body would exceed the hard cap is cut at a LINE
/// boundary — never a partial line — under exactly one footer whose N is the
/// omitted-bullet count and whose range names the git data it came from.
/// (Subjects here sit exactly at the 200-byte per-subject budget, so the
/// body cap is the only cutter in play.)
#[test]
fn over_cap_bodies_are_cut_at_a_line_boundary_with_one_footer() {
    let f = Fixture::new("notes-overcap");
    f.commit("chore: bootstrap");
    f.tag("v0.1.0", "first");
    let subject = format!("chore: {}", "y".repeat(193)); // 200 bytes exactly
    const TOTAL: usize = 600; // 600 × ~214-byte bullets ≈ 128KB > the cap
    for _ in 0..TOTAL {
        f.commit_empty(&subject);
    }
    f.tag("v0.2.0", "second");

    let out = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let notes = stdout(&out);

    assert!(
        notes.len() <= 120_000,
        "the body must fit the script's hard cap (GitHub's limit is 125,000): {}",
        notes.len()
    );
    let footers: Vec<&str> = notes.lines().filter(|l| l.contains("(truncated: ")).collect();
    assert_eq!(footers.len(), 1, "exactly one truncation footer: {footers:?}");
    let footer = footers[0];

    let n: usize = footer
        .strip_prefix("- ... (truncated: ")
        .and_then(|rest| rest.split(' ').next())
        .and_then(|digits| digits.parse().ok())
        .expect("the footer carries its omitted-commit count");
    let kept = notes
        .lines()
        .filter(|l| l.starts_with("- ") && !l.contains("(truncated: "))
        .count();
    assert_eq!(kept + n, TOTAL, "N is the omitted-bullet count: {kept} kept + {n} omitted");
    assert_eq!(
        footer,
        format!(
            "- ... (truncated: {n} more commits in v0.1.0..v0.2.0; full list: git log v0.1.0..v0.2.0)"
        ),
        "the footer names the range and the full-list command"
    );

    // The cut lands between lines: the bullet before the footer is complete.
    let lines: Vec<&str> = notes.lines().collect();
    let before = lines[lines.len() - 2];
    assert!(
        before.starts_with("- ") && before.ends_with("`)"),
        "no partial line at the cut: {before:?}"
    );
    assert!(notes.ends_with(")\n"), "the footer closes the body");

    let again = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(stdout(&again), notes, "the cap must stay deterministic — same input, same N");
}

/// The other side of the T303 pin: a body that fits the cap keeps the
/// pre-T303 bytes exactly — no footer appears, no subject is touched.
#[test]
fn under_cap_output_keeps_the_pre_cap_bytes() {
    let f = Fixture::new("notes-undercap");
    f.cargo_toml("0.1.0");
    f.commit("chore: bootstrap");
    f.tag("v0.1.0", "first");
    f.cargo_toml("0.2.0");
    let s_feat = f.commit("t42: add the thing");
    let s_sparkle = f.commit("feat: sparkle");
    let s_fix = f.commit("fix: crash on empty input");
    let s_docs = f.commit("docs: readme install section");
    let s_spec = f.commit("spec: wrap doctrine for t42");
    let s_todo = f.commit("todo: t42 done");
    f.tag("v0.2.0", "second");

    let out = run_sh(&script("release-notes.sh"), f.path(), &["v0.2.0"], &[]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    // Golden pre-T303 bytes (verified against the T100 output): groups in
    // fixed order, newest-first inside each group, sha-cited bullets.
    let expected = format!(
        "## Features\n\n\
         - feat: sparkle (`{s_sparkle}`)\n\
         - t42: add the thing (`{s_feat}`)\n\
         ## Fixes\n\n\
         - fix: crash on empty input (`{s_fix}`)\n\
         ## Docs\n\n\
         - spec: wrap doctrine for t42 (`{s_spec}`)\n\
         - docs: readme install section (`{s_docs}`)\n\
         ## Chore\n\n\
         - todo: t42 done (`{s_todo}`)\n"
    );
    assert_eq!(stdout(&out), expected, "under-cap output is untouched");
}

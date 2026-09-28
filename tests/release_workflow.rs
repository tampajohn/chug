//! T100 — release workflow guard: YAML parse + SHA-pin grep.
//!
//! `.github/workflows/release.yml` is repo policy executable by a machine we
//! do not control at push time: a malformed YAML (the run dies before any
//! step), an unpinned action (`uses: actions/checkout@v4` floats its commit —
//! the supply-chain hole the spec names), a dropped platform leg, a missing
//! `--locked`, or a release step that could implicitly CREATE a tag would all
//! surface only when a real tag is pushed. This guard pins the workflow's
//! shape from the checkout the test RUNS against (T48 doctrine: runtime
//! `current_dir()`, never the compile-time manifest-dir macro — the T47
//! shared cache hands cached test binaries to other checkouts).
//!
//! actionlint is not on this host (and the loop must not require network
//! tooling), so the spec's fallback is implemented directly: a real YAML
//! parse (serde_yaml, dev-only) plus the SHA-pin grep — every `uses:` ref is
//! a full 40-hex commit SHA. The five known pins are themselves pinned
//! (checkout twice, toolchain, upload-artifact, download-artifact), so a
//! re-pin to a moving ref or a new unpinned action goes red here.

use serde_yaml::Value;
use std::path::PathBuf;

fn workflow_path() -> PathBuf {
    std::env::current_dir()
        .expect("cargo sets the test cwd to the package root")
        .join(".github/workflows/release.yml")
}

fn workflow_text() -> String {
    let path = workflow_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {} (runtime checkout): {e}", path.display()))
}

fn parse(text: &str) -> Value {
    serde_yaml::from_str(text)
        .unwrap_or_else(|e| panic!(".github/workflows/release.yml must be valid YAML: {e}"))
}

/// The `on:` trigger key — YAML 1.1 parsers may resolve the plain scalar
/// `on` to boolean `true`; accept either spelling so the pin guards the
/// trigger, not the parser's scalar-version opinion.
fn trigger(doc: &Value) -> &Value {
    let map = doc
        .as_mapping()
        .expect("workflow top level must be a mapping");
    map.get(Value::String("on".into()))
        .or_else(|| map.get(Value::Bool(true)))
        .unwrap_or_else(|| panic!("workflow must carry an `on:` trigger mapping"))
}

fn job<'a>(doc: &'a Value, name: &str) -> &'a Value {
    doc.as_mapping()
        .expect("top-level mapping")
        .get(Value::String("jobs".into()))
        .expect("workflow must define `jobs:`")
        .as_mapping()
        .expect("`jobs:` must be a mapping")
        .get(Value::String(name.into()))
        .unwrap_or_else(|| panic!("workflow must define job `{name}`"))
}

fn steps(job: &Value) -> &Vec<Value> {
    job.as_mapping()
        .expect("job must be a mapping")
        .get(Value::String("steps".into()))
        .expect("job must define `steps:`")
        .as_sequence()
        .expect("`steps:` must be a sequence")
}

fn step_run(step: &Value) -> String {
    step.as_mapping()
        .and_then(|m| m.get(Value::String("run".into())))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn step_uses(step: &Value) -> String {
    step.as_mapping()
        .and_then(|m| m.get(Value::String("uses".into())))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn is_40hex(s: &str) -> bool {
    s.len() == 40 && s.chars().all(|c| c.is_ascii_hexdigit())
}

/// (a) The file exists and parses as a YAML mapping with the trigger and two
/// jobs — the parse leg of the spec's "YAML parse + SHA-pin grep".
#[test]
fn workflow_parses_with_trigger_and_jobs() {
    let text = workflow_text();
    assert!(
        !text.trim().is_empty(),
        "release.yml must not be empty — the workflow was dropped"
    );
    let doc = parse(&text);
    let push = trigger(&doc)
        .as_mapping()
        .expect("`on:` must be a mapping")
        .get(Value::String("push".into()))
        .expect("`on:` must gate on `push`")
        .as_mapping()
        .expect("`on.push` must be a mapping")
        .get(Value::String("tags".into()))
        .and_then(Value::as_sequence)
        .expect("`on.push.tags` must be a sequence")
        .clone();
    let tags: Vec<String> = push
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert!(
        tags.iter().any(|t| t == "v*"),
        "the workflow must trigger on `v*` tag pushes (got {tags:?}) — the \
         spec's trigger is the tag pattern, not a branch or manual dispatch"
    );
    // Both jobs exist; the release job needs the build job.
    let _ = job(&doc, "build");
    let release = job(&doc, "release");
    let needs = release
        .as_mapping()
        .and_then(|m| m.get(Value::String("needs".into())))
        .and_then(Value::as_str)
        .expect("the release job must `need` the build job");
    assert_eq!(needs, "build", "release must run after build");
}

/// (b) The SHA-pin grep: every non-comment `uses:` line carries a full 40-hex
/// commit SHA, and the five known pins are each present the expected number
/// of times (checkout x2 — build + release jobs — toolchain, upload,
/// download). A re-pin to a tag/branch ref, a new unpinned action, or a
/// dropped action each go red here.
#[test]
fn every_uses_is_pinned_by_full_40_hex_sha() {
    let text = workflow_text();
    let mut uses_lines = 0usize;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue; // comments may name `uses:` in prose; they execute nothing
        }
        if !line.contains("uses:") {
            continue;
        }
        uses_lines += 1;
        let mut value = line.split("uses:").nth(1).unwrap_or("").trim().to_string();
        // A trailing `# vX.Y.Z` version comment is allowed (and encouraged) —
        // it documents WHICH release the pinned SHA corresponds to.
        if let Some(idx) = value.find(" #") {
            value.truncate(idx);
            value = value.trim().to_string();
        }
        let (action, r#ref) = value
            .rsplit_once('@')
            .unwrap_or_else(|| panic!("`uses: {value}` has no @ref"));
        assert!(
            !action.is_empty() && is_40hex(r#ref),
            "`uses: {value}` is not pinned by a full 40-hex commit SHA — \
             supply-chain hygiene pins every action by SHA, never a \
             moving tag/branch ref"
        );
    }
    assert!(
        uses_lines >= 5,
        "expected the 5 known `uses:` pins (checkout x2, rust-toolchain, \
         upload-artifact, download-artifact), found {uses_lines} — a drop is \
         a workflow regression, an addition must be SHA-pinned"
    );
    // The parsed doc must agree with the raw-text scan: no `uses` value in
    // the tree escapes the 40-hex rule.
    let doc = parse(&text);
    let mut seen = 0usize;
    for name in ["build", "release"] {
        for step in steps(job(&doc, name)) {
            let uses = step_uses(step);
            if uses.is_empty() {
                continue;
            }
            seen += 1;
            let (action, r#ref) = uses
                .rsplit_once('@')
                .unwrap_or_else(|| panic!("step `uses: {uses}` has no @ref"));
            assert!(
                is_40hex(r#ref),
                "parsed step `uses: {action}@{ref}` is not 40-hex pinned"
            );
        }
    }
    assert_eq!(
        seen, uses_lines,
        "the parsed `uses:` count must equal the raw-text count — a uses: \
         the parser cannot see (bad indentation) would silently escape the pin"
    );
    // The five known pins, each with its expected multiplicity.
    for (sha, expected, what) in [
        ("11bd71901bbe5b1630ceea73d27597364c9af683", 2usize, "actions/checkout v4.2.2 (both jobs)"),
        ("02cb101ec7c40f2c49e1d9714d64511d8e1b74de", 1, "dtolnay/rust-toolchain"),
        ("ea165f8d65b6e75b540449e92b4886f43607fa02", 1, "actions/upload-artifact v4.6.2"),
        ("d3f86a106a0bac45b974a628896c90dbdf5c8093", 1, "actions/download-artifact v4.3.0"),
    ] {
        assert_eq!(
            text.matches(sha).count(),
            expected,
            "the {what} pin must appear exactly {expected} time(s) — a \
             changed SHA must be a conscious re-pin with its version \
             comment updated"
        );
    }
}

/// (c) The matrix builds exactly the three fleet platforms — macos-14 arm64
/// (primary), ubuntu-22.04 x86_64, and aarch64 (cross-compiled on
/// ubuntu-22.04) — each running `cargo build --release --locked --target`.
#[test]
fn matrix_covers_the_three_fleet_platforms() {
    let doc = parse(&workflow_text());
    let matrix = job(&doc, "build")
        .as_mapping()
        .and_then(|m| m.get(Value::String("strategy".into())))
        .expect("build must define strategy")
        .as_mapping()
        .and_then(|m| m.get(Value::String("matrix".into())))
        .expect("strategy must define a matrix")
        .as_mapping()
        .and_then(|m| m.get(Value::String("include".into())))
        .and_then(Value::as_sequence)
        .expect("matrix.include must be a sequence")
        .clone();
    let mut found = Vec::new();
    for leg in &matrix {
        let map = leg.as_mapping().expect("matrix leg must be a mapping");
        let get = |k: &str| {
            map.get(Value::String(k.into()))
                .and_then(Value::as_str)
                .expect("matrix leg must carry runner/target/platform strings")
        };
        found.push((get("runner").to_string(), get("target").to_string(), get("platform").to_string()));
        // The build step targets THIS leg.
        let build = steps(job(&doc, "build"))
            .iter()
            .map(step_run)
            .find(|r| r.contains("cargo build"))
            .expect("build job must carry the cargo build step");
        assert!(
            build.contains("--release") && build.contains("--locked"),
            "release binaries build `--release --locked` — a drifting \
             lockfile must fail the release, not silently resolve"
        );
    }
    for (runner, target, platform) in [
        ("macos-14", "aarch64-apple-darwin", "macos-arm64"),
        ("ubuntu-22.04", "x86_64-unknown-linux-gnu", "linux-x86_64"),
        ("ubuntu-22.04", "aarch64-unknown-linux-gnu", "linux-aarch64"),
    ] {
        assert!(
            found.iter().any(|(r, t, p)| r == runner && t == target && p == platform),
            "matrix must cover {runner}/{target} (artifact platform \
             {platform}) — install.sh and the README download URLs name \
             exactly these three platform tokens"
        );
    }
    assert_eq!(
        found.len(),
        3,
        "exactly three matrix legs (no stray fourth) — got {found:?}"
    );
}

/// (d) Version discipline and notes generation are wired in, in order: the
/// tag/version divergence check runs BEFORE the build (fail fast, before any
/// minutes are spent), and the release job generates notes with
/// scripts/release-notes.sh and creates the release with `gh release create
/// --verify-tag` (gh can never implicitly create a tag) attaching the
/// tar.gz + sha256 archives.
#[test]
fn version_check_precedes_build_and_release_uses_notes() {
    let doc = parse(&workflow_text());
    let build_steps: Vec<String> = steps(job(&doc, "build"))
        .iter()
        .map(step_run)
        .collect();
    let check_idx = build_steps
        .iter()
        .position(|r| r.contains("check-tag-version.sh"))
        .expect("the build job must run scripts/check-tag-version.sh (req 3's divergence check)");
    let build_idx = build_steps
        .iter()
        .position(|r| r.contains("cargo build --release --locked"))
        .expect("the build job must run cargo build --release --locked");
    assert!(
        check_idx < build_idx,
        "the version check must run BEFORE the build (step {check_idx} vs \
         {build_idx}) — a divergent tag must fail in seconds, not after \
         compiling three toolchains"
    );
    // The check consumes the pushed tag (GITHUB_REF), not a hardcoded one.
    let check_cmd = &build_steps[check_idx];
    assert!(
        check_cmd.contains("GITHUB_REF"),
        "the version check must consume ${{GITHUB_REF}} (the pushed tag) — \
         a hardcoded tag would check the wrong thing"
    );

    let release_steps: Vec<String> = steps(job(&doc, "release"))
        .iter()
        .map(step_run)
        .collect();
    let notes = release_steps
        .iter()
        .find(|r| r.contains("scripts/release-notes.sh"))
        .expect("the release job must generate notes with scripts/release-notes.sh (req 2)");
    assert!(
        notes.contains("release-notes.md"),
        "the generated notes must be written to a file the release step \
         consumes (--notes-file), not pasted inline"
    );
    let create = release_steps
        .iter()
        .find(|r| r.contains("gh release create"))
        .expect("the release must be created with `gh release create` (preinstalled CLI — no third-party release action)");
    for needle in ["--verify-tag", "--notes-file release-notes.md"] {
        assert!(
            create.contains(needle),
            "`gh release create` must carry {needle:?} — --verify-tag makes \
             gh fail rather than implicitly create a tag; --notes-file \
             attaches the generated notes (req 2)"
        );
    }
    assert!(
        create.contains("dist/chug-*.tar.gz") && create.contains("*.tar.gz.sha256"),
        "the release must attach the tar.gz archives AND their sha256 \
         sidecars (req 1)"
    );
}

/// (e) The workflow's permissions are least-privilege (contents: write —
/// the release creation needs it) and the checkouts fetch full history
/// (release-notes.sh needs the previous tag, which a depth-1 checkout
/// cannot see).
#[test]
fn permissions_and_full_history_checkout() {
    let text = workflow_text();
    let doc = parse(&text);
    let perms = doc
        .as_mapping()
        .expect("top-level mapping")
        .get(Value::String("permissions".into()))
        .expect("the workflow must declare its permissions explicitly")
        .as_mapping()
        .expect("`permissions:` must be a mapping")
        .clone();
    assert_eq!(
        perms.get(Value::String("contents".into())).and_then(Value::as_str),
        Some("write"),
        "contents: write is required to create the release"
    );
    assert_eq!(
        perms.len(),
        1,
        "least privilege: the only permission is contents: write"
    );
    let checkouts: Vec<Value> = ["build", "release"]
        .iter()
        .flat_map(|j| steps(job(&doc, j)).to_vec())
        .filter(|s| step_uses(s).starts_with("actions/checkout"))
        .collect();
    assert_eq!(
        checkouts.len(),
        2,
        "both jobs check out (build + release-notes need the repo)"
    );
    for step in &checkouts {
        let depth = step
            .as_mapping()
            .and_then(|m| m.get(Value::String("with".into())))
            .and_then(|w| w.as_mapping().unwrap().get(Value::String("fetch-depth".into())).cloned())
            .and_then(|d| d.as_i64());
        assert_eq!(
            depth,
            Some(0),
            "every checkout uses fetch-depth: 0 — release-notes.sh walks \
             back to the previous tag, which a depth-1 checkout cannot see"
        );
    }
}

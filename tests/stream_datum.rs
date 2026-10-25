//! T305 — the wrap corridor's stream-datum helper: mechanical at-stamp
//! generation.
//!
//! The corridor's stream-datum figures (the eval body's own-stream datum at
//! eval-write, the wrap-notes' T257 color, the state write's datum, the
//! fill's FINAL datum) were hand-composed per wrap, and the composed
//! figures kept being carried from probes run at EARLIER stamps. The
//! cycle-1986 eval (trip 460) found THREE committed corridor texts
//! falsified at their own named stamps in one window — the 1910 notes'
//! `32 iters` (TRUE 55), the 1910 fill's `55 iters` (TRUE 78), the 1982
//! fill's `ok:false 4 == 4` (TRUE 5) + `trim 0 == 0` (TRUE 1) — every fire
//! caught one-or-more chains later by the eval-digest backstop (zero
//! casualty), but each costing correction prose and moving the T272
//! false-at-write census 54 → 58. `scripts/stream_datum.py` is the
//! T273/T295-shaped fix: the mechanical read as ONE stdlib-only command
//! (`python3 scripts/stream_datum.py <events> [--at <ts>]`), emitting the
//! datum as `key: value` lines with every count scoped ts ≤ stamp
//! (string-compare, the corpus convention).
//!
//! These tests run the REAL script against inline jsonl fixture streams in
//! a tempdir (the tests guard the script, they do not reimplement it — the
//! T46 digest-test pattern; T48 doctrine: the script resolves from the
//! RUNTIME checkout via `current_dir()`, never the compile-time
//! manifest-dir path the T47 shared cache invalidates). Pinned here:
//!
//! 1. the empty stream (zero counts, `goal: none`) and malformed-line
//!    tolerance (the digest's fromjson? parity);
//! 2. the bisecting `--at` stamp — the 1982-fill shape is the pin: a trim
//!    BEFORE the stamp is counted, one AFTER is excluded; an ok:false leg
//!    after the stamp is excluded from BOTH the count and the leg list;
//!    every count scopes ≤ stamp (stamped ≤ whole, mechanically);
//! 3. ok:false leg enumeration order and the trim ts list (stream order);
//! 4. ctx_edit accepted + rejected counted separately (the `.accepted`
//!    bool);
//! 5. the goal `.outcome` read in all three shapes (accepted / rejected /
//!    absent → none) — and the field-name pin: a goal event carrying only
//!    `.verdict` is read on NEITHER side (the verdict field is `.outcome`,
//!    NOT `.verdict`);
//! 6. `bytes:` == the fixture's probed length and the UTC Z-form mtime;
//! 7. wall arithmetic derived from the helper's OWN probed anchors (T270:
//!    the expectation is computed from the same probe's actuals, never a
//!    remembered constant);
//! 8. the default stamp is the stream's last `type=="iteration"` anchor —
//!    a verdict event landing after it (the real-stream shape) is EXCLUDED
//!    by default and included under `--at <verdict ts>`;
//! 9. usage errors exit 2 (malformed `--at`, missing events file).

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    std::env::current_dir().expect("cargo sets the test cwd to the package root")
}

fn script_path() -> PathBuf {
    let p = repo_root().join("scripts/stream_datum.py");
    assert!(
        p.is_file(),
        "{} must exist at the repo root's scripts/ — the T305 helper was dropped",
        p.display()
    );
    p
}

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "chug-t305-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("creating fixture dir");
        Fixture { dir }
    }

    fn events_path(&self) -> PathBuf {
        self.dir.join("events.jsonl")
    }

    /// Write the fixture stream and return its path.
    fn write(&self, body: &str) -> PathBuf {
        let path = self.events_path();
        std::fs::write(&path, body).expect("writing fixture stream");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Run the helper; panic unless it exits 0. Returns stdout.
fn run_ok(events: &Path, args: &[&str]) -> String {
    let out = Command::new("python3")
        .arg(script_path())
        .arg(events)
        .args(args)
        .output()
        .expect("spawn python3 scripts/stream_datum.py");
    assert!(
        out.status.success(),
        "stream_datum exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Run the helper; return (exit code, stdout, stderr) raw.
fn run_raw(events: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut cmd = Command::new("python3");
    cmd.arg(script_path());
    if let Some(p) = events {
        cmd.arg(p);
    }
    cmd.args(args);
    let out = cmd
        .output()
        .expect("spawn python3 scripts/stream_datum.py");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

/// The datum's split-read: every `key: value` line into (key, value) —
/// the wrap_assert.py read_key_values shape the corridor consumes.
fn datum(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter(|l| l.contains(": "))
        .map(|l| {
            let (k, v) = l.split_once(": ").expect("a `key: value` line");
            (k.trim().to_string(), v.trim().to_string())
        })
        .collect()
}

/// One field's value by exact `key: ` prefix (the needle carries the ": "
/// so `ok-false` never matches the `ok-false-legs` line).
fn field(stdout: &str, key: &str) -> String {
    let needle = format!("{key}: ");
    stdout
        .lines()
        .find(|l| l.starts_with(&needle))
        .unwrap_or_else(|| panic!("no `{key}:` line in:\n{stdout}"))
        .trim_start_matches(&needle)
        .to_string()
}

fn num_field(stdout: &str, key: &str) -> u64 {
    field(stdout, key)
        .parse()
        .unwrap_or_else(|e| panic!("`{key}:` value is not an integer: {e}"))
}

/// An `accepted A / rejected R` field (ctx-edit, goal) split into (A, R).
fn acc_rej(stdout: &str, key: &str) -> (u64, u64) {
    let v = field(stdout, key);
    let (a, r) = v
        .strip_prefix("accepted ")
        .and_then(|s| s.split_once(" / rejected "))
        .unwrap_or_else(|| panic!("`{key}:` is not `accepted A / rejected R`: {v:?}"));
    (
        a.parse().expect("accepted count"),
        r.parse().expect("rejected count"),
    )
}

/// The `"ts":"..."` value carried by one fixture event line.
fn ts_of(line: &str) -> Option<String> {
    let marker = "\"ts\":\"";
    let i = line.find(marker)? + marker.len();
    let rest = &line[i..];
    let j = rest.find('"')?;
    Some(rest[..j].to_string())
}

/// The expected `legs:` value derived from the fixture text itself: the ts
/// values of the lines matching `pred`, in stream order (T270 — the
/// enumeration is derived from the same fixture the script reads, never a
/// remembered constant), `none` when empty.
fn legs(body: &str, pred: impl Fn(&str) -> bool) -> String {
    let ts_list: Vec<String> = body
        .lines()
        .filter(|l| pred(l))
        .filter_map(ts_of)
        .collect();
    if ts_list.is_empty() {
        "none".to_string()
    } else {
        ts_list.join(", ")
    }
}

/// Count `"type":"<kind>"` occurrences in the fixture text (T270
/// derivation of the whole-stream count).
fn count_type(body: &str, kind: &str) -> usize {
    body.matches(&format!("\"type\":\"{kind}\"")).count()
}

/// Epoch seconds for a fixed-width millisecond Z-form ts (the fixtures'
/// shape), via the civil-days algorithm — the test's own wall arithmetic.
fn epoch_secs(ts: &str) -> f64 {
    assert_eq!(ts.len(), 24, "fixture anchors are millisecond Z-form: {ts}");
    let num = |a: usize, b: usize| -> i64 { ts[a..b].parse().expect("digits") };
    let (y, mo, d) = (num(0, 4), num(5, 7), num(8, 10));
    let (h, mi, s) = (num(11, 13), num(14, 16), num(17, 19));
    let ms = num(20, 23);
    // Days from civil date (Howard Hinnant's days_from_civil).
    let yy = if mo <= 2 { y - 1 } else { y };
    let era = if yy >= 0 { yy } else { yy - 399 } / 400;
    let yoe = yy - era * 400;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    (days * 86400 + h * 3600 + mi * 60 + s) as f64 + ms as f64 / 1000.0
}

// The shared bisect fixture — the 1982-fill shape: events on BOTH sides of
// the stamp for every countable class, plus one event EXACTLY at the stamp
// (the `<=` boundary; string-compare scoping, the corpus convention).
const STAMP: &str = "2026-10-24T04:03:00.000Z";

fn bisect_body() -> String {
    [
        r#"{"type":"run_start","ts":"2026-10-24T04:00:00.000Z","model":"m"}"#,
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":true,"ts":"2026-10-24T04:01:05.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:01:10.000Z"}"#,
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:02:00.000Z"}"#,
        r#"{"type":"ctx_edit","accepted":true,"ts":"2026-10-24T04:02:10.000Z"}"#,
        r#"{"type":"trim","ts":"2026-10-24T04:02:20.000Z"}"#,
        r#"{"type":"iteration","n":3,"ts":"2026-10-24T04:03:00.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:03:30.000Z"}"#,
        r#"{"type":"ctx_edit","accepted":false,"ts":"2026-10-24T04:04:00.000Z"}"#,
        r#"{"type":"iteration","n":4,"ts":"2026-10-24T04:05:00.000Z"}"#,
        r#"{"type":"trim","ts":"2026-10-24T04:06:00.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:06:30.000Z"}"#,
        r#"{"type":"goal","outcome":"accepted","ts":"2026-10-24T04:07:00.000Z"}"#,
        r#"{"type":"iteration","n":5,"ts":"2026-10-24T04:08:00.000Z"}"#,
    ]
    .join("\n")
        + "\n"
}

// ---- pin 1: the empty stream + malformed-line tolerance -----------------

#[test]
fn empty_stream_zero_counts_goal_none() {
    let f = Fixture::new("empty");
    let out = run_ok(&f.write(""), &[]);
    assert_eq!(field(&out, "iters"), "0", "{out}");
    assert_eq!(field(&out, "first-anchor"), "none", "{out}");
    assert_eq!(field(&out, "last-anchor"), "none", "{out}");
    assert_eq!(field(&out, "wall-secs"), "none", "{out}");
    assert_eq!(field(&out, "ok-false"), "0", "{out}");
    assert_eq!(field(&out, "ok-false-legs"), "none", "{out}");
    assert_eq!(acc_rej(&out, "ctx-edit"), (0, 0), "{out}");
    assert_eq!(field(&out, "trim"), "0", "{out}");
    assert_eq!(field(&out, "trim-legs"), "none", "{out}");
    assert_eq!(field(&out, "goal"), "none", "{out}");
    assert_eq!(field(&out, "goal-ts"), "none", "{out}");
    // The file's own facts still read (a 0-byte fixture probed back).
    let len = std::fs::metadata(f.events_path()).expect("stat").len();
    assert_eq!(field(&out, "bytes"), len.to_string(), "{out}");
    assert_eq!(len, 0);
    let mtime = field(&out, "mtime");
    assert_eq!(mtime.len(), 20, "UTC Z form: {mtime}");
    let b = mtime.as_bytes();
    assert!(
        b[4] == b'-' && b[7] == b'-' && b[10] == b'T' && b[13] == b':' && b[16] == b':'
            && b[19] == b'Z'
            && mtime[..19].bytes().all(|c| c.is_ascii_digit() || c == b'-' || c == b'T' || c == b':'),
        "mtime must be `YYYY-MM-DDTHH:MM:SSZ`: {mtime}"
    );
}

#[test]
fn malformed_lines_drop_silently_fromjson_parity() {
    let f = Fixture::new("malformed");
    let body = "not json at all\n\
                \n\
                \"just a json string\"\n\
                [1, 2, 3]\n\
                {\"type\":\"iteration\",\"n\":1,\"ts\":\"2026-10-24T04:01:00.000Z\"}\n\
                {\"type\":\"tool_result\",\"ok\":false,\"ts\":\"2026-10-24T04:00:50.000Z\"}\n\
                {broken\n";
    let path = f.write(body);
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "iters"), "1", "only the one good iteration: {out}");
    assert_eq!(field(&out, "ok-false"), "1", "only the one good leg: {out}");
    // The file's own bytes count the WHOLE fixture (facts, not events).
    let len = std::fs::metadata(&path).expect("stat").len();
    assert_eq!(field(&out, "bytes"), len.to_string(), "{out}");
}

// ---- pin 2: the bisecting --at stamp (the 1982-fill shape) --------------

#[test]
fn at_stamp_scopes_every_count() {
    let f = Fixture::new("bisect");
    let body = bisect_body();
    let path = f.write(&body);
    let whole = run_ok(&path, &[]);
    let stamped = run_ok(&path, &["--at", STAMP]);

    // The output shape is the contract: the split-read yields exactly the
    // 13 pinned fields, in the pinned order (bytes/mtime last — the file's
    // own facts ride below the stream's).
    let keys: Vec<String> = datum(&stamped).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        keys,
        [
            "iters",
            "first-anchor",
            "last-anchor",
            "wall-secs",
            "ok-false",
            "ok-false-legs",
            "ctx-edit",
            "trim",
            "trim-legs",
            "goal",
            "goal-ts",
            "bytes",
            "mtime",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect::<Vec<_>>(),
        "{stamped}"
    );

    // Whole stream (default stamp = the last iteration anchor): every
    // class counted, the expectations derived from the fixture text itself
    // (T270 — never a remembered constant).
    assert_eq!(
        num_field(&whole, "iters"),
        count_type(&body, "iteration") as u64,
        "{whole}"
    );
    assert_eq!(
        num_field(&whole, "ok-false"),
        body.matches("\"ok\":false").count() as u64,
        "{whole}"
    );
    assert_eq!(field(&whole, "ok-false-legs"), legs(&body, |l| l
        .contains("\"type\":\"tool_result\"")
        && l.contains("\"ok\":false")), "{whole}");
    assert_eq!(
        field(&whole, "trim-legs"),
        legs(&body, |l| l.contains("\"type\":\"trim\"")),
        "{whole}"
    );
    assert_eq!(acc_rej(&whole, "ctx-edit"), (1, 1), "{whole}");
    assert_eq!(acc_rej(&whole, "goal"), (1, 0), "{whole}");
    assert_eq!(
        field(&whole, "goal-ts"),
        ts_of(body.lines().find(|l| l.contains("\"type\":\"goal\"")).expect("goal line"))
            .expect("goal ts"),
        "{whole}"
    );

    // At the stamp: every count scopes ≤ stamp — the 1982-fill shape. The
    // trim BEFORE the stamp (04:02:20) is counted, the one AFTER
    // (04:06:00) is excluded from the count AND the leg list; the ok:false
    // leg after the stamp (04:03:30) is excluded from both.
    assert_eq!(num_field(&stamped, "iters"), 3, "{stamped}");
    assert_eq!(field(&stamped, "last-anchor"), STAMP, "{stamped}");
    assert_eq!(num_field(&stamped, "ok-false"), 1, "{stamped}");
    assert_eq!(
        field(&stamped, "ok-false-legs"),
        "2026-10-24T04:01:10.000Z",
        "{stamped}"
    );
    assert_eq!(acc_rej(&stamped, "ctx-edit"), (1, 0), "{stamped}");
    assert_eq!(num_field(&stamped, "trim"), 1, "{stamped}");
    assert_eq!(
        field(&stamped, "trim-legs"),
        "2026-10-24T04:02:20.000Z",
        "{stamped}"
    );
    assert_eq!(field(&stamped, "goal"), "none", "{stamped}");
    assert_eq!(field(&stamped, "goal-ts"), "none", "{stamped}");

    // The mechanical scoping relation: every countable ≤ its whole-stream
    // value (no per-class constant needed — the relation is the pin).
    for key in ["iters", "ok-false", "trim"] {
        assert!(
            num_field(&stamped, key) <= num_field(&whole, key),
            "{key} at the stamp must not exceed the whole-stream count"
        );
    }
    let (sa, sr) = acc_rej(&stamped, "ctx-edit");
    let (wa, wr) = acc_rej(&whole, "ctx-edit");
    assert!(sa <= wa && sr <= wr, "ctx-edit counts scope ≤ whole");
}

#[test]
fn event_exactly_at_the_stamp_is_in_scope() {
    // The `<=` boundary: the n=3 iteration carries the stamp's own ts and
    // IS the last-anchor under --at.
    let f = Fixture::new("boundary-eq");
    let path = f.write(&bisect_body());
    let out = run_ok(&path, &["--at", STAMP]);
    assert_eq!(field(&out, "last-anchor"), STAMP, "{out}");
    assert_eq!(field(&out, "first-anchor"), "2026-10-24T04:01:00.000Z", "{out}");
    // One millisecond earlier excludes it (string-compare: .999 < .000).
    let out = run_ok(&path, &["--at", "2026-10-24T04:02:59.999Z"]);
    assert_eq!(field(&out, "last-anchor"), "2026-10-24T04:02:00.000Z", "{out}");
    assert_eq!(num_field(&out, "iters"), 2, "{out}");
}

// ---- pin 3: leg enumeration order (stream order) ------------------------

#[test]
fn ok_false_legs_enumerate_in_stream_order() {
    let f = Fixture::new("leg-order");
    let body = [
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#,
        // Three legs, deliberately interleaved with other events.
        r#"{"type":"tool_result","name":"read_file","ok":false,"ts":"2026-10-24T04:01:10.000Z"}"#,
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:02:00.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":true,"ts":"2026-10-24T04:02:05.000Z"}"#,
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:02:20.000Z"}"#,
        r#"{"type":"trim","ts":"2026-10-24T04:02:30.000Z"}"#,
        // The third leg rides BEFORE the final iteration anchor (04:03:00)
        // so the default-stamp read sees all three in stream order.
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:02:50.000Z"}"#,
        r#"{"type":"iteration","n":3,"ts":"2026-10-24T04:03:00.000Z"}"#,
    ]
    .join("\n")
        + "\n";
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    assert_eq!(
        field(&out, "ok-false-legs"),
        legs(&body, |l| l.contains("\"type\":\"tool_result\"")
            && l.contains("\"ok\":false")),
        "the legs list must be the fixture's leg ts values in stream order: {out}"
    );
    assert_eq!(field(&out, "ok-false-legs").split(", ").count(), 3, "{out}");
    // The ok:true tool_result never lands in the legs list.
    assert!(
        !field(&out, "ok-false-legs").contains("04:02:05"),
        "{out}"
    );
}

#[test]
fn trim_legs_list_the_stream_ts_values() {
    let f = Fixture::new("trim-list");
    let body = [
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#,
        r#"{"type":"trim","ts":"2026-10-24T04:01:30.000Z"}"#,
        r#"{"type":"trim","ts":"2026-10-24T04:02:30.000Z"}"#,
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:03:00.000Z"}"#,
    ]
    .join("\n")
        + "\n";
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "trim"), "2", "{out}");
    assert_eq!(
        field(&out, "trim-legs"),
        legs(&body, |l| l.contains("\"type\":\"trim\"")),
        "{out}"
    );
    assert_eq!(
        field(&out, "trim-legs"),
        "2026-10-24T04:01:30.000Z, 2026-10-24T04:02:30.000Z",
        "{out}"
    );
}

// ---- pin 4: ctx_edit accepted + rejected counted separately -------------

#[test]
fn ctx_edit_counts_accepted_and_rejected_separately() {
    let f = Fixture::new("ctx-edit");
    let body = [
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#,
        r#"{"type":"ctx_edit","accepted":true,"ts":"2026-10-24T04:01:10.000Z"}"#,
        r#"{"type":"ctx_edit","accepted":true,"ts":"2026-10-24T04:01:20.000Z"}"#,
        r#"{"type":"ctx_edit","accepted":false,"ts":"2026-10-24T04:01:30.000Z"}"#,
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:02:00.000Z"}"#,
    ]
    .join("\n")
        + "\n";
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    assert_eq!(
        acc_rej(&out, "ctx-edit"),
        (2, 1),
        "accepted and rejected count separately (2 + 1 == the 3 fires): {out}"
    );
    // At a stamp between the second accept and the reject: only the two
    // accepted legs are in scope.
    let out = run_ok(&path, &["--at", "2026-10-24T04:01:25.000Z"]);
    assert_eq!(acc_rej(&out, "ctx-edit"), (2, 0), "{out}");
}

// ---- pin 5: the goal .outcome read, all three shapes + the field name ---

fn goal_body(goal_line: Option<&str>) -> String {
    let mut lines = vec![
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#.to_string(),
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:02:00.000Z"}"#.to_string(),
    ];
    if let Some(g) = goal_line {
        lines.push(g.to_string());
    }
    // The last iteration anchor trails the goal (the real-stream shape),
    // so the whole-stream default datum sees the goal event.
    lines.push(
        r#"{"type":"iteration","n":3,"ts":"2026-10-24T04:05:00.000Z"}"#.to_string(),
    );
    lines.join("\n") + "\n"
}

#[test]
fn goal_outcome_read_in_all_three_shapes() {
    let f = Fixture::new("goal-shapes");
    // (a) accepted
    let path = f.write(&goal_body(Some(
        r#"{"type":"goal","outcome":"accepted","ts":"2026-10-24T04:03:00.000Z"}"#,
    )));
    let out = run_ok(&path, &[]);
    assert_eq!(acc_rej(&out, "goal"), (1, 0), "{out}");
    assert_eq!(field(&out, "goal-ts"), "2026-10-24T04:03:00.000Z", "{out}");

    // (b) rejected
    let path = f.write(&goal_body(Some(
        r#"{"type":"goal","outcome":"rejected","ts":"2026-10-24T04:03:00.000Z"}"#,
    )));
    let out = run_ok(&path, &[]);
    assert_eq!(acc_rej(&out, "goal"), (0, 1), "{out}");
    assert_eq!(field(&out, "goal-ts"), "2026-10-24T04:03:00.000Z", "{out}");

    // (c) absent
    let path = f.write(&goal_body(None));
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "goal"), "none", "{out}");
    assert_eq!(field(&out, "goal-ts"), "none", "{out}");
}

#[test]
fn goal_verdict_field_is_never_read_outcome_is() {
    // The verdict field is .outcome, NOT .verdict (the corpus schema's
    // goal event carries .outcome; a helper reading .verdict would read
    // "accepted" off this event). The event exists (goal-ts reads it) but
    // contributes to NEITHER side.
    let f = Fixture::new("goal-verdict-field");
    let path = f.write(&goal_body(Some(
        r#"{"type":"goal","verdict":"accepted","ts":"2026-10-24T04:03:00.000Z"}"#,
    )));
    let out = run_ok(&path, &[]);
    assert_eq!(acc_rej(&out, "goal"), (0, 0), "{out}");
    assert_eq!(field(&out, "goal-ts"), "2026-10-24T04:03:00.000Z", "{out}");
}

// ---- pin 6: the file's own facts ----------------------------------------

#[test]
fn bytes_matches_the_probed_fixture_length() {
    let f = Fixture::new("bytes");
    let body = bisect_body();
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    let len = std::fs::metadata(&path).expect("stat").len();
    assert_eq!(field(&out, "bytes"), len.to_string(), "{out}");
    // The facts never scope: --at before everything leaves bytes unchanged.
    let out = run_ok(&path, &["--at", "2026-10-24T00:00:00.000Z"]);
    assert_eq!(field(&out, "bytes"), len.to_string(), "{out}");
    assert_eq!(len, body.len() as u64);
}

// ---- pin 7: wall arithmetic from the probed anchors (T270) --------------

#[test]
fn wall_arithmetic_derives_from_the_probed_anchors() {
    let f = Fixture::new("wall");
    let path = f.write(&bisect_body());

    // Whole stream: the anchors are the first/last iteration events.
    let whole = run_ok(&path, &[]);
    let first = field(&whole, "first-anchor");
    let last = field(&whole, "last-anchor");
    let wall: f64 = field(&whole, "wall-secs").parse().expect("wall-secs");
    let expected = epoch_secs(&last) - epoch_secs(&first);
    assert!(
        (wall - expected).abs() < 0.0005,
        "wall-secs {wall} must equal last-anchor − first-anchor {expected} (anchors {first} → {last})"
    );

    // At the stamp: the span re-derives from the SCOPED anchors.
    let stamped = run_ok(&path, &["--at", STAMP]);
    let first = field(&stamped, "first-anchor");
    let last = field(&stamped, "last-anchor");
    let wall: f64 = field(&stamped, "wall-secs").parse().expect("wall-secs");
    let expected = epoch_secs(&last) - epoch_secs(&first);
    assert!(
        (wall - expected).abs() < 0.0005,
        "stamped wall-secs {wall} must equal last-anchor − first-anchor {expected}"
    );
    // And the stamped span is the prefix span (n=1 → n=3 == 120s by
    // construction of the fixture anchors, 04:01:00 → 04:03:00).
    assert!(
        (wall - 120.0).abs() < 0.0005,
        "the 04:01:00→04:03:00 anchors span 120s: {wall}"
    );
}

#[test]
fn single_iteration_yields_zero_wall() {
    let f = Fixture::new("one-iter");
    let path = f.write(concat!(
        "{\"type\":\"iteration\",\"n\":1,",
        "\"ts\":\"2026-10-24T04:01:00.123Z\"}\n",
    ));
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "iters"), "1", "{out}");
    assert_eq!(field(&out, "first-anchor"), "2026-10-24T04:01:00.123Z", "{out}");
    assert_eq!(field(&out, "last-anchor"), "2026-10-24T04:01:00.123Z", "{out}");
    assert_eq!(field(&out, "wall-secs"), "0.000", "{out}");
}

// ---- pin 8: the default stamp is the last iteration anchor --------------

#[test]
fn default_stamp_excludes_events_after_the_last_iteration_anchor() {
    // The real-stream shape: the verdict lands AFTER the final iteration
    // event. The default datum (stamped at the last iteration anchor)
    // EXCLUDES it — goal: none — and --at at the verdict's own ts
    // includes it. The corridor names its stamps (the T305 doctrine
    // sentence); the default is the as-of-last-iteration read.
    let f = Fixture::new("default-stamp");
    let body = [
        r#"{"type":"iteration","n":1,"ts":"2026-10-24T04:01:00.000Z"}"#,
        r#"{"type":"iteration","n":2,"ts":"2026-10-24T04:02:00.000Z"}"#,
        r#"{"type":"goal","outcome":"accepted","ts":"2026-10-24T04:02:30.000Z"}"#,
    ]
    .join("\n")
        + "\n";
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "last-anchor"), "2026-10-24T04:02:00.000Z", "{out}");
    assert_eq!(field(&out, "goal"), "none", "{out}");
    assert_eq!(field(&out, "goal-ts"), "none", "{out}");
    let out = run_ok(&path, &["--at", "2026-10-24T04:02:30.000Z"]);
    assert_eq!(acc_rej(&out, "goal"), (1, 0), "{out}");
    assert_eq!(field(&out, "goal-ts"), "2026-10-24T04:02:30.000Z", "{out}");
}

#[test]
fn no_iteration_stream_falls_back_to_its_last_event_ts() {
    // A tool_result-only stream has no iteration anchor; the fallback
    // stamps at the last parsable event ts so the read stays scoped (and
    // non-empty) rather than collapsing to zeros.
    let f = Fixture::new("no-iter");
    let body = [
        r#"{"type":"run_start","ts":"2026-10-24T04:00:00.000Z","model":"m"}"#,
        r#"{"type":"tool_result","name":"bash","ok":false,"ts":"2026-10-24T04:00:10.000Z"}"#,
    ]
    .join("\n")
        + "\n";
    let path = f.write(&body);
    let out = run_ok(&path, &[]);
    assert_eq!(field(&out, "iters"), "0", "{out}");
    assert_eq!(field(&out, "first-anchor"), "none", "{out}");
    assert_eq!(field(&out, "wall-secs"), "none", "{out}");
    assert_eq!(field(&out, "ok-false"), "1", "the fallback keeps the leg: {out}");
    assert_eq!(
        field(&out, "ok-false-legs"),
        "2026-10-24T04:00:10.000Z",
        "{out}"
    );
}

// ---- pin 9: usage errors exit 2 ------------------------------------------

#[test]
fn malformed_at_stamp_exits_2() {
    let f = Fixture::new("bad-at");
    let path = f.write(&bisect_body());
    for bad in [
        "2026-10-24T04:03:00",      // no Z
        "2026-10-24 04:03:00Z",     // space, not T
        "2026-10-24T04:03Z",        // no seconds
        "24-10-2026T04:03:00Z",     // wrong date order
        "not-a-ts",                 // garbage
    ] {
        let (code, stdout, stderr) = run_raw(Some(&path), &["--at", bad]);
        assert_eq!(code, 2, "--at {bad:?} must exit 2 (stdout {stdout:?})");
        assert!(
            stderr.contains("ISO-8601 Z form"),
            "the error names the stamp shape: {stderr}"
        );
    }
}

#[test]
fn missing_events_file_exits_2() {
    let f = Fixture::new("no-file");
    let (code, stdout, stderr) =
        run_raw(Some(&f.dir.join("absent.jsonl")), &[]);
    assert_eq!(code, 2, "an unreadable events file must exit 2 ({stdout:?})");
    assert!(
        stderr.contains("unreadable events file"),
        "the error names the failure: {stderr}"
    );
}

#[test]
fn missing_events_arg_exits_2() {
    let (code, _stdout, stderr) = run_raw(None, &[]);
    assert_eq!(code, 2, "a missing events arg must exit 2");
    assert!(stderr.contains("usage"), "argparse names the usage: {stderr}");
}

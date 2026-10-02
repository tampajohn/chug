//! T70 — `decision_log`: structured loop decision records
//! (`.chug/decisions.jsonl`).
//!
//! F13 phase 1 (operator directive dc18a7a): the loop's long-term speed lever
//! is shrinking kimi's share of loop judgments to the hard cases, which needs
//! a corpus — and today every loop-level judgment (the kimi-validation
//! routing call per item, the verdict accepted, the recovery routing on a
//! child budget death, the model fallback, the evaluator's file/reject
//! triage) lives only as prose. This module gives those judgments a
//! machine-readable home: a model-facing tool that appends ONE JSON object
//! line per call to `<cwd>/.chug/decisions.jsonl`.
//!
//! The precedent is the risk gate's `.chug/risk_verdicts.jsonl`
//! (src/riskgate.rs `log_verdict`): append-only JSONL, one decision per line,
//! best-effort. Here "best-effort" means a write failure surfaces as a tool
//! error (the `dispatch` wrapper converts it to an `is_error` result the
//! model can see and recover from) — it can never abort the run.
//!
//! Module shape follows the T37 webfetch.rs pattern: ALL logic lives here,
//! and src/tools.rs carries only the two registration lines (schema push +
//! dispatch arm) so the tools.rs monolith does not grow.
//!
//! cwd scoping: the log is `<ToolCtx cwd>/.chug/decisions.jsonl`, so children
//! in worktrees write their own stream, harvested exactly like their events
//! streams at merge time — no special casing.
//!
//! No rotation or compaction in v1: the volume is a handful of lines per
//! cycle, so an unbounded append is correct until a distillation corpus
//! exists to care about (F13 phases 2–3 are deferred with reasons in
//! EVALUATION.md §4).
//!
//! Events stream: deliberately OUT of scope. `.chug/events.jsonl` already
//! logs a tool_result preview for every tool call, so no new Event variant is
//! added here — do not "fix" that later; the decision record IS the durable
//! surface, events.jsonl stays the activity log.
//!
//! T88 — corrective validation: the cycle-47 eval (kimi orchestrator) died on
//! five consecutive identical `missing or non-string field: class` errors
//! while batch-logging eval-triage records, because the one-field-at-a-time
//! error never said what the call actually looked like. Validation now
//! diagnoses EVERYTHING wrong with a call in ONE error: every invalid field
//! with its expected shape, in schema order; the received top-level keys and
//! the one-record-per-call contract when the shape itself is unknown (a
//! batched wrapper, an aliased key); and the received JSON type for a
//! non-object input. The success path is byte-identical (req 4).
//!
//! T199 — corpus integrity at the write edge (F13 phase 2a-i): outcome
//! records are the F13 classifier's label rows, so they get two guards the
//! other classes don't. (1) `choice` is a CLOSED set {landed-clean,
//! fixed-up, reverted}, enforced at write time — a prose choice ("landed-clean
//! — merge 7b9b2bf (keep-both conflict resolution) + flip") poisons the label
//! set the classifier trains against; 7 corpus records predate the gate and
//! stay grandfathered (append-only history is immutable;
//! `scripts/decisions-audit.sh` counts them). (2) An advisory subject lint:
//! an outcome backfill whose `subject` names no id the corpus contains gets a
//! `note:` line on the return text — never an error (append-only best-effort
//! stays the T70 invariant). Every other class keeps its free-string
//! `choice`: the seed classes' options are per-decision.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::tools::ToolResult;

/// Per-process monotonically increasing counter for the `<n>` component of
/// record ids (`d<ts>-<n>`). Process-shared (a `static` AtomicU64) is
/// deliberately enough: each `chug` process owns its cwd's log segment, and
/// the counter only has to make ids unique and ordered within it.
static ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// The seed decision classes, named verbatim in the schema description so the
/// model does not invent synonyms. Free-string classes beyond these are
/// accepted by design — a new class must not need a code change.
pub const SEED_CLASSES: [&str; 6] = [
    "validation-routing",
    "validation-verdict",
    "recovery-routing",
    "model-fallback",
    "eval-triage",
    "outcome",
];

/// The closed `choice` set for `class == "outcome"` records (T199 req 1) —
/// exactly the label set the F13 classifier trains against, named verbatim in
/// the schema description. Other classes' `choice` stays free-string.
const OUTCOME_CHOICES: [&str; 3] = ["landed-clean", "fixed-up", "reverted"];

/// One decision record. Field order here IS the line's key order (serde
/// serializes struct fields in declaration order), matching the documented
/// record shape `{"id","ts","class","subject","inputs","options","choice",
/// "confidence"}`.
#[derive(Debug, Clone, Serialize)]
struct DecisionRecord {
    id: String,
    ts: u64,
    class: String,
    subject: String,
    inputs: String,
    options: String,
    choice: String,
    confidence: f64,
}

/// JSON schema for the `decision_log` tool, registered alongside the
/// builtins.
pub fn schema() -> Value {
    let seed_list = SEED_CLASSES.join(", ");
    json!({
        "name": "decision_log",
        "description": format!(
            "Record ONE structured loop decision to `.chug/decisions.jsonl` — the machine-readable \
             decision corpus feeding F13 distillation (class, compact evidence, options considered, \
             choice, confidence; confidence is the deciding model's stated 0..=1 confidence). \
             Required in every call: `class`, `subject`, `inputs`, `options`, `choice`, and \
             `confidence` — all strings except `confidence` (a number in 0..=1) — with no field \
             omitted. \
             Seed classes (use these verbatim so records do not fork into \
             synonyms): {seed_list} — free-string classes beyond these are allowed and need no code \
             change. Outcome backfills are ordinary records with `class: \"outcome\"`, `subject` \
             naming the earlier decision id, and `choice` one of `landed-clean`, `fixed-up`, \
             `reverted`. Append-only and best-effort: each call appends one line, a write failure \
             returns a tool error and never aborts the run, no rotation in v1. Returns \
             `recorded <id>` — keep the id; outcome backfills name it as their subject."
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "class": {"type": "string", "description": format!("Decision class. Seed classes (use verbatim): {seed_list}. Free string beyond these — new classes must not need a code change.")},
                "subject": {"type": "string", "description": "What was decided, compact (e.g. `T70`, or `cycle-34-eval candidate: wait_secs pacing`). Outcome backfills name the earlier decision id here."},
                "inputs": {"type": "string", "description": "The compact evidence (e.g. `files: src/tools.rs+LOOP-SPEC.md; diff +863/-16; 3 mutants 1 survivor`)"},
                "options": {"type": "string", "description": "The options considered (e.g. `kimi-required | kimi-optional | skip`)"},
                "choice": {"type": "string", "description": "The chosen option. For `outcome` records: one of `landed-clean`, `fixed-up`, `reverted`."},
                "confidence": {"type": "number", "minimum": 0, "maximum": 1, "description": "The deciding model's stated confidence, 0..=1 inclusive — the field F13's confidence-gated routing will train against."}
            },
            "required": ["class", "subject", "inputs", "options", "choice", "confidence"]
        }
    })
}

/// Dispatch entry for the `decision_log` arm in `tools::inner`. Validation
/// failures and write I/O failures both come back as `Err` — the dispatch
/// wrapper turns them into `is_error` results, so a failure can never abort
/// the run.
/// The six required fields, in schema order — this order drives the combined
/// error's leg order (T88 req 1) and nothing else.
const REQUIRED_FIELDS: [&str; 6] = [
    "class", "subject", "inputs", "options", "choice", "confidence",
];

/// The contract reminder appended to EVERY validation-failure error (T88
/// reqs 2-3 built it for the unknown-shape and non-object shapes; T182 made
/// it unconditional): the reminder names the fix, not just the fields.
/// Invariant: every failure message carries the full required-field list,
/// because the 5-of-6-fields fumble shape previously bypassed it — the
/// cycle-83 census counted 5 instances in 3 cycles.
const CONTRACT_REMINDER: &str =
    "one record per call; required: class, subject, inputs, options, choice, confidence";

/// Dispatch entry for the `decision_log` arm in `tools::inner`. Validation
/// failures and write I/O failures both come back as `Err` — the dispatch
/// wrapper turns them into `is_error` results, so a failure can never abort
/// the run.
pub fn decision_log(cwd: &Path, input: &Value) -> anyhow::Result<ToolResult> {
    // T88 req 3: a top-level non-object (array / string / number / bool /
    // null) cannot be diagnosed field-by-field, so the error names the
    // received JSON type plus the full required list.
    let Some(obj) = input.as_object() else {
        return Err(anyhow!(
            "invalid decision_log call: expected a JSON object, got {}; {CONTRACT_REMINDER}",
            json_type_name(input)
        ));
    };

    // T88 req 1: validate ALL six fields up front so ONE message carries
    // every leg, instead of one error per retry (the cycle-47
    // five-in-a-row failure). Legs render in REQUIRED_FIELDS order.
    let class = str_field(obj, "class");
    let subject = str_field(obj, "subject");
    let inputs = str_field(obj, "inputs");
    let options = str_field(obj, "options");
    let choice = str_field(obj, "choice");
    let confidence = confidence_field(obj);
    let legs: Vec<String> = [
        err_of(&class),
        err_of(&subject),
        err_of(&inputs),
        err_of(&options),
        err_of(&choice),
        err_of(&confidence),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !legs.is_empty() {
        return Err(anyhow!("{}", validation_message(obj, &legs)));
    }

    // All six validated above; `unwrap_or_default` keeps this panic-free by
    // construction (a panic here would abort the run, which the tool
    // contract forbids) — the defaults are never taken.
    let class = class.unwrap_or_default();
    let subject = subject.unwrap_or_default();
    let inputs = inputs.unwrap_or_default();
    let options = options.unwrap_or_default();
    let choice = choice.unwrap_or_default();
    let confidence = confidence.unwrap_or_default();

    // T199 req 1: the outcome choice enum, enforced at write time — never a
    // silent clamp. This leg sits AFTER the six type legs, so it only fires
    // when class and choice are both well-typed strings (a missing or
    // garbled choice is already named by its own leg above; no
    // double-reporting). The message names the closed set and says where
    // provenance text belongs, and rides the same invalid-call shape
    // (contract reminder included) as every other validation failure.
    if class == "outcome" && !OUTCOME_CHOICES.contains(&choice) {
        let leg = format!(
            "outcome choice must be one of {}, got \"{choice}\" — provenance text belongs in \
             inputs",
            OUTCOME_CHOICES.join(" | ")
        );
        return Err(anyhow!("{}", validation_message(obj, &[leg])));
    }

    let ts = unix_ts();
    let n = ID_COUNTER.fetch_add(1, Ordering::Relaxed) + 1;
    let record = DecisionRecord {
        id: format!("d{ts}-{n}"),
        ts,
        class: class.to_string(),
        subject: subject.to_string(),
        inputs: inputs.to_string(),
        options: options.to_string(),
        choice: choice.to_string(),
        confidence,
    };

    append_record(cwd, &record)?;

    // T199 req 2: the advisory subject lint, outcome records only — an
    // outcome backfill's subject should BE an id the corpus contains (the
    // T152-class bug was a misattributed subject id, and a TODO number or a
    // composite prose subject does not join cleanly at distillation time).
    // Best-effort, never an error: an unreadable corpus skips the check
    // silently and the record stays landed; a miss only appends a note line
    // to the return text. The scan runs after the append (the write path is
    // untouched; the note describes the corpus as the record lands in it).
    let mut content = format!("recorded {}", record.id);
    if class == "outcome"
        && let Some(note) = subject_note(cwd, subject)
    {
        content.push('\n');
        content.push_str(&note);
    }
    Ok(ToolResult {
        content,
        is_error: false,
        images: Vec::new(),
    })
}

/// The advisory note for an outcome record whose `subject` resolves to no
/// record id in the corpus (T199 req 2), or `None` when there is nothing to
/// note: the subject IS an existing id, or the corpus file is unreadable or
/// absent (the silent-skip case — best-effort must never turn a landed
/// record into a failure).
fn subject_note(cwd: &Path, subject: &str) -> Option<String> {
    let path = cwd.join(".chug").join("decisions.jsonl");
    let text = fs::read_to_string(&path).ok()?;
    let resolved = text
        .lines()
        // First-match: the scan stops at the first line whose id equals the
        // subject. Exact equality — the same "resolves to an id" definition
        // scripts/decisions-audit.sh applies, so the lint and the audit
        // agree on what a resolved subject is.
        .any(|line| line_id(line) == Some(subject));
    if resolved {
        None
    } else {
        Some(format!(
            "note: subject {subject} not found in {}",
            path.display()
        ))
    }
}

/// The record id carried by one corpus line, read WITHOUT parsing JSON — the
/// T199 subject-lint scan is a bounded, line-oriented pass (a first-match
/// string check per line is fine at corpus scale, per the spec). The record
/// shape pins `id` as the FIRST key (T70; `assert_key_order` test-pins it),
/// so the first raw `"id"` occurrence in a well-formed line is the key:
/// slice from the value's opening quote to its closing one. Ids are
/// `d<digits>-<digits>`, so no embedded quotes; an escaped `\"id\"` inside a
/// value never produces the bare `"id"` needle. A line without a parsable
/// leading id yields `None` and simply never matches — the lint stays
/// advisory even over malformed corpus lines.
fn line_id(line: &str) -> Option<&str> {
    let key = "\"id\"";
    let after_key = line.find(key)? + key.len();
    let after_colon = after_key + line[after_key..].find(':')? + 1;
    let after_open = after_colon + line[after_colon..].find('"')? + 1;
    let end = after_open + line[after_open..].find('"')?;
    Some(&line[after_open..end])
}

/// Human-readable name for a `serde_json` value's type, used by the
/// corrective legs ("got number", "got array").
/// The `Err` leg of a field validation result, cloned out so all six fields'
/// errors (whose `Ok` payloads differ in type) can ride one homogeneous
/// `Vec<String>`.
fn err_of<T>(r: &Result<T, String>) -> Option<String> {
    r.as_ref().err().cloned()
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// One required string field, as a corrective leg (T88 req 1): `Ok(value)`
/// when valid, `Err("<field> must be a string, got <received>")` otherwise,
/// where `<received>` is `missing` (absent or null) or the JSON type name.
/// Returns the leg text instead of an `anyhow` error so the caller can
/// collect every leg before formatting ONE combined message.
fn str_field<'a>(obj: &'a Map<String, Value>, key: &str) -> Result<&'a str, String> {
    match obj.get(key) {
        Some(Value::String(s)) => Ok(s),
        None | Some(Value::Null) => Err(format!("{key} must be a string, got missing")),
        Some(other) => Err(format!(
            "{key} must be a string, got {}",
            json_type_name(other)
        )),
    }
}

/// The required `confidence` field: a number in 0..=1 inclusive. The leg
/// keeps the T70 range text verbatim (`confidence must be a number in
/// 0..=1, got <n>`) so an out-of-range confidence reads exactly as before
/// (T88 req 5); absent/null and non-number values render `missing` / the
/// JSON type name. Garbage must not enter the corpus: this is the field
/// F13's confidence-gated routing trains against.
fn confidence_field(obj: &Map<String, Value>) -> Result<f64, String> {
    match obj.get("confidence") {
        Some(Value::Number(n)) => {
            let v = n.as_f64().unwrap_or(f64::NAN);
            if (0.0..=1.0).contains(&v) {
                Ok(v)
            } else {
                Err(format!("confidence must be a number in 0..=1, got {v}"))
            }
        }
        None | Some(Value::Null) => {
            Err("confidence must be a number in 0..=1, got missing".into())
        }
        Some(other) => Err(format!(
            "confidence must be a number in 0..=1, got {}",
            json_type_name(other)
        )),
    }
}

/// The combined validation error (T88 reqs 1-2): every leg in schema order,
/// then the one-record-per-call contract reminder — ALWAYS (T182), for
/// every validation-failure shape: the 5-of-6-fields fumble (all keys
/// recognized, one missing) previously took the legs-only path, the error
/// named the ONE missing field but never re-stated the contract, and the
/// retry dropped a DIFFERENT field (cycle-83 census: 5 instances in 3
/// cycles). The `received keys: [...]` list stays conditional on the SHAPE
/// being unknown — an unrecognized key rides along (the events.jsonl-style
/// `type` alias): a merely-missing-field call needs the contract, not its
/// own keys echoed. Fires on failure legs only: extra keys on an
/// otherwise-valid record stay ignored, keeping the success path unchanged
/// (T88 req 4).
fn validation_message(obj: &Map<String, Value>, legs: &[String]) -> String {
    let mut msg = format!("invalid decision_log call: {}", legs.join("; "));
    let has_unrecognized = obj.keys().any(|k| !REQUIRED_FIELDS.contains(&k.as_str()));
    if has_unrecognized {
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        keys.truncate(8);
        msg.push_str(&format!("; received keys: [{}]", keys.join(", ")));
    }
    msg.push_str(&format!("; {CONTRACT_REMINDER}"));
    msg
}

/// Append the record as ONE JSON object line to `<cwd>/.chug/decisions.jsonl`,
/// creating `.chug/` if missing. Append-only: never truncate, never rewrite.
fn append_record(cwd: &Path, record: &DecisionRecord) -> anyhow::Result<()> {
    let line = serde_json::to_string(record).context("serializing decision record")?;
    let dir = cwd.join(".chug");
    fs::create_dir_all(&dir).with_context(|| format!("creating decision-log dir {}", dir.display()))?;
    let path = dir.join("decisions.jsonl");
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("opening decision log {}", path.display()))?;
    writeln!(f, "{line}").with_context(|| format!("appending to {}", path.display()))?;
    Ok(())
}

fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{dispatch, tool_schemas, ToolCtx};
    use std::time::Duration;
    use tempfile::TempDir;

    fn tool_ctx(cwd: &Path) -> ToolCtx {
        ToolCtx {
            cwd: cwd.to_path_buf(),
            bash_timeout: Duration::from_secs(crate::tools::BASH_TIMEOUT_SECS),
        }
    }

    fn log_path(cwd: &Path) -> std::path::PathBuf {
        cwd.join(".chug/decisions.jsonl")
    }

    /// Parse every line of the log as a JSON object (a mangled line fails
    /// loudly here rather than silently).
    fn read_records(cwd: &Path) -> Vec<Value> {
        fs::read_to_string(log_path(cwd))
            .expect("decision log readable")
            .lines()
            .map(|l| serde_json::from_str(l).expect("each line is parseable JSON"))
            .collect()
    }

    fn sample_input() -> Value {
        json!({
            "class": "validation-routing",
            "subject": "T70",
            "inputs": "files: src/tools.rs+LOOP-SPEC.md; diff +863/-16; 3 mutants 1 survivor",
            "options": "kimi-required | kimi-optional | skip",
            "choice": "kimi-required",
            "confidence": 0.9
        })
    }

    /// Split `d<ts>-<n>` and return the counter component; also asserts the
    /// ts component is a non-empty digit run (the documented id shape).
    fn counter_of(id: &str) -> u64 {
        let body = id.strip_prefix('d').unwrap_or_else(|| panic!("id {id:?} starts with 'd'"));
        let (ts, n) = body
            .rsplit_once('-')
            .unwrap_or_else(|| panic!("id {id:?} carries a -<n> suffix"));
        assert!(
            !ts.is_empty() && ts.chars().all(|c| c.is_ascii_digit()),
            "ts component of {id:?} must be unix seconds"
        );
        n.parse().unwrap_or_else(|e| panic!("counter of {id:?} is numeric: {e}"))
    }

    /// Assert `line` carries `"key":` for every key, in the given order —
    /// pins the documented record shape's field order on the RAW line
    /// (parsing into a Value would sort keys and hide a reorder).
    fn assert_key_order(line: &str, keys: &[&str]) {
        let mut cursor = 0;
        for key in keys {
            let needle = format!("\"{key}\":");
            let at = line[cursor..]
                .find(&needle)
                .unwrap_or_else(|| panic!("key {key:?} missing or out of order in {line}"));
            cursor += at + needle.len();
        }
    }

    /// The documented record shape, in key order.
    const RECORD_KEYS: [&str; 8] = [
        "id", "ts", "class", "subject", "inputs", "options", "choice", "confidence",
    ];

    // ---------- schema pin (T22 convention) ----------

    /// LIVE `tool_schemas()` carries exactly one `decision_log` schema with
    /// all six properties in `required`, and the description names the seed
    /// classes verbatim plus the outcome-backfill contract — reverting any
    /// one of these fails the pin.
    #[test]
    fn decision_log_schema_pins_required_six_and_seed_class_tokens() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("decision_log"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one decision_log schema");
        let schema = entries[0];

        let props = schema["input_schema"]["properties"]
            .as_object()
            .expect("input_schema.properties");
        for key in RECORD_KEYS.iter().skip(2) {
            assert!(
                props.contains_key(*key),
                "{key} property missing from decision_log schema"
            );
        }
        let required: Vec<&str> = schema["input_schema"]["required"]
            .as_array()
            .expect("required list")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(
            required,
            vec!["class", "subject", "inputs", "options", "choice", "confidence"],
            "all six fields must be required"
        );

        let desc = schema
            .get("description")
            .and_then(Value::as_str)
            .expect("description");
        // HARDCODED literals, deliberately NOT `SEED_CLASSES`. This pin
        // exists to catch an accidental rename inside SEED_CLASSES, and
        // looping over the const under test moves the expectations with the
        // code — the M1a mutant lesson (renaming `eval-triage` →
        // `eval-triage-X` kept this leg GREEN while it iterated SEED_CLASSES,
        // because the description and the expectations changed together).
        // Changing a seed class is a contract change: update both lists
        // consciously.
        const PINNED_SEED_CLASSES: [&str; 6] = [
            "validation-routing",
            "validation-verdict",
            "recovery-routing",
            "model-fallback",
            "eval-triage",
            "outcome",
        ];
        for token in PINNED_SEED_CLASSES {
            assert!(
                desc.contains(token),
                "description must name seed class {token:?} verbatim: {desc}"
            );
        }
        // Exact rendered-list leg, needed because per-token `contains` cannot
        // kill a rename (`"eval-triage-X"` still contains `"eval-triage"`):
        // the description must carry the six hardcoded tokens joined exactly,
        // so a rename OR reorder in SEED_CLASSES breaks this needle.
        let pinned_list = PINNED_SEED_CLASSES.join(", ");
        assert!(
            desc.contains(&pinned_list),
            "description must carry the seed-class list verbatim ({pinned_list}): {desc}"
        );
        // The `class` property description renders the same list for the
        // model — pin it against the same hardcoded needle.
        let class_desc = props["class"]
            .get("description")
            .and_then(Value::as_str)
            .expect("class property description");
        assert!(
            class_desc.contains(&pinned_list),
            "class property description must carry the seed-class list verbatim \
             ({pinned_list}): {class_desc}"
        );
        // Outcome backfill contract + free-string classes + the confidence
        // range, all in the description the model actually sees.
        for token in [
            "landed-clean",
            "fixed-up",
            "reverted",
            "need no code change",
            "0..=1",
            ".chug/decisions.jsonl",
        ] {
            assert!(
                desc.contains(token),
                "description must carry {token:?}: {desc}"
            );
        }
    }

    // ---------- description pin (T22 convention, T168) ----------

    /// T168: the description the model actually reads must NAME the six
    /// required fields explicitly. Cycle-76 eval §2.3: models omitted
    /// `options`/`choice` in most streams of a delta; T88's corrective errors
    /// self-correct in one iteration but the tax recurs every stream, because
    /// the description never said the fields were required. Pinned against the
    /// LIVE `tool_schemas()` output, with needles that exist ONLY in the
    /// required-fields sentence — the pre-T168 description already carried the
    /// bare tokens `options` and `choice` in the record-shape parenthetical,
    /// so bare-token pins would not be RED-provable.
    #[test]
    fn decision_log_description_pins_required_fields_sentence() {
        let schemas = tool_schemas();
        let entries: Vec<&Value> = schemas
            .iter()
            .filter(|s| s.get("name").and_then(Value::as_str) == Some("decision_log"))
            .collect();
        assert_eq!(entries.len(), 1, "exactly one decision_log schema");
        let desc = entries[0]
            .get("description")
            .and_then(Value::as_str)
            .expect("decision_log schema has a description");
        // The six-field list in backticks, in schema (required-array) order,
        // is the load-bearing needle: the joined sequence appears nowhere else
        // in the description, so dropping the sentence (or any one field from
        // it) fails here. The remaining needles pin the sentence's contract
        // clauses — required-ness, the all-strings-but-confidence typing, and
        // the no-omission rule.
        for token in [
            "Required in every call",
            "`class`, `subject`, `inputs`, `options`, `choice`, and `confidence`",
            "all strings except `confidence`",
            "no field omitted",
        ] {
            assert!(
                desc.contains(token),
                "decision_log description lost the required-fields sentence ({token:?}): {desc}"
            );
        }
        // Exactly ONE sentence added (T22 precedent): 5 before T168, 6 after.
        assert_eq!(
            desc.split(". ").count(),
            6,
            "decision_log description must gain exactly one required-fields sentence: {desc}"
        );
    }

    // ---------- dispatch round-trip ----------

    #[test]
    fn decision_log_dispatch_round_trip_writes_one_parseable_line_with_every_field() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let result = dispatch(&ctx, "decision_log", &sample_input());
        assert!(!result.is_error, "{}", result.content);

        // Exactly one line, carrying every field with the input values.
        let text = fs::read_to_string(log_path(tmp.path())).unwrap();
        let mut lines = text.lines();
        let line = lines.next().expect("one line written");
        assert!(lines.next().is_none(), "exactly one line, got:\n{text}");
        assert!(!line.contains('\n') && line.ends_with('}'), "single-line record");

        let record: Value = serde_json::from_str(line).expect("parseable JSON line");
        assert_eq!(record["class"], "validation-routing");
        assert_eq!(record["subject"], "T70");
        assert_eq!(
            record["inputs"],
            "files: src/tools.rs+LOOP-SPEC.md; diff +863/-16; 3 mutants 1 survivor"
        );
        assert_eq!(record["options"], "kimi-required | kimi-optional | skip");
        assert_eq!(record["choice"], "kimi-required");
        assert_eq!(record["confidence"], 0.9);
        assert!(record["ts"].as_u64().is_some(), "ts is unix seconds");

        // The result content echoes the line's id (the model needs it to
        // backfill outcomes later).
        let id = record["id"].as_str().expect("id is a string").to_string();
        assert_eq!(result.content, format!("recorded {id}"));
        // n >= 1: the counter is process-shared and the suite's decision
        // tests run in parallel, so the absolute first-value claim belongs
        // to no single test (append-only pins the ordering instead).
        assert!(counter_of(&id) >= 1, "counter component of {id}");

        // The raw line's key order is the documented record shape.
        assert_key_order(line, &RECORD_KEYS);
    }

    // ---------- append-only ----------

    #[test]
    fn decision_log_append_only_two_calls_two_lines_distinct_increasing_ids() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let r1 = dispatch(&ctx, "decision_log", &sample_input());
        let r2 = dispatch(&ctx, "decision_log", &sample_input());
        assert!(!r1.is_error, "{}", r1.content);
        assert!(!r2.is_error, "{}", r2.content);

        let records = read_records(tmp.path());
        assert_eq!(records.len(), 2, "two calls append exactly two lines");
        let id1 = records[0]["id"].as_str().unwrap();
        let id2 = records[1]["id"].as_str().unwrap();
        assert_ne!(id1, id2, "ids must be distinct");
        assert!(
            counter_of(id2) > counter_of(id1),
            "the counter component must increase: {id1} then {id2}"
        );
        assert_eq!(r1.content, format!("recorded {id1}"));
        assert_eq!(r2.content, format!("recorded {id2}"));
    }

    // ---------- outcome backfill ----------

    #[test]
    fn decision_log_outcome_record_round_trips_byte_faithfully() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let first = dispatch(&ctx, "decision_log", &sample_input());
        assert!(!first.is_error);
        let prior_id = first
            .content
            .strip_prefix("recorded ")
            .expect("result echoes the id")
            .to_string();

        let outcome = json!({
            "class": "outcome",
            "subject": prior_id,
            "inputs": "merge c6ce238; gates 551/551 green",
            "options": "landed-clean | fixed-up | reverted",
            "choice": "fixed-up",
            "confidence": 1.0
        });
        let result = dispatch(&ctx, "decision_log", &outcome);
        assert!(!result.is_error, "{}", result.content);

        let records = read_records(tmp.path());
        assert_eq!(records.len(), 2);
        let rec = &records[1];
        assert_eq!(rec["class"], "outcome");
        assert_eq!(rec["subject"], prior_id, "subject names the earlier decision id");
        assert_eq!(rec["inputs"], "merge c6ce238; gates 551/551 green");
        assert_eq!(rec["options"], "landed-clean | fixed-up | reverted");
        assert_eq!(rec["choice"], "fixed-up");
        assert_eq!(rec["confidence"], 1.0);

        // Byte-faithful on the raw line too: values appear verbatim, and a
        // control character stays escaped so the record really is ONE line.
        let line = fs::read_to_string(log_path(tmp.path()))
            .unwrap()
            .lines()
            .nth(1)
            .unwrap()
            .to_string();
        assert!(line.contains(&format!("\"subject\":\"{prior_id}\"")), "{line}");
        assert!(line.contains("\"class\":\"outcome\""), "{line}");
        assert!(line.contains("\"choice\":\"fixed-up\""), "{line}");
        assert!(line.contains("\"confidence\":1.0"), "{line}");
        assert!(!line.contains('\n'), "no raw newline inside a record");

        // Escaping round-trip: a value with quotes/newline/backslash is
        // written JSON-escaped and parses back to the same string.
        let weird = json!({
            "class": "eval-triage",
            "subject": "t69 \"collect\" candidate",
            "inputs": "line1\nline2\\slash",
            "options": "file | reject",
            "choice": "file",
            "confidence": 0.0
        });
        let r = dispatch(&ctx, "decision_log", &weird);
        assert!(!r.is_error, "{}", r.content);
        let records = read_records(tmp.path());
        assert_eq!(records[2]["subject"], "t69 \"collect\" candidate");
        assert_eq!(records[2]["inputs"], "line1\nline2\\slash");
        assert_eq!(records[2]["confidence"], 0.0, "0.0 boundary round-trips");
    }

    // ---------- T199 outcome choice enum ----------

    /// Req 1: an outcome record's `choice` is a CLOSED set enforced at write
    /// time — a prose choice (the corpus's 7 grandfathered shapes) is a tool
    /// error naming the set and the `inputs` provenance home, lands nothing;
    /// each of the three valid choices passes byte-verbatim; a non-outcome
    /// class keeps its free-string choice.
    #[test]
    fn decision_log_outcome_choice_enum_is_enforced_at_write_time() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let prior = dispatch(&ctx, "decision_log", &sample_input());
        assert!(!prior.is_error);

        // A prose choice — the exact grandfathered corpus shape — is refused.
        let mut bad = sample_input();
        bad["class"] = json!("outcome");
        bad["subject"] = json!("d1790000000-1");
        bad["options"] = json!("landed-clean | fixed-up | reverted");
        bad["choice"] = json!(
            "landed-clean — merge 7b9b2bf (keep-both conflict resolution) + flip, \
             post-merge gates 1286/1286"
        );
        let r = dispatch(&ctx, "decision_log", &bad);
        assert!(r.is_error, "{}", r.content);
        // Exact pin: names the closed set, echoes the refusal, says where
        // provenance text belongs, and carries the contract reminder (T182
        // invariant: every failure message lists the required fields).
        assert_eq!(
            r.content,
            "tool error: invalid decision_log call: outcome choice must be one of \
             landed-clean | fixed-up | reverted, got \"landed-clean — merge 7b9b2bf \
             (keep-both conflict resolution) + flip, post-merge gates 1286/1286\" — \
             provenance text belongs in inputs; one record per call; required: class, \
             subject, inputs, options, choice, confidence"
        );
        for token in ["landed-clean", "fixed-up", "reverted", "provenance text belongs in inputs"]
        {
            assert!(r.content.contains(token), "{token:?} missing: {}", r.content);
        }
        // An enum failure never writes.
        let before = fs::read_to_string(log_path(tmp.path())).unwrap().lines().count();
        assert_eq!(before, 1, "only the prior record is on disk");

        // Each of the three valid choices passes and lands verbatim.
        for (i, choice) in ["landed-clean", "fixed-up", "reverted"].iter().enumerate() {
            let mut input = bad.clone();
            input["choice"] = json!(choice);
            let r = dispatch(&ctx, "decision_log", &input);
            assert!(!r.is_error, "choice={choice}: {}", r.content);
            let records = read_records(tmp.path());
            assert_eq!(records.last().unwrap()["choice"], *choice);
            assert_eq!(records.len(), 2 + i, "one line per accepted call");
        }

        // Non-outcome classes keep the free-string choice — prose rides.
        let mut free = sample_input();
        free["choice"] = json!("kimi-required — resume with a fresh goal");
        let r = dispatch(&ctx, "decision_log", &free);
        assert!(!r.is_error, "{}", r.content);
        let records = read_records(tmp.path());
        assert_eq!(
            records.last().unwrap()["choice"],
            "kimi-required — resume with a fresh goal"
        );
    }

    /// Req 1, layering leg: the enum check sits AFTER the type legs — a
    /// missing or non-string `choice` on an outcome record is named by its
    /// own type leg (the T88 shape), never by the enum, so the correction
    /// message stays single-purpose.
    #[test]
    fn decision_log_outcome_choice_type_legs_fire_before_the_enum() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        input["class"] = json!("outcome");
        input.as_object_mut().unwrap().remove("choice");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(
            r.content.contains("choice must be a string, got missing"),
            "{}",
            r.content
        );
        assert!(!r.content.contains("outcome choice must be one of"), "{}", r.content);

        input["choice"] = json!(7);
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("choice must be a string, got number"), "{}", r.content);
        assert!(!r.content.contains("outcome choice must be one of"), "{}", r.content);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    // ---------- T199 outcome subject lint ----------

    /// Req 2: the advisory subject lint. A subject that IS an existing id
    /// resolves silently; an unknown id, a TODO number, and a composite
    /// subject embedding a real id each get a `note:` line on the return
    /// text — the return stays non-error and the record stays landed.
    /// Non-outcome classes are never linted (their subjects are prose).
    #[test]
    fn decision_log_outcome_subject_lint_notes_only_unresolved_ids() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let prior = dispatch(&ctx, "decision_log", &sample_input());
        assert!(!prior.is_error);
        let known_id = prior
            .content
            .strip_prefix("recorded ")
            .expect("result echoes the id")
            .to_string();

        // Known id: no note, return text unchanged.
        let outcome = |subject: Value| {
            let mut o = sample_input();
            o["class"] = json!("outcome");
            o["subject"] = subject;
            o["options"] = json!("landed-clean | fixed-up | reverted");
            o["choice"] = json!("fixed-up");
            o
        };
        let r = dispatch(&ctx, "decision_log", &outcome(json!(known_id)));
        assert!(!r.is_error, "{}", r.content);
        let landed_id = read_records(tmp.path())[1]["id"].as_str().unwrap().to_string();
        assert_eq!(r.content, format!("recorded {landed_id}"));

        // Unknown id: the note line, naming the subject and the corpus path.
        let r = dispatch(&ctx, "decision_log", &outcome(json!("d9999999999-99")));
        assert!(!r.is_error, "the lint is advisory, never an error: {}", r.content);
        let landed_id =
            read_records(tmp.path())[2]["id"].as_str().unwrap().to_string();
        assert_eq!(
            r.content,
            format!(
                "recorded {landed_id}\nnote: subject d9999999999-99 not found in {}",
                log_path(tmp.path()).display()
            )
        );
        assert!(r.content.ends_with("/.chug/decisions.jsonl"), "{}", r.content);
        assert_eq!(read_records(tmp.path()).len(), 3, "the record still landed");

        // A composite subject embedding a REAL id still does not resolve:
        // the subject must BE the id (exact equality — the same definition
        // scripts/decisions-audit.sh applies), so the note fires.
        let composite = format!("{known_id} plus recovery routing context");
        let r = dispatch(&ctx, "decision_log", &outcome(json!(composite.clone())));
        assert!(!r.is_error, "{}", r.content);
        assert!(
            r.content.contains(&format!("\nnote: subject {composite} not found in ")),
            "{}",
            r.content
        );

        // Non-outcome classes are never linted: a prose subject on a
        // validation-routing record gets no note.
        let mut prose = sample_input();
        prose["subject"] = json!("T199 arc (routing d9999999999-99)");
        let r = dispatch(&ctx, "decision_log", &prose);
        assert!(!r.is_error, "{}", r.content);
        assert!(!r.content.contains("note:"), "{}", r.content);
    }

    /// Req 2, silent-skip leg: a corpus file that exists but cannot be READ
    /// (write-only, mode 0o222) must not turn the landed record into a
    /// failure — the append proceeds, the lint skips silently, no note.
    #[test]
    fn decision_log_outcome_subject_lint_skips_silently_when_corpus_is_unreadable() {
        let tmp = TempDir::new().unwrap();
        let chug = tmp.path().join(".chug");
        fs::create_dir_all(&chug).unwrap();
        let corpus = chug.join("decisions.jsonl");
        fs::write(&corpus, b"").unwrap();
        let mut perms = fs::metadata(&corpus).unwrap().permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o222);
        fs::set_permissions(&corpus, perms).unwrap();

        let ctx = tool_ctx(tmp.path());
        let mut outcome = sample_input();
        outcome["class"] = json!("outcome");
        outcome["subject"] = json!("d9999999999-99");
        outcome["choice"] = json!("reverted");
        let r = dispatch(&ctx, "decision_log", &outcome);
        assert!(!r.is_error, "write proceeds, lint skips: {}", r.content);
        assert!(
            !r.content.contains("note:"),
            "no note from an unreadable corpus: {}",
            r.content
        );
        assert!(r.content.starts_with("recorded d"), "{}", r.content);
        // The record really did land (append needs only write permission).
        let meta = fs::metadata(&corpus).unwrap();
        assert!(meta.len() > 0, "the outcome record was appended");
    }

    // ---------- validation legs ----------

    #[test]
    fn decision_log_validation_legs_name_the_field() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());

        // Missing string field.
        let mut input = sample_input();
        input.as_object_mut().unwrap().remove("choice");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("choice"), "{}", r.content);

        // Non-string field.
        let mut input = sample_input();
        input["options"] = json!(42);
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("options"), "{}", r.content);

        // confidence out of range on both sides, as a string, and missing.
        let bad_confidences = vec![json!(-0.1), json!(1.1), json!("high")];
        for bad in &bad_confidences {
            let mut input = sample_input();
            input["confidence"] = bad.clone();
            let r = dispatch(&ctx, "decision_log", &input);
            assert!(r.is_error, "confidence={bad}: {}", r.content);
            assert!(
                r.content.contains("confidence"),
                "confidence={bad}: {}",
                r.content
            );
        }
        let mut input = sample_input();
        input.as_object_mut().unwrap().remove("confidence");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("confidence"), "{}", r.content);

        // Failed calls never write: the log does not even exist yet.
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    #[test]
    fn decision_log_confidence_boundaries_zero_and_one_accepted() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        for c in [0.0, 1.0] {
            let mut input = sample_input();
            input["confidence"] = json!(c);
            let r = dispatch(&ctx, "decision_log", &input);
            assert!(!r.is_error, "confidence={c}: {}", r.content);
        }
        let records = read_records(tmp.path());
        assert_eq!(records.len(), 2, "0.0 and 1.0 are both accepted");
        assert_eq!(records[0]["confidence"], 0.0);
        assert_eq!(records[1]["confidence"], 1.0);
        let text = fs::read_to_string(log_path(tmp.path())).unwrap();
        assert!(text.contains("\"confidence\":0.0"), "{text}");
        assert!(text.contains("\"confidence\":1.0"), "{text}");
    }

    // ---------- free-string classes ----------

    #[test]
    fn decision_log_free_string_class_needs_no_code_change() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        input["class"] = json!("retry-routing");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(!r.is_error, "{}", r.content);
        let records = read_records(tmp.path());
        assert_eq!(records[0]["class"], "retry-routing");
    }

    // ---------- best-effort ----------

    #[test]
    fn decision_log_best_effort_file_where_chug_dir_should_be_is_a_tool_error() {
        // A FILE sitting where `.chug/` should be created: the write fails,
        // the tool returns an error (never panics), and the run survives.
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join(".chug"), b"i am a file, not a directory").unwrap();
        let ctx = tool_ctx(tmp.path());
        let r = dispatch(&ctx, "decision_log", &sample_input());
        assert!(r.is_error, "{}", r.content);
        assert!(
            r.content.contains("decision-log dir") && r.content.contains(".chug"),
            "error must name the failure site: {}",
            r.content
        );
        assert!(
            !tmp.path().join(".chug/decisions.jsonl").exists(),
            "no log file can exist under a file-shaped .chug"
        );
    }

    // ---------- T88 corrective validation legs ----------

    /// Assert every needle occurs in `content`, each strictly after the
    /// previous one — pins the schema-order listing of legs without pinning
    /// the full prose.
    fn assert_named_in_order(content: &str, needles: &[&str]) {
        let mut cursor = 0;
        for needle in needles {
            let at = content[cursor..]
                .find(needle)
                .unwrap_or_else(|| panic!("{needle:?} missing or out of order in {content}"));
            cursor += at + needle.len();
        }
    }

    /// The contract reminder's required-list, verbatim (T88 reqs 2-3).
    const REQUIRED_LIST: &str = "required: class, subject, inputs, options, choice, confidence";

    /// Multi-missing: only `{class, subject}` present — the error names ALL
    /// four invalid fields with their expected shapes, in schema order, and
    /// does NOT diagnose the two fields that are fine.
    #[test]
    fn decision_log_multi_missing_names_every_invalid_field_in_schema_order() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        for field in ["inputs", "options", "choice", "confidence"] {
            input.as_object_mut().unwrap().remove(field);
        }
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        // Exact pin: every leg in REQUIRED_FIELDS order, one message —
        // ending with the always-on contract reminder (T182).
        assert_eq!(
            r.content,
            "tool error: invalid decision_log call: inputs must be a string, got missing; \
             options must be a string, got missing; choice must be a string, got missing; \
             confidence must be a number in 0..=1, got missing; one record per call; \
             required: class, subject, inputs, options, choice, confidence"
        );
        assert_named_in_order(&r.content, &["inputs", "options", "choice", "confidence"]);
        assert!(r.content.contains(REQUIRED_LIST), "{}", r.content);
        assert!(!r.content.contains("class must be"), "{}", r.content);
        assert!(!r.content.contains("subject must be"), "{}", r.content);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// T182: the 5-of-6 fumble shape — exactly one required field missing,
    /// ALL keys recognized — previously took the legs-only path and never
    /// re-stated the contract, so the retry could drop a DIFFERENT field
    /// (the cycle-83 census: 5 instances in 3 cycles). Every failure shape
    /// now carries the full required-field list.
    #[test]
    fn decision_log_five_of_six_fields_still_carries_the_contract_reminder() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        input.as_object_mut().unwrap().remove("options");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        // Exact pin: the leg names the ONE missing field, and the message
        // ends with the full contract (T182) — no received-keys echo.
        assert_eq!(
            r.content,
            "tool error: invalid decision_log call: options must be a string, got missing; \
             one record per call; required: class, subject, inputs, options, choice, confidence"
        );
        assert_named_in_order(
            &r.content,
            &["options must be a string, got missing", REQUIRED_LIST],
        );
        // Merely-missing-field calls need the contract, not their own keys
        // echoed: the received-keys list stays unknown-keys-only.
        assert!(!r.content.contains("received keys"), "{}", r.content);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Wrapper leg (the cycle-47 FATAL shape): a batched `{"records": [...]}`
    /// call gets the unknown-shape diagnosis — the received key, the
    /// one-record-per-call reminder, and the full required list — plus every
    /// field's expected shape, all in ONE error.
    #[test]
    fn decision_log_batched_wrapper_diagnoses_unknown_shape_with_contract_reminder() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let input = json!({ "records": [sample_input(), sample_input()] });
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        // Exact pin of the whole corrective message for the fatal shape.
        assert_eq!(
            r.content,
            "tool error: invalid decision_log call: class must be a string, got missing; \
             subject must be a string, got missing; inputs must be a string, got missing; \
             options must be a string, got missing; choice must be a string, got missing; \
             confidence must be a number in 0..=1, got missing; received keys: [records]; \
             one record per call; required: class, subject, inputs, options, choice, confidence"
        );
        assert!(r.content.contains("received keys: [records]"), "{}", r.content);
        assert!(r.content.contains("one record per call"), "{}", r.content);
        assert!(r.content.contains(REQUIRED_LIST), "{}", r.content);
        // Req 1 still holds under the shape diagnosis: every field named.
        assert!(r.content.contains("class must be a string, got missing"), "{}", r.content);
        assert!(
            r.content.contains("confidence must be a number in 0..=1, got missing"),
            "{}",
            r.content
        );
        // Reminder appended after the legs and the received keys.
        assert_named_in_order(&r.content, &["received keys: [records]", "one record per call"]);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Alias leg: the events.jsonl-style mistake — `type` where `class`
    /// belongs, everything else valid — names `class` missing AND lists the
    /// received keys (sorted), so the alias itself is visible.
    #[test]
    fn decision_log_aliased_class_key_names_class_missing_and_the_received_type_key() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        {
            let obj = input.as_object_mut().unwrap();
            obj.remove("class");
            obj.insert("type".into(), json!("eval-triage"));
        }
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("class must be a string, got missing"), "{}", r.content);
        // Received keys, sorted: the aliased `type` rides the list.
        assert!(
            r.content.contains("received keys: [choice, confidence, inputs, options, subject, type]"),
            "{}",
            r.content
        );
        assert!(r.content.contains(REQUIRED_LIST), "{}", r.content);
        // The five valid fields are not diagnosed.
        for other in ["subject", "inputs", "options", "choice"] {
            assert!(
                !r.content.contains(&format!("{other} must be")),
                "{other} leaked: {}",
                r.content
            );
        }
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Non-object leg (req 3): a top-level array / string / number / bool /
    /// null is a tool error naming the received JSON type plus the full
    /// required list — no field-by-field pretense.
    #[test]
    fn decision_log_non_object_input_names_the_json_type_and_required_list() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        // Exact pin for the spec's `[]` leg.
        let r = dispatch(&ctx, "decision_log", &json!([]));
        assert!(r.is_error, "{}", r.content);
        assert_eq!(
            r.content,
            "tool error: invalid decision_log call: expected a JSON object, got array; \
             one record per call; required: class, subject, inputs, options, choice, confidence"
        );
        for (input, type_name) in [
            (json!(["one"]), "array"),
            (json!("eval-triage"), "string"),
            (json!(42), "number"),
            (json!(true), "boolean"),
            (json!(null), "null"),
        ] {
            let r = dispatch(&ctx, "decision_log", &input);
            assert!(r.is_error, "{type_name}: {}", r.content);
            assert!(
                r.content.contains(&format!("got {type_name}")),
                "{type_name}: {}",
                r.content
            );
            assert!(
                r.content.contains("one record per call") && r.content.contains(REQUIRED_LIST),
                "{type_name}: {}",
                r.content
            );
        }
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Single-missing regression pins: each of the six fields omitted ALONE
    /// is still named, and alone — no regression to silence, no
    /// over-diagnosis of the five valid fields.
    #[test]
    fn decision_log_each_field_missing_alone_is_still_named() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let string_fields = ["class", "subject", "inputs", "options", "choice"];
        for field in string_fields {
            let mut input = sample_input();
            input.as_object_mut().unwrap().remove(field);
            let r = dispatch(&ctx, "decision_log", &input);
            assert!(r.is_error, "{field}: {}", r.content);
            assert!(
                r.content.contains(&format!("{field} must be a string, got missing")),
                "{field}: {}",
                r.content
            );
            // T182: the always-on contract reminder rides this shape too.
            assert!(r.content.contains(REQUIRED_LIST), "{field}: {}", r.content);
            for other in string_fields {
                if other != field {
                    assert!(
                        !r.content.contains(&format!("{other} must be")),
                        "{field} alone leaked {other}: {}",
                        r.content
                    );
                }
            }
            assert!(
                !r.content.contains("confidence must be"),
                "{field} alone leaked confidence: {}",
                r.content
            );
        }
        let mut input = sample_input();
        input.as_object_mut().unwrap().remove("confidence");
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(
            r.content.contains("confidence must be a number in 0..=1, got missing"),
            "{}",
            r.content
        );
        // T182: the always-on contract reminder rides this shape too.
        assert!(r.content.contains(REQUIRED_LIST), "{}", r.content);
        for other in string_fields {
            assert!(
                !r.content.contains(&format!("{other} must be")),
                "confidence alone leaked {other}: {}",
                r.content
            );
        }
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Mixed-invalid leg: `confidence: "high"` AND `options` missing are
    /// named together, in schema order (options before confidence).
    #[test]
    fn decision_log_mixed_invalid_names_all_bad_fields_in_schema_order() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        {
            let obj = input.as_object_mut().unwrap();
            obj.remove("options");
            obj.insert("confidence".into(), json!("high"));
        }
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("options must be a string, got missing"), "{}", r.content);
        assert!(
            r.content.contains("confidence must be a number in 0..=1, got string"),
            "{}",
            r.content
        );
        assert_named_in_order(&r.content, &["options must be", "confidence must be"]);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Req 5: the confidence range message text is preserved verbatim —
    /// alone (as the whole message) and alongside another invalid field.
    #[test]
    fn decision_log_confidence_range_message_preserved_alone_and_alongside() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        input["confidence"] = json!(1.1);
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(
            r.content.contains("confidence must be a number in 0..=1, got 1.1"),
            "{}",
            r.content
        );
        // Out-of-range alone: the shape is known, so no received-keys hint.
        assert!(!r.content.contains("received keys"), "{}", r.content);

        let mut input = sample_input();
        input["class"] = json!(7);
        input["confidence"] = json!(-0.1);
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(r.content.contains("class must be a string, got number"), "{}", r.content);
        assert!(
            r.content.contains("confidence must be a number in 0..=1, got -0.1"),
            "{}",
            r.content
        );
        assert_named_in_order(&r.content, &["class must be", "confidence must be"]);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Req 2's "first 8, sorted" cap: nine unrecognized keys list the first
    /// eight, sorted — enough to diagnose, bounded so a garbage call cannot
    /// dump an unbounded key list into the transcript.
    #[test]
    fn decision_log_received_keys_capped_at_first_eight_sorted() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let input = json!({
            "zeta": 1, "alpha": 1, "mu": 1, "beta": 1, "kappa": 1,
            "gamma": 1, "nu": 1, "delta": 1, "omega": 1
        });
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(r.is_error, "{}", r.content);
        assert!(
            r.content
                .contains("received keys: [alpha, beta, delta, gamma, kappa, mu, nu, omega]"),
            "{}",
            r.content
        );
        assert!(!r.content.contains("zeta"), "ninth key dropped: {}", r.content);
        assert!(!log_path(tmp.path()).exists(), "validation failures must not write");
    }

    /// Req 4: the unknown-shape diagnosis fires on FAILURE legs only — extra
    /// keys on an otherwise-valid record are ignored exactly as before, and
    /// the record lands byte-identically.
    #[test]
    fn decision_log_extra_keys_on_valid_record_still_succeed_unchanged() {
        let tmp = TempDir::new().unwrap();
        let ctx = tool_ctx(tmp.path());
        let mut input = sample_input();
        {
            let obj = input.as_object_mut().unwrap();
            obj.insert("records".into(), json!([]));
            obj.insert("type".into(), json!("eval-triage"));
        }
        let r = dispatch(&ctx, "decision_log", &input);
        assert!(!r.is_error, "{}", r.content);
        assert!(r.content.starts_with("recorded d"), "{}", r.content);
        let records = read_records(tmp.path());
        assert_eq!(records.len(), 1, "exactly one line appended");
        assert_eq!(records[0]["class"], "validation-routing");
        assert_eq!(records[0]["subject"], "T70");
        assert_eq!(records[0]["confidence"], 0.9);
    }
}

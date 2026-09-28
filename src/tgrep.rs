//! T76 — `tgrep`: token-budgeted ranked context retrieval.
//!
//! Context economics (operator 2026-09-26): per-call context is dominated by
//! TOOL RESULT bulk — children routinely read whole files (driver.rs ~40k
//! tokens) to find 20 relevant lines. `grep` returns raw matches with no
//! ranking, no context windows, and no cap: a 200-hit grep dumps everything.
//! `tgrep` is the fix class the operator named ("tgrep or something to aid in
//! getting relevant context"):
//!
//! - **Ranked clusters**: every hit anchors a ±3-line window; overlapping
//!   windows merge into ONE cluster; clusters are scored and returned
//!   best-first with a per-cluster score.
//! - **Token budget**: the output stops at `budget` (default
//!   [`TGREP_DEFAULT_BUDGET`] tokens, clamped down to [`TGREP_BUDGET_CEILING`])
//!   with a `[more: N clusters omitted]` marker — the tool result can never
//!   blow up the transcript the way a wide grep or full-file read does.
//! - **Ranking** (deterministic, hand-rolled, no embeddings): exact-phrase
//!   beats all-terms-in-window beats term density; a path-basename boost
//!   breaks density ties. No LLM, no I/O beyond reading the matched tree.
//! - **`symbols` mode**: `symbols: true` with a Rust file in `path` returns a
//!   fn/struct/impl signature skeleton (no bodies) — orientation before a
//!   deep `read_file`. Line-oriented heuristics, not a parser.
//!
//! The taught workflow (in the schema description, per T22/T41 doctrine):
//! tgrep to locate, then a targeted `read_file` with `offset`/`limit` around
//! the best cluster — never a whole-file read to find 20 lines.
//!
//! Module shape follows the T37 webfetch.rs / T70 decisions.rs pattern: ALL
//! logic lives here, and src/tools.rs carries only the two registration
//! lines (schema push + dispatch arm) so the tools.rs monolith does not grow.
//! No new dependencies: the walker, ranking, and budget packing are
//! hand-rolled on std + the `glob` crate chug already ships.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail};
use serde_json::{json, Value};

use crate::tools::{confine_glob_match, resolve_glob_pattern, resolve_safe, GLOB_METACHARS, ToolCtx, ToolResult};

/// Default output budget in tokens (~8000 chars of cluster text).
pub const TGREP_DEFAULT_BUDGET: usize = 2000;
/// Hard ceiling on `budget`; larger requests are clamped down, never
/// rejected (spec req 1: "budget (default 2000 tokens, clamp 8k)").
pub const TGREP_BUDGET_CEILING: usize = 8000;
/// Lines shown on each side of a match (spec req 1: "±3-line window").
pub const TGREP_WINDOW: usize = 3;
/// Files searched per call (bound on walk time/memory).
pub const TGREP_MAX_FILES: usize = 20_000;
/// A file larger than this is skipped (search is for code, not data dumps).
pub const TGREP_MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Total bytes read per call (walk cut-off; the output notes it).
pub const TGREP_MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
/// Longest rendered signature line in symbols mode, before the ellipsis.
const SYMBOL_SIG_MAX_CHARS: usize = 200;
/// Extra continuation lines joined into one signature (multi-line fn sigs).
const SYMBOL_SIG_LOOKAHEAD: usize = 96;
/// Chars reserved for the omission marker when packing, so header + clusters
/// + marker together never exceed the budget.
///
/// Honest bound (validator M-K edge): the widest reachable marker is
/// `"[more: N clusters omitted] — raise `budget` or narrow `path`"` = 59 +
/// digits(N). N (omitted clusters) can never exceed the searched line count,
/// and the byte caps bound that: 64 MiB / 1 byte-per-line = 67,108,864 lines
/// → 8 digits → marker ≤ 67 chars < 96. The symbols-side marker (`"…
/// narrow the file"`) over a ≤ 1 MiB file is ≤ 67 too. A hand-wavy "~69-char
/// overflow edge" was never reachable; the arithmetic here is the proof.
const MARKER_RESERVE_CHARS: usize = 96;

/// Directories never searched. `target*` covers `target/` and the T47/T52
/// role-keyed build caches (`target-shared*`) that live inside checkouts.
fn is_skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || name == "node_modules"
        || name == "target"
        || name.starts_with("target-")
}

/// JSON schema for the `tgrep` tool, registered alongside the builtins.
pub fn schema() -> Value {
    json!({
        "name": "tgrep",
        "description": "Token-budgeted ranked context search — find WHERE to look before reading. `query` is one or more terms (ranked AND-ish: more matching terms rank higher; \"quoted phrases\" must appear exactly). Returns ranked match CLUSTERS: file:line + ±3-line window, best-first, each with a score; the output stops at `budget` (default 2000 tokens, a larger value is clamped to 8000) with `[more: N clusters omitted]`. Optional `path` narrows the search to a glob (e.g. `src/**/*.rs`), a directory, or a single file; default is the whole cwd. `symbols: true` treats `path` as a single Rust file and returns its fn/struct/impl signature skeleton (no bodies) for orientation — `query` is ignored in that mode. Deterministic ranking (no embeddings, no LLM). Workflow: tgrep to locate, then a targeted read_file with offset/limit around the best cluster — never read whole files to find 20 relevant lines. Paths outside the cwd are refused (`path escapes cwd`); cross-tree reads go through `bash`.",
        "input_schema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "One or more terms, ranked AND-ish; a \"quoted phrase\" must appear exactly. Required for search; ignored when symbols=true"},
                "path": {"type": "string", "description": "Optional glob (e.g. `src/**/*.rs`), directory, or file relative to cwd; default the whole cwd. Glob matches resolving outside cwd via a symlink are dropped, never read (T134 F1). In symbols mode: the Rust file to skeleton"},
                "budget": {"type": "integer", "description": "Output token budget (default 2000; a request above 8000 is clamped down, not rejected)"},
                "symbols": {"type": "boolean", "description": "true = signature skeleton mode: `path` must be a single Rust file (.rs); `query` is ignored"}
            },
            "required": ["query"]
        }
    })
}

/// Dispatch entry for the `tgrep` arm in `tools::inner`.
pub fn tgrep(ctx: &ToolCtx, input: &Value) -> anyhow::Result<ToolResult> {
    let budget = budget_from(input)?;
    if input.get("symbols").and_then(Value::as_bool).unwrap_or(false) {
        let raw = input
            .get("path")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("tgrep symbols mode requires `path`: the Rust file to skeleton"))?;
        let path = resolve_safe(&ctx.cwd, raw).map_err(|e| anyhow!("{e}"))?;
        symbols_skeleton(&path, &ctx.cwd, raw, budget)
    } else {
        let raw_query = input
            .get("query")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("missing or non-string field: query"))?;
        let query = parse_query(raw_query)
            .ok_or_else(|| anyhow!("tgrep: `query` is empty — give one or more terms"))?;
        let corpus = collect_corpus(ctx, input)?;
        search(&query, raw_query, corpus, budget)
    }
}

/// Parse the `budget` field: absent → default; non-integer or < 1 → tool
/// error naming the constraint (T39 delegate `max_tokens` precedent);
/// above the ceiling → clamped down (webfetch `max_chars` precedent).
fn budget_from(input: &Value) -> anyhow::Result<usize> {
    match input.get("budget") {
        None | Some(Value::Null) => Ok(TGREP_DEFAULT_BUDGET),
        Some(v) => {
            let n = v.as_u64().ok_or_else(|| {
                anyhow!("tgrep: `budget` must be an integer token count (default {TGREP_DEFAULT_BUDGET})")
            })?;
            if n < 1 {
                bail!("tgrep: `budget` must be at least 1 token, got {n}");
            }
            Ok(n.min(TGREP_BUDGET_CEILING as u64) as usize)
        }
    }
}

/// The parsed query: quoted phrases (exact substring), then the remaining
/// terms (lowercased, distinct). When nothing is quoted and more than one
/// term was given, the whole query string is ALSO a phrase candidate — that
/// is the "exact-phrase beats scatter" signal for plain multi-term queries.
#[derive(Debug, Clone)]
struct Query {
    phrases: Vec<String>,
    terms: Vec<String>,
}

fn parse_query(raw: &str) -> Option<Query> {
    let mut phrases = Vec::new();
    let mut rest = String::new();
    let mut remaining = raw;
    while let Some(start) = remaining.find('"') {
        rest.push_str(&remaining[..start]);
        let after = &remaining[start + 1..];
        match after.find('"') {
            Some(end) => {
                let phrase = after[..end].trim().to_lowercase();
                if !phrase.is_empty() {
                    phrases.push(phrase);
                }
                remaining = &after[end + 1..];
            }
            None => {
                // Unterminated quote: treat the rest as plain terms.
                rest.push_str(after);
                remaining = "";
            }
        }
    }
    rest.push_str(remaining);

    let mut terms = Vec::new();
    for word in rest.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
        let w = word.trim().to_lowercase();
        if !w.is_empty() && !terms.contains(&w) {
            terms.push(w);
        }
    }
    if phrases.is_empty() && terms.len() > 1 {
        // The whole query is the exact-phrase candidate; quote characters
        // are phrase syntax, not phrase content.
        phrases.push(raw.trim().to_lowercase().replace('"', ""));
    }
    if phrases.is_empty() && terms.is_empty() {
        return None;
    }
    // Single-term query: the term is its own exact phrase (uniform tier —
    // every cluster window contains it — so it only affects labels).
    if phrases.is_empty() && terms.len() == 1 {
        phrases.push(terms[0].clone());
    }
    Some(Query { phrases, terms })
}

/// The files one call searches, with an honesty note when a cap cut the
/// corpus short.
struct Corpus {
    files: Vec<PathBuf>,
    capped_note: Option<String>,
}

/// Resolve the optional `path` (glob / directory / single file) or walk the
/// whole cwd, applying the size caps. Lexical path safety first (`path
/// escapes cwd`), same as the other file tools.
fn collect_corpus(ctx: &ToolCtx, input: &Value) -> anyhow::Result<Corpus> {
    match input.get("path").and_then(Value::as_str) {
        None => {
            let mut files = Vec::new();
            let mut stats = WalkStats::default();
            walk_dir(&ctx.cwd, &mut files, &mut stats);
            let capped_note = walk_note(&files, &stats);
            Ok(Corpus { files, capped_note })
        }
        Some(raw) => {
            // Pattern vs literal, decided BEFORE resolution (T134 F1): a
            // metachar path is a glob to expand, not an inert-missing
            // literal — resolve_safe used to pass `**/passwd` through
            // stage 2's missing-arm and the expansion followed an in-tree
            // symlink to an external directory.
            if raw.contains(&GLOB_METACHARS[..]) {
                let pattern = resolve_glob_pattern(&ctx.cwd, raw)
                    .map_err(|e| anyhow!("{e}"))?
                    .to_string_lossy()
                    .into_owned();
                let mut files: Vec<PathBuf> = Vec::new();
                let mut oversized = 0usize;
                for p in glob::glob(&pattern)
                    .map_err(|e| anyhow!("invalid glob pattern: {e}"))?
                    .filter_map(|p| p.ok())
                {
                    // The glob crate follows symlinked directories during
                    // expansion, so every concrete match must re-pass the
                    // full two-stage confinement before a byte of it is
                    // read (T134 F1) — matches resolving outside are
                    // dropped, never surfaced.
                    if !confine_glob_match(&ctx.cwd, &p) {
                        continue;
                    }
                    if !p.is_file() {
                        continue;
                    }
                    // Same size cap the walk arm applies — the glob arm used
                    // to read multi-MB data dumps the walk arm would have
                    // skipped (NB-b: caps must be consistent per path kind).
                    let len = fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if len > TGREP_MAX_FILE_BYTES {
                        oversized += 1;
                        continue;
                    }
                    files.push(p);
                }
                let capped_note = sized_note(
                    (files.len() > TGREP_MAX_FILES).then(|| {
                        files.truncate(TGREP_MAX_FILES);
                        format!("[search truncated at the {TGREP_MAX_FILES} file cap]")
                    }),
                    oversized,
                );
                return Ok(Corpus { files, capped_note });
            }
            let resolved = resolve_safe(&ctx.cwd, raw).map_err(|e| anyhow!("{e}"))?;
            if resolved.is_file() {
                // Single-file path honors the same size cap as walk/glob.
                let len = fs::metadata(&resolved).map(|m| m.len()).unwrap_or(0);
                if len > TGREP_MAX_FILE_BYTES {
                    return Ok(Corpus {
                        files: Vec::new(),
                        capped_note: Some(format!(
                            "[skipped {raw}: {len} bytes is over the {TGREP_MAX_FILE_BYTES}-byte file cap]"
                        )),
                    });
                }
                return Ok(Corpus {
                    files: vec![resolved],
                    capped_note: None,
                });
            }
            if resolved.is_dir() {
                let mut files = Vec::new();
                let mut stats = WalkStats::default();
                walk_dir(&resolved, &mut files, &mut stats);
                let capped_note = walk_note(&files, &stats);
                return Ok(Corpus { files, capped_note });
            }
            bail!("tgrep path not found: {raw}")
        }
    }
}

#[derive(Default)]
struct WalkStats {
    bytes: u64,
    capped: bool,
    oversized: usize,
}

/// The honesty note for a walked corpus: truncation cap and/or oversized
/// files skipped — the same size-cap accounting the glob and single-file
/// arms now apply (NB-b consistency).
fn walk_note(files: &[PathBuf], stats: &WalkStats) -> Option<String> {
    let trunc = stats
        .capped
        .then(|| format!("[search truncated at the {} file / byte cap]", files.len()));
    sized_note(trunc, stats.oversized)
}

/// Compose truncation + oversized-skip notes into one honesty line.
fn sized_note(trunc: Option<String>, oversized: usize) -> Option<String> {
    let size_note = (oversized > 0).then(|| {
        format!("[skipped {oversized} file(s) over the {TGREP_MAX_FILE_BYTES}-byte size cap]")
    });
    match (trunc, size_note) {
        (Some(a), Some(b)) => Some(format!("{a} {b}")),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Depth-first walk, sorted per directory for determinism, skipping
/// [`is_skip_dir`] directories. Caps stop the walk early (and are reported).
fn walk_dir(dir: &Path, out: &mut Vec<PathBuf>, stats: &mut WalkStats) {
    if out.len() >= TGREP_MAX_FILES || stats.bytes >= TGREP_MAX_TOTAL_BYTES {
        stats.capped = true;
        return;
    }
    let mut entries: Vec<std::fs::DirEntry> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return, // unreadable dir: skip silently, keep searching
    };
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= TGREP_MAX_FILES || stats.bytes >= TGREP_MAX_TOTAL_BYTES {
            stats.capped = true;
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            if is_skip_dir(&name) {
                continue;
            }
            walk_dir(&entry.path(), out, stats);
        } else if ft.is_file() {
            let len = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if len > TGREP_MAX_FILE_BYTES {
                stats.oversized += 1;
                continue;
            }
            stats.bytes += len;
            out.push(entry.path());
        }
    }
}

/// Ranking tiers, in spec order: exact-phrase > all-terms-in-window >
/// term density. The tier bases keep the tiers disjoint no matter what the
/// bonus terms add (see [`score_window`]).
const TIER_PHRASE: f64 = 60.0;
const TIER_ALL_TERMS: f64 = 30.0;
const TIER_PARTIAL: f64 = 0.0;

/// Score one cluster window (lowercased text) against the query.
///
/// `score = tier_base + density*10 + min(occurrences,10)*0.5 +
/// min(basename_terms,3)*2`. Tier maxima stay disjoint: partial < 30 (its
/// density is < 1 by definition), all-terms >= 30, phrase >= 60.
fn score_window(query: &Query, window_lc: &str, basename_lc: &str) -> (String, f64) {
    let phrase_hits = query
        .phrases
        .iter()
        .filter(|p| window_lc.contains(p.as_str()))
        .count();
    let mut present = 0usize;
    let mut occurrences = 0usize;
    for term in &query.terms {
        let count = count_occurrences(window_lc, term);
        if count > 0 {
            present += 1;
            occurrences += count;
        }
    }
    let basename_hits = query
        .terms
        .iter()
        .filter(|t| basename_lc.contains(t.as_str()))
        .count();

    let base = if phrase_hits > 0 {
        TIER_PHRASE
    } else if !query.terms.is_empty() && present == query.terms.len() {
        TIER_ALL_TERMS
    } else {
        TIER_PARTIAL
    };
    let density = if query.terms.is_empty() {
        0.0
    } else {
        present as f64 / query.terms.len() as f64
    };
    let label = if phrase_hits > 0 {
        "exact-phrase"
    } else if base == TIER_ALL_TERMS {
        "all-terms"
    } else {
        "partial"
    };
    let score = base
        + density * 10.0
        + (occurrences.min(10) as f64) * 0.5
        + (basename_hits.min(3) as f64) * 2.0;
    (label.to_string(), score)
}

/// Non-overlapping occurrence count of `needle` in `hay` (both lowercase).
fn count_occurrences(hay: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    hay.matches(needle).count()
}

/// One ranked match cluster: the merged ±3-line window around one or more
/// overlapping hits, plus its score.
struct Cluster {
    file: PathBuf,
    start: usize,  // 1-based, inclusive
    end: usize,    // 1-based, inclusive
    anchor: usize, // 1-based line the cluster is named by
    label: String,
    score: f64,
}

/// Line-indexed lowercase view of one file, shared by matching and scoring.
struct FileText {
    path: PathBuf,
    lc_lines: Vec<String>,
}

impl FileText {
    /// Lines (1-based) containing any term or phrase — cluster seeds.
    fn match_lines(&self, query: &Query) -> Vec<usize> {
        let mut hits = Vec::new();
        for (idx, line) in self.lc_lines.iter().enumerate() {
            let hit = query.terms.iter().any(|t| line.contains(t.as_str()))
                || query.phrases.iter().any(|p| line.contains(p.as_str()));
            if hit {
                hits.push(idx + 1);
            }
        }
        hits
    }

    fn window_text(&self, start: usize, end: usize) -> String {
        self.lc_lines[start - 1..end].join("\n")
    }
}

/// Build the merged clusters for one file: overlapping ±3-line windows merge
/// transitively into ONE cluster (spec test: "overlapping windows merge
/// once"). The anchor is the seed line with the most term/phrase hits
/// (ties → lowest line), so the cluster is named by its densest line.
fn clusters_for_file(ft: &FileText, query: &Query) -> Vec<Cluster> {
    let seeds = ft.match_lines(query);
    if seeds.is_empty() {
        return Vec::new();
    }
    let line_count = ft.lc_lines.len();
    let basename_lc = ft
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut clusters = Vec::new();
    let mut idx = 0;
    while idx < seeds.len() {
        // Extend while the next seed's emitted window would overlap or touch
        // this cluster's: cluster window ends at `last + W`, the next seed's
        // window starts at `seed - W` — overlap iff `seed <= last + 2W`,
        // adjacency iff `seed == last + 2W + 1`. Merging on match-line
        // distance alone (the pre-fix `<= last + W`) left hits 4-6 lines
        // apart in two clusters whose windows duplicated lines (validator
        // probe P1); the radius is the WINDOW-OVERLAP radius, not the hit
        // radius. Transitive extension keeps the merge once-only.
        let first = seeds[idx];
        let mut last = first;
        let mut members = vec![first];
        while idx + 1 < seeds.len()
            && seeds[idx + 1] <= last + 2 * TGREP_WINDOW + 1
        {
            idx += 1;
            last = seeds[idx];
            members.push(last);
        }
        idx += 1;
        let start = first.saturating_sub(TGREP_WINDOW).max(1);
        let end = (last + TGREP_WINDOW).min(line_count);
        let anchor = *members
            .iter()
            .max_by_key(|&&m| {
                (
                    seed_hit_count(&ft.lc_lines[m - 1], query),
                    std::cmp::Reverse(m),
                )
            })
            .unwrap();
        let (label, score) = score_window(query, &ft.window_text(start, end), &basename_lc);
        clusters.push(Cluster {
            file: ft.path.clone(),
            start,
            end,
            anchor,
            label,
            score,
        });
    }
    clusters
}

/// Term + phrase occurrences on one (lowercased) seed line.
fn seed_hit_count(line_lc: &str, query: &Query) -> usize {
    query
        .terms
        .iter()
        .map(|t| count_occurrences(line_lc, t))
        .sum::<usize>()
        + query
            .phrases
            .iter()
            .map(|p| count_occurrences(line_lc, p))
            .sum::<usize>()
}

/// Rank clusters best-first: score desc, then path, then position — a
/// deterministic total order (no LLM, no embeddings; spec req 2).
fn rank_clusters(mut clusters: Vec<Cluster>) -> Vec<Cluster> {
    clusters.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.start.cmp(&b.start))
            .then_with(|| a.anchor.cmp(&b.anchor))
    });
    clusters
}

/// Render one cluster: `path:anchor score S (label)` then the window lines,
/// anchor marked with `>`.
fn render_cluster(ft_cache: &std::collections::HashMap<PathBuf, Vec<String>>, c: &Cluster) -> String {
    let lines = match ft_cache.get(&c.file) {
        Some(l) => l,
        None => return format!("{}:{} score {:.2} ({})\n  (unreadable)", c.file.display(), c.anchor, c.score, c.label),
    };
    let mut out = format!(
        "{}:{} score {:.2} ({})",
        c.file.display(),
        c.anchor,
        c.score,
        c.label
    );
    for l in c.start..=c.end {
        let text = lines.get(l - 1).map(|s| s.as_str()).unwrap_or("");
        let marker = if l == c.anchor { '>' } else { ' ' };
        out.push_str(&format!("\n{marker} {l} | {text}"));
    }
    out
}

/// The whole search path: load, match, merge, rank, render under budget.
fn search(
    query: &Query,
    raw_query: &str,
    corpus: Corpus,
    budget: usize,
) -> anyhow::Result<ToolResult> {
    let mut ranked: Vec<Cluster> = Vec::new();
    let mut raw_lines: std::collections::HashMap<PathBuf, Vec<String>> =
        std::collections::HashMap::new();
    let mut searched = 0usize;
    for path in &corpus.files {
        // Read once: lowercase lines for matching, original lines for render.
        let Ok(data) = fs::read_to_string(path) else {
            continue; // binary or unreadable: skip silently
        };
        searched += 1;
        let ft = FileText {
            path: path.clone(),
            lc_lines: data.lines().map(|l| l.to_lowercase()).collect(),
        };
        let file_clusters = clusters_for_file(&ft, query);
        if !file_clusters.is_empty() {
            raw_lines.insert(path.clone(), data.lines().map(|s| s.to_string()).collect());
        }
        ranked.extend(file_clusters);
    }
    let ranked = rank_clusters(ranked);
    let file_count = ranked
        .iter()
        .map(|c| c.file.clone())
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    let mut header = format!(
        "tgrep \"{raw_query}\": {} cluster{} in {} file{} (searched {searched} files, budget {budget} tokens)",
        ranked.len(),
        if ranked.len() == 1 { "" } else { "s" },
        file_count,
        if file_count == 1 { "" } else { "s" },
    );
    if let Some(note) = &corpus.capped_note {
        header.push(' ');
        header.push_str(note);
    }

    if ranked.is_empty() {
        return Ok(ToolResult {
            content: header,
            is_error: false,
            images: Vec::new(),
        });
    }

    // Pack clusters into the budget (chars/4 ≈ tokens, matching
    // driver::estimate_tokens). Header and marker count against it too, so
    // the whole tool result honors the budget. Stop at the FIRST cluster
    // that does not fit — best-first means later clusters are worse;
    // skipping ahead would bury better context under worse. `output stops
    // at budget` (spec req 1).
    let limit = budget
        .saturating_mul(4)
        .saturating_sub(MARKER_RESERVE_CHARS)
        .saturating_sub(header.chars().count());
    let mut body = String::new();
    let mut shown = 0usize;
    for c in &ranked {
        let rendered = render_cluster(&raw_lines, c);
        let rendered = if body.is_empty() {
            rendered
        } else {
            format!("\n\n{rendered}")
        };
        if body.chars().count() + rendered.chars().count() > limit {
            break;
        }
        body.push_str(&rendered);
        shown += 1;
    }

    let omitted = ranked.len() - shown;
    if omitted > 0 {
        header.push_str(&format!(
            "\n[more: {omitted} clusters omitted] — raise `budget` or narrow `path`"
        ));
    }
    let mut content = header;
    if !body.is_empty() {
        content.push('\n');
        content.push_str(&body);
    }
    Ok(ToolResult {
        content,
        is_error: false,
        images: Vec::new(),
    })
}

/// `symbols` mode: the fn/struct/impl signature skeleton of one Rust file —
/// line-oriented heuristics, not a parser (spec req 5). Every emitted
/// declaration carries its 1-based line number so the next step is a
/// targeted `read_file` with `offset`.
///
/// Heuristics (deliberate, documented):
/// - a declaration is a line whose first keyword (after `pub`/`pub(..)`,
///   `async`/`unsafe`/`const`/`extern`) is one of fn/struct/enum/trait/
///   impl/type/mod/union;
/// - a signature is that line plus up to [`SYMBOL_SIG_LOOKAHEAD`]
///   continuation lines when it does not yet end with `{` or `;`
///   (multi-line fn signatures); the body is never included — everything
///   from the first `{` is cut, rendered as `{ ... }`;
/// - comment lines (`//`) are skipped;
/// - a `mod tests` / `mod test` declaration (EXACT idents — `mod latest`,
///   `mod attest`, `mod tests_utils` are real modules and stay) suppresses
///   its contents: the BLOCK form (`mod tests {`, any `pub` prefix) until
///   the matching column-0 `}` (rustfmt closes top-level items at column
///   0), the FILE form (`mod tests;`) only its own line — a file-style mod
///   has no body, so the old latch waited for a `}` that never comes and
///   ate every declaration after it (validator probe P1);
/// - a signature that already ends with `;` (no body: trait method, extern-
///   block fn, unit struct) is emitted verbatim — the old rendering bolted
///   a misleading ` { ... }` onto body-less signatures.
fn symbols_skeleton(
    path: &Path,
    cwd: &Path,
    raw: &str,
    budget: usize,
) -> anyhow::Result<ToolResult> {
    if path.extension().and_then(|e| e.to_str()) != Some("rs") {
        bail!(
            "tgrep symbols mode supports Rust files (.rs), got {raw} — it is a line-oriented Rust skeleton, not a general outline"
        );
    }
    let data = fs::read_to_string(path)
        .map_err(|e| anyhow!("reading {}: {e}", path.display()))?;
    let lines: Vec<&str> = data.lines().collect();
    let rel = path
        .strip_prefix(cwd)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.display().to_string());

    let mut decls: Vec<(usize, String)> = Vec::new();
    // Inside a `mod tests {` block: suppress until the matching column-0
    // close brace. File-style `mod tests;` never latches (no block to
    // close — the old latch ate the whole rest of the file, probe P1b).
    let mut in_test_mod = false;
    let mut i = 0; // 0-based
    while i < lines.len() {
        let line = lines[i];
        let lineno = i + 1;
        if in_test_mod {
            // Inside a test module: resume at the column-0 close brace.
            if line.starts_with('}') {
                in_test_mod = false;
            }
            i += 1;
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        if let Some(kind) = decl_kind(trimmed) {
            if kind == "mod" && is_test_mod_name(mod_name(trimmed).as_deref()) {
                if mod_name_tail(trimmed)
                    .is_some_and(|(_, tail)| tail.trim_start().starts_with(';'))
                {
                    // File-style `mod tests;`: the body lives in another
                    // file, so there is nothing to suppress beyond this one
                    // line — keep walking (probe P1b).
                    i += 1;
                    continue;
                }
                // Block-style `mod tests {`: suppress the block's contents.
                in_test_mod = true;
                i += 1;
                continue;
            }
            // Gather the signature: continuation lines until it ends with
            // `{` or `;` (bounded), then cut the body at the first `{`.
            let mut sig = trimmed.to_string();
            let mut consumed = 0;
            while !sig.contains('{')
                && !sig.ends_with(';')
                && consumed < SYMBOL_SIG_LOOKAHEAD
                && i + 1 < lines.len()
            {
                i += 1;
                consumed += 1;
                sig.push(' ');
                sig.push_str(lines[i].trim());
            }
            sig = sig.split('{').next().unwrap_or("").trim_end().to_string();
            if sig.is_empty() {
                i += 1;
                continue;
            }
            // Body-less signatures (`fn f(&self);`, `struct S;`) already end
            // in `;` — they have no body to elide, so render them verbatim
            // instead of appending a lying ` { ... }`.
            if !sig.ends_with(';') {
                sig.push_str(" { ... }");
            }
            let mut rendered = format!("{lineno}: {sig}");
            if rendered.chars().count() > SYMBOL_SIG_MAX_CHARS {
                rendered = format!(
                    "{} …",
                    rendered.chars().take(SYMBOL_SIG_MAX_CHARS).collect::<String>()
                );
            }
            decls.push((lineno, rendered));
        }
        i += 1;
    }

    if decls.is_empty() {
        return Ok(ToolResult {
            content: format!("symbols {rel}: no fn/struct/impl declarations found"),
            is_error: false,
            images: Vec::new(),
        });
    }

    // Pack under the same budget (header and marker included); stop at the
    // first declaration that does not fit and mark the omission.
    let header = format!(
        "symbols {rel}: {} declarations in {} lines (budget {budget} tokens)",
        decls.len(),
        lines.len()
    );
    let limit = budget
        .saturating_mul(4)
        .saturating_sub(MARKER_RESERVE_CHARS)
        .saturating_sub(header.chars().count());
    let mut body = String::new();
    let mut shown = 0usize;
    for rendered in &decls {
        let entry = format!("  {}", rendered.1);
        let entry = if body.is_empty() {
            entry
        } else {
            format!("\n{entry}")
        };
        if body.chars().count() + entry.chars().count() > limit {
            break;
        }
        body.push_str(&entry);
        shown += 1;
    }
    let omitted = decls.len() - shown;
    let mut content = header;
    if omitted > 0 {
        content.push_str(&format!(
            "\n[more: {omitted} symbols omitted] — raise `budget` or narrow the file"
        ));
    }
    if !body.is_empty() {
        content.push('\n');
        content.push_str(&body);
    }
    Ok(ToolResult {
        content,
        is_error: false,
        images: Vec::new(),
    })
}

/// The declaration keyword a (trimmed) line opens with, after the `pub` and
/// fn-qualifier prefixes. `None` = not a declaration line.
///
/// Qualifier chains (validator probe P2): `const`/`async`/`unsafe` are
/// stripped in a LOOP, so any order and combination works — the old one-shot
/// handling dropped everything past the first qualifier (`async unsafe fn`
/// resolved to None and the fn vanished from the skeleton). Each strip
/// requires a word boundary (whitespace) after the keyword so identifiers
/// like `unsafefn()` or `constants:` are never mis-split into a fake `fn`.
fn decl_kind(trimmed: &str) -> Option<&'static str> {
    let mut t = strip_visibility(trimmed);
    // fn qualifiers, any order/combination: `const fn`, `async fn`,
    // `unsafe fn`, `async unsafe fn`, `const unsafe fn`, `unsafe const fn`.
    loop {
        let before = t;
        for qualifier in ["const", "async", "unsafe"] {
            if let Some(rest) = t.strip_prefix(qualifier)
                && rest.starts_with(char::is_whitespace)
            {
                t = rest.trim_start();
            }
        }
        if t == before {
            break;
        }
    }
    // `extern ["ABI"] fn` — positionally after the other qualifiers
    // (`const unsafe extern "C" fn`); `extern"C"` needs no space.
    if let Some(rest) = t.strip_prefix("extern") {
        let rest = rest.trim_start();
        let rest = if let Some(quoted) = rest.strip_prefix('"') {
            match quoted.find('"') {
                Some(end) => quoted[end + 1..].trim_start(),
                None => rest,
            }
        } else {
            rest
        };
        if rest.starts_with("fn ") || rest.starts_with("fn(") {
            return Some("fn");
        }
        return None;
    }
    for (prefix, kind) in [
        ("fn ", "fn"),
        ("fn(", "fn"),
        ("struct ", "struct"),
        ("enum ", "enum"),
        ("union ", "union"),
        ("trait ", "trait"),
        ("impl ", "impl"),
        // Generic-parameterized impls attach `<` directly to the keyword —
        // `impl<T> Trait for Type` matched neither "impl " nor anything else
        // and was dropped (same drop class as probe P2).
        ("impl<", "impl"),
        ("type ", "type"),
        ("mod ", "mod"),
    ] {
        if t.starts_with(prefix) {
            return Some(kind);
        }
    }
    None
}

/// Strip a leading visibility qualifier: `pub`, `pub(crate)`, `pub(super)`,
/// `pub(self)`, `pub(in path)` — the parenthesized scope is skipped to its
/// close paren, so ANY `pub(...)` form works (the old enumeration missed
/// `pub(self)` and dropped those declarations). Requires a boundary after
/// `pub` (whitespace or `(`) so identifiers like `publication` are not
/// mis-split. Returns the remainder, trimmed.
fn strip_visibility(t: &str) -> &str {
    let Some(rest) = t.strip_prefix("pub") else {
        return t;
    };
    if !(rest.starts_with(char::is_whitespace) || rest.starts_with('(')) {
        return t; // no boundary: `publication…` — `pub` is part of the word
    }
    let rest = rest.trim_start();
    if let Some(paren) = rest.strip_prefix('(') {
        match paren.find(')') {
            Some(close) => paren[close + 1..].trim_start(),
            // Unterminated scope on this line: not a usable declaration
            // opener under line-oriented heuristics.
            None => "",
        }
    } else {
        rest
    }
}

/// The module name of a `mod <name>` line and the text following it, given
/// the RAW trimmed line: `pub`-visibility prefixes are stripped first
/// (decl_kind sees through `pub mod` to return kind `mod`, but suppression
/// needs the NAME from the same line — `pub mod tests {` must resolve to
/// name `tests` or its test fns leak into the skeleton).
fn mod_name_tail(trimmed: &str) -> Option<(&str, &str)> {
    let t = strip_visibility(trimmed);
    let rest = t.strip_prefix("mod ")?;
    let name_end = rest
        .find(|c: char| !(c.is_alphanumeric() || c == '_'))
        .unwrap_or(rest.len());
    if name_end == 0 {
        return None;
    }
    Some((&rest[..name_end], &rest[name_end..]))
}

/// The module name alone (test compatibility + simple callers).
fn mod_name(trimmed: &str) -> Option<String> {
    mod_name_tail(trimmed).map(|(n, _)| n.to_string())
}

/// Test-module suppression fires ONLY on the exact idents `tests`/`test` —
/// any other spelling is a REAL module whose contents belong in the
/// skeleton. The old `contains("test")` check ate `mod latest {` (la-TEST),
/// `mod attest {` (at-TEST), and `mod tests_utils {` (validator probe P1).
fn is_test_mod_name(name: Option<&str>) -> bool {
    matches!(name, Some("tests") | Some("test"))
}

/// Band calibration shared by the tgrep unit tests and the driver T76
/// integration test (round-3 BLOCKING class 1 sweep): a `<=budget` claim is
/// only exercised when the packed body lands inside the omitted-marker band
/// the reserve guards. The helpers measure REAL rendered cluster sizes and
/// pick the budget where the packing straddles that band.
#[cfg(test)]
pub(crate) mod band {
    use super::MARKER_RESERVE_CHARS;

    /// The greedy packing `search`/`symbols_skeleton` perform, over MEASURED
    /// cluster sizes in output order (`\n\n` between clusters): returns
    /// (clusters shown, body chars). Read side-by-side with the real loops.
    pub(crate) fn pack(sizes: &[usize], limit: usize) -> (usize, usize) {
        let mut body = 0usize;
        let mut shown = 0usize;
        for (i, s) in sizes.iter().enumerate() {
            let add = if i == 0 { *s } else { s + 2 };
            if body + add > limit {
                break;
            }
            body += add;
            shown += 1;
        }
        (shown, body)
    }

    /// Measure a search result: (header chars, total clusters, per-cluster
    /// rendered sizes in output order). `content` must come from a measure
    /// pass at `budget: 8000` — the header carries `budget 8000 tokens`,
    /// which the budget rebasing in [`Self::calibrate`] subtracts digit-wise.
    /// The measure pass MAY itself be truncated (marker present): packing is
    /// prefix-based, so the leading measured sizes are exact, and calibrate
    /// refuses picks whose packing would run past the measured prefix.
    pub(crate) fn measure(content: &str) -> (usize, usize, Vec<usize>) {
        let (header, body) = content.split_once('\n').expect("header line");
        let total: usize = header
            .split(" clusters in ")
            .next()
            .expect("total count")
            .rsplit(' ')
            .next()
            .expect("count digits")
            .parse()
            .expect("total is an integer");
        let sizes: Vec<usize> = {
            // A truncated measure pass appends the omission marker to the
            // header; strip that line so it is not counted as a cluster.
            let body = if let Some(rest) = body.strip_prefix("[more: ") {
                rest.split_once('\n').map(|(_, b)| b).unwrap_or("")
            } else {
                body
            };
            body.split("\n\n").map(|c| c.chars().count()).collect()
        };
        (header.chars().count(), total, sizes)
    }

    /// Marker width for `n` omitted clusters:
    /// `[more: n clusters omitted] — raise \`budget\` or narrow \`path\``
    /// = 59 + digits(n) (pinned against the live marker in tests).
    pub(crate) fn marker_len(n: usize) -> usize {
        59 + n.to_string().len()
    }

    /// Pick the budget where the packing straddles the reserve band: with the
    /// shipped [`MARKER_RESERVE_CHARS`] the run shows `shown96` clusters and
    /// stays under budget, while at EACH probed reserve value (delete → 0,
    /// shrink → 8/16/32) one more cluster fits and the rendered output
    /// OVERFLOWS `4*budget` chars by a tight margin — so a mutant that
    /// deletes or shrinks the reserve turns the caller's invariant assert
    /// RED, and the current code is green. Returns
    /// (budget, shown96, body96, over0, header_len).
    pub(crate) fn calibrate(measured: &str, min_shown: usize) -> (usize, usize, usize, usize, usize) {
        let (h4, total, sizes) = measure(measured);
        let hdr = |b: usize| h4 - 4 + b.to_string().len();
        const PROBED_RESERVES: [usize; 4] = [0, 8, 16, 32];
        for b in 40..=8000 {
            let h = hdr(b);
            if 4 * b <= MARKER_RESERVE_CHARS + h {
                continue;
            }
            let (k96, body96) = pack(&sizes, 4 * b - MARKER_RESERVE_CHARS - h);
            if k96 < min_shown {
                continue;
            }
            // The r=0 (delete) overshoot must be a TIGHT band member.
            let (k0, body0) = pack(&sizes, 4 * b);
            let total0 = h + body0 + marker_len(total - k0);
            if total0 <= 4 * b || total0 - 4 * b > 24 || k0 >= sizes.len() {
                continue;
            }
            let over0 = total0 - 4 * b;
            let ok = PROBED_RESERVES.iter().all(|&r| {
                let (k_r, body_r) = pack(&sizes, 4 * b - r - h);
                let omitted = total - k_r;
                // Marker width must stay reachable (2 digits) so the band is
                // the honest ~61-char one, the mutant must overflow, and the
                // simulated stop must be real (not "ran out of measured
                // sizes") so k_r is the true packing, not the prefix end.
                k_r < sizes.len()
                    && (10..=999).contains(&omitted)
                    && h + body_r + marker_len(omitted) > 4 * b
            });
            if ok {
                return (b, k96, body96, over0, h);
            }
        }
        panic!("no band-straddling budget found ({total} clusters, {h4}-char header)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::dispatch;
    use serde_json::json;
    use std::time::Instant;

    fn ctx_for(tmp: &tempfile::TempDir) -> ToolCtx {
        // Round-4 timing sweep (T72 family): bash_timeout is harness CONFIG
        // (a bash-tool ceiling that tgrep never exercises), not a timing
        // assert — load-immune. The ONLY wall-clock assert on this surface
        // is deterministic_and_fast's SPEED leg below (median-of-5, load-
        // robust bound); every other pin in this module is size-based.
        ToolCtx {
            cwd: tmp.path().to_path_buf(),
            bash_timeout: std::time::Duration::from_secs(60),
        }
    }

    /// Three files that separate the ranking tiers for the query
    /// `alpha beta`: phrase line, scattered-terms lines, one-term line.
    fn tier_fixtures(tmp: &tempfile::TempDir) {
        fs::write(
            tmp.path().join("phrase.rs"),
            "pad\npad\nlet x = alpha beta combined;\npad\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("scatter.rs"),
            "pad\npad\nuses alpha here;\nand beta there;\npad\n",
        )
        .unwrap();
        fs::write(
            tmp.path().join("single.rs"),
            "pad\npad\nonly alpha lives;\npad\n",
        )
        .unwrap();
    }

    /// Spec test: ranking order — exact-phrase beats scatter.
    #[test]
    fn exact_phrase_beats_all_terms_beats_partial() {
        let tmp = tempfile::tempdir().unwrap();
        tier_fixtures(&tmp);
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha beta"}));
        assert!(!result.is_error, "{}", result.content);
        let phrase_at = result.content.find("phrase.rs:").unwrap();
        let scatter_at = result.content.find("scatter.rs:").unwrap();
        let single_at = result.content.find("single.rs:").unwrap();
        assert!(
            phrase_at < scatter_at && scatter_at < single_at,
            "tiers out of order: {}",
            result.content
        );
        assert!(result.content.contains("(exact-phrase)"));
        assert!(result.content.contains("(all-terms)"));
        assert!(result.content.contains("(partial)"));
    }

    /// Spec test: overlapping ±3-line windows merge into ONE cluster.
    #[test]
    fn overlapping_windows_merge_once() {
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        for i in 1..=12 {
            let line = if i == 5 || i == 7 {
                format!("needle {i}\n")
            } else {
                format!("filler {i}\n")
            };
            body.push_str(&line);
        }
        fs::write(tmp.path().join("merge.rs"), body).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("1 cluster in 1 file"),
            "expected one merged cluster: {}",
            result.content
        );
        // Window spans [2..10]: 5-3 .. 7+3. Anchor is the first hit (tie).
        assert!(result.content.contains("> 5 | needle 5"), "{}", result.content);
        assert!(result.content.contains("  2 | filler 2"), "{}", result.content);
        assert!(result.content.contains(" 10 | filler 10"), "{}", result.content);
        assert!(!result.content.contains(" 1 |"), "{}", result.content);
        assert!(!result.content.contains(" 11 |"), "{}", result.content);
    }

    /// Spec test: budget truncation — the output stops at the budget and the
    /// exact `[more: N clusters omitted]` marker names what was left out.
    #[test]
    fn budget_truncation_fires_the_omission_marker_and_stays_under_budget() {
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        // 50 disjoint clusters: a hit every 10 lines, windows never touch.
        for i in 1..=500 {
            let line = if i % 10 == 0 {
                format!("needle line {i}\n")
            } else {
                format!("filler line {i} padding padding padding\n")
            };
            body.push_str(&line);
        }
        fs::write(tmp.path().join("many.rs"), body).unwrap();
        let ctx = ctx_for(&tmp);
        let budget = 400usize;
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "budget": budget}),
        );
        assert!(!result.is_error, "{}", result.content);
        let marker_at = result
            .content
            .find("[more: ")
            .unwrap_or_else(|| panic!("no omission marker: {}", result.content));
        // Header carries the TOTAL cluster count; the marker names the tail
        // that did not fit. shown = total - omitted.
        let total: usize = result
            .content
            .split(" clusters in ")
            .next()
            .and_then(|h| h.rsplit(' ').next())
            .and_then(|n| n.parse().ok())
            .unwrap();
        let marker_end = result.content[marker_at..].find(']').unwrap() + marker_at;
        let omitted: usize = result.content[marker_at..marker_end]
            .trim_start_matches("[more: ")
            .trim_end_matches(" clusters omitted")
            .parse()
            .unwrap();
        assert_eq!(total, 50, "{}", result.content);
        let shown = total - omitted;
        assert!(shown > 0 && omitted > 0, "{}", result.content);
        // Whole output (header + clusters + marker) honors the token budget.
        assert!(
            result.content.chars().count() <= budget * 4,
            "output {} chars exceeds budget {budget} tokens ({} chars max): {}",
            result.content.chars().count(),
            budget * 4,
            result.content
        );
    }

    /// Spec test: symbols mode extracts fn signatures, not bodies.
    #[test]
    fn symbols_mode_extracts_signatures_not_bodies() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("thing.rs"),
            r#"//! module doc
use std::fmt;

/// A documented struct.
pub struct Thing {
    field: u32,
}

impl Thing {
    pub fn new(field: u32) -> Self {
        Self { field }
    }
    // a comment between items
    fn helper(&self) -> u32 {
        self.field + 1
    }
}

pub async fn fetch(url: &str,
                   retries: u32) -> Result<String, fmt::Error> {
    Ok(url.to_string() + &retries.to_string())
}

fn one_liner(x: u32) -> u32 { x + 1 }

mod tests {
    #[test]
    fn hidden_test_fn() {
        assert_eq!(1, 1);
    }
}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "thing.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        let file_lines = fs::read_to_string(tmp.path().join("thing.rs"))
            .unwrap()
            .lines()
            .count();
        let expected_header = format!("symbols thing.rs: 6 declarations in {file_lines} lines");
        assert!(c.starts_with(&expected_header), "{c}");
        assert!(c.contains("pub struct Thing { ... }"), "{c}");
        assert!(c.contains("impl Thing { ... }"), "{c}");
        assert!(c.contains("pub fn new(field: u32) -> Self { ... }"), "{c}");
        assert!(c.contains("fn helper(&self) -> u32 { ... }"), "{c}");
        assert!(
            c.contains("pub async fn fetch(url: &str, retries: u32) -> Result<String, fmt::Error> { ... }"),
            "{c}"
        );
        assert!(c.contains("fn one_liner(x: u32) -> u32 { ... }"), "{c}");
        // No bodies, no test-module items, no doc comments.
        assert!(!c.contains("x + 1"), "{c}");
        assert!(!c.contains("self.field"), "{c}");
        assert!(!c.contains("hidden_test_fn"), "{c}");
        assert!(!c.contains("module doc"), "{c}");
        assert!(!c.contains("use std::fmt"), "{c}");
    }

    /// Symbols mode refuses non-Rust files by name (spec: Rust skeleton
    /// only, other languages' parsers out of scope).
    #[test]
    fn symbols_mode_refuses_non_rust_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("notes.md"), "# hello\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "notes.md"}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("supports Rust files (.rs)"), "{}", result.content);
    }

    /// Symbols mode without a path is a named tool error; so is a query-less
    /// search call.
    #[test]
    fn mode_validation_errors_name_the_requirement() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "x", "symbols": true}));
        assert!(result.is_error);
        assert!(result.content.contains("symbols mode requires `path`"), "{}", result.content);

        let result = dispatch(&ctx, "tgrep", &json!({"path": "src"}));
        assert!(result.is_error);
        assert!(
            result.content.contains("missing or non-string field: query"),
            "{}",
            result.content
        );

        let result = dispatch(&ctx, "tgrep", &json!({"query": "   "}));
        assert!(result.is_error);
        assert!(result.content.contains("`query` is empty"), "{}", result.content);
    }

    /// Budget legs: default 2000, above-ceiling clamped to 8000, and the
    /// invalid forms are tool errors naming the constraint (T39 precedent).
    #[test]
    fn budget_defaults_clamps_and_rejects() {
        let tmp = tempfile::tempdir().unwrap();
        tier_fixtures(&tmp);
        let ctx = ctx_for(&tmp);

        // Absent → default 2000, named in the header.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha"}));
        assert!(result.content.contains("budget 2000 tokens"), "{}", result.content);

        // Above the ceiling → clamped down to 8000, not rejected.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha", "budget": 999_999}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("budget 8000 tokens"), "{}", result.content);

        // Non-integer and < 1 → tool error naming the constraint.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha", "budget": "big"}));
        assert!(result.is_error);
        assert!(result.content.contains("`budget` must be an integer"), "{}", result.content);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha", "budget": 0}));
        assert!(result.is_error);
        assert!(result.content.contains("at least 1 token"), "{}", result.content);
    }

    /// `path` narrows the corpus: a glob, a directory, and a single file all
    /// resolve; escapes are refused with the sandbox message.
    #[test]
    fn path_param_glob_dir_file_and_escape() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src/deep")).unwrap();
        fs::write(tmp.path().join("src/hit.rs"), "needle here\n").unwrap();
        fs::write(tmp.path().join("src/deep/miss.rs"), "needle deep\n").unwrap();

        let ctx = ctx_for(&tmp);
        // Glob keeps only src/*.rs (not the deep dir).
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "path": "src/*.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("src/hit.rs"), "{}", result.content);
        assert!(!result.content.contains("miss.rs"), "{}", result.content);

        // Directory walks under it.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle", "path": "src"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("miss.rs"), "{}", result.content);

        // Single file.
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "path": "src/hit.rs"}),
        );
        assert!(result.content.contains("searched 1 files"), "{}", result.content);

        // Escape is refused with the T41/T61 sandbox message.
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "path": "../../etc"}),
        );
        assert!(result.is_error);
        assert!(result.content.contains("path escapes cwd"), "{}", result.content);

        // Missing target is a named error, not a silent empty result.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle", "path": "src/nope.rs"}));
        assert!(result.is_error);
        assert!(result.content.contains("tgrep path not found: src/nope.rs"), "{}", result.content);
    }

    /// The walk skips build caches and hidden dirs (`target*`, dot-dirs) —
    /// `targets` (a legit code dir name) is NOT skipped.
    #[test]
    fn walk_skips_build_caches_and_hidden_dirs_only() {
        assert!(is_skip_dir("target"));
        assert!(is_skip_dir("target-shared"));
        assert!(is_skip_dir("target-shared-validate"));
        assert!(is_skip_dir(".git"));
        assert!(is_skip_dir(".chug"));
        assert!(is_skip_dir("node_modules"));
        assert!(!is_skip_dir("targets"));
        assert!(!is_skip_dir("src"));
    }

    /// No matches: an ok (non-error) result that says so and names the corpus.
    #[test]
    fn no_matches_is_ok_and_honest() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.rs"), "nothing relevant\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "zebra"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("0 clusters"), "{}", result.content);
        assert!(result.content.contains("searched 1 files"), "{}", result.content);
    }

    /// Schema pins (T22/T41 doctrine: assert the LIVE schema, not a copy):
    /// the taught workflow (tgrep → targeted read_file offset/limit), the
    /// sandbox candor, the exact omission marker, and the budget contract.
    #[test]
    fn schema_pins_workflow_candor_and_marker() {
        let schemas = crate::tools::tool_schemas();
        let schema = schemas
            .iter()
            .find(|s| s["name"] == "tgrep")
            .expect("tgrep schema registered");
        let desc = schema["description"].as_str().unwrap();
        assert!(
            desc.contains("read_file with offset/limit"),
            "workflow must be taught: {desc}"
        );
        assert!(desc.contains("path escapes cwd"), "T41 candor missing: {desc}");
        assert!(
            desc.contains("[more: N clusters omitted]"),
            "marker literal missing: {desc}"
        );
        assert!(desc.contains("default 2000"), "{desc}");
        assert!(desc.contains("clamped to 8000"), "{desc}");
        assert!(desc.contains("symbols"), "{desc}");
        let props = &schema["input_schema"]["properties"];
        assert_eq!(props["query"]["type"], "string");
        assert_eq!(props["path"]["type"], "string");
        assert_eq!(props["budget"]["type"], "integer");
        assert_eq!(props["symbols"]["type"], "boolean");
        assert_eq!(schema["input_schema"]["required"], json!(["query"]));
    }

    /// parse_query: quoted phrases win, unterminated quotes degrade to
    /// terms, terms dedupe, a single term is its own phrase.
    #[test]
    fn parse_query_shapes() {
        let q = parse_query("wait_secs \"delegate launch\" loop").unwrap();
        assert_eq!(q.phrases, vec!["delegate launch".to_string()]);
        assert_eq!(q.terms, vec!["wait_secs".to_string(), "loop".to_string()]);

        let q = parse_query("run run Run").unwrap();
        assert_eq!(q.terms, vec!["run".to_string()]);
        // Single term: the term doubles as its phrase.
        assert_eq!(q.phrases, vec!["run".to_string()]);

        let q = parse_query("alpha beta").unwrap();
        // Plain multi-term query: the whole string is the phrase candidate.
        assert_eq!(q.phrases, vec!["alpha beta".to_string()]);

        let q = parse_query("\"unclosed phrase").unwrap();
        // Unterminated quote: the tail degrades to terms, and (the quote
        // being phrase syntax) the whole tail is still the phrase candidate.
        assert_eq!(q.phrases, vec!["unclosed phrase".to_string()]);
        assert_eq!(q.terms, vec!["unclosed".to_string(), "phrase".to_string()]);

        assert!(parse_query("!!! ...").is_none());
    }

    /// Path-basename boost (spec req 2): a same-tier cluster in a file whose
    /// BASENAME carries the query term outranks an identical one whose
    /// basename does not. The fixture names are the whole point (validator
    /// M1: the old fixtures had the term in NEITHER basename, so the leg
    /// never fired and the alphabetical path tiebreak masked the mutant):
    /// the boosted file sorts alphabetically SECOND, so only the boost can
    /// put it first. RED vs M1 (boost -> 0): the scores tie at 70.5 and
    /// aplain.rs wins the path tiebreak instead.
    #[test]
    fn basename_boost_breaks_density_ties() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("aplain.rs"), "pad\nlaunch handling here;\npad\n").unwrap();
        fs::write(tmp.path().join("zlaunch.rs"), "pad\nlaunch handling here;\npad\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "launch"}));
        assert!(!result.is_error, "{}", result.content);
        let boosted = result.content.find("zlaunch.rs:").unwrap();
        let plain = result.content.find("aplain.rs:").unwrap();
        assert!(boosted < plain, "basename boost lost: {}", result.content);
    }

    /// A file of `len` lines with `needle` on exactly the given lines
    /// (distinct filler elsewhere) so hit-line distances are exact.
    fn body_with_hits(len: usize, hits: &[usize]) -> String {
        (1..=len)
            .map(|i| {
                if hits.contains(&i) {
                    format!("needle {i}\n")
                } else {
                    format!("filler {i}\n")
                }
            })
            .collect()
    }

    /// The 1-based line numbers rendered in cluster bodies, in output order —
    /// proves no line is ever emitted twice (the probe-P1 defect class).
    fn emitted_line_numbers(content: &str) -> Vec<usize> {
        content
            .lines()
            .filter_map(|l| {
                let body = l.strip_prefix('>').or_else(|| l.strip_prefix(' '))?;
                let body = body.strip_prefix(' ')?;
                let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
                if digits.is_empty() {
                    return None;
                }
                body[digits.len()..]
                    .starts_with(" | ")
                    .then(|| digits.parse().ok())
                    .flatten()
            })
            .collect()
    }

    fn assert_no_duplicate_lines(content: &str, ctx_label: &str) {
        let nums = emitted_line_numbers(content);
        let mut sorted = nums.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            nums.len(),
            "{ctx_label}: duplicated emitted lines {nums:?}"
        );
    }

    /// Gap class 0 — hits on CONSECUTIVE lines ("overlapping matches"): one
    /// cluster spanning [10-3, 11+3], every line emitted once. (Pre-fix
    /// radius already merged this class; it pins the floor and is RED
    /// against a merge-disabled / radius-0 mutant.)
    #[test]
    fn merge_gap0_hits_on_consecutive_lines_one_cluster() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("g0.rs"), body_with_hits(20, &[10, 11])).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("1 cluster in 1 file"),
            "{}",
            result.content
        );
        assert_no_duplicate_lines(&result.content, "gap0");
        assert_eq!(
            emitted_line_numbers(&result.content),
            (7..=14).collect::<Vec<_>>(),
            "{}",
            result.content
        );
    }

    /// Gap class 1-3 — hits 3 lines apart (the pre-fix boundary): still ONE
    /// cluster, span [first-3, last+3] exactly. Pins the floor against
    /// radius-1/-0 mutants.
    #[test]
    fn merge_gap1_to_3_hits_one_cluster_exact_span() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("g3.rs"), body_with_hits(20, &[10, 13])).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("1 cluster in 1 file"),
            "{}",
            result.content
        );
        assert_eq!(
            emitted_line_numbers(&result.content),
            (7..=16).collect::<Vec<_>>(),
            "{}",
            result.content
        );
    }

    /// Gap class 4-6 — THE validator hole (probe P1): the ±3-line windows of
    /// hits 4-6 lines apart OVERLAP, so merging on hit-line distance (the
    /// pre-fix radius of 3) emitted TWO clusters with duplicated lines. Each
    /// distance must give ONE cluster spanning [first-3, last+3] with every
    /// line emitted once. RED vs the pre-fix radius (M2 merge-shrink).
    #[test]
    fn merge_gap4_to_6_overlapping_windows_one_cluster_no_duplicates() {
        for d in 4..=6 {
            let tmp = tempfile::tempdir().unwrap();
            let hits = [10, 10 + d];
            fs::write(tmp.path().join("hole.rs"), body_with_hits(30, &hits)).unwrap();
            let ctx = ctx_for(&tmp);
            let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
            assert!(!result.is_error, "d={d}: {}", result.content);
            assert!(
                result.content.contains("1 cluster in 1 file"),
                "d={d}: {}",
                result.content
            );
            assert_no_duplicate_lines(&result.content, &format!("d={d}"));
            assert_eq!(
                emitted_line_numbers(&result.content),
                (7..=(13 + d)).collect::<Vec<_>>(),
                "d={d}: {}",
                result.content
            );
        }
    }

    /// Window ADJACENCY — hits exactly 2*WINDOW+1 = 7 lines apart: the
    /// windows [7,13] and [14,20] touch with no gap (14 = 13+1), so they
    /// merge into one cluster per the "overlap or adjacent" rule. RED vs the
    /// pre-fix radius AND vs an overlap-only radius of 2*WINDOW that forgot
    /// adjacency.
    #[test]
    fn merge_adjacent_windows_touch_at_d7_one_cluster() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("adj.rs"), body_with_hits(30, &[10, 17])).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("1 cluster in 1 file"),
            "{}",
            result.content
        );
        assert_eq!(
            emitted_line_numbers(&result.content),
            (7..=20).collect::<Vec<_>>(),
            "{}",
            result.content
        );
    }

    /// Negative control — hits 2*WINDOW+2 = 8 lines apart: windows [7,13]
    /// and [15,21] neither overlap nor touch, so they stay TWO clusters with
    /// disjoint lines (kills radius-overshoot mutants: merging everything
    /// into one cluster is as wrong as splitting overlapping windows).
    #[test]
    fn distinct_clusters_when_windows_neither_overlap_nor_touch() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("far.rs"), body_with_hits(30, &[10, 18])).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("2 clusters in 1 file"),
            "{}",
            result.content
        );
        assert_no_duplicate_lines(&result.content, "d=8");
        let mut expected: Vec<usize> = (7..=13).collect();
        expected.extend(15..=21);
        assert_eq!(
            emitted_line_numbers(&result.content),
            expected,
            "{}",
            result.content
        );
    }

    /// Transitive merge-once — three hits chained 4 lines apart (10, 14, 18):
    /// windows [7,13], [11,17], [15,21] overlap pairwise in a chain and must
    /// collapse into exactly ONE cluster [7,21] with every line emitted once
    /// (spec: "overlapping windows merge once"). RED vs the pre-fix radius
    /// (three separate clusters).
    #[test]
    fn transitive_three_hits_merge_once() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("chain.rs"), body_with_hits(30, &[10, 14, 18])).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            result.content.contains("1 cluster in 1 file"),
            "{}",
            result.content
        );
        assert_no_duplicate_lines(&result.content, "transitive");
        assert_eq!(
            emitted_line_numbers(&result.content),
            (7..=21).collect::<Vec<_>>(),
            "{}",
            result.content
        );
    }

    /// All-terms tier (spec req 2): a window holding ALL query terms
    /// outranks a partial window even when the partial one is far denser —
    /// only the tier base keeps them apart. RED vs M7 (TIER_ALL_TERMS -> 0):
    /// 11.5 < 11.67, the dense partial flips to the top.
    #[test]
    fn all_terms_tier_beats_high_density_partial() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("allterms.rs"),
            "pad\ngamma fires alpha fires beta\npad\n",
        )
        .unwrap();
        // 5 alphas + 5 betas, no gamma, never the contiguous phrase: partial
        // tier, density 2/3, occurrences capped at 10.
        fs::write(
            tmp.path().join("dense_partial.rs"),
            "pad\nalpha zz beta zz alpha zz beta zz alpha zz beta zz alpha zz beta\npad\n",
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha beta gamma"}));
        assert!(!result.is_error, "{}", result.content);
        let all_at = result.content.find("allterms.rs:").unwrap();
        let dense_at = result.content.find("dense_partial.rs:").unwrap();
        assert!(
            all_at < dense_at,
            "all-terms tier lost to a dense partial: {}",
            result.content
        );
        assert!(result.content.contains("(all-terms)"), "{}", result.content);
        assert!(result.content.contains("(partial)"), "{}", result.content);
    }

    /// Term-density leg (spec req 2): within the partial tier, a window
    /// holding MORE of the query's terms outranks a sparser one at EQUAL
    /// occurrence counts — only density separates them. RED vs M6 (density
    /// contribution -> 0): both clusters tie at 1.0 and the
    /// alphabetically-first file wins the path tiebreak instead.
    #[test]
    fn density_orders_clusters_within_the_partial_tier() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a_one_term.rs"), "pad\nalpha alpha\npad\n").unwrap();
        fs::write(tmp.path().join("z_two_terms.rs"), "pad\nalpha fires beta\npad\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "alpha beta gamma"}));
        assert!(!result.is_error, "{}", result.content);
        let two_at = result.content.find("z_two_terms.rs:").unwrap();
        let one_at = result.content.find("a_one_term.rs:").unwrap();
        assert!(two_at < one_at, "density leg lost: {}", result.content);
    }

    /// Each ranking leg's exact contribution, pinned at the fn where a
    /// mutation would regress it (validator class: a leg whose tests never
    /// fire it is vacuous). RED-proven against: TIER_PHRASE -> 0, M5 tier
    /// flip (60<->30), TIER_ALL_TERMS -> 0, density -> 0, M1 basename -> 0.
    #[test]
    fn score_window_pins_each_ranking_leg() {
        // exact-phrase tier: quoted phrase hit, no terms → bare 60.0.
        let q = parse_query("\"alpha beta\"").unwrap();
        let (label, score) = score_window(&q, "let x = alpha beta combined", "plain.rs");
        assert_eq!(label, "exact-phrase");
        assert_eq!(score, 60.0, "TIER_PHRASE contribution moved");
        assert_eq!(score_window(&q, "no hit here", "plain.rs").0, "partial");

        // all-terms tier: every term present (no contiguous phrase) →
        // 30 + density 10 + occurrences 2*0.5 = 41.0.
        let q = parse_query("alpha beta").unwrap();
        let (label, score) = score_window(&q, "beta then alpha", "plain.rs");
        assert_eq!(label, "all-terms");
        assert_eq!(score, 41.0, "TIER_ALL_TERMS contribution moved");

        // density leg: one of two terms → partial base 0 + density 5.0 +
        // occurrences 2*0.5 = 6.0.
        let q = parse_query("alpha beta").unwrap();
        let (label, score) = score_window(&q, "alpha zz alpha", "plain.rs");
        assert_eq!(label, "partial");
        assert_eq!(score, 6.0, "density contribution moved");

        // basename boost: identical window, basename carrying the term adds
        // exactly 2.0 over a basename without it (70.5 vs 72.5).
        let q = parse_query("launch").unwrap();
        let (_, plain) = score_window(&q, "launch handling here", "aplain.rs");
        let (_, boosted) = score_window(&q, "launch handling here", "zlaunch.rs");
        assert_eq!(plain, 70.5);
        assert_eq!(boosted, 72.5, "basename-boost contribution moved");

        // occurrence cap: 12 hits still add only 10*0.5 = 5.0 over the base.
        let q = parse_query("alpha").unwrap();
        let (_, score) = score_window(&q, "alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha", "plain.rs");
        assert_eq!(score, 75.0, "occurrence cap moved");
    }

    /// Symbols mode: `pub mod tests` (and `pub(crate) mod …`) suppresses its
    /// fns like bare `mod tests` does — pre-fix the suppression matched only
    /// `mod <name>` because mod_name saw the raw `pub mod tests {` line and
    /// resolved no name, so test fns leaked into the skeleton (NB-a; the
    /// same fix covers NB-c).
    #[test]
    fn symbols_mode_pub_mod_tests_suppressed() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("pubmod.rs"),
            r#"pub fn visible() -> u32 { 1 }

pub mod tests {
    fn hidden_a() {}
}

pub(crate) mod tests {
    fn hidden_b() {}
}

mod tests {
    fn hidden_c() {}
}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "pubmod.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        // 1 declaration: the visible fn; all three test mods are suppressed.
        assert!(c.starts_with("symbols pubmod.rs: 1 declarations in "), "{c}");
        assert!(c.contains("pub fn visible() -> u32 { ... }"), "{c}");
        assert!(!c.contains("hidden_a"), "pub mod tests leaked: {c}");
        assert!(!c.contains("hidden_b"), "pub(crate) mod tests leaked: {c}");
        assert!(!c.contains("hidden_c"), "bare mod tests leaked: {c}");
    }

    /// Probe P1a (round-2 BLOCKING): suppression fires ONLY on the exact
    /// idents `mod tests` / `mod test` — the old `contains("test")` ate REAL
    /// modules (`mod latest`, `mod attest`, `mod tests_utils`) and silently
    /// dropped their contents from the skeleton.
    #[test]
    fn symbols_mode_test_mod_suppression_is_exact_ident() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("exact.rs"),
            r#"mod latest {
    fn kept_latest() {}
}

mod attest {
    fn kept_attest() {}
}

mod tests_utils {
    fn kept_utils() {}
}

mod tests {
    fn hidden_plain() {}
}

mod test {
    fn hidden_singular() {}
}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "exact.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.contains("fn kept_latest() { ... }"), "mod latest eaten: {c}");
        assert!(c.contains("fn kept_attest() { ... }"), "mod attest eaten: {c}");
        assert!(c.contains("fn kept_utils() { ... }"), "mod tests_utils eaten: {c}");
        assert!(!c.contains("hidden_plain"), "{c}");
        assert!(!c.contains("hidden_singular"), "{c}");
    }

    /// Probe P1b (round-2 BLOCKING): file-style `mod tests;` has no block —
    /// it suppresses ONLY its own line. The old latch set suppression for the
    /// file form too and then waited for a column-0 `}` that never comes,
    /// eating every declaration after it (here: ALL of them, since each latch
    /// is only released by the other items' stray column-0 braces).
    #[test]
    fn symbols_mode_file_style_test_mod_suppresses_only_its_own_line() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("filemod.rs"),
            r#"pub mod tests;

pub struct Real {
    field: u32,
}

pub(crate) mod test;

fn survivor() -> u32 {
    7
}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "filemod.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.starts_with("symbols filemod.rs: 2 declarations in "), "{c}");
        assert!(c.contains("pub struct Real { ... }"), "struct eaten: {c}");
        assert!(c.contains("fn survivor() -> u32 { ... }"), "fn eaten: {c}");
        // The suppressed lines themselves stay out of the skeleton.
        assert!(!c.contains("mod tests"), "{c}");
        assert!(!c.contains("mod test;"), "{c}");
    }

    /// Probe P2 (round-2 BLOCKING): qualifier chains in any combination and
    /// order — the old one-shot qualifier handling resolved everything past
    /// the first qualifier to None and silently dropped the fn
    /// (`async unsafe fn`, `const unsafe fn`, `pub async unsafe fn`, …).
    #[test]
    fn symbols_mode_qualifier_chains_any_order() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("quals.rs"),
            r#"async fn qa() { 1 }
const fn qc() { 2 }
unsafe fn qu() { 3 }
async unsafe fn qau() { 4 }
const unsafe fn qcu() { 5 }
unsafe const fn quc() { 6 }
pub async unsafe fn qpu() { 7 }
pub(crate) const fn qpc() { 8 }
const unsafe extern "C" fn qce() { 9 }
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "quals.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.starts_with("symbols quals.rs: 9 declarations in "), "{c}");
        for sig in [
            "async fn qa() { ... }",
            "const fn qc() { ... }",
            "unsafe fn qu() { ... }",
            "async unsafe fn qau() { ... }",
            "const unsafe fn qcu() { ... }",
            "unsafe const fn quc() { ... }",
            "pub async unsafe fn qpu() { ... }",
            "pub(crate) const fn qpc() { ... }",
            "const unsafe extern \"C\" fn qce() { ... }",
        ] {
            assert!(c.contains(sig), "missing `{sig}`: {c}");
        }
        // Bodies are cut in every case.
        assert!(!c.contains("{ 9 }"), "{c}");
    }

    /// decl_kind unit pins: every qualifier chain (any order) plus the
    /// boundary cases — a qualifier NOT followed by whitespace is part of a
    /// longer identifier and must not manufacture a fake `fn` (`unsafefn()`
    /// mislabeled as a declaration under the old boundary-less strip), and
    /// `const` items stay out of the skeleton.
    #[test]
    fn decl_kind_pins_qualifier_chains_visibility_and_boundaries() {
        for line in [
            "async fn a()",
            "const fn a()",
            "unsafe fn a()",
            "async unsafe fn a()",
            "const unsafe fn a()",
            "unsafe const fn a()",
            "pub async unsafe fn a()",
            "pub(crate) const fn a()",
            "pub(self) async fn a()",
            "extern fn a()",
            "extern \"C\" fn a()",
            "extern\"C\" fn a()",
            "pub extern \"C\" fn a()",
            "const unsafe extern \"C\" fn a()",
        ] {
            assert_eq!(decl_kind(line), Some("fn"), "{line}");
        }
        assert_eq!(decl_kind("impl<T> Send for W<T> {"), Some("impl"));
        assert_eq!(decl_kind("pub(self) struct S;"), Some("struct"));
        assert_eq!(decl_kind("const MAX: usize = 3;"), None);
        assert_eq!(decl_kind("constants: Vec<u8>,"), None);
        assert_eq!(decl_kind("unsafefn();"), None);
        assert_eq!(decl_kind("asyncfn();"), None);
        assert_eq!(decl_kind("publication note;"), None);
    }

    /// In-family sweep (visibility): `pub(self)` — and any `pub(scope)` the
    /// enumeration forgot — must not drop the declaration. The old prefix
    /// list handled only crate/super/in and silently dropped `pub(self)`.
    #[test]
    fn symbols_mode_pub_self_visibility_not_dropped() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("scope.rs"),
            "pub(self) fn scoped() -> u32 { 1 }\n\npub(in crate::deep) fn deep_scoped() {}\n",
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "scope.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.contains("pub(self) fn scoped() -> u32 { ... }"), "{c}");
        assert!(c.contains("pub(in crate::deep) fn deep_scoped() { ... }"), "{c}");
    }

    /// In-family sweep (generics): `impl<T> Trait for Type` attaches `<`
    /// directly to the keyword and matched no prefix — the whole impl was
    /// dropped from the skeleton.
    #[test]
    fn symbols_mode_generic_impl_kept() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("generic.rs"),
            "impl<T> Send for Wrapper<T> {\n    fn send(&mut self) {}\n}\n",
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "generic.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.contains("impl<T> Send for Wrapper<T> { ... }"), "{c}");
        assert!(c.contains("fn send(&mut self) { ... }"), "{c}");
    }

    /// In-family sweep (body-less signatures): a signature that already ends
    /// in `;` (trait method, extern-block fn, unit struct) has no body to
    /// elide — the old rendering bolted a lying ` { ... }` onto it.
    #[test]
    fn symbols_mode_bodyless_signature_rendered_verbatim() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("bodyless.rs"),
            r#"pub trait Speaker {
    fn speak(&self) -> String;
    fn loud(&self);
}

struct Unit;
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "bodyless.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.contains("fn speak(&self) -> String;"), "{c}");
        assert!(c.contains("fn loud(&self);"), "{c}");
        assert!(c.contains("struct Unit;"), "{c}");
        assert!(!c.contains("; { ... }"), "lying body on a `;` signature: {c}");
    }

    /// M-F fold (cluster packing): the loop stops at the FIRST cluster that
    /// does not fit — best-first means later clusters are worse, so a
    /// break→continue mutant (skip the big one, keep packing smaller later
    /// ones) must go RED: `c_fit.rs` would fit after the oversized
    /// `b_big.rs` but must NOT be rendered, and the marker must count BOTH
    /// omitted clusters. Sizing is path-length robust: a_first + c_fit stay
    /// ≪ the 200-token limit even with long tmpdir paths, while b_big's
    /// 2000-char line can never fit after them.
    #[test]
    fn packing_stops_at_first_non_fitting_cluster() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a_first.rs"), "needle here\n").unwrap();
        fs::write(tmp.path().join("b_big.rs"), format!("needle {}\n", "x".repeat(2000))).unwrap();
        fs::write(tmp.path().join("c_fit.rs"), "needle\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "budget": 200}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.contains("a_first.rs:1"), "best cluster missing: {c}");
        assert!(!c.contains("c_fit.rs:"), "packed past a non-fitting cluster: {c}");
        assert!(c.contains("[more: 2 clusters omitted]"), "{c}");
    }

    /// M-F fold (symbols packing): same policy at the symbols loop — a
    /// declaration that does not fit stops packing even though a later,
    /// smaller one would fit; a break→continue mutant renders `fn tiny()`.
    #[test]
    fn symbols_packing_stops_at_first_non_fitting_decl() {
        let tmp = tempfile::tempdir().unwrap();
        let params = "a: ParamType, ".repeat(12);
        fs::write(
            tmp.path().join("sk.rs"),
            format!("pub fn huge({params}) -> u32 {{ 1 }}\n\nfn tiny() {{}}\n"),
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "sk.rs", "budget": 80}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(!c.contains("fn tiny()"), "packed past a non-fitting declaration: {c}");
        assert!(c.contains("[more: 2 symbols omitted]"), "{c}");
    }

    /// M-K fold: the packing reserve is PROVABLY wide enough — the widest
    /// reachable marker (max omitted count bounded by the byte caps: 64 MiB /
    /// 1 byte-per-line = 8 digits for clusters; 1 MiB / line = 7 digits for
    /// symbols) stays under MARKER_RESERVE_CHARS. RED vs any reserve < 67.
    #[test]
    fn marker_reserve_holds_at_max_reachable_marker_width() {
        let clusters = format!(
            "[more: {} clusters omitted] — raise `budget` or narrow `path`",
            99_999_999usize
        );
        assert!(
            clusters.chars().count() <= MARKER_RESERVE_CHARS,
            "cluster marker {clusters:?} ({} chars) overflows the reserve",
            clusters.chars().count()
        );
        let symbols = format!(
            "[more: {} symbols omitted] — raise `budget` or narrow the file",
            9_999_999usize
        );
        assert!(
            symbols.chars().count() <= MARKER_RESERVE_CHARS,
            "symbols marker {symbols:?} ({} chars) overflows the reserve",
            symbols.chars().count()
        );
    }

    /// Round-3 BLOCKING class 1: the `<=budget` invariant, exercised IN the
    /// marker band. Prior fixtures left so much headroom that deleting
    /// MARKER_RESERVE_CHARS changed nothing — the reserve was pinned only
    /// analytically. Here the budget is calibrated at runtime (band::calibrate)
    /// from MEASURED cluster sizes so the packing genuinely straddles the
    /// ~61-char omitted-marker band: the shipped reserve stays under budget,
    /// while delete (→0) and shrink (→8/16/32) mutants re-pack one more
    /// cluster and overflow. RED vs those mutants; also RED if the fixture
    /// drifts out of the band.
    #[test]
    fn search_packing_exercises_the_marker_reserve_band() {
        // 24 disjoint clusters, uniform scores: hits every 8 lines from 10,
        // all windows fully inside the file, all line numbers rendered.
        let tmp = tempfile::tempdir().unwrap();
        let mut body = String::new();
        for i in 1..=300 {
            let line = if (10..=194).contains(&i) && (i - 10) % 8 == 0 {
                format!("needle {i}\n")
            } else {
                format!("filler {i}\n")
            };
            body.push_str(&line);
        }
        fs::write(tmp.path().join("band.rs"), body).unwrap();
        let ctx = ctx_for(&tmp);

        // Measure pass at the ceiling: every cluster rendered, no marker.
        let measured = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "budget": TGREP_BUDGET_CEILING}),
        );
        assert!(!measured.is_error, "{}", measured.content);
        assert!(
            !measured.content.contains("[more: "),
            "measure pass truncated: {}",
            measured.content
        );
        let (budget, shown96, body96, over0, header_len) =
            band::calibrate(&measured.content, 3);

        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "needle", "budget": budget}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;

        // The marker names exactly the cluster the reserve pushed out.
        let marker_at = c
            .find("[more: ")
            .unwrap_or_else(|| panic!("no omission marker: {c}"));
        let marker_end = c[marker_at..].find(']').unwrap() + marker_at;
        let omitted: usize = c[marker_at..marker_end]
            .trim_start_matches("[more: ")
            .trim_end_matches(" clusters omitted")
            .parse()
            .unwrap();
        assert_eq!(omitted, 24 - shown96, "reserve pushed out exactly one cluster: {c}");

        // The live marker's width matches the packing arithmetic.
        let line_end = c[marker_at..].find('\n').map(|e| marker_at + e).unwrap_or(c.len());
        assert_eq!(
            c[marker_at..line_end].chars().count(),
            band::marker_len(omitted),
            "marker format drifted: {:?}",
            &c[marker_at..line_end]
        );

        // The output is EXACTLY header + reserve-limited body + marker.
        assert_eq!(
            c.chars().count(),
            header_len + 2 + body96 + band::marker_len(omitted), // line1\n + marker\n
            "packing drifted: {c}"
        );

        // The invariant itself — RED vs delete/shrink reserve mutants, which
        // re-pack one more cluster and land `over0` chars past the ceiling.
        assert!(
            c.chars().count() <= budget * 4,
            "output {} chars exceeds budget {budget} tokens ({} chars): {c}",
            c.chars().count(),
            budget * 4
        );

        // Band-sensitivity pins (drift guards, independently RED): the
        // no-reserve packing overflows the budget, tightly.
        assert!(over0 > 0, "fixture drifted out of the reserve band (over0 = {over0})");
        assert!(over0 <= 24, "band too loose: over0 = {over0}");
    }

    /// Round-3 BLOCKING class 2a: suppression inside a block-style test mod
    /// must NOT end at a NESTED close brace (a fn body's indented `}`) — only
    /// the mod's own column-0 `}` releases it, and real declarations after
    /// the mod MUST appear. RED vs the any-indent close-brace mutant
    /// (`hidden_late`, which follows a nested multi-line fn body, leaks).
    #[test]
    fn symbols_test_mod_suppression_survives_nested_close_brace() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("nested.rs"),
            r#"pub fn before() -> u32 { 1 }

mod tests {
    use std::fmt;

    #[test]
    fn hidden_early() {
        assert_eq!(1, 1);
    }

    fn hidden_late() -> u32 {
        42
    }
}

pub struct Real {
    field: u32,
}

pub fn visible_after() -> u32 {
    7
}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "nested.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        // The mod's own column-0 close brace RELEASES suppression.
        assert!(c.starts_with("symbols nested.rs: 3 declarations in "), "{c}");
        assert!(
            c.contains("pub struct Real { ... }"),
            "declaration after the test mod missing: {c}"
        );
        assert!(c.contains("pub fn visible_after() -> u32 { ... }"), "{c}");
        // Everything INSIDE the mod stays suppressed — including the item
        // after a nested fn body's indented close brace.
        assert!(!c.contains("hidden_early"), "{c}");
        assert!(
            !c.contains("hidden_late"),
            "nested close brace released suppression early: {c}"
        );
    }

    /// Round-3 BLOCKING class 2b: the `in_test_mod` reset is load-bearing —
    /// deleting it lets the first block-style `mod tests {` (with multi-line
    /// items) eat the REST OF THE FILE. Every declaration after the mod must
    /// survive. RED vs the reset-deletion mutant.
    #[test]
    fn symbols_declarations_after_block_test_mod_survive_rest_of_file() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(
            tmp.path().join("rest.rs"),
            r#"pub fn before() -> u32 { 1 }

mod tests {
    fn hidden_one() {
        helper();
    }

    fn hidden_two() -> u32 {
        5
    }
}

pub struct After {
    field: u32,
}

impl After {
    pub fn method(&self) -> u32 {
        self.field
    }
}

pub fn last_survivor() {}
"#,
        )
        .unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "rest.rs"}),
        );
        assert!(!result.is_error, "{}", result.content);
        let c = &result.content;
        assert!(c.starts_with("symbols rest.rs: 5 declarations in "), "{c}");
        assert!(c.contains("pub fn before() -> u32 { ... }"), "{c}");
        assert!(
            c.contains("pub struct After { ... }"),
            "rest of the file eaten after the test mod: {c}"
        );
        assert!(c.contains("impl After { ... }"), "{c}");
        assert!(c.contains("pub fn method(&self) -> u32 { ... }"), "{c}");
        assert!(c.contains("pub fn last_survivor() { ... }"), "{c}");
        assert!(!c.contains("hidden_one"), "{c}");
        assert!(!c.contains("hidden_two"), "{c}");
    }

    /// Determinism + speed (spec req 2), as two legs with DIFFERENT
    /// robustness classes. The fixture is calibrated to this repo's src/
    /// scale (26 files, ~29k lines, ~1.2 MB, measured 2026-09-26) — the
    /// 3-file toy fixture this test shipped with could not support the
    /// "repo-sized" claim in its own doc comment.
    ///
    /// DETERMINISM leg (byte-strong, load-immune): identical calls give
    /// byte-identical output. Pure — no wall clock involved.
    ///
    /// SPEED leg (T74 doctrine: widen keeping the discrimination, document
    /// the classification — round-4 fix-up). The original single-run
    /// `<100ms` pin was the T31/T74/T66 false-red class: the corpus'
    /// latency band on this host is 70–140ms under scheduler load
    /// (validator-observed RED on the round-4 baseline `cargo test`; local
    /// reproduction 32–67ms unloaded, 82–83ms under 4-way load), which
    /// STRADDLES any sub-100ms pin — and every cycle's goal gate runs
    /// `cargo test`, so a load-straddling pin blocks any future wrap. What
    /// this leg exists to catch is an ALGORITHMIC blowup — an accidental
    /// O(n²) cluster merge or quadratic corpus re-scan — which shows up as
    /// 10–100x the band (0.7–14s), not as the ±30% wobble of ordinary
    /// contention. So: median of 5 runs against a bound of 1500ms ≈ 10.7x
    /// the top of the measured band. The median discards scattered load
    /// spikes (tripping it needs ≥3 of 5 samples each ≥10x the band top —
    /// a sustained order-of-magnitude slowdown, not noise); constant-factor
    /// drift of ≲10x is OUT of scope for this leg, which is what the
    /// structure/budget asserts on this module pin instead. RED-proven both
    /// directions 2026-09-26: an injected 1.6s sleep in the timed region
    /// turns it RED (median ≈1.6s > 1.5s); revert is green; the shipped
    /// bound stayed green across the consecutive full-suite runs counted in
    /// the round-4 commit message.
    #[test]
    fn deterministic_and_fast() {
        /// SPEED-leg sample count (median = samples[N/2]).
        const SPEED_SAMPLES: usize = 5;
        /// SPEED-leg bound: ≈10.7x the top of the measured 70–140ms band
        /// (see the test's doc comment for the classification).
        const SPEED_BOUND: std::time::Duration = std::time::Duration::from_millis(1500);

        let tmp = tempfile::tempdir().unwrap();
        let files = 26;
        let lines_per_file = 1105; // 26 * 1105 ≈ 28.7k lines ≈ repo src/
        for f in 0..files {
            let mut body = String::with_capacity(lines_per_file * 40);
            for l in 0..lines_per_file {
                if l % 110 == 0 {
                    body.push_str(&format!("fn needle_{f}_{l}(alpha: u32, beta: u32) {{}}\n"));
                } else {
                    body.push_str(&format!("// filler {f}:{l} padding padding padding\n"));
                }
            }
            fs::write(tmp.path().join(format!("mod{f}.rs")), body).unwrap();
        }
        let ctx = ctx_for(&tmp);
        let input = json!({"query": "alpha beta"});
        // DETERMINISM leg: the original pair assert, byte-identical output.
        let start = Instant::now();
        let first = dispatch(&ctx, "tgrep", &input);
        let mut samples = vec![start.elapsed()];
        let start = Instant::now();
        let second = dispatch(&ctx, "tgrep", &input);
        samples.push(start.elapsed());
        assert_eq!(first.content, second.content);
        // SPEED-leg samples 3..=5 — each also pinned byte-identical to the
        // first, so the determinism pin now covers every timed run.
        for _ in 2..SPEED_SAMPLES {
            let start = Instant::now();
            let run = dispatch(&ctx, "tgrep", &input);
            samples.push(start.elapsed());
            assert_eq!(run.content, first.content);
        }
        // SPEED leg: median-of-5 vs the load-robust bound (doc comment above).
        samples.sort();
        let median = samples[SPEED_SAMPLES / 2];
        assert!(
            median < SPEED_BOUND,
            "tgrep median-of-{} latency {:?} exceeds the {:?} load-robust bound (samples: {:?})",
            SPEED_SAMPLES,
            median,
            SPEED_BOUND,
            samples
        );
    }

    /// mod_name resolves the module name through `pub`-visibility prefixes
    /// (NB-c): pub/pub(crate)/pub(super)/pub(in …) all strip.
    #[test]
    fn mod_name_strips_pub_prefixes() {
        assert_eq!(mod_name("mod plain;"), Some("plain".to_string()));
        assert_eq!(mod_name("pub mod tests {"), Some("tests".to_string()));
        assert_eq!(
            mod_name("pub(crate) mod fixtures;"),
            Some("fixtures".to_string())
        );
        assert_eq!(
            mod_name("pub(super) mod inner;"),
            Some("inner".to_string())
        );
        assert_eq!(
            mod_name("pub(in crate::x) mod scoped;"),
            Some("scoped".to_string())
        );
        assert_eq!(mod_name("fn not_a_mod()"), None);
    }

    /// Glob paths honor the same per-file size cap as walked directories
    /// (NB-b: the glob arm used to happily read a multi-MB file the walk arm
    /// skipped) and say so in an honesty note. RED pre-fix: the big file's
    /// clusters appeared and no note was printed.
    #[test]
    fn glob_path_skips_oversized_files_with_note() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("data")).unwrap();
        fs::write(tmp.path().join("data/small.rs"), "needle here\n").unwrap();
        let big = format!("// {}\n{}", "x".repeat(80), "needle big\n".repeat(100_000));
        assert!(big.len() as u64 > TGREP_MAX_FILE_BYTES);
        fs::write(tmp.path().join("data/big.rs"), &big).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle", "path": "data/*.rs"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("small.rs:1"), "{}", result.content);
        assert!(
            !result.content.contains("needle big"),
            "oversized file was read: {}",
            result.content
        );
        assert!(
            result.content.contains("[skipped 1 file(s) over the"),
            "{}",
            result.content
        );
    }

    // ---- T134 F1 (kimi validator follow-up): the corpus glob arm was the
    // content-leak leg of the glob-metachar bypass — `resolve_safe` accepted
    // `**/passwd` (metachar components are lexically missing, so stage 2 had
    // nothing to confine) and the `glob` crate expanded it THROUGH an in-tree
    // symlink to an external directory, then the arm read the matched files'
    // bytes. The validator exfiltrated /etc/passwd first lines end-to-end
    // with a byte-exact resolve_safe + chug's own glob crate. The arm now
    // confines the pattern's literal prefix and drops every concrete match
    // whose real resolution escapes cwd before a byte is read. ----

    /// The validator's two demo patterns against an in-tree symlink to an
    /// external directory: no external byte may reach the result. RED
    /// pre-fix (`**/passwd` and `*/*passwd*` both read the external files).
    #[cfg(unix)]
    #[test]
    fn t134f1_tgrep_corpus_glob_never_reads_outside_sandbox() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("passwd"), "root:RED-TEST-MARK\n").unwrap();
        fs::create_dir_all(outside.path().join("pam.d")).unwrap();
        fs::write(outside.path().join("pam.d/passwd"), "root:NESTED-MARK\n").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("etcdir")).unwrap();
        fs::write(tmp.path().join("local.txt"), "root: in-tree haystack root\n").unwrap();
        let ctx = ctx_for(&tmp);

        for raw in ["**/passwd", "*/*passwd*"] {
            let result = dispatch(&ctx, "tgrep", &json!({"query": "root", "path": raw}));
            assert!(!result.is_error, "{raw}: {}", result.content);
            assert!(
                !result.content.contains("RED-TEST-MARK"),
                "{raw} leaked external content: {}",
                result.content
            );
            assert!(
                !result.content.contains("NESTED-MARK"),
                "{raw} leaked external content: {}",
                result.content
            );
            assert!(
                !result.content.contains("etcdir/"),
                "{raw} leaked an external-resolving path: {}",
                result.content
            );
        }
        // The inverse pin: in-tree glob search still works.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "haystack", "path": "*.txt"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("local.txt:1"), "{}", result.content);
    }

    /// Class sweep — tgrep's OTHER path surfaces. The default no-path walk
    /// uses `entry.file_type()` (symlinks are neither dirs to recurse nor
    /// files to read) and symbols mode rides strict `resolve_safe`: both
    /// stay green pre-fix, pinned here so the class can't regress through
    /// them. A pattern rooted AT the outside symlink is refused up front by
    /// the literal-prefix confinement.
    #[cfg(unix)]
    #[test]
    fn t134f1_tgrep_walk_and_symbols_never_cross_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("leak.rs"), "red-test-mark here\n").unwrap();
        std::os::unix::fs::symlink(outside.path(), tmp.path().join("etcdir")).unwrap();
        fs::write(tmp.path().join("keep.rs"), "harmless\n").unwrap();
        let ctx = ctx_for(&tmp);

        // No path → walk arm: the external file is neither entered nor read.
        // (Assert on the external FILENAME, not the marker: the header
        // echoes the query verbatim.)
        let result = dispatch(&ctx, "tgrep", &json!({"query": "red-test-mark"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(
            !result.content.contains("leak.rs"),
            "walk followed the symlink: {}",
            result.content
        );
        assert!(result.content.contains("searched 1 files"), "{}", result.content);
        // Pattern rooted at the symlinked dir: refused before globbing.
        let result = dispatch(&ctx, "tgrep", &json!({"query": "mark", "path": "etcdir/*.rs"}));
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("escapes cwd"), "{}", result.content);
        // Symbols mode: the single-file path is stage-2 confined too.
        let result = dispatch(
            &ctx,
            "tgrep",
            &json!({"query": "ignored", "symbols": true, "path": "etcdir/leak.rs"}),
        );
        assert!(result.is_error, "{}", result.content);
        assert!(result.content.contains("escapes cwd"), "{}", result.content);
    }

    /// Single-file paths honor the size cap too (consistency): an oversized
    /// file is skipped with the same honest note, not read.
    #[test]
    fn single_file_path_skips_oversized_file_with_note() {
        let tmp = tempfile::tempdir().unwrap();
        let big = "needle big\n".repeat(120_000);
        assert!(big.len() as u64 > TGREP_MAX_FILE_BYTES);
        fs::write(tmp.path().join("huge.rs"), &big).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle", "path": "huge.rs"}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("0 clusters"), "{}", result.content);
        assert!(
            !result.content.contains("needle big"),
            "oversized file was read: {}",
            result.content
        );
        assert!(
            result.content.contains("is over the"),
            "{}",
            result.content
        );
    }

    /// Directory walks carry the same honesty note when they skip oversized
    /// files (walk/glob/single-file size-cap arms consistent end to end).
    #[test]
    fn dir_walk_skips_oversized_files_with_note() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("small.rs"), "needle here\n").unwrap();
        let big = "needle big\n".repeat(120_000);
        fs::write(tmp.path().join("big.rs"), &big).unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "needle", "path": "."}));
        assert!(!result.is_error, "{}", result.content);
        assert!(result.content.contains("small.rs:1"), "{}", result.content);
        assert!(
            !result.content.contains("needle big"),
            "oversized file was read: {}",
            result.content
        );
        assert!(
            result.content.contains("[skipped 1 file(s) over the"),
            "{}",
            result.content
        );
    }
}

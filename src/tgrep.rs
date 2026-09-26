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

use crate::tools::{resolve_safe, ToolCtx, ToolResult};

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
const SYMBOL_SIG_LOOKAHEAD: usize = 8;
/// Chars reserved for the omission marker when packing, so header + clusters
/// + marker together never exceed the budget.
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
                "path": {"type": "string", "description": "Optional glob (e.g. `src/**/*.rs`), directory, or file relative to cwd; default the whole cwd. In symbols mode: the Rust file to skeleton"},
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
            let capped_note = stats
                .capped
                .then(|| format!("[search truncated at the {} file / byte cap]", files.len()));
            Ok(Corpus { files, capped_note })
        }
        Some(raw) => {
            let resolved = resolve_safe(&ctx.cwd, raw).map_err(|e| anyhow!("{e}"))?;
            if raw.contains('*') || raw.contains('?') || raw.contains('[') {
                let pattern = resolved.to_string_lossy().into_owned();
                let mut files: Vec<PathBuf> = glob::glob(&pattern)
                    .map_err(|e| anyhow!("invalid glob pattern: {e}"))?
                    .filter_map(|p| p.ok())
                    .filter(|p| p.is_file())
                    .collect();
                let capped_note = (files.len() > TGREP_MAX_FILES).then(|| {
                    files.truncate(TGREP_MAX_FILES);
                    format!("[search truncated at the {TGREP_MAX_FILES} file cap]")
                });
                return Ok(Corpus { files, capped_note });
            }
            if resolved.is_file() {
                return Ok(Corpus {
                    files: vec![resolved],
                    capped_note: None,
                });
            }
            if resolved.is_dir() {
                let mut files = Vec::new();
                let mut stats = WalkStats::default();
                walk_dir(&resolved, &mut files, &mut stats);
                let capped_note = stats
                    .capped
                    .then(|| format!("[search truncated at the {} file / byte cap]", files.len()));
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
        // Extend the window while the next seed's window overlaps it.
        let first = seeds[idx];
        let mut last = first;
        let mut members = vec![first];
        while idx + 1 < seeds.len() && seeds[idx + 1] <= last + TGREP_WINDOW {
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
/// - a `mod <name containing "test">` suppresses everything until the
///   matching column-0 `}` (rustfmt closes top-level items at column 0), so
///   test helpers do not drown the real skeleton.
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
    let mut suppress_tests: Option<()> = None;
    let mut i = 0; // 0-based
    while i < lines.len() {
        let line = lines[i];
        let lineno = i + 1;
        if let Some(()) = suppress_tests {
            // Inside a test module: resume at the column-0 close brace.
            if line.starts_with('}') {
                suppress_tests = None;
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
            if kind == "mod" && mod_name(trimmed).is_some_and(|n| n.contains("test")) {
                suppress_tests = Some(());
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
            sig.push_str(" { ... }");
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
    })
}

/// The declaration keyword a (trimmed) line opens with, after the `pub` and
/// fn-qualifier prefixes. `None` = not a declaration line.
fn decl_kind(trimmed: &str) -> Option<&'static str> {
    let mut t = trimmed;
    // Most specific first: `pub(crate)` must win over `pub`.
    for prefix in ["pub(crate)", "pub(super)", "pub(in", "pub"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            let rest = rest.trim_start();
            // `pub(in path)` carries a parenthesized scope.
            let rest = if prefix == "pub(in" {
                match rest.find(')') {
                    Some(pos) => rest[pos + 1..].trim_start(),
                    None => rest,
                }
            } else {
                rest
            };
            t = rest;
            break;
        }
    }
    // fn qualifiers: `async fn`, `unsafe fn`, `const fn`, `extern "C" fn`.
    for qualifier in ["async", "unsafe", "const"] {
        if let Some(rest) = t.strip_prefix(qualifier) {
            let rest = rest.trim_start();
            if rest.starts_with("fn ") || rest.starts_with("fn(") {
                return Some("fn");
            }
            // A qualifier we do not follow (e.g. `const X: u32 = ...`) is a
            // const item — not part of the skeleton.
            if qualifier == "const" {
                return None;
            }
            t = rest;
            break;
        }
    }
    if let Some(rest) = t.strip_prefix("extern") {
        let rest = rest.trim_start();
        let rest = rest.strip_prefix('"').map(|r| match r.find('"') {
            Some(end) => r[end + 1..].trim_start(),
            None => rest,
        }).unwrap_or(rest);
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
        ("type ", "type"),
        ("mod ", "mod"),
    ] {
        if t.starts_with(prefix) {
            return Some(kind);
        }
    }
    None
}

/// The module name of a `mod <name>` line.
fn mod_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("mod ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::dispatch;
    use serde_json::json;
    use std::time::Instant;

    fn ctx_for(tmp: &tempfile::TempDir) -> ToolCtx {
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

    /// Determinism: identical calls give byte-identical output (spec req 2),
    /// and a repo-sized search is fast (<100ms, spec req 2).
    #[test]
    fn deterministic_and_fast() {
        let tmp = tempfile::tempdir().unwrap();
        tier_fixtures(&tmp);
        let ctx = ctx_for(&tmp);
        let input = json!({"query": "alpha beta"});
        let start = Instant::now();
        let first = dispatch(&ctx, "tgrep", &input);
        let elapsed = start.elapsed();
        let second = dispatch(&ctx, "tgrep", &input);
        assert_eq!(first.content, second.content);
        assert!(elapsed < std::time::Duration::from_millis(100), "{elapsed:?}");
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

    /// Path-basename boost: a same-tier cluster in a file whose basename
    /// carries a query term outranks one that does not (spec req 2).
    #[test]
    fn basename_boost_breaks_density_ties() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("delegate.rs"), "pad\nlaunch handling here;\npad\n").unwrap();
        fs::write(tmp.path().join("elsewhere.rs"), "pad\nlaunch handling here;\npad\n").unwrap();
        let ctx = ctx_for(&tmp);
        let result = dispatch(&ctx, "tgrep", &json!({"query": "launch"}));
        assert!(!result.is_error, "{}", result.content);
        let d = result.content.find("delegate.rs:").unwrap();
        let e = result.content.find("elsewhere.rs:").unwrap();
        assert!(d < e, "basename boost lost: {}", result.content);
    }
}

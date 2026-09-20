//! SPEC-5 §3 foundation: Tab-completion engine for the chat input dock.
//!
//! Everything here is pure and terminal-free so the TUI round can wire it up
//! with thin glue:
//! - [`token_at_cursor`] finds the whitespace-delimited word ending at the
//!   cursor (byte-safe).
//! - `/`-tokens complete against [`SLASH_COMMANDS`] (prefix, case-sensitive)
//!   via [`slash_candidates`].
//! - `@`-tokens complete against a lazily-built, 30s-cached [`FileIndex`] of
//!   paths under the chat cwd (`git ls-files` inside a repo, a bounded tree
//!   walk otherwise) via [`FileIndex::candidates`] — case-insensitive
//!   substring match, basename hits first, then shorter paths, capped.
//! - [`decide`] turns a candidate list into the spec's behavior: single
//!   candidate completes inline; multiple complete to the longest common
//!   prefix; if that changes nothing, a [`CandidateStrip`] opens and
//!   Tab/Shift-Tab cycle it with wraparound.
//!
//! Wired into the TUI in a later round.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Slash commands offered for `/`-token completion, in `/help` order.
/// (Mirrors the chat parser's command set; kept here so the TUI round can
/// build both completion and help from one list.)
pub const SLASH_COMMANDS: &[&str] = &[
    "spec", "goal", "check", "model", "budget", "ledger", "quit", "help",
];

/// Rebuild the file index when the cached one is this old (SPEC-5 §3: 30s).
const INDEX_TTL: Duration = Duration::from_secs(30);

/// Walk-fallback depth limit (SPEC-5 §3): files deeper than this many path
/// components below the cwd are not indexed.
const WALK_MAX_DEPTH: usize = 6;

/// Walk-fallback entry cap (SPEC-5 §3).
const WALK_MAX_ENTRIES: usize = 5000;

/// Candidate cap for a single completion request (SPEC-5 §3).
pub const MAX_CANDIDATES: usize = 20;

/// Directory names the walk fallback skips at any depth (SPEC-5 §3).
const WALK_SKIP_DIRS: &[&str] = &[".git", "target", "node_modules"];

// --- token at cursor ---

/// The whitespace-delimited word ending at `cursor_byte`, as
/// `(start_byte, token)`. `cursor_byte` is clamped to the line and backed off
/// to a char boundary, so any byte offset is safe. The token may be empty
/// (cursor at the line start or right after whitespace).
pub fn token_at_cursor(line: &str, cursor_byte: usize) -> (usize, &str) {
    let mut cursor = cursor_byte.min(line.len());
    while !line.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let mut start = cursor;
    while start > 0 {
        let prev = line[..start].chars().next_back().expect("start > 0");
        if prev.is_whitespace() {
            break;
        }
        start -= prev.len_utf8();
    }
    (start, &line[start..cursor])
}

// --- slash commands ---

/// Candidates for a `/`-token (the token includes the leading `/`).
/// Case-sensitive prefix match on the command name; a bare `/` lists every
/// command. Candidates include the leading `/` so they can replace the token
/// verbatim. Empty for tokens that do not start with `/`.
pub fn slash_candidates(token: &str) -> Vec<String> {
    let Some(prefix) = token.strip_prefix('/') else {
        return Vec::new();
    };
    SLASH_COMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(prefix))
        .map(|cmd| format!("/{cmd}"))
        .collect()
}

// --- file index ---

/// Lazily-built, briefly-cached index of file paths (relative, `/`-joined)
/// under a chat cwd. Inside a git repo the source is `git ls-files` (fast,
/// gitignore-aware); otherwise a bounded tree walk (see [`WALK_MAX_DEPTH`],
/// [`WALK_MAX_ENTRIES`], [`WALK_SKIP_DIRS`]). The index is built on first
/// use and rebuilt only once it is older than [`INDEX_TTL`].
pub struct FileIndex {
    cwd: PathBuf,
    paths: Vec<String>,
    built_at: Option<Instant>,
}

impl FileIndex {
    pub fn new(cwd: &Path) -> Self {
        Self {
            cwd: cwd.to_path_buf(),
            paths: Vec::new(),
            built_at: None,
        }
    }

    /// The indexed paths, building or rebuilding the index first if stale.
    pub fn paths(&mut self) -> &[String] {
        self.freshen(Instant::now());
        &self.paths
    }

    /// Completion candidates for an `@`-token query (the part after `@`):
    /// case-insensitive substring match over the index, ranked and capped.
    pub fn candidates(&mut self, query: &str) -> Vec<String> {
        self.freshen(Instant::now());
        match_paths(&self.paths, query, MAX_CANDIDATES)
    }

    /// Build if never built or if the cached index is at least [`INDEX_TTL`]
    /// old. `now` is a parameter (rather than read here) so cache expiry is
    /// testable without sleeping.
    fn freshen(&mut self, now: Instant) {
        if needs_rebuild(self.built_at, now) {
            self.paths = build_index(&self.cwd);
            self.built_at = Some(now);
        }
    }
}

/// The cache-expiry rule, kept pure for tests: an index built at `built_at`
/// is rebuilt once it is [`INDEX_TTL`] old; `None` (never built) always
/// rebuilds. `saturating_duration_since` makes a `now` earlier than
/// `built_at` (clock weirdness) read as fresh.
fn needs_rebuild(built_at: Option<Instant>, now: Instant) -> bool {
    match built_at {
        None => true,
        Some(at) => now.saturating_duration_since(at) >= INDEX_TTL,
    }
}

/// Index the paths under `cwd`: `git ls-files` when inside a git repo,
/// the bounded walk otherwise.
fn build_index(cwd: &Path) -> Vec<String> {
    git_ls_files(cwd).unwrap_or_else(|| walk_tree(cwd, WALK_MAX_DEPTH, WALK_MAX_ENTRIES))
}

/// `git ls-files -z` in `cwd`: tracked paths relative to `cwd`, or `None`
/// when `cwd` is not inside a git repo (or git is unavailable/fails), which
/// sends the caller to the walk fallback.
fn git_ls_files(cwd: &Path) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut paths: Vec<String> = out
        .stdout
        .split(|&b| b == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| String::from_utf8_lossy(entry).into_owned())
        .collect();
    paths.sort();
    Some(paths)
}

/// Bounded recursive walk: files only, sorted, skipping [`WALK_SKIP_DIRS`]
/// at any depth, not descending below `max_depth` components, capped at
/// `max_entries`. Parameters (rather than the constants) so tests can probe
/// the limits with tiny trees.
fn walk_tree(root: &Path, max_depth: usize, max_entries: usize) -> Vec<String> {
    let mut out = Vec::new();
    walk_into(root, root, 0, max_depth, max_entries, &mut out);
    out.sort();
    out
}

/// List `dir` (at `depth` components below `root`) into `out`. Files land at
/// `depth + 1`, so a `dir` at `max_depth` is not listed at all — its files
/// would be too deep.
fn walk_into(
    root: &Path,
    dir: &Path,
    depth: usize,
    max_depth: usize,
    max_entries: usize,
    out: &mut Vec<String>,
) {
    if depth >= max_depth || out.len() >= max_entries {
        return;
    }
    let Ok(read) = fs::read_dir(dir) else {
        return;
    };
    // Sorted per directory so the capped prefix of entries is deterministic.
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= max_entries {
            return;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            if WALK_SKIP_DIRS.iter().any(|skip| name == **skip) {
                continue;
            }
            walk_into(root, &entry.path(), depth + 1, max_depth, max_entries, out);
        } else if kind.is_file()
            && let Ok(rel) = entry.path().strip_prefix(root)
        {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

// --- matching & ranking ---

/// Case-insensitive substring matches of `query` against `index`, ranked:
/// paths whose basename matches first, then shorter paths, then
/// lexicographic for determinism. Capped at `cap`.
pub fn match_paths(index: &[String], query: &str, cap: usize) -> Vec<String> {
    let query = query.to_lowercase();
    let mut scored: Vec<(u8, usize, &String)> = index
        .iter()
        .filter(|path| path.to_lowercase().contains(&query))
        .map(|path| {
            let basename = path.rsplit('/').next().unwrap_or(path);
            // Basename hits rank first: hit -> 0, miss -> 1.
            let rank = u8::from(!basename.to_lowercase().contains(&query));
            (rank, path.len(), path)
        })
        .collect();
    scored.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
    scored.truncate(cap);
    scored.into_iter().map(|(_, _, path)| path.clone()).collect()
}

/// Longest common prefix of `candidates` (`""` for an empty list). Backed
/// off to a char boundary: a byte-wise common prefix can end inside a
/// multibyte char (e.g. "é…" vs "è…" share one byte), and the result must be
/// valid to splice into the input line.
pub fn longest_common_prefix(candidates: &[String]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut end = first.len();
    for other in &candidates[1..] {
        end = first
            .bytes()
            .zip(other.bytes())
            .take(end)
            .take_while(|(a, b)| a == b)
            .count();
        while !first.is_char_boundary(end) {
            end -= 1;
        }
    }
    first[..end].to_string()
}

// --- completion decision & candidate strip ---

/// What a Tab press should do with the candidates, per SPEC-5 §3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No candidates: leave the token alone.
    NoMatch,
    /// Exactly one candidate: replace the token with it inline, done.
    Inline(String),
    /// Multiple candidates with a common prefix longer than the typed token:
    /// replace the token with the prefix (the strip opens on the next Tab if
    /// still ambiguous).
    Extend(String),
    /// Multiple candidates but the common prefix adds nothing: open the
    /// candidate strip.
    OpenStrip(Vec<String>),
}

/// Turn a candidate list into a completion action. `typed` is the token text
/// the candidates match against (without the `@` for file tokens, with the
/// `/` for slash tokens); `Inline`/`Extend` payloads replace that span
/// verbatim.
pub fn decide(typed: &str, candidates: Vec<String>) -> Decision {
    match candidates.len() {
        0 => Decision::NoMatch,
        1 => Decision::Inline(candidates.into_iter().next().expect("len 1")),
        _ => {
            let prefix = longest_common_prefix(&candidates);
            if prefix != typed {
                Decision::Extend(prefix)
            } else {
                Decision::OpenStrip(candidates)
            }
        }
    }
}

/// The one-row candidate strip shown above the input dock when completion is
/// ambiguous: Tab/Shift-Tab cycle the highlight with wraparound, and the
/// highlighted candidate replaces the token when accepted. Pure state; the
/// TUI round renders it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateStrip {
    candidates: Vec<String>,
    highlighted: usize,
}

impl CandidateStrip {
    /// A strip over `candidates`; `None` for an empty list (no strip to show).
    pub fn new(candidates: Vec<String>) -> Option<Self> {
        if candidates.is_empty() {
            None
        } else {
            Some(Self {
                candidates,
                highlighted: 0,
            })
        }
    }

    /// All candidates, in display order.
    pub fn candidates(&self) -> &[String] {
        &self.candidates
    }

    /// Index of the highlighted candidate.
    pub fn highlighted_index(&self) -> usize {
        self.highlighted
    }

    /// The highlighted candidate (what accepting the strip inserts).
    pub fn highlighted(&self) -> &str {
        &self.candidates[self.highlighted]
    }

    /// Tab: highlight the next candidate, wrapping at the end.
    pub fn next(&mut self) {
        self.highlighted = (self.highlighted + 1) % self.candidates.len();
    }

    /// Shift-Tab: highlight the previous candidate, wrapping at the start.
    pub fn prev(&mut self) {
        self.highlighted = (self.highlighted + self.candidates.len() - 1)
            % self.candidates.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // --- token_at_cursor ---

    #[test]
    fn token_ends_at_cursor() {
        assert_eq!(token_at_cursor("hello @driv", 11), (6, "@driv"));
        assert_eq!(token_at_cursor("/he", 3), (0, "/he"));
        assert_eq!(token_at_cursor("see @a.rs, please", 9), (4, "@a.rs"));
    }

    #[test]
    fn token_stops_at_whitespace_before_cursor() {
        // Cursor right after "hello": the token is "hello".
        assert_eq!(token_at_cursor("hello @driv", 5), (0, "hello"));
        // Cursor right after the space: empty token.
        assert_eq!(token_at_cursor("hello @driv", 6), (6, ""));
        assert_eq!(token_at_cursor("hello @driv", 0), (0, ""));
    }

    #[test]
    fn token_cursor_is_clamped_and_char_boundary_safe() {
        // Past-the-end clamps to the line end.
        assert_eq!(token_at_cursor("@ab", 100), (0, "@ab"));
        // "héllo" — é is 2 bytes; a cursor inside it backs off to before it.
        let line = "héllo";
        let inside_e = 2; // between the é's two bytes
        assert_eq!(token_at_cursor(line, inside_e), (0, "h"));
        assert_eq!(token_at_cursor(line, line.len()), (0, "héllo"));
    }

    // --- slash candidates ---

    #[test]
    fn slash_prefix_match_includes_leading_slash() {
        assert_eq!(slash_candidates("/he"), vec!["/help"]);
        assert_eq!(slash_candidates("/"), SLASH_COMMANDS.iter().map(|c| format!("/{c}")).collect::<Vec<_>>());
        assert_eq!(slash_candidates("/b"), vec!["/budget"]);
        assert!(slash_candidates("/xyzzy").is_empty());
    }

    #[test]
    fn slash_match_is_case_sensitive_and_requires_slash() {
        assert!(slash_candidates("/He").is_empty());
        assert!(slash_candidates("help").is_empty());
        assert!(slash_candidates("").is_empty());
    }

    // --- longest_common_prefix ---

    #[test]
    fn common_prefix_basics() {
        assert_eq!(longest_common_prefix(&strings(&["src/driver.rs", "src/drill.rs"])), "src/dri");
        assert_eq!(longest_common_prefix(&strings(&["abc", "abd", "ab"])), "ab");
        assert_eq!(longest_common_prefix(&strings(&["abc"])), "abc");
        assert_eq!(longest_common_prefix(&strings(&["abc", "xyz"])), "");
        assert_eq!(longest_common_prefix(&[]), "");
    }

    #[test]
    fn common_prefix_never_splits_a_char() {
        // é (C3 A9) and è (C3 A8) share one byte; the prefix must be "".
        assert_eq!(longest_common_prefix(&strings(&["éx", "èy"])), "");
        assert_eq!(longest_common_prefix(&strings(&["aé", "aè"])), "a");
    }

    // --- decide ---

    #[test]
    fn decide_single_candidate_completes_inline() {
        assert_eq!(
            decide("driv", strings(&["src/driver.rs"])),
            Decision::Inline("src/driver.rs".to_string())
        );
        assert_eq!(
            decide("/he", strings(&["/help"])),
            Decision::Inline("/help".to_string())
        );
    }

    #[test]
    fn decide_no_candidates() {
        assert_eq!(decide("nope", Vec::new()), Decision::NoMatch);
    }

    #[test]
    fn decide_multiple_extends_to_common_prefix() {
        // Typed "dri" extends to the candidates' common prefix "src/dri".
        assert_eq!(
            decide("dri", strings(&["src/driver.rs", "src/drill.rs"])),
            Decision::Extend("src/dri".to_string())
        );
    }

    #[test]
    fn decide_unchanged_prefix_opens_strip() {
        let candidates = strings(&["src/driver.rs", "src/drill.rs"]);
        // Typed text already equals the common prefix: nothing to extend.
        assert_eq!(
            decide("src/dri", candidates.clone()),
            Decision::OpenStrip(candidates)
        );
    }

    // --- CandidateStrip ---

    #[test]
    fn strip_cycles_with_wraparound() {
        let mut strip = CandidateStrip::new(strings(&["a", "b", "c"])).unwrap();
        assert_eq!(strip.highlighted(), "a");
        strip.next();
        strip.next();
        assert_eq!(strip.highlighted(), "c");
        assert_eq!(strip.highlighted_index(), 2);
        strip.next(); // wraps to the start
        assert_eq!(strip.highlighted(), "a");
        strip.prev(); // wraps to the end
        assert_eq!(strip.highlighted(), "c");
        strip.prev();
        assert_eq!(strip.highlighted(), "b");
    }

    #[test]
    fn strip_single_candidate_stays_put_and_empty_is_none() {
        let mut strip = CandidateStrip::new(strings(&["only"])).unwrap();
        strip.next();
        strip.prev();
        assert_eq!(strip.highlighted(), "only");
        assert!(CandidateStrip::new(Vec::new()).is_none());
    }

    // --- match_paths ranking ---

    #[test]
    fn match_is_case_insensitive_substring() {
        let index = strings(&["Src/Driver.rs", "docs/DRIVERS.md", "README"]);
        assert_eq!(match_paths(&index, "driv", 20), strings(&["Src/Driver.rs", "docs/DRIVERS.md"]));
    }

    #[test]
    fn match_ranks_basename_first_then_shorter() {
        let index = strings(&[
            "driver/docs/readme.md", // matches in a dir component only
            "docs/drivers-guide.md", // basename hit, longer
            "src/driver.rs",         // basename hit, shorter
        ]);
        assert_eq!(
            match_paths(&index, "driv", 20),
            strings(&["src/driver.rs", "docs/drivers-guide.md", "driver/docs/readme.md"])
        );
    }

    #[test]
    fn match_caps_candidates() {
        let index: Vec<String> = (0..30).map(|n| format!("f{n:02}.rs")).collect();
        let out = match_paths(&index, "f", 20);
        assert_eq!(out.len(), 20);
        // Shortest-first with a lexicographic tiebreak: f0..f9 then f10...
        assert_eq!(out[0], "f00.rs");
    }

    #[test]
    fn match_empty_query_matches_all_capped() {
        let index = strings(&["bbb.rs", "a.rs"]);
        assert_eq!(match_paths(&index, "", 20), strings(&["a.rs", "bbb.rs"]));
    }

    // --- file index: walk fallback ---

    #[test]
    fn walk_finds_files_relative_and_sorted() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/main.rs"), "").unwrap();
        fs::write(tmp.path().join("README.md"), "").unwrap();
        assert_eq!(
            walk_tree(tmp.path(), WALK_MAX_DEPTH, WALK_MAX_ENTRIES),
            strings(&["README.md", "src/main.rs"])
        );
    }

    #[test]
    fn walk_skips_git_target_and_node_modules() {
        let tmp = tempfile::tempdir().unwrap();
        for dir in [".git", "target", "node_modules", "src/target"] {
            fs::create_dir_all(tmp.path().join(dir)).unwrap();
            fs::write(tmp.path().join(dir).join("skip.rs"), "").unwrap();
        }
        fs::write(tmp.path().join("keep.rs"), "").unwrap();
        assert_eq!(
            walk_tree(tmp.path(), WALK_MAX_DEPTH, WALK_MAX_ENTRIES),
            strings(&["keep.rs"])
        );
    }

    #[test]
    fn walk_respects_depth_limit() {
        let tmp = tempfile::tempdir().unwrap();
        // File at depth 3 (d1/d2/f.rs): included when max_depth is 3...
        fs::create_dir_all(tmp.path().join("d1/d2/d3")).unwrap();
        fs::write(tmp.path().join("d1/d2/ok.rs"), "").unwrap();
        // ...file at depth 4 (d1/d2/d3/deep.rs): excluded.
        fs::write(tmp.path().join("d1/d2/d3/deep.rs"), "").unwrap();
        assert_eq!(walk_tree(tmp.path(), 3, 100), strings(&["d1/d2/ok.rs"]));
        // Depth 6 (the real limit) reaches d1/d2/d3/deep.rs (depth 4).
        assert_eq!(
            walk_tree(tmp.path(), WALK_MAX_DEPTH, 100),
            strings(&["d1/d2/d3/deep.rs", "d1/d2/ok.rs"])
        );
    }

    #[test]
    fn walk_respects_entry_cap_deterministically() {
        let tmp = tempfile::tempdir().unwrap();
        for n in 0..5 {
            fs::write(tmp.path().join(format!("f{n}.rs")), "").unwrap();
        }
        assert_eq!(walk_tree(tmp.path(), 6, 3), strings(&["f0.rs", "f1.rs", "f2.rs"]));
    }

    // --- file index: git source ---

    /// `git init` a repo in `dir` with `tracked` added to the index;
    /// returns false (caller skips) when git is unavailable.
    fn init_git_repo(dir: &Path, tracked: &[&str]) -> bool {
        let ok = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            return false;
        }
        for rel in tracked {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
        }
        std::process::Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[test]
    fn index_uses_git_ls_files_inside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        if !init_git_repo(tmp.path(), &["tracked.rs", "src/main.rs"]) {
            return; // no git available: nothing to assert
        }
        // Untracked files prove the source is git, not the walk.
        fs::write(tmp.path().join("untracked.rs"), "").unwrap();
        assert_eq!(
            build_index(tmp.path()),
            strings(&["src/main.rs", "tracked.rs"])
        );
    }

    #[test]
    fn index_falls_back_to_walk_outside_a_repo() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        // Not a repo: both files show, including one git would never list.
        assert!(git_ls_files(tmp.path()).is_none());
        assert_eq!(build_index(tmp.path()), strings(&["src/lib.rs"]));
    }

    // --- file index: cache expiry ---

    #[test]
    fn needs_rebuild_rule() {
        let t0 = Instant::now();
        assert!(needs_rebuild(None, t0));
        assert!(!needs_rebuild(Some(t0), t0));
        assert!(!needs_rebuild(Some(t0), t0 + Duration::from_secs(29)));
        assert!(needs_rebuild(Some(t0), t0 + Duration::from_secs(30)));
        // Clock going backwards reads as fresh, never panics.
        assert!(!needs_rebuild(Some(t0 + Duration::from_secs(5)), t0));
    }

    #[test]
    fn index_caches_then_rebuilds_after_ttl() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("a.rs"), "").unwrap();
        let mut index = FileIndex::new(tmp.path());
        let t0 = Instant::now();
        index.freshen(t0);
        assert_eq!(index.paths, strings(&["a.rs"]));

        // New file, still fresh: the cache serves the old list.
        fs::write(tmp.path().join("b.rs"), "").unwrap();
        index.freshen(t0 + Duration::from_secs(29));
        assert_eq!(index.paths, strings(&["a.rs"]));

        // Past the TTL: rebuild picks it up.
        index.freshen(t0 + Duration::from_secs(31));
        assert_eq!(index.paths, strings(&["a.rs", "b.rs"]));
    }

    #[test]
    fn candidates_run_through_the_cached_index() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::create_dir_all(tmp.path().join("docs")).unwrap();
        fs::write(tmp.path().join("src/driver.rs"), "").unwrap();
        fs::write(tmp.path().join("docs/driver.md"), "").unwrap();
        fs::write(tmp.path().join("README"), "").unwrap();
        let mut index = FileIndex::new(tmp.path());
        let got = index.candidates("driv");
        assert_eq!(got, strings(&["src/driver.rs", "docs/driver.md"]));
    }
}

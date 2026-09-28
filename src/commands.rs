//! T113 — F9 phase 1: slash-command packs (`.chug/commands/*.md`).
//!
//! FEATURES.md F9: repo-local commands invocable from chat (`/review`,
//! `/triage`) and — phase 2 — as run goals. "Community-extensible without
//! code": a pack is a plain markdown file dropped into the run cwd's
//! `.chug/commands/`; it is discovered by name, no registration, no
//! rebuild (benchmark: Claude Code's `.claude/commands/`).
//!
//! Directory semantics follow the T83 hooks / T90 permissions precedent:
//! `.chug/commands/` lives in the gitignored `.chug/` — per-checkout, no
//! search chain, no CLI flag; a worktree child has its own (or none).
//! A missing directory is the normal zero-cost leg: empty set, like an
//! absent hooks config.
//!
//! Expansion is Claude-Code-style: `$ARGUMENTS` in the body is replaced
//! with everything after the command word; a body without the token gets
//! the arguments appended after a blank line. Built-ins always win (a pack
//! named `goal.md` is shadowed by `/goal`) by construction: chat.rs's
//! parser dispatches built-in names before the pack lookup ever runs, so
//! the shadow case never reaches this module.
//!
//! Failure posture is the hooks precedent, fail-open: an unreadable or
//! invalid-UTF-8 pack file is skipped with one stderr note per load and
//! the run continues — a broken pack must never take the session down.
//!
//! Phase 2a (T117, landed): run-side invocation. `chug run` / `chug plan`
//! resolve a `--goal "/name args"` through the packs at the CLI boundary
//! ([`parse_invocation`] + [`resolve_goal`]): the goal becomes the expanded
//! body, the unknown/empty legs are hard errors naming the remedy, and the
//! pack name rides `run_start` as `goal_pack`.
//!
//! Phase 2b (T118, deferred): Tab completion of pack names, frontmatter
//! (description/allowed-tools).

use std::fs;
use std::path::{Path, PathBuf};

/// The placeholder substituted with the command's arguments at expansion.
pub const ARGUMENTS_TOKEN: &str = "$ARGUMENTS";

/// The pack directory: `<cwd>/.chug/commands` — cwd-confined like every
/// other `.chug/` surface, so worktree children get their own packs by
/// construction.
pub fn commands_dir(cwd: &Path) -> PathBuf {
    cwd.join(".chug").join("commands")
}

/// One discovered pack: the file stem (exact, case-sensitive — `Review.md`
/// and `review.md` are different commands) and the loaded body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pack {
    pub name: String,
    pub body: String,
}

/// Discover the packs in `<cwd>/.chug/commands/*.md`. Only `.md` files
/// count (other files are ignored, not errors); names enumerate sorted.
/// Missing directory = empty set. A file that cannot be read (or is not
/// valid UTF-8) is skipped with one stderr note — fail-open, hooks
/// precedent — and the remaining packs still load.
pub fn discover(cwd: &Path) -> Vec<Pack> {
    let dir = commands_dir(cwd);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        // Absent `.chug/commands/` (often `.chug/` itself): the normal
        // zero-cost leg.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        // An unreadable directory fails open to zero packs, with a note.
        Err(e) => {
            eprintln!(
                "chug: warning: commands: cannot read {}: {e}; no packs loaded",
                dir.display()
            );
            return Vec::new();
        }
    };
    let mut packs = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        match fs::read_to_string(&path) {
            Ok(body) => packs.push(Pack {
                name: name.to_string(),
                body,
            }),
            Err(e) => {
                eprintln!("chug: warning: commands: skipping {}: {e}", path.display());
            }
        }
    }
    packs.sort_by(|a, b| a.name.cmp(&b.name));
    packs
}

/// Pack names, sorted — the listing shared by `/help`'s count and the
/// unknown-command remedy line (one discovery call, one truth).
pub fn names(cwd: &Path) -> Vec<String> {
    discover(cwd).into_iter().map(|p| p.name).collect()
}

/// The outcome of trying to expand a chat `/name args` line as a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expansion {
    /// Hit: the expanded body, ready to send as the user message.
    Body(String),
    /// Miss: no pack by that name. Carries the available pack names
    /// (sorted) so the caller can extend the unknown-command line.
    Unknown(Vec<String>),
    /// The pack exists but expanded to nothing (empty body, no args) —
    /// sending it would be an empty turn, so the caller renders the
    /// remedy-naming note instead.
    Empty,
}

/// Expand pack `name` with `args` (everything after the command word;
/// `None` when the line was just `/name`). `$ARGUMENTS` in the body is
/// replaced with the args string (empty string when no args); a body
/// without the token gets the args appended after a blank line. An unknown
/// name is a [`Expansion::Unknown`] carrying the available pack names —
/// the remedy-naming doctrine, aimed at the user who just typed it.
pub fn expand(cwd: &Path, name: &str, args: Option<&str>) -> Expansion {
    let packs = discover(cwd);
    let Some(pack) = packs.iter().find(|p| p.name == name) else {
        return Expansion::Unknown(packs.into_iter().map(|p| p.name).collect());
    };
    let expanded = substitute(&pack.body, args.unwrap_or_default());
    if expanded.trim().is_empty() {
        return Expansion::Empty;
    }
    Expansion::Body(expanded)
}

/// `$ARGUMENTS` substitution (spec'd legs):
/// - token present → every occurrence becomes the args string,
/// - token absent, args present → args appended after a blank line,
/// - token absent, no args → the body unchanged.
fn substitute(body: &str, args: &str) -> String {
    if body.contains(ARGUMENTS_TOKEN) {
        return body.replace(ARGUMENTS_TOKEN, args);
    }
    if args.is_empty() {
        return body.to_string();
    }
    // Append after a blank line; trailing newlines in the body collapse
    // into the one blank separator (pack files conventionally end in \n).
    let body = body.trim_end_matches('\n');
    if body.is_empty() {
        args.to_string()
    } else {
        format!("{body}\n\n{args}")
    }
}

// ---------- run-side goal resolution (F9 phase 2a, T117) ----------

/// Parse a run/plan `--goal` into a pack invocation. `Some((name, args))`
/// when the goal is `/name` or `/name args…`: the goal starts with `/`
/// immediately followed by a non-whitespace name (running to the first
/// whitespace), and args are the whitespace-trimmed remainder — `None`
/// when empty. `/` alone, `/` followed by whitespace, and any goal not
/// `/`-prefixed are NOT invocations (`None` — the passthrough leg).
///
/// Pure and allocation-light so the common literal-goal path is a prefix
/// check: no pack discovery, no I/O.
pub fn parse_invocation(goal: &str) -> Option<(String, Option<String>)> {
    let rest = goal.strip_prefix('/')?;
    // `/` alone or `/` + whitespace: no command word, not an invocation.
    let name_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    if name_end == 0 {
        return None;
    }
    let name = rest[..name_end].to_string();
    let args = rest[name_end..].trim();
    Some((name, (!args.is_empty()).then(|| args.to_string())))
}

/// The outcome of resolving a run/plan `--goal` through the packs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoalResolution {
    /// Not a `/name args` invocation: the goal is used verbatim —
    /// byte-identical behavior, and no pack discovery (the zero-cost leg).
    Passthrough,
    /// An invocation that expanded: `body` is the goal text the model will
    /// actually receive (transmission truth — `goal_sha256` hashes it), and
    /// `pack` names the pack that produced it (recorded on `run_start` as
    /// `goal_pack`).
    Expanded { body: String, pack: String },
}

/// Resolve a run/plan `--goal` through the packs — the CLI-boundary seam
/// both `chug run` and `chug plan` go through BEFORE the driver (or plan
/// loop) starts, so a failed resolution never triggers a `.chug/` write.
///
/// - `Ok(Passthrough)`: not an invocation; the caller keeps the goal
///   byte-identical ([`GoalResolution::Passthrough`] carries nothing for
///   exactly that reason).
/// - `Ok(Expanded { body, pack })`: run with `body` as the goal; the caller
///   prints the one stderr note naming `pack`.
/// - `Err(message)`: the rendered hard-error text for the two failure legs
///   — an unknown `/name` (naming the available packs, or "no packs
///   discovered" plus the pack dir path) or a pack that expands to empty
///   (naming the pack). The caller surfaces it as a non-zero exit; a
///   typo'd pack name is never silently run as a literal goal.
pub fn resolve_goal(cwd: &Path, goal: &str) -> Result<GoalResolution, String> {
    let Some((name, args)) = parse_invocation(goal) else {
        return Ok(GoalResolution::Passthrough);
    };
    match expand(cwd, &name, args.as_deref()) {
        Expansion::Body(body) => Ok(GoalResolution::Expanded { body, pack: name }),
        Expansion::Unknown(packs) => {
            if packs.is_empty() {
                Err(format!(
                    "unknown command '/{name}' — no packs discovered in {}",
                    commands_dir(cwd).display()
                ))
            } else {
                Err(format!(
                    "unknown command '/{name}' — available packs: {}",
                    packs.join(", ")
                ))
            }
        }
        Expansion::Empty => Err(format!(
            "pack '{name}' expands to an empty goal — nothing to run; give it \
             arguments or put text in {}/{}.md",
            commands_dir(cwd).display(),
            name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir_with(packs: &[(&str, &str)]) -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for (name, body) in packs {
            let path = commands_dir(tmp.path()).join(format!("{name}.md"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        tmp
    }

    // ---------- discovery ----------

    #[test]
    fn missing_dir_is_an_empty_set_zero_cost() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(discover(tmp.path()).is_empty());
        assert!(names(tmp.path()).is_empty());
    }

    #[test]
    fn names_enumerate_sorted_and_are_file_stems() {
        let tmp = tmp_dir_with(&[("triage", "t"), ("review", "r"), ("audit", "a")]);
        assert_eq!(names(tmp.path()), vec!["audit", "review", "triage"]);
    }

    #[test]
    fn non_md_files_are_ignored_not_errors() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = commands_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a pack").unwrap();
        std::fs::write(dir.join("README.MD"), "uppercase ext is not .md").unwrap();
        std::fs::create_dir_all(dir.join("subdir.md")).unwrap(); // dir, not a file pack
        std::fs::write(dir.join("review.md"), "real pack").unwrap();
        assert_eq!(names(tmp.path()), vec!["review"]);
    }

    /// Fail-open: an unreadable/invalid-UTF-8 pack is skipped with one
    /// stderr note (the note is the hooks-precedent eprintln, fired right
    /// beside this skip) and the valid siblings still load.
    #[test]
    fn corrupt_file_is_skipped_and_siblings_still_load() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = commands_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        // Invalid UTF-8: read_to_string fails with InvalidData.
        std::fs::write(dir.join("broken.md"), [0xFF, 0xFE, 0x00]).unwrap();
        std::fs::write(dir.join("review.md"), "real pack").unwrap();
        assert_eq!(names(tmp.path()), vec!["review"]);
    }

    // ---------- expansion ----------

    #[test]
    fn token_present_is_replaced_with_args() {
        let tmp = tmp_dir_with(&[("review", "Review the diff. Focus: $ARGUMENTS.")]);
        assert_eq!(
            expand(tmp.path(), "review", Some("the login bug")),
            Expansion::Body("Review the diff. Focus: the login bug.".into())
        );
    }

    #[test]
    fn token_present_with_no_args_becomes_empty_string() {
        let tmp = tmp_dir_with(&[("review", "Review the diff. Focus: $ARGUMENTS")]);
        assert_eq!(
            expand(tmp.path(), "review", None),
            Expansion::Body("Review the diff. Focus: ".into())
        );
    }

    #[test]
    fn every_token_occurrence_is_replaced() {
        let tmp = tmp_dir_with(&[("pair", "$ARGUMENTS and $ARGUMENTS")]);
        assert_eq!(
            expand(tmp.path(), "pair", Some("x")),
            Expansion::Body("x and x".into())
        );
    }

    #[test]
    fn token_absent_with_args_appends_after_blank_line() {
        let tmp = tmp_dir_with(&[("review", "Review the diff.\n")]);
        assert_eq!(
            expand(tmp.path(), "review", Some("the login bug")),
            Expansion::Body("Review the diff.\n\nthe login bug".into())
        );
    }

    #[test]
    fn token_absent_with_no_args_leaves_body_unchanged() {
        let tmp = tmp_dir_with(&[("review", "Review the diff.")]);
        assert_eq!(
            expand(tmp.path(), "review", None),
            Expansion::Body("Review the diff.".into())
        );
    }

    #[test]
    fn args_on_empty_body_are_the_whole_expansion() {
        let tmp = tmp_dir_with(&[("note", "")]);
        assert_eq!(
            expand(tmp.path(), "note", Some("just this")),
            Expansion::Body("just this".into())
        );
    }

    /// An empty body with no args would send an empty user message (an API
    /// error waiting to happen) — the caller gets the remedy instead.
    #[test]
    fn empty_expansion_is_its_own_outcome() {
        let tmp = tmp_dir_with(&[("note", "")]);
        assert_eq!(expand(tmp.path(), "note", None), Expansion::Empty);
        assert_eq!(expand(tmp.path(), "note", Some("   ")), Expansion::Empty);
    }

    // ---------- unknown names ----------

    #[test]
    fn unknown_name_miss_lists_available_packs() {
        let tmp = tmp_dir_with(&[("review", "r"), ("triage", "t")]);
        assert_eq!(
            expand(tmp.path(), "nosuch", None),
            Expansion::Unknown(vec!["review".into(), "triage".into()])
        );
    }

    #[test]
    fn unknown_name_with_no_packs_misses_with_empty_list() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(expand(tmp.path(), "nosuch", None), Expansion::Unknown(vec![]));
    }

    // ---------- run-side invocation parse (F9 phase 2a) ----------

    #[test]
    fn invocation_parses_name_and_args() {
        assert_eq!(
            parse_invocation("/review the login bug"),
            Some(("review".into(), Some("the login bug".into())))
        );
    }

    #[test]
    fn invocation_without_args_has_none_args() {
        assert_eq!(parse_invocation("/review"), Some(("review".into(), None)));
        // Trailing whitespace after the name is not args.
        assert_eq!(parse_invocation("/review   "), Some(("review".into(), None)));
    }

    #[test]
    fn invocation_args_are_trimmed_of_surrounding_whitespace() {
        assert_eq!(
            parse_invocation("/review   the login bug\t"),
            Some(("review".into(), Some("the login bug".into())))
        );
        // A tab or newline after the command word separates it just the same.
        assert_eq!(
            parse_invocation("/review\tthe login bug"),
            Some(("review".into(), Some("the login bug".into())))
        );
    }

    #[test]
    fn slash_alone_is_not_an_invocation() {
        assert_eq!(parse_invocation("/"), None);
    }

    #[test]
    fn slash_then_whitespace_is_not_an_invocation() {
        assert_eq!(parse_invocation("/ review"), None);
        assert_eq!(parse_invocation("/ "), None);
        assert_eq!(parse_invocation("/\treview"), None);
        assert_eq!(parse_invocation("/\n"), None);
    }

    #[test]
    fn leading_whitespace_is_not_an_invocation() {
        assert_eq!(parse_invocation(" /review"), None);
        assert_eq!(parse_invocation("\t/review x"), None);
    }

    #[test]
    fn plain_text_is_not_an_invocation() {
        assert_eq!(parse_invocation("fix the login bug"), None);
        assert_eq!(parse_invocation(""), None);
        assert_eq!(parse_invocation("path/with/slash"), None);
    }

    // ---------- run-side goal resolution (F9 phase 2a) ----------

    /// Hit: the expanded body is the resolution, with the pack named for
    /// `run_start`'s `goal_pack`.
    #[test]
    fn resolution_hit_expands_and_names_the_pack() {
        let tmp = tmp_dir_with(&[("smoke", "Say hello to $ARGUMENTS.")]);
        assert_eq!(
            resolve_goal(tmp.path(), "/smoke hello"),
            Ok(GoalResolution::Expanded {
                body: "Say hello to hello.".into(),
                pack: "smoke".into(),
            })
        );
    }

    /// Unknown: the rendered error names the received `/name` and every
    /// available pack — the remedy-naming doctrine, never a silent literal
    /// run of a typo'd pack name.
    #[test]
    fn resolution_unknown_names_the_available_packs() {
        let tmp = tmp_dir_with(&[("review", "r"), ("triage", "t")]);
        let err = resolve_goal(tmp.path(), "/nosuch hello").unwrap_err();
        assert!(err.contains("'/nosuch'"), "{err}");
        assert!(err.contains("review") && err.contains("triage"), "{err}");
    }

    /// Unknown with an empty (or absent) pack dir: "no packs discovered"
    /// plus the pack dir path, so the remedy is actionable.
    #[test]
    fn resolution_unknown_with_no_packs_names_the_pack_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let err = resolve_goal(tmp.path(), "/nosuch").unwrap_err();
        assert!(err.contains("'/nosuch'"), "{err}");
        assert!(err.contains("no packs discovered"), "{err}");
        assert!(
            err.contains(&commands_dir(tmp.path()).display().to_string()),
            "{err}"
        );
    }

    /// Empty expansion: a hard error naming the pack and the empty leg.
    /// (An empty body WITH args expands to the args — the args-carrying leg
    /// above — so this leg is the pack firing with nothing to say.)
    #[test]
    fn resolution_empty_expansion_is_a_hard_error_naming_the_pack() {
        let tmp = tmp_dir_with(&[("note", "")]);
        let err = resolve_goal(tmp.path(), "/note").unwrap_err();
        assert!(err.contains("'note'"), "{err}");
        assert!(err.contains("empty"), "{err}");
        assert!(
            err.contains(&commands_dir(tmp.path()).display().to_string()),
            "{err}"
        );
    }

    /// Non-invocation goals pass through byte-identical — the zero-cost leg
    /// (no pack discovery happens on it; `/`, `/ x`, and plain text all land
    /// here).
    #[test]
    fn resolution_non_invocation_is_a_passthrough() {
        let tmp = tmp_dir_with(&[("review", "r")]);
        for goal in ["fix the login bug", "/", "/ review", " /review", ""] {
            assert_eq!(
                resolve_goal(tmp.path(), goal),
                Ok(GoalResolution::Passthrough),
                "goal {goal:?} must pass through"
            );
        }
    }
}

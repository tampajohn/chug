// T104 family: unit — driver pure-function pins: parse_check_command, system prompt, tool_use extraction, is_allow_destructive. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
use super::*; // the shared harness (driver::tests) + driver's own imports
#[test]
fn check_line_parsing() {
    assert_eq!(
        parse_check_command("goal: x\ncheck: cargo test\n"),
        Some("cargo test".to_string())
    );
    assert_eq!(parse_check_command("no check here"), None);
    assert_eq!(
        parse_check_command("check:   echo hi  "),
        Some("echo hi".to_string())
    );
    assert_eq!(
        parse_check_command("check: first\ncheck: second"),
        Some("first".to_string())
    );
    assert_eq!(parse_check_command("check:"), None);
    assert_eq!(
        parse_check_command("  check: go build ./..."),
        Some("go build ./...".to_string())
    );
}

/// T335 killing test (RED pre-fix): the t334 spec's Tests bullet wrapped so
/// a continuation line began with `check:` — `  check:-contract sentence.`
/// at spec line 53 — and first-match-wins extracted the prose as the gate
/// command (pre-fix extraction: `Some("-contract sentence.")`, so the gate
/// ran `sh -c '-contract sentence.'` and rejected a goal_complete whose
/// validation had already landed its written PASS verdict). The real line
/// sat at 73 under the `## check:` header at 71; the header-anchored scan
/// must return it.
#[test]
fn parse_check_command_prose_continuation_loses_to_real_line_after_header() {
    let t334_prefix_shape = "\
- Scaffold validity: the generated TODO.md parses under the
  todo_consistency rules; the generated LOOP-SPEC.md contains the
  check:-contract sentence.
- run fixture: missing scaffold exits 2 with the init hint; with
  scaffold, the supervisor is invoked with cwd = the temp repo (assert
  via a stubbed exec).

The tests land under `tests/loop_subcommand.rs` — that filename is the
gate contract (the filing-time check line names it, the c06a555 lesson:
the gate chain is unpiped).

## Out of scope

- Porting loopd.sh to Rust (the subcommand may exec the script; a Rust
  rewrite is a separate item if ever wanted).

## check:

check: cargo test --release --test loop_subcommand
";
    assert_eq!(
        parse_check_command(t334_prefix_shape),
        Some("cargo test --release --test loop_subcommand".to_string())
    );
}

/// T335: a `## check:` section header line is itself never the command —
/// its trimmed form starts with `##`, so the `check:` prefix never strips
/// (the t333 gate ran green with a `## check:` header present; t52's
/// command-on-the-header-line shape stays None, byte-identical pre/post).
#[test]
fn parse_check_command_section_header_itself_is_never_the_command() {
    assert_eq!(
        parse_check_command("## check:\n\ncheck: echo ok\n"),
        Some("echo ok".to_string())
    );
    // t52's shape: the command rode ON the header line — never selected.
    assert_eq!(
        parse_check_command("## check: cargo test --test shared_target_dir\n"),
        None
    );
    // Header with nothing after it (t285's shape): None, as before T335.
    assert_eq!(parse_check_command("## check:\n"), None);
}

/// T335: the t309/t310 shape — a single `check:` line near the top, no
/// `## check:` header — extracts byte-identically to the pre-T335
/// first-match parse (no header → the historical whole-spec scan).
#[test]
fn parse_check_command_single_occurrence_no_header_unchanged() {
    let spec = "\
Title line.

check: python3 scripts/test_wrap_assert.py && grep -q 'assert_fill_datum' \
scripts/wrap_assert.py && grep -q 'T309' LOOP-SPEC.md
";
    assert_eq!(
        parse_check_command(spec),
        Some(
            "python3 scripts/test_wrap_assert.py && grep -q 'assert_fill_datum' \
             scripts/wrap_assert.py && grep -q 'T309' LOOP-SPEC.md"
                .to_string()
        )
    );
}

/// T335: a bare `check:` (empty command, before or inside the check
/// section) is still skipped — None when it is the only occurrence, and
/// the next real line wins when one follows.
#[test]
fn parse_check_command_empty_command_still_none_or_skipped() {
    assert_eq!(parse_check_command("check:"), None);
    assert_eq!(parse_check_command("check:   \n"), None);
    assert_eq!(
        parse_check_command("## check:\ncheck:\ncheck: echo ok\n"),
        Some("echo ok".to_string())
    );
}

#[test]
fn system_prompt_contains_all_sections() {
    let prompt = build_system_prompt("SPEC TEXT", "do the thing", "# Ledger\n...", "");
    assert!(prompt.contains("## Spec"));
    assert!(prompt.contains("SPEC TEXT"));
    assert!(prompt.contains("## Goal"));
    assert!(prompt.contains("do the thing"));
    assert!(prompt.contains("## Ledger"));
    assert!(prompt.contains("goal_complete"));
}

/// T111: the `## Todos` section rides after `## Ledger` when the todo
/// list is non-empty (run mode), rendering `t<N> [status] title` lines.
#[test]
fn system_prompt_todos_section_after_ledger_when_non_empty() {
    let todos = crate::todos::render(&[crate::todos::Todo {
        id: "t3".into(),
        title: "wire the store".into(),
        status: crate::todos::Status::InProgress,
    }]);
    let prompt = build_system_prompt("SPEC TEXT", "do the thing", "LEDGER BODY", &todos);
    let ledger_at = prompt.find("## Ledger").expect("ledger section");
    let todos_at = prompt.find("## Todos").expect("todos section");
    assert!(
        todos_at > ledger_at,
        "## Todos must follow ## Ledger: {prompt}"
    );
    assert!(
        prompt.contains("## Todos\n\nt3 [in_progress] wire the store"),
        "{prompt}"
    );
}

/// T111: an empty todo list emits NO `## Todos` heading at all (run
/// mode) — no empty section.
#[test]
fn system_prompt_has_no_todos_section_when_list_is_empty() {
    let prompt = build_system_prompt("SPEC TEXT", "do the thing", "LEDGER BODY", "");
    assert!(!prompt.contains("## Todos"), "{prompt}");
}

/// T111: chat mode gets the same treatment — `## Todos` after `## Ledger`
/// when non-empty, nothing when empty.
#[test]
fn chat_prompt_todos_section_after_ledger_and_absent_when_empty() {
    let todos = crate::todos::render(&[crate::todos::Todo {
        id: "t1".into(),
        title: "ship it".into(),
        status: crate::todos::Status::Pending,
    }]);
    let prompt =
        build_chat_system_prompt(Some("CHAT SPEC"), Some("chat goal"), "LEDGER BODY", &todos);
    let ledger_at = prompt.find("## Ledger").expect("ledger section");
    let todos_at = prompt.find("## Todos").expect("todos section");
    assert!(
        todos_at > ledger_at,
        "## Todos must follow ## Ledger: {prompt}"
    );
    assert!(
        prompt.contains("## Todos\n\nt1 [pending] ship it"),
        "{prompt}"
    );

    let empty = build_chat_system_prompt(None, None, "LEDGER BODY", "");
    assert!(!empty.contains("## Todos"), "{empty}");
}

#[test]
fn tool_use_blocks_extracted_from_assistant_content() {
    let msg = Message::assistant(vec![
        ContentBlock::text_block("thinking out loud"),
        ContentBlock::Known(KnownBlock::ToolUse {
            id: "tu_1".into(),
            name: "bash".into(),
            input: json!({"command": "ls"}),
        }),
        ContentBlock::Other(json!({"type": "mystery"})),
    ]);
    let uses: Vec<_> = msg
        .content
        .iter()
        .filter_map(ContentBlock::tool_use)
        .collect();
    assert_eq!(uses.len(), 1);
    assert_eq!(uses[0].0, "tu_1");
    assert_eq!(uses[0].1, "bash");
}

#[test]
fn allow_destructive_note_matching() {
    assert!(is_allow_destructive("allow destructive"));
    assert!(is_allow_destructive("  ALLOW DESTRUCTIVE  "));
    assert!(!is_allow_destructive("allow destructively"));
    assert!(!is_allow_destructive("please allow destructive commands"));
}

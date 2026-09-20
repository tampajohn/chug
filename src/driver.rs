use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Context;
use serde_json::{Value, json};

use crate::api::{Client, ContentBlock, KnownBlock, Message};
use crate::ledger;
use crate::tools::{self, ToolCtx, ToolResult};
use crate::transcript;

pub const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

const PREAMBLE: &str = "You are chug, an autonomous coding agent driven by a code loop, not a conversation. Work in small, verified steps. After each step, update LEDGER.md with the update_ledger tool (what is done, what is next, any blockers). Verify your work by running builds/tests before claiming success. Never declare the goal complete without running the relevant checks. When the goal is fully met and verified, call the goal_complete tool with a short summary.";

const KICK: &str = "Ledger and goal are above. You have not called goal_complete. Continue with the next ledger item, or update the ledger if the plan changed.";

const TRIM_ABOVE_TOKENS: usize = 120_000;
const TRIM_TARGET_TOKENS: usize = 80_000;
const KEEP_LAST_MESSAGES: usize = 20;
const STUCK_WINDOW: usize = 3;

pub struct RunConfig {
    pub cwd: PathBuf,
    pub spec_path: PathBuf,
    pub goal: String,
    pub model: String,
    pub max_iters: u32,
    pub max_minutes: u64,
    pub resume: bool,
}

enum VerifyOutcome {
    Accepted,
    Failed(String),
    NoCheck,
}

pub fn run(cfg: RunConfig) -> anyhow::Result<i32> {
    let client = Client::new(&cfg.model)?;
    let tool_schemas = tools::tool_schemas();
    let ctx = ToolCtx { cwd: cfg.cwd.clone() };

    ledger::ensure_seeded(&cfg.cwd)?;

    let mut messages: Vec<Message> = if cfg.resume {
        transcript::load(&cfg.cwd)?
    } else {
        Vec::new()
    };
    if messages.is_empty() {
        let first = Message::user(vec![ContentBlock::text_block(format!(
            "Goal: {}\n\nThe spec, goal, and ledger are in your system prompt. Start working.",
            cfg.goal
        ))]);
        transcript::append(&cfg.cwd, &first)?;
        messages.push(first);
    }
    // On resume, trim before continuing so an over-large transcript starts compact.
    if cfg.resume && transcript_trim(&mut messages) {
        transcript::rewrite(&cfg.cwd, &messages)?;
    }

    let mut spec_text = fs::read_to_string(&cfg.spec_path)
        .with_context(|| format!("reading spec {}", cfg.spec_path.display()))?;

    let start = Instant::now();
    let mut iteration: u32 = 0;
    let mut recent: VecDeque<ToolResult> = VecDeque::with_capacity(STUCK_WINDOW);

    loop {
        if iteration >= cfg.max_iters {
            return abort_run(&cfg.cwd, "iteration budget exceeded", 1);
        }
        if start.elapsed() >= Duration::from_secs(cfg.max_minutes.saturating_mul(60)) {
            return abort_run(&cfg.cwd, "time budget exceeded", 1);
        }

        // Re-read spec every iteration: the user may edit it mid-run. Keep the
        // last good copy if it becomes unreadable.
        if let Ok(text) = fs::read_to_string(&cfg.spec_path) {
            spec_text = text;
        }
        let ledger_text = ledger::read(&cfg.cwd)?;
        let system = build_system_prompt(&spec_text, &cfg.goal, &ledger_text);

        eprintln!(
            "[chug] iteration {} / {} ({} messages)",
            iteration + 1,
            cfg.max_iters,
            messages.len()
        );
        let resp = client.complete(&system, &messages, &tool_schemas)?;

        // Store the assistant message verbatim (text, thinking, tool_use, and
        // any unknown block types) so multi-turn echo stays valid.
        let assistant = Message::assistant(resp.content_blocks());
        transcript::append(&cfg.cwd, &assistant)?;
        let model_text = resp.text();
        if !model_text.is_empty() {
            eprintln!("[chug] model: {}", preview(&model_text, 200));
        }
        if let Some(reason) = resp.stop_reason()
            && reason != "end_turn"
            && reason != "tool_use"
        {
            eprintln!("[chug] stop_reason: {reason}");
        }

        let mut user_blocks: Vec<ContentBlock> = Vec::new();
        let mut tool_count = 0usize;
        let mut goal_summary: Option<String> = None;

        for (id, name, input) in assistant.content.iter().filter_map(ContentBlock::tool_use) {
            let result = tools::dispatch(&ctx, name, input);
            eprintln!(
                "[chug] tool {name} -> {}",
                if result.is_error { "error" } else { "ok" }
            );
            if name == "goal_complete" {
                goal_summary = Some(
                    input
                        .get("summary")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                );
            }
            user_blocks.push(ContentBlock::tool_result_block(
                id,
                result.content.clone(),
                result.is_error,
            ));
            recent.push_back(result);
            while recent.len() > STUCK_WINDOW {
                recent.pop_front();
            }
            tool_count += 1;
        }

        if tool_count == 0 {
            // Model stopped talking without finishing — the anti-stall kick.
            user_blocks.push(ContentBlock::text_block(KICK));
        } else if goal_summary.is_some() {
            match verify(&cfg.cwd, &spec_text)? {
                VerifyOutcome::NoCheck | VerifyOutcome::Accepted => {
                    let user_msg = Message::user(user_blocks);
                    transcript::append(&cfg.cwd, &user_msg)?;
                    println!("chug: goal complete");
                    println!("summary: {}", goal_summary.unwrap_or_default());
                    println!("\n--- LEDGER.md ---");
                    println!("{ledger_text}");
                    return Ok(0);
                }
                VerifyOutcome::Failed(output) => {
                    eprintln!("[chug] goal_complete rejected: check command failed");
                    user_blocks.push(ContentBlock::text_block(format!(
                        "goal_complete rejected: the spec check command failed. Output:\n\n{output}\n\nFix the failure and try again. Update the ledger to reflect the current state."
                    )));
                }
            }
        }

        messages.push(assistant);
        let user_msg = Message::user(user_blocks);
        transcript::append(&cfg.cwd, &user_msg)?;
        messages.push(user_msg);

        if is_stuck(recent.make_contiguous()) {
            return abort_run(&cfg.cwd, "stuck: repeated error", 2);
        }

        if transcript_trim(&mut messages) {
            transcript::rewrite(&cfg.cwd, &messages)?;
        }

        iteration += 1;
    }
}

/// Verification on `goal_complete`: run the spec's `check:` command if present.
fn verify(cwd: &Path, spec_text: &str) -> anyhow::Result<VerifyOutcome> {
    let Some(command) = parse_check_command(spec_text) else {
        return Ok(VerifyOutcome::NoCheck);
    };
    let outcome = tools::run_shell(cwd, &command, Duration::from_secs(tools::CHECK_TIMEOUT_SECS))?;
    if !outcome.timed_out && outcome.exit_code == Some(0) {
        return Ok(VerifyOutcome::Accepted);
    }
    let output = tools::truncate_middle(&outcome.output, 5_000, 5_000);
    let exit_label = match outcome.exit_code {
        Some(code) => code.to_string(),
        None => "timeout".to_string(),
    };
    Ok(VerifyOutcome::Failed(format!(
        "$ {command}\nexit code: {exit_label}\n{output}"
    )))
}

/// First `check: <shell command>` line in the spec text, if any.
pub fn parse_check_command(spec: &str) -> Option<String> {
    spec.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("check:")?;
        let command = rest.trim();
        if command.is_empty() {
            None
        } else {
            Some(command.to_string())
        }
    })
}

/// Stuck tripwire: the last 3 tool results are all errors with identical
/// content (compared on the first 500 chars).
pub fn is_stuck(recent: &[ToolResult]) -> bool {
    if recent.len() < STUCK_WINDOW {
        return false;
    }
    let last: Vec<&ToolResult> = recent.iter().rev().take(STUCK_WINDOW).collect();
    last.iter().all(|r| r.is_error)
        && error_marker(&last[0].content) == error_marker(&last[1].content)
        && error_marker(&last[1].content) == error_marker(&last[2].content)
}

fn error_marker(content: &str) -> String {
    content.chars().take(500).collect()
}

pub fn estimate_tokens(messages: &[Message]) -> usize {
    let chars: usize = messages
        .iter()
        .filter_map(|m| serde_json::to_string(m).ok())
        .map(|s| s.len())
        .sum();
    chars / 4
}

/// Transcript trimming: above 120k estimated tokens, replace tool_result /
/// tool_use payloads older than the last 20 messages with `"[trimmed]"` until
/// under 80k. Message 0 and the last 20 messages are never touched.
pub fn transcript_trim(messages: &mut [Message]) -> bool {
    if estimate_tokens(messages) <= TRIM_ABOVE_TOKENS {
        return false;
    }
    let trim_end = messages.len().saturating_sub(KEEP_LAST_MESSAGES);
    let mut changed = false;
    let mut i = 1; // never trim message 0
    while i < trim_end {
        let mut replaced_any = false;
        for block in messages[i].content.iter_mut() {
            match block {
                ContentBlock::Known(KnownBlock::ToolResult { content, .. }) => {
                    *content = Value::String("[trimmed]".to_string());
                    replaced_any = true;
                }
                ContentBlock::Known(KnownBlock::ToolUse { input, .. }) => {
                    *input = json!({});
                    replaced_any = true;
                }
                _ => {}
            }
        }
        if replaced_any {
            changed = true;
            if estimate_tokens(messages) <= TRIM_TARGET_TOKENS {
                break;
            }
        }
        i += 1;
    }
    changed
}

pub fn build_system_prompt(spec: &str, goal: &str, ledger_text: &str) -> String {
    format!("{PREAMBLE}\n\n## Spec\n\n{spec}\n\n## Goal\n\n{goal}\n\n## Ledger\n\n{ledger_text}")
}

fn abort_run(cwd: &Path, reason: &str, code: i32) -> anyhow::Result<i32> {
    eprintln!("chug: abort: {reason}");
    println!("--- LEDGER.md ---");
    println!(
        "{}",
        ledger::read(cwd).unwrap_or_else(|_| "(ledger unavailable)".to_string())
    );
    println!("---");
    println!(
        "resume with: chug run --spec <spec> --goal \"<goal>\" --cwd {} --resume",
        cwd.display()
    );
    Ok(code)
}

fn preview(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let head: String = s.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tool_result_msg(text: &str) -> Message {
        Message::user(vec![ContentBlock::tool_result_block("t1", text.to_string(), false)])
    }

    #[test]
    fn tripwire_fires_on_three_identical_errors() {
        let recent: Vec<ToolResult> = (0..3)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
            })
            .collect();
        assert!(is_stuck(&recent));
    }

    #[test]
    fn tripwire_ignores_different_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "different failure".to_string(),
                is_error: true,
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_requires_all_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: false,
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_needs_full_window() {
        let recent: Vec<ToolResult> = (0..2)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
            })
            .collect();
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_compares_first_500_chars() {
        let long_err = format!("{}{}", "e".repeat(600), "A");
        let long_err2 = format!("{}{}", "e".repeat(600), "B");
        let mut recent = Vec::new();
        for content in [long_err.clone(), long_err, long_err2] {
            recent.push(ToolResult {
                content,
                is_error: true,
            });
        }
        // identical in the first 500 chars despite differing tails
        assert!(is_stuck(&recent));
    }

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
        assert_eq!(parse_check_command("  check: go build ./..."), Some("go build ./...".to_string()));
    }

    #[test]
    fn trimming_preserves_last_20_and_first_message() {
        let long = "x".repeat(25_000);
        let mut messages = vec![tool_result_msg("first message")];
        for _ in 0..30 {
            messages.push(tool_result_msg(&long));
        }
        assert!(estimate_tokens(&messages) > TRIM_ABOVE_TOKENS);
        assert!(transcript_trim(&mut messages));

        for (i, msg) in messages.iter().enumerate() {
            let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &msg.content[0]
            else {
                panic!("expected tool_result at index {i}");
            };
            if i == 0 {
                assert_eq!(content, "first message", "message 0 must never be trimmed");
            } else if i < messages.len() - KEEP_LAST_MESSAGES {
                assert_eq!(content, "[trimmed]", "index {i} should be trimmed");
            } else {
                assert_eq!(content, &long, "index {i} must stay intact");
            }
        }
    }

    #[test]
    fn trimming_noop_under_threshold() {
        let mut messages = vec![tool_result_msg("small"), tool_result_msg("tiny")];
        assert!(!transcript_trim(&mut messages));
        let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &messages[0].content[0]
        else {
            panic!("expected tool_result");
        };
        assert_eq!(content, "small");
    }

    #[test]
    fn trimming_respects_target_or_exhausts() {
        let long = "y".repeat(10_000);
        let mut messages = vec![tool_result_msg("first")];
        for _ in 0..60 {
            messages.push(tool_result_msg(&long));
        }
        transcript_trim(&mut messages);
        // 61 messages of ~10k chars start above the 120k-token threshold; the
        // trimmable pool (everything older than the last 20) is big enough to
        // reach the 80k-token target.
        assert!(estimate_tokens(&messages) <= TRIM_TARGET_TOKENS);
        // The last 20 are untouched.
        for msg in &messages[messages.len() - KEEP_LAST_MESSAGES..] {
            let ContentBlock::Known(KnownBlock::ToolResult { content, .. }) = &msg.content[0]
            else {
                panic!("expected tool_result");
            };
            assert_eq!(content, &long);
        }
    }

    #[test]
    fn system_prompt_contains_all_sections() {
        let prompt = build_system_prompt("SPEC TEXT", "do the thing", "# Ledger\n...");
        assert!(prompt.contains("## Spec"));
        assert!(prompt.contains("SPEC TEXT"));
        assert!(prompt.contains("## Goal"));
        assert!(prompt.contains("do the thing"));
        assert!(prompt.contains("## Ledger"));
        assert!(prompt.contains("goal_complete"));
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
        let uses: Vec<_> = msg.content.iter().filter_map(ContentBlock::tool_use).collect();
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, "tu_1");
        assert_eq!(uses[0].1, "bash");
    }
}

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event as CtEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{execute, terminal::{EnterAlternateScreen, LeaveAlternateScreen}};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::chat::{self, ChatState, SlashCommand};
use crate::driver::SlashUpdate;
use crate::events::{Event, TurnEndReason};

/// Sink used by the driver thread: forwards events to the UI thread.
pub struct TuiSink {
    pub tx: Sender<Event>,
}

impl crate::events::EventSink for TuiSink {
    fn emit(&mut self, e: Event) {
        // UI thread gone (e.g. forced quit): dropping events is fine.
        let _ = self.tx.send(e);
    }
}

pub struct TuiConfig {
    pub goal: String,
    pub model: String,
    pub abort: Arc<AtomicBool>,
    pub events: Receiver<Event>,
    pub steering_tx: Sender<String>,
    /// Set by the worker thread when the driver loop returns.
    pub driver_done: Arc<AtomicBool>,
    /// Chat-mode wiring; `None` in `run` mode.
    pub chat: Option<ChatWiring>,
}

/// Channels + session constants the chat-mode UI needs.
pub struct ChatWiring {
    pub cwd: std::path::PathBuf,
    /// Current per-turn budgets (iters, minutes); updated by `/budget`.
    pub budget: (u32, u64),
    pub objective_tx: Sender<String>,
    pub update_tx: Sender<SlashUpdate>,
}

/// Chat-mode UI state: the Idle/Working state machine plus the channels used
/// to submit objectives and slash-command updates to the worker.
pub struct ChatUi {
    pub state: ChatState,
    /// The objective of the turn in flight (shown in the status bar).
    pub objective: String,
    pub budget: (u32, u64),
    pub cwd: std::path::PathBuf,
    pub objective_tx: Sender<String>,
    pub update_tx: Sender<SlashUpdate>,
    /// Reason string of the most recent Aborted event, used for the
    /// `─ turn interrupted: <reason> ─` banner.
    pub last_abort_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Running,
    Verifying,
    Done,
    Aborted,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Running => "running",
            Status::Verifying => "verifying",
            Status::Done => "done",
            Status::Aborted => "aborted",
        }
    }

    fn color(self) -> Color {
        match self {
            Status::Running => Color::Reset,
            Status::Verifying => Color::Yellow,
            Status::Done => Color::Green,
            Status::Aborted => Color::Red,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Editing,
}

/// One activity entry. Tool entries start pending and are completed by the
/// matching ToolResult event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Activity {
    Model(String),
    Tool {
        name: String,
        ok: Option<bool>,
        preview: String,
    },
    Notice {
        text: String,
        color: Color,
    },
}

pub struct App {
    pub goal: String,
    pub model: String,
    pub started: Instant,
    pub iter: (u32, u32),
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub activity: Vec<Activity>,
    pub ledger: String,
    pub ledger_changed_at: Option<Instant>,
    pub status: Status,
    /// Rows scrolled back from the bottom; 0 = auto-follow.
    pub scroll_back: u16,
    pub input_mode: InputMode,
    pub input: String,
    pub should_quit: bool,
    pub steering_tx: Sender<String>,
    /// Chat-mode state; `None` in `run` mode (the run TUI is unchanged).
    pub chat: Option<ChatUi>,
}

impl App {
    pub fn new(goal: String, model: String, steering_tx: Sender<String>) -> Self {
        App {
            goal,
            model,
            started: Instant::now(),
            iter: (0, 0),
            input_tokens: 0,
            output_tokens: 0,
            activity: Vec::new(),
            ledger: String::new(),
            ledger_changed_at: None,
            status: Status::Running,
            scroll_back: 0,
            input_mode: InputMode::Normal,
            input: String::new(),
            should_quit: false,
            steering_tx,
            chat: None,
        }
    }

    /// Chat-mode app: input dock always focused, Idle/Working state machine.
    pub fn new_chat(model: String, steering_tx: Sender<String>, wiring: ChatWiring) -> Self {
        let mut app = App::new(String::new(), model, steering_tx);
        app.chat = Some(ChatUi {
            state: ChatState::Idle,
            objective: String::new(),
            budget: wiring.budget,
            cwd: wiring.cwd,
            objective_tx: wiring.objective_tx,
            update_tx: wiring.update_tx,
            last_abort_reason: None,
        });
        app
    }

    /// Event reducer: maps a driver event onto UI state.
    pub fn apply(&mut self, e: Event) {
        match e {
            Event::Iteration { n, max, .. } => self.iter = (n, max),
            Event::ModelText(text) => {
                for (i, line) in text.lines().enumerate() {
                    let text = if i == 0 {
                        format!("▸ model: {line}")
                    } else {
                        format!("  {line}")
                    };
                    self.push_activity(Activity::Model(text));
                }
            }
            Event::ToolStart { name } => {
                self.push_activity(Activity::Tool {
                    name,
                    ok: None,
                    preview: String::new(),
                });
            }
            Event::ToolResult { name, ok, preview } => {
                // Complete the most recent pending entry with the same name;
                // if none is pending, record the result directly.
                let idx = self.activity.iter().rposition(|a| match a {
                    Activity::Tool { name: n, ok: None, .. } => n == &name,
                    _ => false,
                });
                match idx {
                    Some(i) => {
                        if let Activity::Tool { ok: slot_ok, preview: slot_preview, .. } =
                            &mut self.activity[i]
                        {
                            *slot_ok = Some(ok);
                            *slot_preview = preview;
                        }
                    }
                    None => self.activity.push(Activity::Tool {
                        name,
                        ok: Some(ok),
                        preview,
                    }),
                }
            }
            Event::LedgerChanged(text) => {
                self.ledger = text;
                self.ledger_changed_at = Some(Instant::now());
            }
            Event::Verifying { cmd } => {
                if self.chat.is_none() {
                    self.status = Status::Verifying;
                }
                self.push_activity(Activity::Notice {
                    text: format!("▸ check: {cmd}"),
                    color: Color::Yellow,
                });
            }
            Event::GoalRejected { reason } => {
                if self.chat.is_none() {
                    self.status = Status::Running;
                }
                self.push_activity(Activity::Notice {
                    text: format!("✗ goal rejected: {reason}"),
                    color: Color::Red,
                });
            }
            Event::GoalAccepted { summary } => {
                if self.chat.is_none() {
                    self.status = Status::Done;
                }
                self.push_activity(Activity::Notice {
                    text: format!("✓ {summary}"),
                    color: Color::Green,
                });
            }
            Event::Aborted { reason } => {
                if let Some(chat) = &mut self.chat {
                    // Chat mode: the turn (not the app) is over; TurnEnd
                    // follows and drives the state machine back to Idle.
                    chat.last_abort_reason = Some(reason.clone());
                } else {
                    self.status = Status::Aborted;
                }
                self.push_activity(Activity::Notice {
                    text: format!("✗ aborted: {reason}"),
                    color: Color::Red,
                });
            }
            Event::Usage { input, output } => {
                // The driver sends cumulative totals after each response.
                self.input_tokens = input;
                self.output_tokens = output;
            }
            Event::SteeringQueued(note) => {
                self.push_activity(Activity::Notice {
                    text: format!("▸ [operator] {note}"),
                    color: Color::Yellow,
                });
            }
            Event::RiskVerdict {
                blocked,
                choice,
                p,
                ..
            } => {
                let (text, color) = if blocked {
                    (
                        format!("✗ risk gate: {choice} (p={p:.2}) — command blocked"),
                        Color::Red,
                    )
                } else {
                    (format!("▸ risk gate: {choice} (p={p:.2})"), Color::Yellow)
                };
                self.push_activity(Activity::Notice { text, color });
            }
            Event::RiskGateDisabled => {
                self.push_activity(Activity::Notice {
                    text: "▸ risk gate disabled by operator".to_string(),
                    color: Color::Yellow,
                });
            }
            Event::TurnStart { objective } => {
                if let Some(chat) = &mut self.chat {
                    chat.state = ChatState::Working;
                    chat.last_abort_reason = None;
                    let truncated: String = objective.chars().take(80).collect();
                    chat.objective = objective;
                    self.push_activity(Activity::Notice {
                        text: format!("─ objective: {truncated} ─"),
                        color: Color::Cyan,
                    });
                }
            }
            Event::TurnEnd { reason } => {
                if let Some(chat) = &mut self.chat {
                    chat.state = ChatState::Idle;
                    chat.objective.clear();
                    let (text, color) = match reason {
                        TurnEndReason::Completed | TurnEndReason::GoalAccepted => {
                            ("─ turn complete ─".to_string(), Color::Cyan)
                        }
                        TurnEndReason::Interrupted | TurnEndReason::BudgetExceeded => {
                            let detail = chat
                                .last_abort_reason
                                .take()
                                .unwrap_or_else(|| reason.label().to_string());
                            (
                                format!("─ turn interrupted: {detail} ─"),
                                Color::Yellow,
                            )
                        }
                    };
                    self.push_activity(Activity::Notice { text, color });
                }
            }
        }
    }

    fn push_activity(&mut self, entry: Activity) {
        self.activity.push(entry);
    }

    pub fn on_key(&mut self, key: KeyEvent, abort: &AtomicBool) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.chat.is_some() {
            self.on_key_chat(key, abort);
            return;
        }
        match self.input_mode {
            InputMode::Normal => match key.code {
                KeyCode::Char('q') => {
                    // First press: graceful abort at the next iteration boundary.
                    // Second press: force the UI closed.
                    self.should_quit =
                        abort.swap(true, std::sync::atomic::Ordering::SeqCst);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    abort.store(true, std::sync::atomic::Ordering::SeqCst);
                    self.should_quit = true;
                }
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Editing;
                    self.input.clear();
                }
                KeyCode::PageUp => self.scroll_back = self.scroll_back.saturating_add(10),
                KeyCode::PageDown => self.scroll_back = self.scroll_back.saturating_sub(10),
                _ => {}
            },
            InputMode::Editing => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                    self.input.clear();
                }
                KeyCode::Enter => {
                    let note = self.input.trim().to_string();
                    if !note.is_empty() {
                        let _ = self.steering_tx.send(note);
                    }
                    self.input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.input.pop();
                }
                KeyCode::Char(c) => self.input.push(c),
                _ => {}
            },
        }
    }

    pub fn title(&self) -> String {
        let goal: String = self.goal.chars().take(60).collect();
        let elapsed = self.started.elapsed();
        let mins = elapsed.as_secs() / 60;
        let secs = elapsed.as_secs() % 60;
        let scope = if self.chat.is_some() {
            "chug chat".to_string()
        } else {
            format!("chug ─ {goal}")
        };
        format!(
            " {scope} ─ model: {} ─ iter {}/{} ─ {mins:02}:{secs:02} ─ in {} / out {} tok ",
            self.model,
            self.iter.0,
            self.iter.1,
            fmt_k(self.input_tokens),
            fmt_k(self.output_tokens)
        )
    }

    /// Chat-mode key handling. The input dock is always focused, so
    /// printable characters go straight into it. Single-letter hotkeys only
    /// fire on an empty dock: `q` quits (Idle) or interrupts (Working).
    fn on_key_chat(&mut self, key: KeyEvent, abort: &AtomicBool) {
        let state = self.chat.as_ref().map(|c| c.state);
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match state {
                    Some(ChatState::Working) | Some(ChatState::Interrupting) => {
                        // Interrupt the turn; the app stays up.
                        abort.store(true, std::sync::atomic::Ordering::SeqCst);
                        self.set_chat_state(ChatState::Interrupting);
                    }
                    _ => {
                        abort.store(true, std::sync::atomic::Ordering::SeqCst);
                        self.should_quit = true;
                    }
                }
            }
            KeyCode::Char('q') if self.input.is_empty() => match state {
                Some(ChatState::Working) => {
                    abort.store(true, std::sync::atomic::Ordering::SeqCst);
                    self.set_chat_state(ChatState::Interrupting);
                }
                Some(ChatState::Interrupting) => {
                    // Second `q`: force the UI closed (same as run mode).
                    self.should_quit = true;
                }
                _ => {
                    abort.store(true, std::sync::atomic::Ordering::SeqCst);
                    self.should_quit = true;
                }
            },
            KeyCode::Esc => match state {
                Some(ChatState::Working) => {
                    abort.store(true, std::sync::atomic::Ordering::SeqCst);
                    self.set_chat_state(ChatState::Interrupting);
                }
                Some(ChatState::Interrupting) => {}
                _ => self.input.clear(),
            },
            KeyCode::Enter => {
                let line = self.input.trim().to_string();
                if !line.is_empty() {
                    self.submit_chat_line(&line, abort);
                }
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            KeyCode::PageUp => self.scroll_back = self.scroll_back.saturating_add(10),
            KeyCode::PageDown => self.scroll_back = self.scroll_back.saturating_sub(10),
            _ => {}
        }
    }

    fn set_chat_state(&mut self, state: ChatState) {
        if let Some(chat) = &mut self.chat {
            chat.state = state;
        }
    }

    /// Route a submitted dock line: slash commands are handled locally, idle
    /// submissions become objectives, working submissions become steering
    /// notes (the existing `[operator]` mechanism).
    fn submit_chat_line(&mut self, line: &str, abort: &AtomicBool) {
        if line.trim_start().starts_with('/')
            && let Some(cmd) = chat::parse_slash(line)
        {
            self.handle_slash(cmd, abort);
            return;
        }
        let Some(chat) = &self.chat else { return };
        match chat.state {
            ChatState::Idle => {
                // The worker emits TurnStart, which flips the state machine.
                let _ = chat.objective_tx.send(line.to_string());
            }
            ChatState::Working | ChatState::Interrupting => {
                let _ = self.steering_tx.send(line.to_string());
            }
        }
    }

    fn notice(&mut self, text: String, color: Color) {
        self.push_activity(Activity::Notice { text, color });
    }

    /// Slash commands never reach the LLM; they either update session state
    /// (forwarded to the worker) or produce a local activity-stream line.
    fn handle_slash(&mut self, cmd: SlashCommand, abort: &AtomicBool) {
        match cmd {
            SlashCommand::Spec(Some(path)) => {
                let Some(chat) = &self.chat else { return };
                let candidate = if std::path::Path::new(&path).is_absolute() {
                    std::path::PathBuf::from(&path)
                } else {
                    chat.cwd.join(&path)
                };
                match candidate.canonicalize() {
                    Ok(resolved) => {
                        let _ = chat
                            .update_tx
                            .send(SlashUpdate::Spec(Some(resolved.clone())));
                        self.notice(format!("spec: {}", resolved.display()), Color::Cyan);
                    }
                    Err(e) => {
                        self.notice(format!("✗ spec {path}: {e}"), Color::Red);
                    }
                }
            }
            SlashCommand::Spec(None) => {
                self.send_update(SlashUpdate::Spec(None));
                self.notice("spec cleared".to_string(), Color::Cyan);
            }
            SlashCommand::Goal(Some(text)) => {
                self.send_update(SlashUpdate::Goal(Some(text.clone())));
                self.notice(format!("goal: {text}"), Color::Cyan);
            }
            SlashCommand::Goal(None) => {
                self.send_update(SlashUpdate::Goal(None));
                self.notice("goal cleared".to_string(), Color::Cyan);
            }
            SlashCommand::Check(Some(cmd)) => {
                self.send_update(SlashUpdate::Check(Some(cmd.clone())));
                self.notice(format!("check: {cmd}"), Color::Cyan);
            }
            SlashCommand::Check(None) => {
                self.send_update(SlashUpdate::Check(None));
                self.notice(
                    "check cleared — goal_complete ends the turn unverified".to_string(),
                    Color::Cyan,
                );
            }
            SlashCommand::Ledger => {
                // The ledger panel is already live; "refocus" = flash the
                // freshness border and reset activity scroll to the bottom.
                self.ledger_changed_at = Some(Instant::now());
                self.scroll_back = 0;
            }
            SlashCommand::Model(Some(id)) => {
                self.model = id.clone();
                self.send_update(SlashUpdate::Model(id.clone()));
                self.notice(format!("model: {id}"), Color::Cyan);
            }
            SlashCommand::Model(None) => {
                self.notice(format!("current model: {}", self.model), Color::Cyan);
            }
            SlashCommand::Budget(Some((iters, minutes))) => {
                if let Some(chat) = &mut self.chat {
                    chat.budget = (iters, minutes);
                }
                self.send_update(SlashUpdate::Budget { iters, minutes });
                self.notice(
                    format!("budget: {iters} iters / {minutes} min per turn"),
                    Color::Cyan,
                );
            }
            SlashCommand::Budget(None) => {
                let budget = self.chat.as_ref().map(|c| c.budget).unwrap_or((0, 0));
                self.notice(
                    format!("budget: {} iters / {} min per turn", budget.0, budget.1),
                    Color::Cyan,
                );
            }
            SlashCommand::Quit => {
                // Same as `q` in Idle: graceful quit on the normal exit path.
                abort.store(true, std::sync::atomic::Ordering::SeqCst);
                self.should_quit = true;
            }
            SlashCommand::Help => {
                self.notice(chat::HELP_LINE.to_string(), Color::Cyan);
            }
            SlashCommand::Unknown(name) => {
                self.notice(format!("unknown command: /{name} (see /help)"), Color::Yellow);
            }
            SlashCommand::Usage(usage) => {
                self.notice(format!("usage: {usage}"), Color::Yellow);
            }
        }
    }

    fn send_update(&mut self, update: SlashUpdate) {
        if let Some(chat) = &self.chat {
            let _ = chat.update_tx.send(update);
        }
    }
}

fn fmt_k(tokens: u64) -> String {
    if tokens >= 1000 {
        format!("{:.1}k", tokens as f64 / 1000.0)
    } else {
        tokens.to_string()
    }
}

/// Greedy word-wrap on chars (good enough for a terminal dashboard).
pub fn wrap_rows(text: &str, width: usize, style: Style) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for raw_line in text.split('\n') {
        if width == 0 {
            out.push(Line::from(Span::styled(raw_line.to_string(), style)));
            continue;
        }
        let mut current = String::new();
        for word in raw_line.split(' ') {
            if !current.is_empty() {
                let candidate_len =
                    current.chars().count() + 1 + word.chars().count();
                if candidate_len > width {
                    out.push(Line::from(Span::styled(
                        std::mem::take(&mut current),
                        style,
                    )));
                }
            }
            // Very long single word: hard-split into width-sized chunks.
            let mut remainder = word;
            while remainder.chars().count() > width {
                let head: String = remainder.chars().take(width).collect();
                let head_len = head.len();
                out.push(Line::from(Span::styled(head, style)));
                remainder = &remainder[head_len..];
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(remainder);
        }
        out.push(Line::from(Span::styled(current, style)));
    }
    out
}

pub fn run_tui(cfg: TuiConfig) -> std::io::Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    // Panic-safe restore: any panic anywhere in the process restores the
    // terminal first, so the screen is never left in raw/alternate mode.
    std::panic::set_hook(Box::new(|info| {
        restore_terminal();
        // Fall through to the default panic reporting (message + location).
        eprintln!("{info}");
    }));

    let result = ui_loop(cfg);

    restore_terminal();
    // Uninstall our wrapper; the std default hook takes over again.
    let _ = std::panic::take_hook();
    result
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
}

fn ui_loop(cfg: TuiConfig) -> std::io::Result<()> {
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let mut app = match cfg.chat {
        Some(wiring) => App::new_chat(cfg.model.clone(), cfg.steering_tx.clone(), wiring),
        None => App::new(cfg.goal.clone(), cfg.model.clone(), cfg.steering_tx.clone()),
    };

    loop {
        while let Ok(e) = cfg.events.try_recv() {
            app.apply(e);
        }

        terminal.draw(|f| draw(f, &app))?;

        if event::poll(Duration::from_millis(100))?
            && let CtEvent::Key(key) = event::read()?
        {
            app.on_key(key, &cfg.abort);
        }

        if app.should_quit {
            break;
        }
        if cfg.driver_done.load(std::sync::atomic::Ordering::SeqCst) {
            // Drain any final events (GoalAccepted / Aborted) before exiting.
            while let Ok(e) = cfg.events.try_recv() {
                app.apply(e);
            }
            terminal.draw(|f| draw(f, &app))?;
            break;
        }
    }
    Ok(())
}

fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let outer = Block::bordered().title(app.title());
    let inner = outer.inner(area);
    f.render_widget(outer, area);

    let layout = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);
    let main = Layout::horizontal([
        Constraint::Percentage(62),
        Constraint::Percentage(38),
    ])
    .split(layout[0]);

    draw_activity(f, main[0], app);
    draw_ledger(f, main[1], app);
    draw_status(f, layout[1], app);
    draw_input(f, layout[2], app);
}

fn draw_activity(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let height = area.height as usize;
    let width = area.width as usize;
    let rows = activity_lines(app, width, height);
    let paragraph = Paragraph::new(rows);
    f.render_widget(paragraph, area);
}

fn activity_lines(app: &App, width: usize, height: usize) -> Vec<Line<'static>> {
    let mut rows: Vec<Line> = Vec::new();
    for entry in &app.activity {
        match entry {
            Activity::Model(text) => rows.extend(wrap_rows(text, width, Style::default())),
            Activity::Tool { name, ok, preview } => {
                let (text, color) = match ok {
                    Some(true) => (format!("▸ {name} -> ok"), Color::Green),
                    Some(false) => (format!("▸ {name} -> FAIL {preview}"), Color::Red),
                    None => (format!("▸ {name} …"), Color::Reset),
                };
                rows.extend(wrap_rows(&text, width, Style::default().fg(color)));
            }
            Activity::Notice { text, color } => {
                rows.extend(wrap_rows(text, width, Style::default().fg(*color)));
            }
        }
    }
    let total = rows.len();
    let start = total
        .saturating_sub(height)
        .saturating_sub(app.scroll_back as usize);
    rows.into_iter().skip(start).take(height).collect()
}

fn draw_ledger(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let fresh = app
        .ledger_changed_at
        .is_some_and(|t| t.elapsed() < Duration::from_secs(5));
    let border_color = if fresh { Color::Cyan } else { Color::Reset };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(" LEDGER ");
    let inner_area = block.inner(area);
    f.render_widget(block, area);

    let width = inner_area.width as usize;
    let height = inner_area.height as usize;
    let mut rows: Vec<Line> = Vec::new();
    for line in app.ledger.split('\n') {
        rows.extend(wrap_rows(line, width, Style::default()));
    }
    // Ledger auto-follows the bottom.
    let start = rows.len().saturating_sub(height);
    let window: Vec<Line> = rows.into_iter().skip(start).collect();
    f.render_widget(Paragraph::new(window).wrap(Wrap { trim: false }), inner_area);
}

fn draw_status(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    if let Some(chat) = &app.chat {
        let (label, color, hints) = match chat.state {
            ChatState::Idle => (
                format!("{} — type a request", chat.state.label()),
                Color::Cyan,
                " ─ Enter: send · /help · q: quit · PgUp/PgDn: scroll",
            ),
            ChatState::Working => {
                let objective: String = chat.objective.chars().take(50).collect();
                (
                    format!("{} — {objective}", chat.state.label()),
                    Color::Yellow,
                    " ─ Enter: steer · Esc: interrupt · PgUp/PgDn: scroll",
                )
            }
            ChatState::Interrupting => (
                chat.state.label().to_string(),
                Color::Red,
                " ─ waiting for the turn to stop · q: force quit",
            ),
        };
        let line = Line::from(vec![
            Span::styled(
                format!("status: {label}"),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(hints, Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(line, area);
        return;
    }
    let status_span = Span::styled(
        format!("status: {}", app.status.label()),
        Style::default()
            .fg(app.status.color())
            .add_modifier(if app.status == Status::Verifying {
                Modifier::BOLD
            } else {
                Modifier::empty()
            }),
    );
    let hints = Span::styled(
        " ─ i: steer  PgUp/PgDn: scroll  q: quit",
        Style::default().fg(Color::DarkGray),
    );
    f.render_widget(Line::from(vec![status_span, hints]), area);
}

fn draw_input(f: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    if let Some(chat) = &app.chat {
        // Chat mode: the input dock is always visible and focused.
        let (glyph, color) = match chat.state {
            ChatState::Idle => ("❯", Color::Cyan),
            ChatState::Working | ChatState::Interrupting => ("…", Color::Yellow),
        };
        let line = Line::from(Span::styled(
            format!("{glyph} {}_", app.input),
            Style::default().fg(color),
        ));
        f.render_widget(line, area);
        return;
    }
    if app.input_mode != InputMode::Editing {
        return;
    }
    let line = Line::from(Span::styled(
        format!("steer> {}_", app.input),
        Style::default().fg(Color::Yellow),
    ));
    f.render_widget(line, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn app() -> App {
        let (tx, _rx) = mpsc::channel::<String>();
        App::new("build the thing".into(), "test-model".into(), tx)
    }

    #[test]
    fn reducer_iteration_updates_counter() {
        let mut a = app();
        a.apply(Event::Iteration {
            n: 7,
            max: 40,
            messages: 12,
        });
        assert_eq!(a.iter, (7, 40));
    }

    #[test]
    fn reducer_model_text_grows_activity_with_prefix() {
        let mut a = app();
        a.apply(Event::ModelText("first line\nsecond line".into()));
        assert_eq!(a.activity.len(), 2);
        match (&a.activity[0], &a.activity[1]) {
            (Activity::Model(t0), Activity::Model(t1)) => {
                assert_eq!(t0, "▸ model: first line");
                assert_eq!(t1, "  second line");
            }
            _ => panic!("expected model entries"),
        }
    }

    #[test]
    fn reducer_tool_start_then_result_completes_entry() {
        let mut a = app();
        a.apply(Event::ToolStart {
            name: "bash".into(),
        });
        a.apply(Event::ToolResult {
            name: "bash".into(),
            ok: false,
            preview: "boom".into(),
        });
        assert_eq!(a.activity.len(), 1);
        match &a.activity[0] {
            Activity::Tool { name, ok, preview } => {
                assert_eq!(name, "bash");
                assert_eq!(*ok, Some(false));
                assert_eq!(preview, "boom");
            }
            _ => panic!("expected tool entry"),
        }
    }

    #[test]
    fn reducer_ledger_replaces_and_timestamps() {
        let mut a = app();
        assert!(a.ledger.is_empty());
        a.apply(Event::LedgerChanged("# Ledger\n\n## Next\n- x".into()));
        assert_eq!(a.ledger, "# Ledger\n\n## Next\n- x");
        assert!(a.ledger_changed_at.is_some());
    }

    #[test]
    fn reducer_status_transitions() {
        let mut a = app();
        assert_eq!(a.status, Status::Running);
        a.apply(Event::Verifying {
            cmd: "cargo test".into(),
        });
        assert_eq!(a.status, Status::Verifying);
        a.apply(Event::GoalRejected {
            reason: "check command failed".into(),
        });
        assert_eq!(a.status, Status::Running);
        a.apply(Event::GoalAccepted {
            summary: "done".into(),
        });
        assert_eq!(a.status, Status::Done);

        let mut b = app();
        b.apply(Event::Aborted {
            reason: "operator abort".into(),
        });
        assert_eq!(b.status, Status::Aborted);
    }

    #[test]
    fn reducer_usage_overwrites_with_cumulative_totals() {
        let mut a = app();
        a.apply(Event::Usage {
            input: 100,
            output: 10,
        });
        a.apply(Event::Usage {
            input: 250,
            output: 40,
        });
        assert_eq!(a.input_tokens, 250);
        assert_eq!(a.output_tokens, 40);
    }

    #[test]
    fn reducer_steering_and_abort_notices() {
        let mut a = app();
        a.apply(Event::SteeringQueued("focus on tests".into()));
        match &a.activity[0] {
            Activity::Notice { text, .. } => assert_eq!(text, "▸ [operator] focus on tests"),
            _ => panic!("expected notice"),
        }
    }

    #[test]
    fn title_contains_goal_model_iter_and_tokens() {
        let mut a = app();
        a.apply(Event::Iteration {
            n: 7,
            max: 40,
            messages: 1,
        });
        a.apply(Event::Usage {
            input: 12_300,
            output: 4_100,
        });
        let t = a.title();
        assert!(t.starts_with(" chug ─ build the thing ─ model: test-model ─ iter 7/40 ─ "));
        assert!(t.ends_with("in 12.3k / out 4.1k tok "));
    }

    #[test]
    fn key_q_sets_abort_then_second_q_forces_quit() {
        let mut a = app();
        let abort = Arc::new(AtomicBool::new(false));
        a.on_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            &abort,
        );
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!a.should_quit);
        a.on_key(
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            &abort,
        );
        assert!(a.should_quit);
    }

    #[test]
    fn key_i_opens_input_enter_sends_esc_cancels() {
        let (tx, rx) = mpsc::channel::<String>();
        let mut a = App::new("g".into(), "m".into(), tx);
        let abort = Arc::new(AtomicBool::new(false));
        a.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), &abort);
        assert_eq!(a.input_mode, InputMode::Editing);
        for c in "fix the parser".chars() {
            a.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), &abort);
        }
        assert_eq!(a.input, "fix the parser");
        a.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &abort);
        assert_eq!(a.input_mode, InputMode::Normal);
        assert!(a.input.is_empty());
        assert_eq!(rx.try_recv().unwrap(), "fix the parser");

        a.on_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE), &abort);
        a.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE), &abort);
        a.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &abort);
        assert_eq!(a.input_mode, InputMode::Normal);
        assert!(a.input.is_empty());
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn scroll_pages_and_auto_follow() {
        let mut a = app();
        let abort = Arc::new(AtomicBool::new(false));
        a.on_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &abort);
        a.on_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE), &abort);
        assert_eq!(a.scroll_back, 20);
        a.on_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &abort);
        assert_eq!(a.scroll_back, 10);
        a.on_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &abort);
        assert_eq!(a.scroll_back, 0);
    }

    #[test]
    fn activity_window_scrolls_and_follows_bottom() {
        let mut a = app();
        for i in 0..50 {
            a.apply(Event::ModelText(format!("line {i}")));
        }
        // Full render at width 80: 50 rows.
        let rows = activity_lines(&a, 80, 50);
        assert_eq!(rows.len(), 50);
        // Small window follows the bottom by default.
        let rows = activity_lines(&a, 80, 5);
        assert_eq!(rows.len(), 5);
        // scroll_back of 10 shows rows from further up.
        a.scroll_back = 10;
        let rows = activity_lines(&a, 80, 5);
        assert_eq!(rows.len(), 5);
    }

    #[test]
    fn wrap_rows_respects_width_and_hard_splits_long_words() {
        let rows = wrap_rows("aaaa bbbb cccc", 5, Style::default());
        let text: Vec<String> = rows
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.to_string()).collect())
            .collect();
        assert_eq!(text, vec!["aaaa", "bbbb", "cccc"]);

        let rows = wrap_rows(&"x".repeat(12), 5, Style::default());
        assert_eq!(rows.len(), 3);

        // Multi-line input produces one entry per line.
        let rows = wrap_rows("a\nb", 10, Style::default());
        assert_eq!(rows.len(), 2);
    }

    // ---------- chat mode ----------

    struct ChatFixture {
        app: App,
        objective_rx: mpsc::Receiver<String>,
        update_rx: mpsc::Receiver<SlashUpdate>,
        steer_rx: mpsc::Receiver<String>,
        abort: Arc<AtomicBool>,
        _tmp: tempfile::TempDir,
    }

    fn chat_app() -> ChatFixture {
        let tmp = tempfile::tempdir().unwrap();
        let (steer_tx, steer_rx) = mpsc::channel::<String>();
        let (objective_tx, objective_rx) = mpsc::channel::<String>();
        let (update_tx, update_rx) = mpsc::channel::<SlashUpdate>();
        let abort = Arc::new(AtomicBool::new(false));
        let app = App::new_chat(
            "test-model".into(),
            steer_tx,
            ChatWiring {
                cwd: tmp.path().to_path_buf(),
                budget: (40, 120),
                objective_tx,
                update_tx,
            },
        );
        ChatFixture {
            app,
            objective_rx,
            update_rx,
            steer_rx,
            abort,
            _tmp: tmp,
        }
    }

    fn press(app: &mut App, code: KeyCode, abort: &AtomicBool) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE), abort);
    }

    fn type_text(app: &mut App, text: &str, abort: &AtomicBool) {
        for c in text.chars() {
            press(app, KeyCode::Char(c), abort);
        }
    }

    fn notice_texts(app: &App) -> Vec<String> {
        app.activity
            .iter()
            .filter_map(|a| match a {
                Activity::Notice { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chat_turn_start_sets_working_and_objective_banner() {
        let mut f = chat_app();
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Idle);
        f.app.apply(Event::TurnStart {
            objective: "build the thing".into(),
        });
        let chat = f.app.chat.as_ref().unwrap();
        assert_eq!(chat.state, ChatState::Working);
        assert_eq!(chat.objective, "build the thing");
        let notices = notice_texts(&f.app);
        assert_eq!(notices, vec!["─ objective: build the thing ─"]);
    }

    #[test]
    fn chat_objective_banner_truncates_at_80_chars() {
        let mut f = chat_app();
        let long = "x".repeat(100);
        f.app.apply(Event::TurnStart {
            objective: long.clone(),
        });
        let notices = notice_texts(&f.app);
        assert_eq!(notices, vec![format!("─ objective: {} ─", "x".repeat(80))]);
    }

    #[test]
    fn chat_turn_end_completed_returns_idle_with_banner() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        f.app.apply(Event::TurnEnd {
            reason: TurnEndReason::Completed,
        });
        let chat = f.app.chat.as_ref().unwrap();
        assert_eq!(chat.state, ChatState::Idle);
        assert!(chat.objective.is_empty());
        let notices = notice_texts(&f.app);
        assert_eq!(notices.last().unwrap(), "─ turn complete ─");
    }

    #[test]
    fn chat_turn_end_interrupted_uses_abort_reason_in_banner() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        f.app.apply(Event::Aborted {
            reason: "operator interrupt".into(),
        });
        f.app.apply(Event::TurnEnd {
            reason: TurnEndReason::Interrupted,
        });
        let notices = notice_texts(&f.app);
        assert_eq!(
            notices.last().unwrap(),
            "─ turn interrupted: operator interrupt ─"
        );
        // Chat mode never latches the run-mode Aborted status.
        assert_ne!(f.app.status, Status::Aborted);
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Idle);
    }

    #[test]
    fn chat_turn_end_budget_exceeded_banner() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        f.app.apply(Event::Aborted {
            reason: "iteration budget exceeded".into(),
        });
        f.app.apply(Event::TurnEnd {
            reason: TurnEndReason::BudgetExceeded,
        });
        let notices = notice_texts(&f.app);
        assert_eq!(
            notices.last().unwrap(),
            "─ turn interrupted: iteration budget exceeded ─"
        );
    }

    #[test]
    fn chat_goal_accepted_prints_summary_without_latching_status() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        f.app.apply(Event::GoalAccepted {
            summary: "did it".into(),
        });
        f.app.apply(Event::TurnEnd {
            reason: TurnEndReason::GoalAccepted,
        });
        assert_ne!(f.app.status, Status::Done);
        let notices = notice_texts(&f.app);
        assert!(notices.iter().any(|t| t == "✓ did it"));
        assert_eq!(notices.last().unwrap(), "─ turn complete ─");
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Idle);
    }

    #[test]
    fn chat_submit_in_idle_becomes_objective() {
        let mut f = chat_app();
        type_text(&mut f.app, "create hello.py", &f.abort.clone());
        let abort = Arc::clone(&f.abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(f.objective_rx.try_recv().unwrap(), "create hello.py");
        assert!(f.steer_rx.try_recv().is_err());
        assert!(f.app.input.is_empty());
    }

    #[test]
    fn chat_submit_in_working_becomes_operator_steering() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "focus on tests", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        // Steering carries the raw note; the worker prefixes [operator].
        assert_eq!(f.steer_rx.try_recv().unwrap(), "focus on tests");
        assert!(f.objective_rx.try_recv().is_err());
    }

    #[test]
    fn chat_slash_lines_never_reach_objective_or_steering_channels() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "/help", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert!(f.objective_rx.try_recv().is_err());
        assert!(f.steer_rx.try_recv().is_err());
        let notices = notice_texts(&f.app);
        assert_eq!(notices, vec![chat::HELP_LINE]);

        type_text(&mut f.app, "/xyzzy", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        let notices = notice_texts(&f.app);
        assert_eq!(notices.last().unwrap(), "unknown command: /xyzzy (see /help)");
    }

    #[test]
    fn chat_slash_with_leading_whitespace_still_parses() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "  /goal ship it", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(
            f.update_rx.try_recv().unwrap(),
            SlashUpdate::Goal(Some("ship it".into()))
        );
    }

    #[test]
    fn chat_slash_spec_goal_check_model_budget_updates() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        let tmp_spec = f.app.chat.as_ref().unwrap().cwd.join("SPEC.md");
        std::fs::write(&tmp_spec, "spec body").unwrap();

        type_text(&mut f.app, "/spec SPEC.md", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(
            f.update_rx.try_recv().unwrap(),
            SlashUpdate::Spec(Some(tmp_spec.canonicalize().unwrap()))
        );

        type_text(&mut f.app, "/spec", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(f.update_rx.try_recv().unwrap(), SlashUpdate::Spec(None));

        type_text(&mut f.app, "/check cargo test", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(
            f.update_rx.try_recv().unwrap(),
            SlashUpdate::Check(Some("cargo test".into()))
        );

        type_text(&mut f.app, "/model opus-4", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(
            f.update_rx.try_recv().unwrap(),
            SlashUpdate::Model("opus-4".into())
        );
        assert_eq!(f.app.model, "opus-4");

        type_text(&mut f.app, "/budget 5 15", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert_eq!(
            f.update_rx.try_recv().unwrap(),
            SlashUpdate::Budget {
                iters: 5,
                minutes: 15
            }
        );
        assert_eq!(f.app.chat.as_ref().unwrap().budget, (5, 15));

        // Malformed budget: usage line, nothing forwarded.
        type_text(&mut f.app, "/budget 5", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert!(f.update_rx.try_recv().is_err());
        let notices = notice_texts(&f.app);
        assert_eq!(notices.last().unwrap(), "usage: /budget <iters> <minutes>");
    }

    #[test]
    fn chat_slash_spec_missing_file_echoes_error_and_sends_nothing() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "/spec nope.md", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert!(f.update_rx.try_recv().is_err());
        let notices = notice_texts(&f.app);
        assert!(notices.last().unwrap().starts_with("✗ spec nope.md:"));
    }

    #[test]
    fn chat_slash_quit_quits_like_q_in_idle() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "/quit", &abort);
        press(&mut f.app, KeyCode::Enter, &abort);
        assert!(f.app.should_quit);
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn chat_esc_in_working_interrupts_but_app_stays_up() {
        let mut f = chat_app();
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        let abort = Arc::clone(&f.abort);
        press(&mut f.app, KeyCode::Esc, &abort);
        assert!(abort.load(std::sync::atomic::Ordering::SeqCst));
        assert!(!f.app.should_quit);
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Interrupting);
        // Turn end returns to Idle.
        f.app.apply(Event::Aborted {
            reason: "operator interrupt".into(),
        });
        f.app.apply(Event::TurnEnd {
            reason: TurnEndReason::Interrupted,
        });
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Idle);
    }

    #[test]
    fn chat_q_empty_input_quits_in_idle_interrupts_in_working() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        // Working: q = interrupt, not quit.
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        press(&mut f.app, KeyCode::Char('q'), &abort);
        assert!(!f.app.should_quit);
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Interrupting);
        // Second q while interrupting: force quit (same as run mode).
        press(&mut f.app, KeyCode::Char('q'), &abort);
        assert!(f.app.should_quit);

        // Idle: q quits.
        let mut f2 = chat_app();
        let abort2 = Arc::clone(&f2.abort);
        press(&mut f2.app, KeyCode::Char('q'), &abort2);
        assert!(f2.app.should_quit);
        assert!(abort2.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn chat_q_with_text_in_dock_is_just_text() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        type_text(&mut f.app, "select ", &abort);
        press(&mut f.app, KeyCode::Char('q'), &abort);
        assert!(!f.app.should_quit);
        assert_eq!(f.app.input, "select q");
        assert!(!abort.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn chat_ctrl_c_idle_quits_working_interrupts() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        f.app.apply(Event::TurnStart {
            objective: "work".into(),
        });
        f.app.on_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &abort,
        );
        assert!(!f.app.should_quit);
        assert_eq!(f.app.chat.as_ref().unwrap().state, ChatState::Interrupting);

        let mut f2 = chat_app();
        let abort2 = Arc::clone(&f2.abort);
        f2.app.on_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &abort2,
        );
        assert!(f2.app.should_quit);
    }

    #[test]
    fn chat_i_does_not_open_run_mode_input_line() {
        let mut f = chat_app();
        let abort = Arc::clone(&f.abort);
        press(&mut f.app, KeyCode::Char('i'), &abort);
        // The dock is always focused: `i` is literal text, never a mode flip.
        assert_eq!(f.app.input_mode, InputMode::Normal);
        assert_eq!(f.app.input, "i");
    }

    #[test]
    fn chat_title_uses_chat_scope() {
        let f = chat_app();
        let title = f.app.title();
        assert!(title.starts_with(" chug chat ─ model: test-model ─ iter 0/0 ─ "));
    }

    #[test]
    fn run_mode_title_unchanged_by_chat_scope() {
        let a = app();
        assert!(a.title().starts_with(" chug ─ build the thing ─ model: test-model"));
    }
}

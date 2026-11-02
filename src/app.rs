use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, stdout};

use crate::features::core::{self, Environment, RenderMode};
use crate::features::tui;
use crate::features::tui::completion::{self, CompState};

pub struct App {
    pub input: String,
    pub history: Vec<String>,
    pub should_quit: bool,
    pub env: Environment,
    pub render_mode: RenderMode,
    pub selected: Option<usize>,
    pub comp: Option<CompState>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            should_quit: false,
            env: Environment::new(),
            render_mode: RenderMode::Decimal,
            selected: None,
            comp: None,
        }
    }
}

fn is_selectable(line: &str) -> bool {
    !line.trim().is_empty() && !line.starts_with("-- Mode")
}

fn insertable(line: &str) -> String {
    if let Some(cmd) = line.strip_prefix("> ") {
        cmd.trim().to_string()
    } else {
        let t = line.trim();
        t.strip_prefix('=').unwrap_or(t).trim().to_string()
    }
}

fn handle_tab(app: &mut App) {
    let cands = core::completions(&app.env);
    let (start, word) = completion::current_word(&app.input);

    let (prefix, cycling) = match &app.comp {
        Some(c) => (c.prefix.clone(), true),
        None => (word.to_string(), false),
    };

    let matches = completion::matching(&cands, &prefix);
    if matches.is_empty() {
        app.comp = None;
        return;
    }

    if !cycling {
        if matches.len() == 1 {
            let m = matches[0];
            let mut text = m.name.clone();
            if m.is_function {
                text.push('(');
            }
            app.input.truncate(start);
            app.input.push_str(&text);
            return;
        }
        let lcp = completion::common_prefix(&matches);
        if lcp.len() > prefix.len() {
            app.input.truncate(start);
            app.input.push_str(&lcp);
            return;
        }
    }

    let idx = match &app.comp {
        Some(c) => (c.idx + 1) % matches.len(),
        None => 0,
    };
    let name = matches[idx].name.clone();
    app.input.truncate(start);
    app.input.push_str(&name);
    app.comp = Some(CompState { prefix, idx });
}

pub fn run() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|f| tui::draw(f, &app))?;

        if event::poll(std::time::Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            if key.code != KeyCode::Tab {
                app.comp = None;
            }

            match key.code {
                KeyCode::Esc => {
                    if app.selected.is_some() {
                        app.selected = None;
                    } else {
                        app.should_quit = true;
                    }
                }
                KeyCode::F(2) => {
                    app.render_mode = match app.render_mode {
                        RenderMode::Symbolic => RenderMode::Decimal,
                        RenderMode::Decimal => RenderMode::Symbolic,
                    };
                    app.history
                        .push(format!("-- Mode: {:?} --", app.render_mode));
                    app.selected = None;
                }
                KeyCode::Tab => {
                    if app.selected.is_none() {
                        handle_tab(&mut app);
                    }
                }
                KeyCode::Char(c) => {
                    app.selected = None;
                    app.input.push(c);
                }
                KeyCode::Backspace => {
                    app.selected = None;
                    app.input.pop();
                }
                KeyCode::Up => {
                    app.selected = match app.selected {
                        None => app.history.iter().rposition(|l| is_selectable(l)),
                        Some(s) => (0..s)
                            .rev()
                            .find(|&i| is_selectable(&app.history[i]))
                            .or(Some(s)),
                    };
                }
                KeyCode::Down => {
                    if let Some(s) = app.selected {
                        app.selected =
                            (s + 1..app.history.len()).find(|&i| is_selectable(&app.history[i]));
                    }
                }
                KeyCode::Enter => {
                    if let Some(s) = app.selected {
                        let text = insertable(&app.history[s]);
                        app.input.push_str(&text);
                        app.selected = None;
                    } else if app.input.is_empty() {
                        if let Some(last_cmd) =
                            app.history.iter().rev().find(|line| line.starts_with("> "))
                        {
                            let query = last_cmd.strip_prefix("> ").unwrap_or("").to_string();
                            app.history.push(format!("> {}", query));
                            let result = core::evaluate(&mut app.env, &query, app.render_mode);
                            app.history.push(format!("  {}", result));
                        }
                    } else {
                        let query = app.input.clone();
                        app.history.push(format!("> {}", query));

                        let result = core::evaluate(&mut app.env, &query, app.render_mode);
                        app.history.push(format!("  {}", result));

                        app.input.clear();
                    }
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

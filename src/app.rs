use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io::{self, stdout};

use crate::features::core::{self, Environment, RenderMode};
use crate::features::tui;

pub struct App {
    pub input: String,
    pub history: Vec<String>,
    pub should_quit: bool,
    pub env: Environment,
    pub render_mode: RenderMode,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
            should_quit: false,
            env: Environment::new(),
            render_mode: RenderMode::Symbolic,
        }
    }
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
            match key.code {
                KeyCode::Esc => {
                    app.should_quit = true;
                }
                KeyCode::F(2) => {
                    app.render_mode = match app.render_mode {
                        RenderMode::Symbolic => RenderMode::Decimal,
                        RenderMode::Decimal => RenderMode::Symbolic,
                    };
                    app.history
                        .push(format!("-- Mode: {:?} --", app.render_mode));
                }
                KeyCode::Char(c) => {
                    app.input.push(c);
                }
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Enter => {
                    let query = app.input.clone();
                    app.history.push(format!("> {}", query));

                    let result = core::evaluate(&mut app.env, &query, app.render_mode);
                    app.history.push(format!("  {}", result));

                    app.input.clear();
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

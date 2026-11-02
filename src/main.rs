use crossterm::{
    ExecutableCommand,
    event::{self, Event, KeyCode},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use std::io::{self, stdout};

struct App {
    input: String,
    history: Vec<String>,
}

impl App {
    fn new() -> Self {
        Self {
            input: String::new(),
            history: Vec::new(),
        }
    }
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let mut should_quit = false;

    while !should_quit {
        terminal.draw(|f| {
            let size = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(3)])
                .split(size);

            let history_height = chunks[0].height.saturating_sub(2) as usize;
            let total_items = app.history.len();

            let mut display_lines = Vec::new();
            if history_height > total_items {
                let padding = history_height - total_items;
                for _ in 0..padding {
                    display_lines.push(String::new());
                }
            }
            display_lines.extend(app.history.clone());

            let lines_to_render = if display_lines.len() > history_height {
                let start = display_lines.len() - history_height;
                &display_lines[start..]
            } else {
                &display_lines
            };

            let num_lines = lines_to_render.len();
            let midpoint = num_lines / 5;

            let mut styled_lines = Vec::new();
            for (idx, line_str) in lines_to_render.iter().enumerate() {
                let dist_from_bottom = num_lines.saturating_sub(1 + idx);

                let color = if line_str.is_empty() {
                    Color::Reset
                } else if dist_from_bottom < (num_lines / 2) {
                    Color::Rgb(230, 230, 230)
                } else {
                    let fade_span = (num_lines / 2).max(1) as f32;
                    let current_dist = (dist_from_bottom - (num_lines / 2)) as f32;
                    let factor = (1.0 - (current_dist / fade_span)).clamp(0.1, 1.0);

                    let val = (40.0 + (190.0 * factor)) as u8;
                    Color::Rgb(val, val, val)
                };

                styled_lines.push(Line::from(Span::styled(
                    line_str.clone(),
                    Style::default().fg(color),
                )));
            }

            let history_widget = Paragraph::new(styled_lines)
                .block(Block::default().borders(Borders::ALL).title(" Calculatr "));
            f.render_widget(history_widget, chunks[0]);

            let input_widget = Paragraph::new(app.input.as_str())
                .block(Block::default().borders(Borders::ALL).title(" Input "));
            f.render_widget(input_widget, chunks[1]);
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        should_quit = true;
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Enter => {
                        if !app.input.is_empty() {
                            app.history.push(format!("> {}", app.input));
                            app.input.clear();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    Ok(())
}

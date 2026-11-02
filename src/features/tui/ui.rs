use crate::app::App;
use crate::features::core::RenderMode;
use crate::features::tui::{highlighter::highlight, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();
    let theme = Theme::cyberpunk();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(size);

    let history_height = chunks[0].height.saturating_sub(2) as usize;
    let total = app.history.len();

    // Sichtbares Fenster: standardmässig die letzten Zeilen,
    // bei Auswahl weiter oben scrollt das Fenster mit.
    let default_start = total.saturating_sub(history_height);
    let start_idx = match app.selected {
        Some(s) if s < default_start => s,
        _ => default_start,
    };
    let end_idx = (start_idx + history_height).min(total);
    let visible = &app.history[start_idx..end_idx];

    let padding = history_height.saturating_sub(visible.len());
    let num_lines = padding + visible.len();

    let mut styled_lines: Vec<Line> = Vec::new();
    for _ in 0..padding {
        styled_lines.push(Line::from(""));
    }

    for (i, line_str) in visible.iter().enumerate() {
        let abs_idx = start_idx + i;
        let pos = padding + i;
        let dist_from_bottom = num_lines.saturating_sub(1 + pos);
        let is_selected = app.selected == Some(abs_idx);

        if line_str.is_empty() {
            styled_lines.push(Line::from(""));
            continue;
        }

        let brightness_factor = if dist_from_bottom < (num_lines / 2) {
            1.0
        } else {
            let fade_span = (num_lines / 2).max(1) as f32;
            let current_dist = (dist_from_bottom - (num_lines / 2)) as f32;
            (1.0 - (current_dist / fade_span)).clamp(0.1, 1.0)
        };

        let is_result = line_str.trim_start().starts_with('=');

        let spans: Vec<Span> = if is_result {
            vec![Span::styled(
                line_str.clone(),
                Style::default().fg(theme.number),
            )]
        } else {
            highlight(line_str, &theme)
                .into_iter()
                .map(|s| {
                    let color = if s.content == "("
                        || s.content == ")"
                        || s.content == "["
                        || s.content == "]"
                        || s.content == "{"
                        || s.content == "}"
                    {
                        theme.bracket
                    } else {
                        Color::Rgb(140, 150, 170)
                    };
                    Span::styled(s.content, Style::default().fg(color))
                })
                .collect()
        };

        let faded_spans: Vec<Span> = spans
            .into_iter()
            .map(|span| {
                if is_selected {
                    Span::styled(
                        span.content,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    let original_color = span.style.fg.unwrap_or(theme.input_text);
                    let (r, g, b) = match original_color {
                        Color::Cyan => (0, 255, 255),
                        Color::Magenta => (255, 0, 255),
                        Color::Green => (0, 255, 0),
                        Color::Yellow => (255, 255, 0),
                        Color::White => (255, 255, 255),
                        Color::Rgb(r, g, b) => (r, g, b),
                        _ => (180, 190, 210),
                    };

                    let nr = (r as f32 * brightness_factor) as u8;
                    let ng = (g as f32 * brightness_factor) as u8;
                    let nb = (b as f32 * brightness_factor) as u8;

                    Span::styled(span.content, Style::default().fg(Color::Rgb(nr, ng, nb)))
                }
            })
            .collect();

        let mut line = Line::from(faded_spans);
        if is_selected {
            // Hintergrund über die ganze Zeilenbreite
            line = line.style(Style::default().bg(Color::Rgb(50, 50, 80)));
        }
        styled_lines.push(line);
    }

    // --- ab hier unverändert ---
    let mode_title = match app.render_mode {
        RenderMode::Decimal => Line::from(vec![
            Span::styled(" Calculatr ", Style::default().fg(theme.title)),
            Span::styled("[", Style::default().fg(theme.bracket)),
            Span::styled(" Decimal ", Style::default().fg(theme.input_text)),
            Span::styled("]", Style::default().fg(theme.bracket)),
            Span::styled(" - Symbolic  ", Style::default().fg(theme.title)),
        ]),
        RenderMode::Symbolic => Line::from(vec![
            Span::styled(" Calculatr    Decimal - ", Style::default().fg(theme.title)),
            Span::styled("[", Style::default().fg(theme.bracket)),
            Span::styled(" Symbolic ", Style::default().fg(theme.input_text)),
            Span::styled("]", Style::default().fg(theme.bracket)),
            Span::styled(" ", Style::default().fg(theme.title)),
        ]),
    };

    let history_widget = Paragraph::new(styled_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(mode_title),
    );
    f.render_widget(history_widget, chunks[0]);

    let input_spans = highlight(&app.input, &theme);
    let styled_input_spans: Vec<Span> = input_spans
        .into_iter()
        .map(|s| {
            let color = if s.content == "("
                || s.content == ")"
                || s.content == "["
                || s.content == "]"
                || s.content == "{"
                || s.content == "}"
            {
                theme.bracket
            } else {
                s.style.fg.unwrap_or(theme.input_text)
            };
            Span::styled(s.content, Style::default().fg(color))
        })
        .collect();

    let input_widget = Paragraph::new(Line::from(styled_input_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(Span::styled(" Input ", Style::default().fg(theme.title))),
    );
    f.render_widget(input_widget, chunks[1]);
}

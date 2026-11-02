use crate::app::App;
use crate::features::tui::{highlighter::highlight, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
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

    let mut styled_lines = Vec::new();
    for (idx, line_str) in lines_to_render.iter().enumerate() {
        let dist_from_bottom = num_lines.saturating_sub(1 + idx);

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

        let spans = if is_result {
            vec![Span::styled(
                line_str.clone(),
                Style::default().fg(theme.number),
            )]
        } else {
            let raw_spans = highlight(line_str, &theme);
            raw_spans
                .into_iter()
                .map(|s| Span::styled(s.content, Style::default().fg(Color::Rgb(140, 150, 170))))
                .collect()
        };

        let faded_spans: Vec<Span> = spans
            .into_iter()
            .map(|span| {
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
            })
            .collect();

        styled_lines.push(Line::from(faded_spans));
    }

    let history_widget = Paragraph::new(styled_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(Span::styled(
                " Calculatr ",
                Style::default().fg(theme.title),
            )),
    );
    f.render_widget(history_widget, chunks[0]);

    let input_spans = highlight(&app.input, &theme);
    let input_widget = Paragraph::new(Line::from(input_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(Span::styled(" Input ", Style::default().fg(theme.title))),
    );
    f.render_widget(input_widget, chunks[1]);
}

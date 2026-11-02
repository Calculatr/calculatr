use crate::app::{ActivePane, App};
use crate::features::core::{RenderMode, completions};
use crate::features::tui::completion::{current_word, matching};
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

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(3)])
        .split(size);

    let history_chunk = main_chunks[0];
    let input_chunk = main_chunks[1];

    let (left_chunk, right_chunk) = if app.variables_drawer_open {
        let h_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(history_chunk);
        (h_splits[0], Some(h_splits[1]))
    } else {
        (history_chunk, None)
    };

    let history_height = left_chunk.height.saturating_sub(2) as usize;
    let total = app.history.len();

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
                    let color = if matches!(s.content.as_ref(), "(" | ")" | "[" | "]" | "{" | "}") {
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
                if is_selected && app.active_pane == ActivePane::HistoryInput {
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
        if is_selected && app.active_pane == ActivePane::HistoryInput {
            line = line.style(Style::default().bg(Color::Rgb(50, 50, 80)));
        }
        styled_lines.push(line);
    }

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

    let history_border_color =
        if app.active_pane == ActivePane::HistoryInput && app.variables_drawer_open {
            Color::Yellow
        } else {
            theme.border
        };

    let history_widget = Paragraph::new(styled_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(history_border_color))
            .title(mode_title),
    );
    f.render_widget(history_widget, left_chunk);

    if let Some(r_chunk) = right_chunk {
        let mut var_lines = Vec::new();
        let mut vars: Vec<_> = app.env.get_vars().iter().collect();
        vars.sort_by(|a, b| a.0.cmp(b.0));

        let filtered_vars: Vec<_> = vars
            .into_iter()
            .filter(|(name, _)| name.contains(&app.var_filter))
            .collect();

        for (i, (name, val)) in filtered_vars.iter().enumerate() {
            let is_sel =
                app.var_selected == Some(i) && app.active_pane == ActivePane::VariablesDrawer;
            let prefix_str = if is_sel { "> " } else { "  " };
            let val_str = val.to_string_pretty();
            let line_content = format!("{}{} = {}", prefix_str, name, val_str);

            let style = if is_sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
                    .bg(Color::Rgb(50, 50, 80))
            } else {
                Style::default().fg(theme.input_text)
            };
            var_lines.push(Line::from(Span::styled(line_content, style)));
        }

        let drawer_border_color = if app.active_pane == ActivePane::VariablesDrawer {
            Color::Yellow
        } else {
            theme.border
        };

        let drawer_title = if app.var_filter_active {
            format!(" Variables [/{} ] ", app.var_filter)
        } else if !app.var_filter.is_empty() {
            format!(" Variables (filter: {}) ", app.var_filter)
        } else {
            " Variables [F3/Tab] ".to_string()
        };

        let var_widget = Paragraph::new(var_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(drawer_border_color))
                .title(Span::styled(drawer_title, Style::default().fg(theme.title))),
        );
        f.render_widget(var_widget, r_chunk);
    }

    let input_spans = highlight(&app.input, &theme);
    let mut styled_input_spans: Vec<Span> = input_spans
        .into_iter()
        .map(|s| {
            let color = if matches!(s.content.as_ref(), "(" | ")" | "[" | "]" | "{" | "}") {
                theme.bracket
            } else {
                s.style.fg.unwrap_or(theme.input_text)
            };
            Span::styled(s.content, Style::default().fg(color))
        })
        .collect();

    let cands = completions(&app.env);
    let (_, word) = current_word(&app.input);
    let (prefix, active_idx) = match &app.comp {
        Some(c) => (c.prefix.as_str(), Some(c.idx)),
        None => (word, None),
    };
    let matches = matching(&cands, prefix);

    if app.selected.is_none() && app.comp.is_none() {
        if let Some(first) = matches.first() {
            if first.name.len() > word.len() {
                let mut ghost = first.name[word.len()..].to_string();
                if matches.len() == 1 && first.is_function {
                    ghost.push('(');
                }
                styled_input_spans.push(Span::styled(ghost, Style::default().fg(Color::DarkGray)));
            }
        }
    }

    let mut title = vec![Span::styled(" Input ", Style::default().fg(theme.title))];
    if matches.len() > 1 {
        title.push(Span::styled("[", Style::default().fg(theme.bracket)));
        for (i, m) in matches.iter().take(6).enumerate() {
            if i > 0 {
                title.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
            }
            let style = if Some(i) == active_idx {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            title.push(Span::styled(m.name.clone(), style));
        }
        if matches.len() > 6 {
            title.push(Span::styled(" …", Style::default().fg(Color::DarkGray)));
        }
        title.push(Span::styled("] ", Style::default().fg(theme.bracket)));
    }

    let input_widget = Paragraph::new(Line::from(styled_input_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .title(Line::from(title)),
    );
    f.render_widget(input_widget, input_chunk);
}

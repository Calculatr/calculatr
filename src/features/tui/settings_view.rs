use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::{App, SettingsRow, settings_rows};

const LABEL_WIDTH: usize = 42;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let rows = settings_rows();
    let cursor = app.settings_view.nav.cursor;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(Span::styled(" Settings ", Style::default().fg(theme.title)));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::vertical([Constraint::Min(1), Constraint::Length(2)]).split(inner);

    let mut lines: Vec<Line> = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let selected = cursor == Some(i);
        let marker = if selected { "▶ " } else { "  " };
        let base = if selected {
            Style::default()
                .fg(theme.highlight_fg)
                .bg(theme.highlight_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.input_text)
        };

        match *row {
            SettingsRow::Header(title) => {
                if i > 0 {
                    lines.push(Line::raw(""));
                }
                lines.push(Line::from(Span::styled(
                    format!(" {title}"),
                    Style::default()
                        .fg(theme.title)
                        .add_modifier(Modifier::BOLD),
                )));
            }
            SettingsRow::Toggle(t) => {
                let on = app.settings.get(t);
                let value = if on { "[x] on " } else { "[ ] off" };
                let value_style = if selected {
                    base
                } else {
                    Style::default().fg(if on { theme.message } else { theme.dim })
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{marker}{:<LABEL_WIDTH$}", t.label()), base),
                    Span::styled(value.to_string(), value_style),
                ]));
            }
            SettingsRow::Hotkey(action) => {
                let capturing = app.settings_view.capturing == Some(action);
                let (value, value_style) = if capturing {
                    (
                        "press a key…".to_string(),
                        Style::default()
                            .fg(theme.error)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    let key = app.settings.hotkeys.binding(action).to_string();
                    let style = if selected {
                        base
                    } else {
                        Style::default().fg(theme.bracket)
                    };
                    (key, style)
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{marker}{:<LABEL_WIDTH$}", action.label()), base),
                    Span::styled(value, value_style),
                ]));
            }
        }
    }
    f.render_widget(Paragraph::new(lines), parts[0]);

    let help = match cursor.map(|i| rows[i]) {
        Some(SettingsRow::Toggle(t)) => t.description().to_string(),
        Some(SettingsRow::Hotkey(a)) => format!(
            "Enter rebinds \"{}\" (F-key or Ctrl/Alt combination). Default: {}",
            a.label(),
            a.default_binding()
        ),
        _ => String::new(),
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            help,
            Style::default().fg(theme.dim),
        ))),
        parts[1],
    );
}

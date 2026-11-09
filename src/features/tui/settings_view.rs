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
    let mut cursor_line = 0;
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
            SettingsRow::Setting(setting) => {
                let applicable = app.settings.is_applicable(setting);
                let (value, color) = match app.settings.switch(setting) {
                    Some(true) => ("[x] on".to_string(), theme.message),
                    Some(false) => ("[ ] off".to_string(), theme.dim),
                    None => (
                        format!("◂ {} ▸", app.settings.value_label(setting)),
                        theme.bracket,
                    ),
                };
                let value_style = if selected {
                    base
                } else if applicable {
                    Style::default().fg(color)
                } else {
                    Style::default().fg(theme.dim)
                };
                let label_style = if selected || applicable {
                    base
                } else {
                    Style::default().fg(theme.dim)
                };
                let mut spans = vec![
                    Span::styled(
                        format!("{marker}{:<LABEL_WIDTH$}", setting.label()),
                        label_style,
                    ),
                    Span::styled(value, value_style),
                ];
                if let Some(reason) = app.settings.inactive_reason(setting) {
                    spans.push(Span::styled(
                        format!("  ({reason})"),
                        Style::default().fg(theme.dim),
                    ));
                }
                lines.push(Line::from(spans));
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
        if selected {
            cursor_line = lines.len().saturating_sub(1);
        }
    }
    let visible = parts[0].height as usize;
    let scroll = (cursor_line + 1).saturating_sub(visible) as u16;
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), parts[0]);

    let (help, command) = match cursor.map(|i| rows[i]) {
        Some(SettingsRow::Setting(setting)) => (
            setting.description().to_string(),
            format!("Command: {}   ·   ← → change", setting.command_hint()),
        ),
        Some(SettingsRow::Hotkey(a)) => (
            format!(
                "Enter rebinds \"{}\" (F-key or Ctrl/Alt combination). Default: {}",
                a.label(),
                a.default_binding()
            ),
            String::new(),
        ),
        _ => (String::new(), String::new()),
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(help, Style::default().fg(theme.dim))),
            Line::from(Span::styled(command, Style::default().fg(theme.bracket))),
        ]),
        parts[1],
    );
}

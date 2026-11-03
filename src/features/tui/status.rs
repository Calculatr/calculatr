use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, Focus, Tab};
use crate::features::settings::Action;

pub fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let tab = |label: &str, active: bool| {
        if active {
            Span::styled(
                format!(" {label} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(theme.border)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {label} "), Style::default().fg(theme.dim))
        }
    };
    let switch_key = app.settings.hotkeys.binding(Action::NextTab);
    let line = Line::from(vec![
        Span::styled(
            " Calculatr ",
            Style::default()
                .fg(theme.title)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("│", Style::default().fg(theme.dim)),
        tab("Calculator", app.tab == Tab::Calculator),
        tab("Settings", app.tab == Tab::Settings),
        Span::styled(
            format!("  {switch_key} switches tab"),
            Style::default().fg(theme.dim),
        ),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

pub fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let mode = app.current_mode_label();
    let chip_bg = match mode {
        "INSERT" => Color::Rgb(0, 200, 120),
        "VISUAL" => Color::Rgb(200, 90, 255),
        "SETTINGS" => Color::Rgb(255, 160, 60),
        _ => Color::Rgb(90, 130, 255),
    };

    let pending = match (app.tab, app.focus) {
        (Tab::Calculator, Focus::Input) => app.editor.pending_text(),
        (Tab::Calculator, Focus::History) => app.history_nav.pending_text(),
        (Tab::Calculator, Focus::Variables) => app.var_nav.pending_text(),
        (Tab::Settings, _) => app.settings_view.nav.pending_text(),
    };

    let mut spans = vec![Span::styled(
        format!(" {mode} "),
        Style::default()
            .fg(Color::Black)
            .bg(chip_bg)
            .add_modifier(Modifier::BOLD),
    )];
    if !pending.is_empty() {
        spans.push(Span::styled(
            format!(" {pending}"),
            Style::default().fg(theme.bracket),
        ));
    }
    spans.push(Span::raw("  "));

    match &app.status {
        Some(s) => spans.push(Span::styled(
            s.text.clone(),
            Style::default().fg(if s.is_error {
                theme.error
            } else {
                theme.message
            }),
        )),
        None => spans.push(Span::styled(hints(app), Style::default().fg(theme.dim))),
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn hints(app: &App) -> String {
    let hk = |a: Action| app.settings.hotkeys.binding(a).to_string();
    match (app.tab, app.focus) {
        (Tab::Settings, _) if app.settings_view.capturing.is_some() => {
            "Press the new key · Esc cancels".to_string()
        }
        (Tab::Settings, _) => {
            "j/k move · Enter/Space change · r reset hotkey · Esc back".to_string()
        }
        (Tab::Calculator, Focus::Input) => format!(
            "{} mode · {} variables · {} typst · {} settings · Tab complete · Esc normal · {} quit",
            hk(Action::ToggleRenderMode),
            hk(Action::ToggleVariables),
            hk(Action::ToggleTypst),
            hk(Action::NextTab),
            hk(Action::Quit),
        ),
        (Tab::Calculator, Focus::History) => {
            "j/k move · v select · Enter insert · yy yank · dd delete · l variables · Esc back"
                .to_string()
        }
        (Tab::Calculator, Focus::Variables) if app.var_filter_active => {
            "type to filter · Enter/Esc done".to_string()
        }
        (Tab::Calculator, Focus::Variables) => {
            "j/k move · / filter · v select · Enter insert · dd delete · h history · Esc back"
                .to_string()
        }
    }
}

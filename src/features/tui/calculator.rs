use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::StatefulImage;

use super::highlighter::tokenize;
use super::images::fade_level;
use super::theme::{LineRole, Theme, fade};
use super::typst::markup;
use crate::app::{App, Focus};
use crate::features::core::number::NumFmt;
use crate::features::core::parser::parse_stmt;
use crate::features::core::{Answer, RenderMode, completions};
use crate::features::editor::Mode;
use crate::features::history::{self, LineKind};

const GUTTER: u16 = 2;
const MAX_IMG_ROWS: u16 = 8;

pub fn draw(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

    let (left, right) = if app.variables_open {
        let h = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(chunks[0]);
        (h[0], Some(h[1]))
    } else {
        (chunks[0], None)
    };

    draw_history(f, app, left);
    if let Some(right) = right {
        draw_variables(f, app, right);
    }
    draw_input(f, app, chunks[1]);
}

// ---------------------------------------------------------------------------
// History
// ---------------------------------------------------------------------------

struct Row {
    line: usize,
    height: u16,
    image: Option<RowImage>,
}

struct RowImage {
    source: String,
    cols: u16,
}

fn typst_source(app: &App, line: usize) -> Option<String> {
    if !app.settings.typst_rendering {
        return None;
    }
    let (idx, kind) = history::locate(line);
    let entry = app.history.get(idx)?;
    match kind {
        LineKind::Input => parse_stmt(&entry.input)
            .ok()
            .map(|stmt| markup::input_source(&stmt, &app.theme)),
        LineKind::Result => markup::answer_source(&entry.answer, &app.theme),
    }
}

fn measure(app: &mut App, line: usize, max_width: u16) -> Row {
    let avail = u32::from(max_width.saturating_sub(GUTTER).max(1));
    if let Some(source) = typst_source(app, line)
        && let Some((px_w, px_h)) = app.images.size(&source)
    {
        let font = app.picker.font_size();
        let (cell_w, cell_h) = (u32::from(font.width).max(1), u32::from(font.height).max(1));
        let mut cols = px_w.div_ceil(cell_w).max(1);
        let mut rows = ((px_h as f32 / cell_h as f32).round() as u32).max(1);
        if cols > avail {
            rows = (rows * avail).div_ceil(cols).max(1);
            cols = avail;
        }
        return Row {
            line,
            height: rows.min(u32::from(MAX_IMG_ROWS)) as u16,
            image: Some(RowImage {
                source,
                cols: cols as u16,
            }),
        };
    }
    Row {
        line,
        height: 1,
        image: None,
    }
}

fn brightness(row: usize, num_rows: usize) -> f32 {
    let dist_from_bottom = num_rows.saturating_sub(1 + row);
    if dist_from_bottom < num_rows / 2 {
        1.0
    } else {
        let fade_span = (num_rows / 2).max(1) as f32;
        let d = (dist_from_bottom - num_rows / 2) as f32;
        (1.0 - d / fade_span).clamp(0.1, 1.0)
    }
}

fn line_pieces(app: &App, line: usize) -> Vec<(String, Color)> {
    let theme = &app.theme;
    let (idx, kind) = history::locate(line);
    let Some(entry) = app.history.get(idx) else {
        return Vec::new();
    };
    match kind {
        LineKind::Input => tokenize(&entry.input)
            .into_iter()
            .map(|t| (t.text, theme.token_color(LineRole::HistoryInput, t.kind)))
            .collect(),
        LineKind::Result => {
            let color = match &entry.answer {
                Answer::Value { .. } => {
                    theme.token_color(LineRole::Result, super::theme::TokenKind::Number)
                }
                Answer::Error(_) => theme.error,
                Answer::Message(_) => theme.message,
            };
            vec![(entry.answer.text(), color)]
        }
    }
}

fn mode_title(app: &App, theme: &Theme) -> Line<'static> {
    let on = Style::default().fg(theme.input_text);
    let off = Style::default().fg(theme.title);
    let bracket = Style::default().fg(theme.bracket);
    let (decimal, symbolic) = match app.render_mode {
        RenderMode::Decimal => (true, false),
        RenderMode::Symbolic => (false, true),
    };
    let item = |label: &'static str, active: bool| -> Vec<Span<'static>> {
        if active {
            vec![
                Span::styled("[", bracket),
                Span::styled(format!(" {label} "), on),
                Span::styled("]", bracket),
            ]
        } else {
            vec![Span::styled(format!("  {label}  "), off)]
        }
    };
    let mut spans = vec![Span::styled(" ", off)];
    spans.extend(item("Decimal", decimal));
    spans.push(Span::styled("-", off));
    spans.extend(item("Symbolic", symbolic));
    Line::from(spans)
}

fn draw_history(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = app.theme.clone();
    let focused = app.focus == Focus::History;

    let typst_label = if app.settings.typst_rendering {
        " typst on "
    } else {
        " typst off "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if focused {
            theme.border_focus
        } else {
            theme.border
        }))
        .title(mode_title(app, &theme))
        .title(
            Line::from(Span::styled(typst_label, Style::default().fg(theme.dim))).right_aligned(),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let total = history::line_count(&app.history);
    let cursor = if focused {
        app.history_nav.cursor
    } else {
        None
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut used: u16 = 0;
    let mut bottom_start = total;
    while bottom_start > 0 {
        let row = measure(app, bottom_start - 1, inner.width);
        if used + row.height > inner.height {
            break;
        }
        used += row.height;
        bottom_start -= 1;
        rows.push(row);
    }
    rows.reverse();

    let mut top_anchored = false;
    if let Some(sel) = cursor
        && sel < bottom_start
    {
        rows.clear();
        used = 0;
        let mut i = sel;
        while i < total {
            let row = measure(app, i, inner.width);
            if used + row.height > inner.height {
                break;
            }
            used += row.height;
            rows.push(row);
            i += 1;
        }
        top_anchored = true;
    }

    let selection = if focused {
        app.history_nav.highlighted()
    } else {
        None
    };
    let num_rows = inner.height as usize;
    let mut y = inner.y
        + if top_anchored {
            0
        } else {
            inner.height.saturating_sub(used)
        };

    for row in rows {
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: row.height,
        };
        y += row.height;

        let highlighted = selection.as_ref().is_some_and(|r| r.contains(&row.line));
        let bright = if highlighted {
            1.0
        } else {
            brightness((rect.y - inner.y) as usize, num_rows)
        };
        if highlighted {
            f.render_widget(
                Block::default().style(Style::default().bg(theme.highlight_bg)),
                rect,
            );
        }

        let (_, kind) = history::locate(row.line);
        let gutter_text = if kind == LineKind::Input { "> " } else { "  " };
        let color = |c: Color| {
            if highlighted {
                theme.highlight_fg
            } else {
                fade(c, bright)
            }
        };
        let modifier = if highlighted {
            Modifier::BOLD
        } else {
            Modifier::empty()
        };

        if let Some(img) = &row.image {
            let gutter_rect = Rect {
                y: rect.y + (rect.height - 1) / 2,
                width: GUTTER.min(rect.width),
                height: 1,
                ..rect
            };
            f.render_widget(
                Paragraph::new(Span::styled(
                    gutter_text,
                    Style::default()
                        .fg(color(theme.prompt))
                        .add_modifier(modifier),
                )),
                gutter_rect,
            );

            let img_rect = Rect {
                x: rect.x + GUTTER.min(rect.width),
                width: img.cols.min(rect.width.saturating_sub(GUTTER)),
                ..rect
            };
            let level = fade_level(bright);
            if let Some(proto) =
                app.images
                    .protocol(&app.picker, &theme, &img.source, level, highlighted)
            {
                f.render_stateful_widget(StatefulImage::default(), img_rect, proto);
            }
            continue;
        }

        let mut spans = vec![Span::styled(
            gutter_text,
            Style::default()
                .fg(color(theme.prompt))
                .add_modifier(modifier),
        )];
        spans.extend(line_pieces(app, row.line).into_iter().map(|(text, c)| {
            Span::styled(text, Style::default().fg(color(c)).add_modifier(modifier))
        }));
        f.render_widget(Paragraph::new(Line::from(spans)), rect);
    }
}

// ---------------------------------------------------------------------------
// Variables drawer
// ---------------------------------------------------------------------------

fn draw_variables(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let focused = app.focus == Focus::Variables;

    let names = app.env.sorted_names(&app.var_filter);
    let selection = if focused {
        app.var_nav.highlighted()
    } else {
        None
    };

    let lines: Vec<Line> = names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let value = app
                .env
                .get_expr(name)
                .map(|e| e.to_text(NumFmt::Decimal))
                .unwrap_or_default();
            let is_selected = selection.as_ref().is_some_and(|r| r.contains(&i));
            let is_cursor = focused && app.var_nav.cursor == Some(i);
            let marker = if is_cursor { "> " } else { "  " };
            let text = format!("{marker}{name} = {value}");
            let style = if is_selected {
                Style::default()
                    .fg(theme.highlight_fg)
                    .bg(theme.highlight_bg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.input_text)
            };
            Line::from(Span::styled(text, style))
        })
        .collect();

    let lines = if lines.is_empty() {
        let hint = if app.var_filter.is_empty() {
            "  No variables yet - define one with  x := 2"
        } else {
            "  No match"
        };
        vec![Line::from(Span::styled(
            hint,
            Style::default().fg(theme.dim),
        ))]
    } else {
        lines
    };

    let title = if app.var_filter_active {
        format!(" Variables [/{}▏] ", app.var_filter)
    } else if !app.var_filter.is_empty() {
        format!(" Variables (filter: {}) ", app.var_filter)
    } else {
        " Variables ".to_string()
    };

    let visible = area.height.saturating_sub(2) as usize;
    let scroll = app
        .var_nav
        .cursor
        .filter(|_| focused)
        .map_or(0, |c| (c + 1).saturating_sub(visible)) as u16;

    let widget = Paragraph::new(lines).scroll((scroll, 0)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if focused {
                theme.border_focus
            } else {
                theme.border
            }))
            .title(Span::styled(title, Style::default().fg(theme.title))),
    );
    f.render_widget(widget, area);
}

// ---------------------------------------------------------------------------
// Input line
// ---------------------------------------------------------------------------

fn input_spans(app: &App) -> Vec<Span<'static>> {
    let theme = &app.theme;
    let buffer = &app.editor.buffer;

    let mut cells: Vec<(char, Style)> = Vec::with_capacity(buffer.len());
    for token in tokenize(&buffer.text()) {
        let style = Style::default().fg(theme.token_color(LineRole::Live, token.kind));
        cells.extend(token.text.chars().map(|c| (c, style)));
    }

    let focused = app.focus == Focus::Input;
    let highlight = Style::default()
        .fg(theme.highlight_fg)
        .bg(theme.highlight_bg)
        .add_modifier(Modifier::BOLD);

    if let Some(range) = buffer.placeholder() {
        for i in range {
            if let Some(cell) = cells.get_mut(i) {
                cell.1 = highlight;
            }
        }
    }
    if let Some(range) = app.editor.visual_range().filter(|_| focused) {
        for i in range {
            if let Some(cell) = cells.get_mut(i) {
                cell.1 = highlight;
            }
        }
    }
    if focused && app.editor.mode() != Mode::Insert {
        let at = buffer.cursor();
        if let Some(partner) = buffer.matching_bracket(at) {
            for i in [at, partner] {
                if let Some(cell) = cells.get_mut(i) {
                    cell.1 = cell.1.add_modifier(Modifier::UNDERLINED | Modifier::BOLD);
                }
            }
        }
    }

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut run = String::new();
    let mut run_style = Style::default();
    for (c, style) in cells {
        if !run.is_empty() && style != run_style {
            spans.push(Span::styled(std::mem::take(&mut run), run_style));
        }
        run_style = style;
        run.push(c);
    }
    if !run.is_empty() {
        spans.push(Span::styled(run, run_style));
    }

    if focused && let Some(ghost) = app.editor.ghost(&completions(&app.env)) {
        spans.push(Span::styled(ghost, Style::default().fg(theme.placeholder)));
    }
    spans
}

fn input_title(app: &App) -> Line<'static> {
    let theme = &app.theme;
    let mut title = vec![Span::styled(" Input ", Style::default().fg(theme.title))];

    let candidates = completions(&app.env);
    let (matches, active) = app.editor.completion_matches(&candidates);
    if !matches.is_empty() {
        title.push(Span::styled("[", Style::default().fg(theme.bracket)));
        for (i, m) in matches.iter().take(6).enumerate() {
            if i > 0 {
                title.push(Span::styled(" | ", Style::default().fg(theme.dim)));
            }
            let is_active = Some(i) == active;
            let style = if is_active {
                Style::default()
                    .fg(theme.highlight_fg)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.dim)
            };
            let label = if matches.len() == 1 || is_active {
                m.signature()
            } else {
                m.name.clone()
            };
            title.push(Span::styled(label, style));
        }
        if matches.len() > 6 {
            title.push(Span::styled(" …", Style::default().fg(theme.dim)));
        }
        title.push(Span::styled("] ", Style::default().fg(theme.bracket)));
    }
    Line::from(title)
}

fn draw_input(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border))
        .title(input_title(app));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width <= GUTTER {
        return;
    }

    let prompt_rect = Rect {
        width: GUTTER,
        ..inner
    };
    let text_rect = Rect {
        x: inner.x + GUTTER,
        width: inner.width - GUTTER,
        ..inner
    };
    f.render_widget(
        Paragraph::new(Span::styled("> ", Style::default().fg(theme.prompt))),
        prompt_rect,
    );

    let cursor = app.editor.buffer.cursor();
    let scroll = (cursor + 1).saturating_sub(text_rect.width as usize);
    f.render_widget(
        Paragraph::new(Line::from(input_spans(app))).scroll((0, scroll as u16)),
        text_rect,
    );

    if app.focus == Focus::Input {
        f.set_cursor_position((text_rect.x + (cursor - scroll) as u16, text_rect.y));
    } else if app.focus == Focus::Variables && app.var_filter_active {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_fades_towards_the_top() {
        let n = 20;
        assert_eq!(brightness(n - 1, n), 1.0);
        assert_eq!(brightness(n / 2, n), 1.0);
        assert!(brightness(0, n) <= 0.1 + f32::EPSILON);
        let mut prev = 0.0;
        for row in 0..n {
            let b = brightness(row, n);
            assert!(b >= prev, "monotonic");
            prev = b;
        }
    }
}

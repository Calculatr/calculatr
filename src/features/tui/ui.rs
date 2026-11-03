use crate::app::{ActivePane, App, TypstImage};
use crate::features::core::ast::Stmt;
use crate::features::core::parser::parse_stmt;
use crate::features::core::{RenderMode, completions};
use crate::features::tui::completion::{current_word, matching};
use crate::features::tui::renderer::TypstRenderer;
use crate::features::tui::{highlighter::highlight, theme::Theme};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{StatefulImage, picker::Picker};
use std::collections::HashMap;

const MAX_IMG_ROWS: u16 = 8;

struct Entry {
    idx: usize,
    height: u16,
    typst: Option<(String, u16)>,
}

fn typst_code_for(line: &str, mode: RenderMode) -> Option<String> {
    if mode != RenderMode::Symbolic {
        return None;
    }
    let expr_str = line.strip_prefix("> ")?.trim();
    if expr_str.is_empty() {
        return None;
    }
    let expr = match parse_stmt(expr_str).ok()? {
        Stmt::Expr(e) => e,
        Stmt::Assign(_, e) => e,
    };
    Some(expr.to_typst())
}

fn ensure_image(
    images: &mut HashMap<String, Option<TypstImage>>,
    picker: &mut Picker,
    theme: &Theme,
    code: &str,
) {
    if images.contains_key(code) {
        return;
    }
    let png = TypstRenderer::new(code, theme).render_to_png();
    if let Err(e) = &png {
        let _ = std::fs::write("/tmp/calculatr-typst.log", e);
    }
    let img = png
        .ok()
        .and_then(|png| image::load_from_memory(&png).ok())
        .map(|dyn_img| {
            let (px_w, px_h) = (dyn_img.width(), dyn_img.height());
            TypstImage {
                proto: picker.new_resize_protocol(dyn_img),
                px_w,
                px_h,
            }
        });
    images.insert(code.to_string(), img);
}

fn measure(app: &mut App, theme: &Theme, idx: usize, max_w: u16) -> Entry {
    if let Some(code) = typst_code_for(&app.history[idx], app.render_mode) {
        ensure_image(&mut app.images, &mut app.picker, theme, &code);
        if let Some(Some(img)) = app.images.get(&code) {
            let fs = app.picker.font_size();
            let cw = (fs.width as u32).max(1);
            let ch = (fs.height as u32).max(1);
            let max_w32 = (max_w as u32).max(1);
            let mut cols = img.px_w.div_ceil(cw).max(1);
            let mut rows = img.px_h.div_ceil(ch).max(1);
            if cols > max_w32 {
                rows = (rows * max_w32).div_ceil(cols).max(1);
                cols = max_w32;
            }
            let rows = rows.min(MAX_IMG_ROWS as u32) as u16;
            return Entry {
                idx,
                height: rows,
                typst: Some((code, cols as u16)),
            };
        }
    }
    Entry {
        idx,
        height: 1,
        typst: None,
    }
}

fn text_line(line_str: &str, highlighted: bool, brightness: f32, theme: &Theme) -> Line<'static> {
    let is_result = line_str.trim_start().starts_with('=');

    let spans: Vec<Span<'static>> = if is_result {
        vec![Span::styled(
            line_str.to_string(),
            Style::default().fg(theme.number),
        )]
    } else {
        highlight(line_str, theme)
            .into_iter()
            .map(|s| {
                let color = if matches!(s.content.as_ref(), "(" | ")" | "[" | "]" | "{" | "}") {
                    theme.bracket
                } else {
                    Color::Rgb(140, 150, 170)
                };
                Span::styled(s.content.to_string(), Style::default().fg(color))
            })
            .collect()
    };

    let faded: Vec<Span<'static>> = spans
        .into_iter()
        .map(|span| {
            if highlighted {
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
                let nr = (r as f32 * brightness) as u8;
                let ng = (g as f32 * brightness) as u8;
                let nb = (b as f32 * brightness) as u8;
                Span::styled(span.content, Style::default().fg(Color::Rgb(nr, ng, nb)))
            }
        })
        .collect();

    let mut line = Line::from(faded);
    if highlighted {
        line = line.style(Style::default().bg(Color::Rgb(50, 50, 80)));
    }
    line
}

pub fn draw(f: &mut Frame, app: &mut App) {
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

    let history_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(history_border_color))
        .title(mode_title);
    let inner = history_block.inner(left_chunk);
    f.render_widget(history_block, left_chunk);

    let total = app.history.len();
    let mut entries: Vec<Entry> = Vec::new();
    let mut used: u16 = 0;
    let mut bottom_start = total;
    while bottom_start > 0 {
        let e = measure(app, &theme, bottom_start - 1, inner.width);
        if used + e.height > inner.height {
            break;
        }
        used += e.height;
        bottom_start -= 1;
        entries.push(e);
    }
    entries.reverse();

    let mut top_anchored = false;
    if let Some(s) = app.selected
        && s < bottom_start
    {
        entries.clear();
        used = 0;
        let mut i = s;
        while i < total {
            let e = measure(app, &theme, i, inner.width);
            if used + e.height > inner.height {
                break;
            }
            used += e.height;
            entries.push(e);
            i += 1;
        }
        top_anchored = true;
    }

    let num_lines = inner.height as usize;
    let mut y = inner.y
        + if top_anchored {
            0
        } else {
            inner.height.saturating_sub(used)
        };

    for e in &entries {
        let rect = Rect {
            x: inner.x,
            y,
            width: inner.width,
            height: e.height,
        };
        y += e.height;

        let highlighted =
            app.selected == Some(e.idx) && app.active_pane == ActivePane::HistoryInput;
        if highlighted {
            f.render_widget(
                Block::default().style(Style::default().bg(Color::Rgb(50, 50, 80))),
                rect,
            );
        }

        if let Some((code, cols)) = &e.typst {
            if let Some(Some(img)) = app.images.get_mut(code) {
                let img_rect = Rect {
                    width: (*cols).min(rect.width),
                    ..rect
                };
                f.render_stateful_widget(StatefulImage::default(), img_rect, &mut img.proto);
            }
            continue;
        }

        let line_str = &app.history[e.idx];
        if line_str.is_empty() {
            continue;
        }

        let row = (rect.y - inner.y) as usize;
        let dist_from_bottom = num_lines.saturating_sub(1 + row);
        let brightness = if dist_from_bottom < num_lines / 2 {
            1.0
        } else {
            let fade_span = (num_lines / 2).max(1) as f32;
            let current_dist = (dist_from_bottom - num_lines / 2) as f32;
            (1.0 - (current_dist / fade_span)).clamp(0.1, 1.0)
        };

        f.render_widget(
            Paragraph::new(text_line(line_str, highlighted, brightness, &theme)),
            rect,
        );
    }

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

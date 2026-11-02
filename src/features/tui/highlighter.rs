use super::theme::Theme;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

pub fn highlight(input: &str, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current_token = String::new();

    let flush_token = |token: &mut String, spans: &mut Vec<Span<'static>>, color: Color| {
        if !token.is_empty() {
            spans.push(Span::styled(token.clone(), Style::default().fg(color)));
            token.clear();
        }
    };

    // Einfacher Tokenizer für Mathe-Ausdrücke
    for c in input.chars() {
        if c.is_ascii_digit() || c == '.' {
            flush_token(&mut current_token, &mut spans, theme.variable); // Falls vorher was anderes war
            current_token.push(c);
        } else if c.is_alphabetic() {
            // Buchstaben / Variablen / Funktionsnamen (z.B. sin, x)
            current_token.push(c);
        } else {
            // Erstes Token wegschreiben falls vorhanden
            if !current_token.is_empty() {
                let color = if current_token
                    .chars()
                    .all(|ch| ch.is_ascii_digit() || ch == '.')
                {
                    theme.number
                } else {
                    theme.variable
                };
                flush_token(&mut current_token, &mut spans, color);
            }

            let color = match c {
                '+' | '-' | '*' | '/' | '^' | '=' => theme.operator,
                '(' | ')' | '{' | '}' | '[' | ']' => theme.bracket,
                '>' => theme.prompt,
                _ => Color::White,
            };

            spans.push(Span::styled(c.to_string(), Style::default().fg(color)));
        }
    }

    if !current_token.is_empty() {
        let color = if current_token
            .chars()
            .all(|ch| ch.is_ascii_digit() || ch == '.')
        {
            theme.number
        } else {
            theme.variable
        };
        spans.push(Span::styled(current_token, Style::default().fg(color)));
    }

    if spans.is_empty() && !input.is_empty() {
        spans.push(Span::styled(
            input.to_string(),
            Style::default().fg(theme.input_text),
        ));
    }

    spans
}

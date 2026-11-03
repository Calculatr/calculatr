use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Number,
    Variable,
    Function,
    Constant,
    Operator,
    Bracket,
    Punct,
    Placeholder,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineRole {
    Live,
    HistoryInput,
    Result,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub border: Color,
    pub border_focus: Color,
    pub title: Color,
    pub input_text: Color,
    pub prompt: Color,
    pub number: Color,
    pub operator: Color,
    pub bracket: Color,
    pub variable: Color,
    pub function: Color,
    pub constant: Color,
    pub placeholder: Color,
    pub dim: Color,
    pub error: Color,
    pub message: Color,
    pub highlight_fg: Color,
    pub highlight_bg: Color,
}

impl Theme {
    pub fn cyberpunk() -> Self {
        Self {
            border: Color::Rgb(35, 43, 53),         // border_inactive (#232b35)
            border_focus: Color::Rgb(245, 197, 24), // border_active / yellow (#f5c518)
            title: Color::Rgb(245, 197, 24),        // accent (#f5c518)
            input_text: Color::Rgb(234, 238, 242),  // text (#eaeef2)
            prompt: Color::Rgb(78, 170, 123),       // green (#4eaa7b)
            number: Color::Rgb(61, 139, 136),       // accent_alt (#3d8b88)
            operator: Color::Rgb(245, 197, 24),     // accent / yellow (#f5c518)
            bracket: Color::Rgb(140, 155, 168),     // subtext (#8c9ba8)
            variable: Color::Rgb(234, 238, 242),    // text (#eaeef2)
            function: Color::Rgb(61, 139, 136),     // accent_alt (#3d8b88)
            constant: Color::Rgb(78, 170, 123),     // green (#4eaa7b)
            placeholder: Color::Rgb(140, 155, 168), // subtext (#8c9ba8)
            dim: Color::Rgb(140, 155, 168),         // subtext (#8c9ba8)
            error: Color::Rgb(224, 85, 85),         // red (#e05555)
            message: Color::Rgb(78, 170, 123),      // green (#4eaa7b)
            highlight_fg: Color::Rgb(245, 197, 24), // accent (#f5c518)
            highlight_bg: Color::Rgb(29, 35, 42),   // surface1 (#1d232a)
        }
    }

    pub fn token_color(&self, role: LineRole, kind: TokenKind) -> Color {
        match role {
            LineRole::Live => match kind {
                TokenKind::Number => self.number,
                TokenKind::Variable => self.variable,
                TokenKind::Function => self.function,
                TokenKind::Constant => self.constant,
                TokenKind::Operator => self.operator,
                TokenKind::Bracket => self.bracket,
                TokenKind::Placeholder => self.placeholder,
                TokenKind::Punct | TokenKind::Text => self.input_text,
            },
            LineRole::HistoryInput => match kind {
                TokenKind::Bracket => self.bracket,
                TokenKind::Placeholder => self.placeholder,
                _ => self.dim,
            },
            LineRole::Result => self.number,
        }
    }
}

pub fn rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Gray => (192, 192, 192),
        Color::DarkGray => (128, 128, 128),
        _ => (180, 190, 210),
    }
}

pub fn fade(color: Color, brightness: f32) -> Color {
    let (r, g, b) = rgb(color);
    let f = |v: u8| (v as f32 * brightness).round().clamp(0.0, 255.0) as u8;
    Color::Rgb(f(r), f(g), f(b))
}

pub fn hex(color: Color) -> String {
    let (r, g, b) = rgb(color);
    format!("{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_scales_channels() {
        assert_eq!(fade(Color::Rgb(200, 100, 50), 0.5), Color::Rgb(100, 50, 25));
        assert_eq!(fade(Color::Cyan, 1.0), Color::Rgb(0, 255, 255));
    }

    #[test]
    fn hex_is_lowercase_six_digits() {
        assert_eq!(hex(Color::Rgb(255, 0, 10)), "ff000a");
    }

    #[test]
    fn history_inputs_are_dim_except_brackets() {
        let t = Theme::cyberpunk();
        assert_eq!(
            t.token_color(LineRole::HistoryInput, TokenKind::Number),
            t.dim
        );
        assert_eq!(
            t.token_color(LineRole::HistoryInput, TokenKind::Bracket),
            t.bracket
        );
        assert_eq!(
            t.token_color(LineRole::Result, TokenKind::Bracket),
            t.number
        );
    }
}

use ratatui::style::Color;

pub struct Theme {
    pub border: Color,
    pub title: Color,
    pub input_text: Color,
    pub prompt: Color,
    pub number: Color,
    pub operator: Color,
    pub bracket: Color,
    pub variable: Color,
    pub fade_min: u8,
    pub fade_max: u8,
}

impl Theme {
    pub fn cyberpunk() -> Self {
        Self {
            border: Color::Cyan,
            title: Color::Magenta,
            input_text: Color::White,
            prompt: Color::Green,
            number: Color::Cyan,
            operator: Color::Magenta,
            bracket: Color::Yellow,
            variable: Color::Green,
            fade_min: 40,
            fade_max: 230,
        }
    }
}

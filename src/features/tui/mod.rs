pub mod completion;
pub mod highlighter;
pub mod renderer;
pub mod theme;
pub mod ui;

use crate::app::App;
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &mut App) {
    ui::draw(f, app);
}

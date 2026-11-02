pub mod ui;

use crate::app::App;
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &App) {
    ui::draw(f, app);
}

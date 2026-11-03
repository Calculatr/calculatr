mod calculator;
pub mod highlighter;
pub mod images;
mod settings_view;
mod status;
pub mod theme;
pub mod typst;

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    app.images
        .set_scale(images::pixel_per_pt(app.picker.font_size().height as u32));

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
    ])
    .split(f.area());

    status::draw_tab_bar(f, app, rows[0]);
    match app.tab {
        crate::app::Tab::Calculator => calculator::draw(f, app, rows[1]),
        crate::app::Tab::Settings => settings_view::draw(f, app, rows[1]),
    }
    status::draw_status_bar(f, app, rows[2]);
}

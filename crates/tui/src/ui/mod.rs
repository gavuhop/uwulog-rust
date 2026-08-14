pub mod input;
pub mod status;
pub mod table;

use crate::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

pub fn render(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.area());

    input::render_input(f, app, chunks[0]);
    table::render_table(f, app, chunks[1]);
    status::render_status(f, app, chunks[2]);
}

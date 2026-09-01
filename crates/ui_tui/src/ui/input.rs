use crate::app::{App, InputMode};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let input_style = match app.input_mode {
        InputMode::Normal => Style::default().fg(Color::Gray),
        InputMode::Editing => Style::default().fg(Color::Yellow),
    };

    let input_title = match app.input_mode {
        InputMode::Normal => " Filter Query (Nhấn '/' để lọc, Esc để thoát) ",
        InputMode::Editing => " Editing Filter Query (Nhấn Enter/Esc để hoàn tất) ",
    };

    let input_widget = Paragraph::new(app.query.as_str())
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title(input_title));

    f.render_widget(input_widget, area);
}

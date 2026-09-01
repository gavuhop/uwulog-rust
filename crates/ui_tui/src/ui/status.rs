use crate::app::{App, InputMode};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

pub fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let status_text = match app.input_mode {
        InputMode::Normal => {
            if app.is_auto_scroll {
                " [LIVE TAIL] Tự động cuộn theo log mới ở dưới cùng | [Up/Dn]: Cuộn quan sát log cũ | [/]: Tìm kiếm | [Q]: Thoát "
            } else {
                " [FROZEN] ĐÃ ĐÓNG BĂNG MÀN HÌNH ĐỂ QUAN SÁT | Nhấn [End/G/Space] để bật lại Live Tail | [Up/Dn]: Di chuyển "
            }
        }
        InputMode::Editing => " [Enter/Esc]: Đóng ô nhập từ khóa ",
    };

    let bg_color = if app.is_auto_scroll {
        Color::Blue
    } else {
        Color::Rgb(180, 100, 0)
    };

    let status_bar =
        Paragraph::new(status_text).style(Style::default().bg(bg_color).fg(Color::White));

    f.render_widget(status_bar, area);
}

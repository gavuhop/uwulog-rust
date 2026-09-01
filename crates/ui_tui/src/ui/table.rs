use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem},
    Frame,
};
use uwu_core_schema::LogLevel;

pub fn render_table(f: &mut Frame, app: &mut App, area: Rect) {
    let total_logs = app.engine.total_logs();
    let displayed_count = app.cached_logs.len();

    let items: Vec<ListItem> = app
        .cached_logs
        .iter()
        .map(|log| {
            let level_color = match log.level {
                LogLevel::Error | LogLevel::Fatal => Color::Red,
                LogLevel::Warn => Color::Yellow,
                LogLevel::Info => Color::Green,
                LogLevel::Debug => Color::Cyan,
                LogLevel::Trace => Color::Magenta,
                LogLevel::Unknown => Color::Gray,
            };

            let line_content = format!(
                "[{}] [{:<5}] [{}] {}",
                log.timestamp,
                log.level.as_str(),
                log.source_id,
                log.message
            );

            ListItem::new(line_content).style(Style::default().fg(level_color))
        })
        .collect();

    let mode_tag = if app.is_auto_scroll {
        " [LIVE AUTO-SCROLL (TAIL -F)] "
    } else {
        " [ĐÃ ĐÓNG BĂNG MÀN HÌNH QUAN SÁT - FROZEN] "
    };

    let title_text = if app.total_matched > displayed_count {
        format!(
            " Live Logs (Khớp: {} | Hiển thị: {} log mới nhất | RAM: {}){} ",
            app.total_matched, displayed_count, total_logs, mode_tag
        )
    } else {
        format!(
            " Live Logs (Khớp: {} | RAM: {}){} ",
            app.total_matched, total_logs, mode_tag
        )
    };

    let log_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title_text))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::Rgb(40, 40, 70)),
        );

    f.render_stateful_widget(log_list, area, &mut app.list_state);
}

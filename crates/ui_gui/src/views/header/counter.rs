use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme::{self, ActiveTheme};
use eframe::egui;

/// Hiển thị bộ đếm số lượng log nhất quán cho cả trạng thái Live và Paused:
/// - Chưa lọc / Raw: tổng số log đã ingest (`total_processed`)
/// - Đang lọc: tổng số log khớp (`total_matched`, hoặc `hiển_thị/tổng` nếu vượt giới hạn)
pub fn render_log_counter(ui: &mut egui::Ui, session: &GuiSession) {
    let theme = ui.app_theme();
    let query = session.view.search.query.trim();
    let is_filtering = session.view.active_tab == ActiveTab::Filtered && !query.is_empty();
    let status = if session.view.stream(session.view.active_tab).is_auto_scroll {
        "Live"
    } else {
        "Paused"
    };

    let (count_text, count_color, count_tooltip) = if is_filtering {
        let main = session.view.main_tab();
        let total_matched = main.total_matched();
        let cached_len = main.stream.cached_logs.len();
        let is_exact = main.is_count_exact();
        let total_label = if is_exact {
            theme::format_number(total_matched)
        } else {
            format!("{}+", theme::format_number(total_matched))
        };
        let text = if total_matched > cached_len || !is_exact {
            format!("{}/{}", theme::format_number(cached_len), total_label)
        } else {
            total_label.clone()
        };
        let tooltip = format!(
            "Filter Query: \"{query}\" ({status})\n• Total Matched: {total_label}\n• Displayed: {} (Limit: {})",
            theme::format_number(cached_len),
            theme::format_number(session.session.display_limit),
        );
        (text, theme.text.accent, tooltip)
    } else {
        let total_now = session.session.engine.total_processed() as usize;
        let spilled = session.session.engine.total_spilled() as usize;
        let spill_line = if spilled > 0 {
            format!("\n• Disk Spill: {} logs", theme::format_number(spilled))
        } else {
            String::new()
        };
        let stream_name = match session.view.active_tab {
            ActiveTab::Filtered => "Main Stream",
            ActiveTab::Unfiltered => "Raw Stream",
        };
        let displayed = session
            .view
            .stream(session.view.active_tab)
            .cached_logs
            .len();
        let tooltip = format!(
            "{stream_name} ({status})\n• Total Ingested: {}\n• In-Memory Buffer: {} / {}{spill_line}\n• Displayed: {}",
            theme::format_number(total_now),
            theme::format_number(session.session.engine.total_logs()),
            theme::format_number(session.session.engine.max_capacity()),
            theme::format_number(displayed),
        );
        (theme::format_number(total_now), theme.text.muted, tooltip)
    };

    crate::components::ui::CountBadge::new(&count_text)
        .flat()
        .text_color(count_color)
        .tooltip(&count_tooltip)
        .show(ui);
}

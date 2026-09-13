use crate::session::GuiSession;
use crate::state::ActiveTab;
use crate::theme;
use eframe::egui;

/// Hiển thị bộ đếm số lượng log theo từng trạng thái chuẩn hóa:
/// - main (chưa lọc): Live -> số log hiện tại     | Pause -> số log mới đến / số log tại pause
/// - main (đã lọc):   Live -> số log khớp         | Pause -> số log khớp mới đến / số log khớp tại pause
/// - Raw:             Live -> số log hiện tại     | Pause -> số log mới đến / số log tại pause
pub fn render_log_counter(ui: &mut egui::Ui, session: &GuiSession) {
    let is_filtering = !session.view.search.query.trim().is_empty();

    let (count_text, count_color, count_tooltip) = match session.view.active_tab {
        ActiveTab::Unfiltered => {
            let total_now = session.session.engine.total_processed() as usize;

            if session.view.unfiltered.is_live {
                let text = theme::format_number(total_now);
                let tooltip = format!(
                    "Raw Stream (Live)\n• Total Ingested / Seen: {}\n• In-Memory Buffer: {} / {}\n• Buffer Limit: 500",
                    theme::format_number(total_now),
                    theme::format_number(session.session.engine.total_logs()),
                    theme::format_number(session.session.engine.max_capacity()),
                );
                (text, theme::TEXT_MUTED, tooltip)
            } else {
                format_paused_stream_counter(
                    "Raw Stream",
                    session.view.unfiltered.snapshot_processed_count as usize,
                    total_now,
                )
            }
        }
        ActiveTab::Filtered => {
            if is_filtering {
                if session.view.viewport.is_auto_scroll {
                    let total_matched = session.view.viewport.total_matched;
                    let cached_len = session.view.viewport.cached_logs.len();
                    let text = if total_matched > cached_len {
                        format!(
                            "{}/{}",
                            theme::format_number(cached_len),
                            theme::format_number(total_matched)
                        )
                    } else {
                        theme::format_number(total_matched)
                    };
                    let tooltip = format!(
                        "Filter Query: \"{}\" (Live)\n• Total Matched: {}\n• Displayed: {} (Limit: {})",
                        session.view.search.query.trim(),
                        theme::format_number(total_matched),
                        theme::format_number(cached_len),
                        theme::format_number(session.session.display_limit),
                    );
                    (text, theme::TEXT_KEY, tooltip)
                } else {
                    let seen_matched_at_pause = session.view.viewport.pause_snapshot.filtered_seen;
                    let new_matched = session
                        .view
                        .viewport
                        .pause_snapshot
                        .paused_new_matched_count;
                    let text = format_fraction(new_matched, seen_matched_at_pause);
                    let tooltip = format!(
                        "Filter Query: \"{}\" (Paused)\n• New Matched Logs Since Pause: {}\n• Matched at Pause: {}\n• Displayed: {}",
                        session.view.search.query.trim(),
                        theme::format_number(new_matched),
                        theme::format_number(seen_matched_at_pause),
                        theme::format_number(session.view.viewport.cached_logs.len()),
                    );
                    (text, theme::TEXT_KEY, tooltip)
                }
            } else {
                let total_now = session.session.engine.total_processed() as usize;

                if session.view.viewport.is_auto_scroll {
                    let text = theme::format_number(total_now);
                    let tooltip = format!(
                        "Main Stream (Live)\n• Total Ingested: {}\n• In-Memory Buffer: {} / {}\n• Displayed: {}",
                        theme::format_number(total_now),
                        theme::format_number(session.session.engine.total_logs()),
                        theme::format_number(session.session.engine.max_capacity()),
                        theme::format_number(session.view.viewport.cached_logs.len()),
                    );
                    (text, theme::TEXT_MUTED, tooltip)
                } else {
                    format_paused_stream_counter(
                        "Main Stream",
                        session.view.viewport.pause_snapshot.global_seen as usize,
                        total_now,
                    )
                }
            }
        }
    };

    crate::components::ui::CountBadge::new(&count_text)
        .text_color(count_color)
        .tooltip(&count_tooltip)
        .show(ui);
}

#[inline]
fn format_fraction(numerator: usize, denominator: usize) -> String {
    format!(
        "{}/{}",
        theme::format_number(numerator),
        theme::format_number(denominator)
    )
}

fn format_paused_stream_counter(
    stream_name: &str,
    seen_at_pause: usize,
    total_now: usize,
) -> (String, egui::Color32, String) {
    let new_incoming = total_now.saturating_sub(seen_at_pause);
    let text = format_fraction(new_incoming, seen_at_pause);
    let tooltip = format!(
        "{stream_name} (Paused)\n• New Logs Since Pause: {}\n• Total Logs at Pause: {}\n• Total Ingested: {}",
        theme::format_number(new_incoming),
        theme::format_number(seen_at_pause),
        theme::format_number(total_now),
    );
    (text, theme::TEXT_MUTED, tooltip)
}

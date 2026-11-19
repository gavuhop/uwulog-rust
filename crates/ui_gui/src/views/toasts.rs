use crate::actions::AppAction;
use crate::components::ui::{AppButton, ButtonVariant, IconButton, IconName};
use crate::session::GuiSession;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Align2, CornerRadius, Order, Stroke};

/// Mức độ hiển thị màu icon & viền của toast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ToastTone {
    Neutral,
    Success,
    Warning,
    Error,
}

/// Nút hành động phụ (tuỳ chọn) gắn kèm một toast.
#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    pub variant: ButtonVariant,
    pub action: AppAction,
}

/// Cách xử lý khi người dùng nhấn nút đóng (`×`) trên toast.
#[derive(Debug, Clone)]
pub enum ToastDismiss {
    Action(Box<AppAction>),
    BufferOverflow,
    DevContainerSuggest(String),
}

/// Dữ liệu mô tả một thông báo toast trong ngăn xếp góc dưới-phải.
#[derive(Debug, Clone)]
pub struct ToastItem {
    pub icon: IconName,
    pub tone: ToastTone,
    pub title: String,
    pub subtitle: String,
    pub action: Option<ToastAction>,
    pub dismiss: ToastDismiss,
}

/// Định dạng nội dung tóm tắt cho cảnh báo va chạm key JSON (chống tràn và chống spam).
pub(crate) fn format_collision_summary(visible: &[(&str, u64)]) -> String {
    let total_logs: u64 = visible.iter().map(|(_, c)| *c).sum();
    match visible {
        [] => String::new(),
        [(key, count)] => format!(
            "Key '{key}' conflicts with nested path ({count} log{}) — showing latest value",
            if *count == 1 { "" } else { "s" }
        ),
        [(first_key, _), rest @ ..] => format!(
            "Key '{first_key}' (+{} more) conflicts with nested path ({total_logs} logs) — showing latest value",
            rest.len()
        ),
    }
}

/// Tạo toast cảnh báo va chạm key JSON nếu session hiện tại có cảnh báo chưa bị ẩn.
pub fn build_field_collision_toast(session: &GuiSession) -> Option<ToastItem> {
    let visible = session.view.warnings.visible_collisions();
    if visible.is_empty() {
        return None;
    }

    Some(ToastItem {
        icon: IconName::AlertTriangle,
        tone: ToastTone::Warning,
        title: "JSON key path collision".to_string(),
        subtitle: format_collision_summary(&visible),
        action: None,
        dismiss: ToastDismiss::Action(Box::new(AppAction::DismissFieldWarnings)),
    })
}

/// Tạo toast thông báo khi bộ đệm log trong RAM vượt quá `max_capacity` và bắt đầu spill ra đĩa.
pub fn build_buffer_overflow_toast(session: &GuiSession) -> Option<ToastItem> {
    let spilled = session.session.engine.total_spilled();
    if spilled == 0 || session.view.buffer_overflow_dismissed {
        return None;
    }

    let max_cap = session.session.engine.max_capacity();
    Some(ToastItem {
        icon: IconName::AlertTriangle,
        tone: ToastTone::Warning,
        title: "Buffer limit exceeded".to_string(),
        subtitle: format!(
            "Buffer exceeded {} logs — spilling older logs to disk ({} spilled)",
            crate::theme::format_number(max_cap),
            crate::theme::format_number(spilled as usize),
        ),
        action: None,
        dismiss: ToastDismiss::BufferOverflow,
    })
}

/// Render toàn bộ ngăn xếp toast ở góc dưới-phải (`Align2::RIGHT_BOTTOM`, offset `(-16, -16)`).
///
/// Phần tử đầu tiên trong `items` nằm trên cùng (cũ hơn), phần tử cuối cùng nằm dưới cùng
/// (mới nhất / ưu tiên cao nhất), tự động xếp chồng cách nhau `8px` không bao giờ chồng đè.
pub fn render_toast_stack(
    ctx: &egui::Context,
    items: &[ToastItem],
    dispatch: &mut impl FnMut(AppAction),
) -> Vec<ToastDismiss> {
    render_toast_stack_with_rects(ctx, items, dispatch).0
}

fn render_toast_stack_with_rects(
    ctx: &egui::Context,
    items: &[ToastItem],
    dispatch: &mut impl FnMut(AppAction),
) -> (Vec<ToastDismiss>, Vec<egui::Rect>) {
    if items.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let theme = ctx.app_theme();
    let mut dismissed = Vec::new();
    let mut rects = Vec::with_capacity(items.len());
    let mut bottom_offset = -16.0_f32;

    for (idx, item) in items.iter().enumerate().rev() {
        let area_resp = egui::Area::new(egui::Id::new(("app_toast_item", idx, &item.title)))
            .order(Order::Foreground)
            .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, bottom_offset))
            .show(ctx, |ui| {
                let (icon_color, border_color) = match item.tone {
                    ToastTone::Neutral => (theme.text.muted, theme.borders.border),
                    ToastTone::Success => (theme.status.success, theme.borders.border),
                    ToastTone::Warning => (theme.status.warning, theme.status.warning),
                    ToastTone::Error => (theme.status.error, theme.status.error),
                };

                let frame = egui::Frame::default()
                    .fill(theme.surfaces.mantle)
                    .stroke(Stroke::new(1.0, border_color))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 6],
                        blur: 20,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(55),
                    });

                frame
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon_r, _) = ui
                                .allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
                            item.icon.paint(ui.painter(), icon_r, icon_color);
                            ui.add_space(6.0);

                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(&item.title)
                                        .size(12.0)
                                        .strong()
                                        .color(theme.text.primary),
                                );
                                ui.label(
                                    egui::RichText::new(&item.subtitle)
                                        .size(11.0)
                                        .color(theme.text.muted),
                                );
                            });

                            ui.add_space(10.0);

                            if let Some(ref action) = item.action {
                                if AppButton::new()
                                    .label(&action.label)
                                    .small()
                                    .variant(action.variant)
                                    .show(ui)
                                    .clicked()
                                {
                                    dispatch(action.action.clone());
                                }
                            }

                            if IconButton::new(IconName::Close)
                                .size(22.0)
                                .tooltip("Dismiss")
                                .show(ui)
                                .clicked()
                            {
                                dismissed.push(item.dismiss.clone());
                            }
                        });
                    })
                    .response
                    .rect
            });

        rects.push(area_resp.inner);
        bottom_offset -= area_resp.response.rect.height() + 8.0;
    }

    (dismissed, rects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uwu_core_schema::{RawLogEntry, RawPayload};
    use uwu_core_workspace::{SourceConfig, SourceType, WorkspaceLocation, WorkspaceSession};

    #[test]
    fn test_format_collision_summary() {
        assert_eq!(format_collision_summary(&[]), "");
        assert_eq!(
            format_collision_summary(&[("a.b", 1)]),
            "Key 'a.b' conflicts with nested path (1 log) — showing latest value"
        );
        assert_eq!(
            format_collision_summary(&[("a.b", 5)]),
            "Key 'a.b' conflicts with nested path (5 logs) — showing latest value"
        );
        assert_eq!(
            format_collision_summary(&[("a.b", 3), ("x.y", 2)]),
            "Key 'a.b' (+1 more) conflicts with nested path (5 logs) — showing latest value"
        );
    }

    #[test]
    fn test_toast_stack_renders_multiple_toasts_without_overlap() {
        let ctx = egui::Context::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };

        let items = vec![
            ToastItem {
                icon: IconName::AlertTriangle,
                tone: ToastTone::Warning,
                title: "JSON key path collision".to_string(),
                subtitle: "Key 'a.b' conflicts with nested path (2 logs)".to_string(),
                action: None,
                dismiss: ToastDismiss::Action(Box::new(AppAction::DismissFieldWarnings)),
            },
            ToastItem {
                icon: IconName::AlertTriangle,
                tone: ToastTone::Warning,
                title: "Buffer limit exceeded".to_string(),
                subtitle: "Buffer exceeded 5 logs — spilling older logs to disk (3 spilled)"
                    .to_string(),
                action: None,
                dismiss: ToastDismiss::BufferOverflow,
            },
            ToastItem {
                icon: IconName::Box,
                tone: ToastTone::Success,
                title: "Container running".to_string(),
                subtitle: "dev-container".to_string(),
                action: Some(ToastAction {
                    label: "Open in Dev Container".to_string(),
                    variant: ButtonVariant::Success,
                    action: AppAction::DismissFieldWarnings,
                }),
                dismiss: ToastDismiss::DevContainerSuggest("/tmp/app".to_string()),
            },
        ];

        let mut actions = Vec::new();
        for frame_idx in 0..2 {
            let mut card_rects = Vec::new();
            let mut out = ctx.run_ui(raw_input.clone(), |ui| {
                let (dismissed, rects) =
                    render_toast_stack_with_rects(ui.ctx(), &items, &mut |a| actions.push(a));
                assert!(dismissed.is_empty());
                card_rects = rects;
            });
            out.textures_delta.clear();
            assert!(!out.shapes.is_empty());
            assert_eq!(card_rects.len(), 3);
            card_rects.sort_by(|a, b| a.min.y.total_cmp(&b.min.y));

            for rect in &card_rects {
                assert!(
                    rect.width() > 150.0 && rect.width() < 550.0,
                    "Toast card width out of compact bounds on frame {frame_idx}: {:?}",
                    rect
                );
            }

            // Frame đầu tiên egui đo kích thước Area; từ frame thứ 2 phải neo chính xác góc dưới-phải
            if frame_idx >= 1 {
                for rect in &card_rects {
                    assert!(
                        rect.min.x > 700.0 && (rect.max.x - 1264.0).abs() <= 1.5,
                        "Toast card not anchored at bottom-right corner: {:?}",
                        rect
                    );
                }
                for pair in card_rects.windows(2) {
                    assert!(
                        pair[0].max.y < pair[1].min.y,
                        "Toast cards overlapped vertically: {:?} vs {:?}",
                        pair[0],
                        pair[1]
                    );
                }
                assert!(
                    (card_rects[2].max.y - 784.0).abs() <= 1.5,
                    "Newest toast should sit at bottom=-16px: {:?}",
                    card_rects[2]
                );
            }
        }
        assert!(actions.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_overflow_toast_visibility_and_reset() {
        let ws = WorkspaceSession::new(
            "OverflowTest".to_string(),
            WorkspaceLocation::local("/tmp"),
            SourceConfig {
                source_type: SourceType::Process,
                command_str: String::new(),
                file_path: String::new(),
                working_dir: String::new(),
                capacity: 5,
                display_limit: 5,
            },
        );
        let mut session = GuiSession::new(ws);
        let ctx = egui::Context::default();
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, 800.0),
            )),
            ..Default::default()
        };

        // 1. Chưa tràn buffer -> không có toast
        assert!(build_buffer_overflow_toast(&session).is_none());

        // 2. Nạp vượt capacity (5) -> tổng spilled > 0 -> tạo & hiển thị toast
        let tx = session.session.engine.get_channel();
        for i in 0..8 {
            tx.send(RawLogEntry {
                payload: RawPayload::Text(format!("msg {i}")),
            })
            .await
            .unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        session.session.engine.flush_spill();
        assert_eq!(session.session.engine.total_spilled(), 3);

        let toast = build_buffer_overflow_toast(&session).expect("expected buffer overflow toast");
        assert!(matches!(toast.dismiss, ToastDismiss::BufferOverflow));
        let mut out = ctx.run_ui(raw_input.clone(), |ui| {
            let _ = render_toast_stack(ui.ctx(), std::slice::from_ref(&toast), &mut |_| {});
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty());

        // 3. Đã dismiss -> không hiển thị lại nữa; reset_stream_data -> cho phép thông báo lại
        session.view.buffer_overflow_dismissed = true;
        assert!(build_buffer_overflow_toast(&session).is_none());

        session.view.reset_stream_data();
        assert!(!session.view.buffer_overflow_dismissed);
        assert!(build_buffer_overflow_toast(&session).is_some());
    }
}

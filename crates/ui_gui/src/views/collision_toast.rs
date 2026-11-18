use crate::actions::AppAction;
use crate::components::ui::{IconButton, IconName};
use crate::session::GuiSession;
use crate::theme::ActiveTheme;
use eframe::egui::{self, Align2, CornerRadius, Order, Stroke};

/// Định dạng nội dung tóm tắt cho popup cảnh báo va chạm key JSON (chống tràn và chống spam).
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

/// Hiển thị toast cảnh báo va chạm key JSON ở góc phải dưới màn hình (cùng phong cách Dev Container toast).
/// Khi Dev Container toast đang hiển thị, tự đẩy lên trên để không chồng đè lên nhau.
pub fn render_field_collision_toast(
    ctx: &egui::Context,
    session: &GuiSession,
    devcontainer_toast_visible: bool,
    dispatch: &mut impl FnMut(AppAction),
) {
    let visible = session.view.warnings.visible_collisions();
    if visible.is_empty() {
        return;
    }

    let theme = ctx.app_theme();
    let mut dismiss_clicked = false;
    let bottom_offset = if devcontainer_toast_visible {
        -84.0
    } else {
        -16.0
    };
    let subtitle = format_collision_summary(&visible);

    egui::Area::new(egui::Id::new("field_collision_warning_toast"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-16.0, bottom_offset))
        .show(ctx, |ui| {
            let frame = egui::Frame::default()
                .fill(theme.surfaces.mantle)
                .stroke(Stroke::new(1.0, theme.status.warning))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(egui::Margin::symmetric(12, 10))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 6],
                    blur: 20,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(55),
                });

            frame.show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (icon_r, _) =
                        ui.allocate_exact_size(egui::vec2(15.0, 15.0), egui::Sense::hover());
                    IconName::AlertTriangle.paint(ui.painter(), icon_r, theme.status.warning);
                    ui.add_space(6.0);

                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("JSON key path collision")
                                .size(12.0)
                                .strong()
                                .color(theme.text.primary),
                        );
                        ui.label(
                            egui::RichText::new(&subtitle)
                                .size(11.0)
                                .color(theme.text.muted),
                        );
                    });

                    ui.add_space(10.0);

                    if IconButton::new(IconName::Close)
                        .size(22.0)
                        .tooltip("Dismiss")
                        .show(ui)
                        .clicked()
                    {
                        dismiss_clicked = true;
                    }
                });
            });
        });

    if dismiss_clicked {
        dispatch(AppAction::DismissFieldWarnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

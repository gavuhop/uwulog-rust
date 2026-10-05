use eframe::egui::{self, Color32, CursorIcon, Id, PointerButton, Pos2, Rect, Stroke, Vec2};

/// Trạng thái lưu trữ của chế độ cuộn tự động chuột giữa phụ thuộc tâm (Web-style autoscroll)
#[derive(Clone, Copy, Debug, Default)]
pub struct AutoScrollState {
    pub origin: Pos2,
    pub is_holding: bool,
    pub max_displacement: f32,
}

pub struct AutoScrollOutput {
    pub scroll_delta: Vec2,
    pub is_active: bool,
}

const DEADZONE: f32 = 10.0;

/// Xử lý cơ chế cuộn chuột giữa phụ thuộc tâm giống lướt web (Chrome / Firefox / Edge):
/// - Khi bấm giữ nút con lăn: định vị tâm tại con trỏ, kéo chuột ra xa tâm để di chuyển cả chiều dọc và ngang.
/// - Vận tốc tăng dần theo khoảng cách tới tâm.
/// - Thả nút khi đã kéo -> dừng cuộn (Press & Drag).
/// - Click nhấp thả tại chỗ -> duy trì chế độ cuộn theo tâm cho tới khi click chuột hoặc bấm Esc (Click & Release).
pub fn handle_middle_autoscroll(ui: &egui::Ui, table_rect: Rect) -> AutoScrollOutput {
    let state_id = Id::new("table_middle_autoscroll_state");
    let mut state: Option<AutoScrollState> = ui.data(|d| d.get_temp(state_id));

    let middle_pressed = ui.input(|i| i.pointer.button_pressed(PointerButton::Middle));
    let middle_released = ui.input(|i| i.pointer.button_released(PointerButton::Middle));
    let other_cancelled = ui.input(|i| {
        i.pointer.button_pressed(PointerButton::Primary)
            || i.pointer.button_pressed(PointerButton::Secondary)
            || i.key_pressed(egui::Key::Escape)
            || i.smooth_scroll_delta.y.abs() > 0.1
            || i.smooth_scroll_delta.x.abs() > 0.1
    });
    let hover_pos = ui.input(|i| i.pointer.hover_pos().or_else(|| i.pointer.interact_pos()));

    // 1. Kiểm tra kích hoạt mới hoặc tắt toggle khi bấm chuột giữa
    if middle_pressed {
        if state.is_some() {
            state = None;
        } else if let Some(pos) = hover_pos {
            if table_rect.contains(pos) {
                state = Some(AutoScrollState {
                    origin: pos,
                    is_holding: true,
                    max_displacement: 0.0,
                });
            }
        }
    }

    // 2. Tắt chế độ nếu người dùng click chuột trái/phải, lăn bánh xe, hoặc bấm Escape
    if other_cancelled {
        state = None;
    }

    // 3. Xử lý sự kiện thả nút giữa (Release)
    if let Some(ref mut st) = state {
        let curr_pos = hover_pos.unwrap_or(st.origin);
        let dist = (curr_pos - st.origin).length();
        if dist > st.max_displacement {
            st.max_displacement = dist;
        }

        if middle_released && st.is_holding {
            if st.max_displacement > 6.0 {
                // Đã kéo chuột xa hơn 6px -> Kết thúc ngay khi thả nút giữa (Press & Drag)
                state = None;
            } else {
                // Nhấp chuột tại chỗ -> Chuyển sang chế độ duy trì (Click to toggle)
                st.is_holding = false;
            }
        }
    }

    // 4. Tính toán vận tốc cuộn và vẽ hiển thị trực quan
    let mut scroll_delta = Vec2::ZERO;
    let is_active = state.is_some();

    if let Some(st) = state {
        let curr_pos = hover_pos.unwrap_or(st.origin);
        let delta = curr_pos - st.origin;

        let speed_x = calc_autoscroll_speed(delta.x);
        let speed_y = calc_autoscroll_speed(delta.y);
        scroll_delta = Vec2::new(speed_x, speed_y);

        // Đổi con trỏ chuột theo hướng dịch chuyển
        let cursor = pick_autoscroll_cursor(speed_x, speed_y);
        ui.ctx().set_cursor_icon(cursor);

        // Yêu cầu vẽ lại frame liên tục nếu đang di chuyển ra khỏi vùng chết
        if speed_x != 0.0 || speed_y != 0.0 {
            ui.ctx().request_repaint();
        }

        // Vẽ biểu tượng tâm trực quan (Center Anchor Indicator)
        render_autoscroll_anchor(ui, st.origin, curr_pos, DEADZONE);
    }

    // Lưu lại trạng thái vào bộ nhớ tạm
    if let Some(st) = state {
        ui.data_mut(|d| d.insert_temp(state_id, st));
    } else {
        ui.data_mut(|d| d.remove_temp::<AutoScrollState>(state_id));
    }

    AutoScrollOutput {
        scroll_delta,
        is_active,
    }
}

/// Chọn icon con trỏ chuột tương ứng theo hướng cuộn
fn pick_autoscroll_cursor(speed_x: f32, speed_y: f32) -> CursorIcon {
    if speed_x == 0.0 && speed_y == 0.0 {
        CursorIcon::AllScroll
    } else if speed_x.abs() > speed_y.abs() * 2.4 {
        if speed_x > 0.0 {
            CursorIcon::ResizeEast
        } else {
            CursorIcon::ResizeWest
        }
    } else if speed_y.abs() > speed_x.abs() * 2.4 {
        if speed_y > 0.0 {
            CursorIcon::ResizeSouth
        } else {
            CursorIcon::ResizeNorth
        }
    } else if speed_x > 0.0 && speed_y > 0.0 {
        CursorIcon::ResizeSouthEast
    } else if speed_x > 0.0 && speed_y < 0.0 {
        CursorIcon::ResizeNorthEast
    } else if speed_x < 0.0 && speed_y > 0.0 {
        CursorIcon::ResizeSouthWest
    } else if speed_x < 0.0 && speed_y < 0.0 {
        CursorIcon::ResizeNorthWest
    } else {
        CursorIcon::AllScroll
    }
}

/// Vẽ tâm trực quan (Visual Center Anchor Indicator giống lướt web cao cấp)
fn render_autoscroll_anchor(ui: &egui::Ui, origin: Pos2, curr_pos: Pos2, deadzone: f32) {
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        ui.make_persistent_id("autoscroll_anchor_layer"),
    ));

    let delta = curr_pos - origin;

    // 4 trạng thái hướng hoạt động
    let up_active = delta.y < -deadzone;
    let down_active = delta.y > deadzone;
    let left_active = delta.x < -deadzone;
    let right_active = delta.x > deadzone;
    let any_active = up_active || down_active || left_active || right_active;

    // Hiệu ứng bóng mờ nhẹ bên ngoài (Drop Shadow)
    painter.circle_filled(origin, 17.5, Color32::from_black_alpha(35));
    painter.circle_filled(origin, 16.0, Color32::from_black_alpha(70));

    // Vòng tròn nền kính tối mờ (Dark glass disc) với viền thanh mảnh
    let border_color = if any_active {
        Color32::from_rgba_unmultiplied(56, 189, 248, 90)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 60)
    };
    painter.circle(
        origin,
        14.5,
        Color32::from_rgba_unmultiplied(20, 24, 33, 230),
        Stroke::new(1.2, border_color),
    );

    // Chấm tròn tâm
    let center_color = if any_active {
        Color32::from_rgb(56, 189, 248)
    } else {
        Color32::from_rgba_unmultiplied(225, 230, 240, 190)
    };
    painter.circle_filled(origin, 2.0, center_color);

    let active_color = Color32::from_rgb(56, 189, 248);
    let normal_color = Color32::from_rgba_unmultiplied(200, 210, 225, 140);
    let glow_color = Color32::from_rgba_unmultiplied(56, 189, 248, 45);

    // Mũi tên Lên (Up)
    if up_active {
        painter.circle_filled(origin + Vec2::new(0.0, -8.0), 4.5, glow_color);
    }
    painter.add(egui::Shape::convex_polygon(
        vec![
            origin + Vec2::new(0.0, -10.5),
            origin + Vec2::new(-3.2, -5.8),
            origin + Vec2::new(3.2, -5.8),
        ],
        if up_active {
            active_color
        } else {
            normal_color
        },
        Stroke::NONE,
    ));

    // Mũi tên Xuống (Down)
    if down_active {
        painter.circle_filled(origin + Vec2::new(0.0, 8.0), 4.5, glow_color);
    }
    painter.add(egui::Shape::convex_polygon(
        vec![
            origin + Vec2::new(0.0, 10.5),
            origin + Vec2::new(-3.2, 5.8),
            origin + Vec2::new(3.2, 5.8),
        ],
        if down_active {
            active_color
        } else {
            normal_color
        },
        Stroke::NONE,
    ));

    // Mũi tên Trái (Left)
    if left_active {
        painter.circle_filled(origin + Vec2::new(-8.0, 0.0), 4.5, glow_color);
    }
    painter.add(egui::Shape::convex_polygon(
        vec![
            origin + Vec2::new(-10.5, 0.0),
            origin + Vec2::new(-5.8, -3.2),
            origin + Vec2::new(-5.8, 3.2),
        ],
        if left_active {
            active_color
        } else {
            normal_color
        },
        Stroke::NONE,
    ));

    // Mũi tên Phải (Right)
    if right_active {
        painter.circle_filled(origin + Vec2::new(8.0, 0.0), 4.5, glow_color);
    }
    painter.add(egui::Shape::convex_polygon(
        vec![
            origin + Vec2::new(10.5, 0.0),
            origin + Vec2::new(5.8, -3.2),
            origin + Vec2::new(5.8, 3.2),
        ],
        if right_active {
            active_color
        } else {
            normal_color
        },
        Stroke::NONE,
    ));
}

/// Tính vận tốc cuộn mượt mà theo khoảng cách từ tâm, có vùng chết (deadzone) và gia tốc
pub(crate) fn calc_autoscroll_speed(diff: f32) -> f32 {
    if diff.abs() <= DEADZONE {
        0.0
    } else {
        let d = diff - diff.signum() * DEADZONE;
        let norm = (d / 25.0).abs();
        let speed = d.signum() * (norm * 2.0 + norm * norm * 0.4);
        speed.clamp(-120.0, 120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_autoscroll_speed_deadzone() {
        assert_eq!(calc_autoscroll_speed(0.0), 0.0);
        assert_eq!(calc_autoscroll_speed(5.0), 0.0);
        assert_eq!(calc_autoscroll_speed(-5.0), 0.0);
        assert_eq!(calc_autoscroll_speed(DEADZONE), 0.0);
        assert_eq!(calc_autoscroll_speed(-DEADZONE), 0.0);
    }

    #[test]
    fn test_calc_autoscroll_speed_proportional() {
        let speed_slow = calc_autoscroll_speed(DEADZONE + 25.0);
        assert!(speed_slow > 0.0);
        let speed_fast = calc_autoscroll_speed(DEADZONE + 100.0);
        assert!(speed_fast > speed_slow);

        let speed_neg = calc_autoscroll_speed(-(DEADZONE + 25.0));
        assert_eq!(speed_neg, -speed_slow);
    }

    #[test]
    fn test_calc_autoscroll_speed_clamped() {
        let speed_extreme = calc_autoscroll_speed(10_000.0);
        assert_eq!(speed_extreme, 120.0);
        let speed_extreme_neg = calc_autoscroll_speed(-10_000.0);
        assert_eq!(speed_extreme_neg, -120.0);
    }

    #[test]
    fn test_pick_autoscroll_cursor_directions() {
        assert_eq!(pick_autoscroll_cursor(0.0, 0.0), CursorIcon::AllScroll);
        assert_eq!(pick_autoscroll_cursor(10.0, 0.0), CursorIcon::ResizeEast);
        assert_eq!(pick_autoscroll_cursor(-10.0, 0.0), CursorIcon::ResizeWest);
        assert_eq!(pick_autoscroll_cursor(0.0, 10.0), CursorIcon::ResizeSouth);
        assert_eq!(pick_autoscroll_cursor(0.0, -10.0), CursorIcon::ResizeNorth);
        assert_eq!(
            pick_autoscroll_cursor(10.0, 10.0),
            CursorIcon::ResizeSouthEast
        );
        assert_eq!(
            pick_autoscroll_cursor(10.0, -10.0),
            CursorIcon::ResizeNorthEast
        );
        assert_eq!(
            pick_autoscroll_cursor(-10.0, 10.0),
            CursorIcon::ResizeSouthWest
        );
        assert_eq!(
            pick_autoscroll_cursor(-10.0, -10.0),
            CursorIcon::ResizeNorthWest
        );
    }
}

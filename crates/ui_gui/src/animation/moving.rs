use eframe::egui::{self, Pos2, Rect};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Hàm nội suy đường cong làm chậm dần (Cubic Ease-Out).
/// Đạt vận tốc cao ngay khi bắt đầu và chậm dần êm ái khi tiếp đất.
#[inline]
pub fn ease_out_cubic(t: f32) -> f32 {
    let clamped = t.clamp(0.0, 1.0);
    1.0 - (1.0 - clamped).powi(3)
}

/// Lưu trữ thông tin một animation di chuyển 2D (Position Interpolation)
#[derive(Clone, Debug)]
pub struct MoveAnimation {
    /// Vị trí xuất phát
    pub from: Pos2,
    /// Vị trí đích đến (logical position mới)
    pub to: Pos2,
    /// Thời điểm bắt đầu animation
    pub start: Instant,
    /// Thời lượng chuyển động
    pub duration: Duration,
}

impl MoveAnimation {
    pub fn new(from: Pos2, to: Pos2, duration: Duration, now: Instant) -> Self {
        Self {
            from,
            to,
            start: now,
            duration,
        }
    }

    /// Tiến độ animation từ 0.0 đến 1.0
    #[inline]
    pub fn progress(&self, now: Instant) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let elapsed = now.saturating_duration_since(self.start);
        (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// Đã hoàn tất animation chưa
    #[inline]
    pub fn is_finished(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }

    /// Tọa độ hiển thị hiện tại thông qua `Pos2::lerp()` 2D
    pub fn current_pos(&self, now: Instant) -> Pos2 {
        let t = ease_out_cubic(self.progress(now));
        self.from.lerp(self.to, t)
    }
}

/// Trình quản lý animation di chuyển vị trí cho các item trong Table hoặc List.
/// Hoạt động theo cơ chế Position Interpolation (2D Vector Lerp), thống nhất
/// cho cả di chuyển ngang, dọc lẫn chéo mà không phân mảnh logic riêng.
#[derive(Clone, Debug)]
pub struct MoveAnimationManager {
    animations: HashMap<String, MoveAnimation>,
    previous_rects: HashMap<String, Rect>,
    default_duration: Duration,
}

impl Default for MoveAnimationManager {
    fn default() -> Self {
        Self::new(Duration::from_millis(180))
    }
}

impl MoveAnimationManager {
    /// Khởi tạo manager với thời lượng mặc định (khuyên dùng 150ms - 200ms)
    pub fn new(default_duration: Duration) -> Self {
        Self {
            animations: HashMap::new(),
            previous_rects: HashMap::new(),
            default_duration,
        }
    }

    /// Bắt đầu một animation di chuyển từ `from` đến `to` với thời lượng mặc định
    pub fn start(&mut self, id: &str, from: Pos2, to: Pos2, now: Instant) {
        if from.distance_sq(to) > 0.25 {
            self.animations.insert(
                id.to_string(),
                MoveAnimation::new(from, to, self.default_duration, now),
            );
        } else {
            self.animations.remove(id);
        }
    }

    /// Chuyển đổi `logical_rect` sang `visual_rect` dựa trên vị trí animation hiện thời
    pub fn visual_rect(&self, id: &str, logical_rect: Rect, now: Instant) -> Rect {
        if let Some(anim) = self.animations.get(id) {
            Rect::from_min_size(anim.current_pos(now), logical_rect.size())
        } else {
            logical_rect
        }
    }

    /// Kiểm tra item cụ thể có đang trong quá trình chuyển động hay không
    pub fn is_animating(&self, id: &str, now: Instant) -> bool {
        self.animations
            .get(id)
            .is_some_and(|anim| !anim.is_finished(now))
    }

    /// Kiểm tra xem có bất kỳ item nào đang chuyển động không
    pub fn has_active_animations(&self, now: Instant) -> bool {
        self.animations.values().any(|anim| !anim.is_finished(now))
    }

    /// Tự động theo dõi rect của item qua từng frame và trả về visual_rect ngay lập tức.
    /// Nếu vị trí logic thay đổi so với frame trước (do swap, reorder),
    /// tự động kích hoạt animation chuyển động mượt từ vị trí cũ sang vị trí mới.
    pub fn track_rect(&mut self, id: &str, current_rect: Rect, now: Instant) -> Rect {
        let prev = self.previous_rects.get(id).copied();
        if let Some(prev_rect) = prev {
            let dist_sq = prev_rect.min.distance_sq(current_rect.min);
            if dist_sq > 0.25 {
                // Nếu đang animate dở dang, lấy visual pos hiện thời làm điểm xuất phát mới (tránh giật cục)
                let from = if let Some(anim) = self.animations.get(id) {
                    anim.current_pos(now)
                } else {
                    prev_rect.min
                };
                self.start(id, from, current_rect.min, now);
            }
            if let Some(r) = self.previous_rects.get_mut(id) {
                *r = current_rect;
            }
        } else {
            self.previous_rects.insert(id.to_string(), current_rect);
        }
        self.visual_rect(id, current_rect, now)
    }

    /// Dọn dẹp các animation đã kết thúc và yêu cầu repaint nếu còn animation đang chạy (1 lượt duyệt)
    pub fn update(&mut self, ctx: &egui::Context, now: Instant) {
        self.animations.retain(|_, anim| !anim.is_finished(now));
        if !self.animations.is_empty() {
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ease_out_cubic() {
        assert!((ease_out_cubic(0.0) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 1e-6);
        // Tại t = 0.5, ease_out_cubic(0.5) = 1 - 0.125 = 0.875
        assert!((ease_out_cubic(0.5) - 0.875).abs() < 1e-6);
        // Clamp kiểm thử ngoài biên
        assert!((ease_out_cubic(-0.5) - 0.0).abs() < 1e-6);
        assert!((ease_out_cubic(1.5) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_move_animation_directions() {
        let start = Instant::now();
        let duration = Duration::from_millis(200);

        // 1. Horizontal: (100, 100) -> (300, 100)
        let h_anim = MoveAnimation::new(
            Pos2::new(100.0, 100.0),
            Pos2::new(300.0, 100.0),
            duration,
            start,
        );
        let pos_start = h_anim.current_pos(start);
        assert!((pos_start.x - 100.0).abs() < 1e-4);
        assert!((pos_start.y - 100.0).abs() < 1e-4);

        let mid = start + Duration::from_millis(100);
        let pos_mid = h_anim.current_pos(mid);
        // Với ease_out_cubic, tại t=0.5: x = 100 + 200 * 0.875 = 275
        assert!((pos_mid.x - 275.0).abs() < 1e-2);
        assert!((pos_mid.y - 100.0).abs() < 1e-4);

        let end = start + Duration::from_millis(200);
        let pos_end = h_anim.current_pos(end);
        assert!((pos_end.x - 300.0).abs() < 1e-4);
        assert!((pos_end.y - 100.0).abs() < 1e-4);

        // 2. Vertical: (100, 100) -> (100, 300)
        let v_anim = MoveAnimation::new(
            Pos2::new(100.0, 100.0),
            Pos2::new(100.0, 300.0),
            duration,
            start,
        );
        let v_mid = v_anim.current_pos(mid);
        assert!((v_mid.x - 100.0).abs() < 1e-4);
        assert!((v_mid.y - 275.0).abs() < 1e-2);

        // 3. Diagonal: (100, 100) -> (300, 300)
        let d_anim = MoveAnimation::new(
            Pos2::new(100.0, 100.0),
            Pos2::new(300.0, 300.0),
            duration,
            start,
        );
        let d_mid = d_anim.current_pos(mid);
        assert!((d_mid.x - 275.0).abs() < 1e-2);
        assert!((d_mid.y - 275.0).abs() < 1e-2);
    }

    #[test]
    fn test_manager_visual_rect() {
        let mut mgr = MoveAnimationManager::new(Duration::from_millis(200));
        let start = Instant::now();

        mgr.start(
            "col_b",
            Pos2::new(100.0, 50.0),
            Pos2::new(300.0, 50.0),
            start,
        );

        let logical_rect = Rect::from_min_size(Pos2::new(300.0, 50.0), egui::vec2(80.0, 30.0));

        // Tại thời điểm bắt đầu: visual rect phải nằm ở vị trí cũ (100, 50)
        let visual_start = mgr.visual_rect("col_b", logical_rect, start);
        assert!((visual_start.min.x - 100.0).abs() < 1e-3);
        assert!((visual_start.min.y - 50.0).abs() < 1e-3);

        // Tại thời điểm kết thúc: visual rect trùng khít logical rect
        let end = start + Duration::from_millis(200);
        let visual_end = mgr.visual_rect("col_b", logical_rect, end);
        assert!((visual_end.min.x - 300.0).abs() < 1e-3);
        assert!((visual_end.min.y - 50.0).abs() < 1e-3);
    }

    #[test]
    fn test_manager_automatic_track_rect() {
        let mut mgr = MoveAnimationManager::new(Duration::from_millis(180));
        let t0 = Instant::now();

        // Frame 1: item A ở (50, 100) -> trả về chính rect_1
        let rect_1 = Rect::from_min_size(Pos2::new(50.0, 100.0), egui::vec2(100.0, 24.0));
        let v0 = mgr.track_rect("item_a", rect_1, t0);
        assert_eq!(v0, rect_1);
        assert!(!mgr.is_animating("item_a", t0));

        // Frame 2: item A bị swap sang (200, 100) -> track_rect tự kích hoạt và trả về visual_rect
        let t1 = t0 + Duration::from_millis(16);
        let rect_2 = Rect::from_min_size(Pos2::new(200.0, 100.0), egui::vec2(100.0, 24.0));
        let v1 = mgr.track_rect("item_a", rect_2, t1);

        assert!(mgr.is_animating("item_a", t1));
        assert!((v1.min.x - 50.0).abs() < 1.0);

        // Sau khi hoàn thành
        let t_finish = t1 + Duration::from_millis(190);
        assert!(!mgr.is_animating("item_a", t_finish));
        let visual_finish = mgr.visual_rect("item_a", rect_2, t_finish);
        assert!((visual_finish.min.x - 200.0).abs() < 1e-3);

        let dummy_ctx = egui::Context::default();
        mgr.update(&dummy_ctx, t_finish);
        assert!(!mgr.has_active_animations(t_finish));
    }
}

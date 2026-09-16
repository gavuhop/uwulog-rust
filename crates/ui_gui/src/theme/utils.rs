use eframe::egui::{self, Color32};

/// Định dạng số với dấu phẩy phân cách hàng nghìn (ví dụ: 50,000)
pub fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Vẽ icon drag handle 6 chấm (2 cột x 3 hàng) sắc nét bằng vector painter, không phụ thuộc font chữ hệ thống
pub fn draw_drag_handle(painter: &egui::Painter, center: egui::Pos2, color: Color32) {
    let dx = 2.5;
    let dy = 3.5;
    let r = 1.25;
    for &x in &[center.x - dx, center.x + dx] {
        for &y in &[center.y - dy, center.y, center.y + dy] {
            painter.circle_filled(egui::pos2(x, y), r, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(50000), "50,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }
}

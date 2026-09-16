use eframe::egui::Color32;

/// Parse chuỗi hex (hỗ trợ các định dạng #RGB, #RGBA, #RRGGBB, #RRGGBBAA) sang Color32
pub fn hex_to_color(hex: &str) -> Option<Color32> {
    let hex = hex.trim().trim_start_matches('#');
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            Some(Color32::from_rgb(r, g, b))
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
            let a = u8::from_str_radix(&hex[3..4], 16).ok()? * 17;
            Some(Color32::from_rgba_premultiplied(r, g, b, a))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color32::from_rgba_premultiplied(r, g, b, a))
        }
        _ => None,
    }
}

/// Chuyển đổi Color32 sang chuỗi hex dạng `#rrggbb` hoặc `#rrggbbaa`
pub fn color_to_hex(c: Color32) -> String {
    if c.a() == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r(), c.g(), c.b(), c.a())
    }
}

/// Thay đổi độ mờ alpha (0..=255)
#[inline]
pub fn with_alpha(c: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}

/// Thay đổi độ mờ opacity dạng float (0.0..=1.0) tương tự phương thức opacity() trong GPUI/Zed
#[inline]
pub fn with_opacity(c: Color32, opacity: f32) -> Color32 {
    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    with_alpha(c, alpha)
}

/// Làm tối màu sắc theo tỷ lệ factor (0.0..=1.0)
pub fn darken(c: Color32, factor: f32) -> Color32 {
    let f = (1.0 - factor).clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        ((c.r() as f32) * f).round() as u8,
        ((c.g() as f32) * f).round() as u8,
        ((c.b() as f32) * f).round() as u8,
        c.a(),
    )
}

/// Làm sáng màu sắc theo tỷ lệ factor (0.0..=1.0)
pub fn lighten(c: Color32, factor: f32) -> Color32 {
    let f = factor.clamp(0.0, 1.0);
    Color32::from_rgba_premultiplied(
        ((c.r() as f32) + (255.0 - c.r() as f32) * f).round() as u8,
        ((c.g() as f32) + (255.0 - c.g() as f32) * f).round() as u8,
        ((c.b() as f32) + (255.0 - c.b() as f32) * f).round() as u8,
        c.a(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_conversion() {
        assert_eq!(
            hex_to_color("#14161f"),
            Some(Color32::from_rgb(0x14, 0x16, 0x1f))
        );
        assert_eq!(
            hex_to_color("14161f"),
            Some(Color32::from_rgb(0x14, 0x16, 0x1f))
        );
        assert_eq!(
            hex_to_color("#fff"),
            Some(Color32::from_rgb(0xff, 0xff, 0xff))
        );
        assert_eq!(
            hex_to_color("#14161f80"),
            Some(Color32::from_rgba_premultiplied(0x14, 0x16, 0x1f, 0x80))
        );

        let c = Color32::from_rgb(0x14, 0x16, 0x1f);
        assert_eq!(color_to_hex(c), "#14161f");

        let c_alpha = Color32::from_rgba_premultiplied(0x14, 0x16, 0x1f, 0xaa);
        assert_eq!(color_to_hex(c_alpha), "#14161faa");
    }

    #[test]
    fn test_alpha_and_opacity() {
        let c = Color32::from_rgb(100, 150, 200);
        let c_alpha = with_alpha(c, 128);
        assert_eq!(c_alpha.a(), 128);

        let c_op = with_opacity(c, 0.5);
        assert_eq!(c_op.a(), 128);
    }
}

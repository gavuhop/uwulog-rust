use super::tokens::AnsiColors;
use eframe::egui::Color32;

/// Phân tách chuỗi chứa ANSI escape sequences (SGR) thành các đoạn văn bản kèm mã màu tương ứng
pub fn parse_ansi_segments_with_palette(
    text: &str,
    default_color: Color32,
    ansi: &AnsiColors,
) -> Vec<(String, Color32)> {
    if !text.contains('\x1b') {
        return vec![(text.to_string(), default_color)];
    }

    let mut segments = Vec::new();
    let mut current_color = default_color;
    let mut i = 0;
    let bytes = text.as_bytes();
    let mut current_text = String::new();

    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            if !current_text.is_empty() {
                segments.push((std::mem::take(&mut current_text), current_color));
            }
            i += 2;
            let start_code = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b';') {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'm' {
                let code_str = &text[start_code..i];
                i += 1;
                for part in code_str.split(';') {
                    if let Ok(code) = part.parse::<u32>() {
                        current_color = ansi.color_for_code(code, default_color);
                    }
                }
            }
        } else {
            let ch = text[i..].chars().next().unwrap();
            current_text.push(ch);
            i += ch.len_utf8();
        }
    }

    if !current_text.is_empty() {
        segments.push((current_text, current_color));
    }

    if segments.is_empty() {
        vec![(String::new(), default_color)]
    } else {
        segments
    }
}

/// Parse ANSI escape sequences sử dụng bảng màu ANSI của theme đang hoạt động
pub fn parse_ansi_segments(text: &str, default_color: Color32) -> Vec<(String, Color32)> {
    let theme = crate::theme::active();
    parse_ansi_segments_with_palette(text, default_color, &theme.ansi)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::presets;

    #[test]
    fn test_parse_ansi_segments() {
        let ansi_theme = presets::nord_dimmed().ansi;
        let ansi_text = "\x1b[31mRed text\x1b[0m normal \x1b[32mGreen text\x1b[0m";
        let default_color = Color32::from_rgb(0xc5, 0xcd, 0xd9);
        let segments = parse_ansi_segments_with_palette(ansi_text, default_color, &ansi_theme);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].0, "Red text");
        assert_eq!(segments[0].1, ansi_theme.red);
        assert_eq!(segments[1].0, " normal ");
        assert_eq!(segments[1].1, default_color);
        assert_eq!(segments[2].0, "Green text");
        assert_eq!(segments[2].1, ansi_theme.green);
    }
}

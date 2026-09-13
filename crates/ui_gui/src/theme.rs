use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
    Visuals,
};
use std::sync::Arc;

// Soft Eye-Pleasing Dark Palette (Nord / Tokyo Night Dimmed - Không chói mắt)
pub const BG_BASE: Color32 = Color32::from_rgb(0x14, 0x16, 0x1f); // #14161f - Nền tối êm mắt
pub const BG_MANTLE: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x28); // #1a1d28 - Header, Sidebar
pub const BG_CRUST: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x16); // #0e0f16 - Inputs, Code
pub const BG_SURFACE0: Color32 = Color32::from_rgb(0x28, 0x2c, 0x3c); // #282c3c - Viền phân cách
pub const BG_SURFACE1: Color32 = Color32::from_rgb(0x34, 0x3a, 0x4e); // #343a4e - Nút bấm / Hover
pub const BG_ROW_HOVER: Color32 = Color32::from_rgb(0x1e, 0x23, 0x33); // #1e2333 - Hover hàng log
pub const BG_ROW_SELECTED: Color32 = Color32::from_rgb(0x20, 0x29, 0x3d); // #20293d - Nền chọn hàng (Dark Slate Navy dịu mắt)
pub const BG_TEXT_SELECTION: Color32 = Color32::from_rgb(0x38, 0x66, 0x9e); // #38669e - Nền tô bôi đen chữ bằng chuột (Vibrant Electric Blue)
pub const STROKE_TEXT_SELECTION: Color32 = Color32::from_rgb(0x78, 0xa0, 0xd4); // #78a0d4
pub const BG_ROW_HIGHLIGHT: Color32 = Color32::from_rgb(0x35, 0x2e, 0x1a); // #352e1a - Highlight hàng log (hổ phách tối dịu)
#[allow(dead_code)]
pub const BG_ROW_HIGHLIGHT_HOVER: Color32 = Color32::from_rgb(0x42, 0x3a, 0x22); // #423a22 - Hover hàng highlight
pub const BG_TERM_HIGHLIGHT: Color32 = Color32::from_rgb(0x6e, 0x4f, 0x15); // #6e4f15 - Nền hổ phách sáng làm nổi bật từ khóa
pub const TEXT_TERM_HIGHLIGHT: Color32 = Color32::from_rgb(0xff, 0xf2, 0xcc); // #fff2cc - Chữ vàng kem sáng nổi bật

// Màu chữ êm dịu, không chói lóa (Soft pastel & warm slate)
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xc5, 0xcd, 0xd9); // #c5cdd9 - Chữ xám ấm dịu mắt
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x72, 0x7a, 0x90); // #727a90 - Timestamp & nhãn phụ
pub const TEXT_PLACEHOLDER: Color32 = Color32::from_rgb(0x55, 0x5d, 0x73); // #555d73 - Placeholder tìm kiếm tối chìm dịu mắt
pub const TEXT_KEY: Color32 = Color32::from_rgb(0x78, 0xa0, 0xd4); // #78a0d4 - Pastel Blue dịu

// Màu trạng thái mềm mại (Pastel Muted)
pub const COLOR_ERROR: Color32 = Color32::from_rgb(0xd9, 0x65, 0x70); // #d96570 - Đỏ san hô mềm (không chói)
pub const COLOR_WARN: Color32 = Color32::from_rgb(0xd4, 0xa3, 0x59); // #d4a359 - Vàng hổ phách ấm (không chói)
pub const COLOR_INFO: Color32 = Color32::from_rgb(0x7e, 0xc7, 0x87); // #7ec787 - Xanh lá pastel êm mắt
pub const COLOR_DEBUG: Color32 = TEXT_MUTED; // #727a90 - Đồng bộ dịu mắt

pub const FONT_SEGOE_ICONS: &str = "segoe_icons";

pub fn log_color_to_egui(color: uwu_core_schema::LogColor) -> Color32 {
    match color {
        uwu_core_schema::LogColor::Red => COLOR_ERROR,
        uwu_core_schema::LogColor::Yellow => COLOR_WARN,
        uwu_core_schema::LogColor::Green => COLOR_INFO,
        uwu_core_schema::LogColor::Gray => COLOR_DEBUG,
        uwu_core_schema::LogColor::Default => TEXT_PRIMARY,
    }
}

// Nút bấm mềm mại
pub const BTN_RESTART_BG: Color32 = Color32::from_rgb(0x1a, 0x33, 0x24); // Xanh lục tối dịu
pub const BTN_RESTART_BORDER: Color32 = Color32::from_rgb(0x3a, 0x74, 0x52);
pub const BTN_STOP_BG: Color32 = Color32::from_rgb(0x4a, 0x1e, 0x24); // Đỏ tối dịu
pub const BTN_STOP_BORDER: Color32 = Color32::from_rgb(0x94, 0x38, 0x42);

// Nút bấm Latch / Auto-scroll
pub const BTN_LATCHED_BG: Color32 = BTN_RESTART_BG;
pub const BTN_LATCHED_BORDER: Color32 = BTN_RESTART_BORDER;
pub const BTN_UNLATCHED_BG: Color32 = Color32::from_rgb(0x38, 0x2b, 0x16); // Hổ phách tối dịu
pub const BTN_UNLATCHED_BORDER: Color32 = Color32::from_rgb(0x7a, 0x56, 0x25);

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

pub fn create_visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_fill = BG_MANTLE;
    visuals.panel_fill = BG_MANTLE;
    visuals.faint_bg_color = BG_BASE;
    visuals.extreme_bg_color = BG_CRUST;
    visuals.code_bg_color = BG_CRUST;

    // Window & Dialog
    visuals.window_corner_radius = CornerRadius::same(6);
    visuals.window_stroke = Stroke::new(1.0, BG_SURFACE0);

    // Non-interactive
    visuals.widgets.noninteractive.bg_fill = BG_BASE;
    visuals.widgets.noninteractive.weak_bg_fill = BG_BASE;
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BG_SURFACE0);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Inactive
    visuals.widgets.inactive.bg_fill = BG_SURFACE0;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.corner_radius = CornerRadius::same(4);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BG_SURFACE0);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Hovered
    visuals.widgets.hovered.bg_fill = BG_SURFACE1;
    visuals.widgets.hovered.weak_bg_fill = BG_ROW_HOVER;
    visuals.widgets.hovered.corner_radius = CornerRadius::same(4);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, TEXT_KEY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Active / Pressed
    visuals.widgets.active.bg_fill = BG_ROW_SELECTED;
    visuals.widgets.active.weak_bg_fill = BG_ROW_SELECTED;
    visuals.widgets.active.corner_radius = CornerRadius::same(4);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, TEXT_KEY);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Selection (Bôi đen chọn chữ)
    visuals.selection.bg_fill = BG_TEXT_SELECTION;
    visuals.selection.stroke = Stroke::new(1.0, STROKE_TEXT_SELECTION);

    visuals
}

pub fn apply_theme(ctx: &egui::Context) {
    // 1. Setup Terminal Monospace Fonts
    let mut fonts = FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        // Cascadia Code (Windows Terminal font) hoặc Consolas
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\CascadiaCode.ttf") {
            fonts.font_data.insert(
                "terminal_mono".to_owned(),
                Arc::new(FontData::from_owned(data)),
            );
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "terminal_mono".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "terminal_mono".to_owned());
            }
        } else if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\consola.ttf") {
            fonts.font_data.insert(
                "terminal_mono".to_owned(),
                Arc::new(FontData::from_owned(data)),
            );
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "terminal_mono".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "terminal_mono".to_owned());
            }
        }

        // Segoe Fluent Icons (Windows 11) hoặc Segoe MDL2 Assets (Windows 10) cho caption buttons chuẩn Windows
        let icon_font_path = if std::path::Path::new("C:\\Windows\\Fonts\\SegoeIcons.ttf").exists()
        {
            Some("C:\\Windows\\Fonts\\SegoeIcons.ttf")
        } else if std::path::Path::new("C:\\Windows\\Fonts\\segmdl2.ttf").exists() {
            Some("C:\\Windows\\Fonts\\segmdl2.ttf")
        } else {
            None
        };

        if let Some(path) = icon_font_path {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "segoe_icons".to_owned(),
                    Arc::new(FontData::from_owned(data)),
                );
                if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                    family.push("segoe_icons".to_owned());
                }
                if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                    family.push("segoe_icons".to_owned());
                }
                fonts.families.insert(
                    FontFamily::Name("segoe_icons".into()),
                    vec!["segoe_icons".to_owned()],
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Cố gắng nạp các font Monospace sắc nét, x-height cao phổ biến trên Linux (ưu tiên JetBrains Mono, Source Code Pro, Adwaita Mono, Liberation)
        let mono_candidates = [
            "/usr/share/fonts/jetbrains-mono-fonts/JetBrainsMono-Medium.otf",
            "/usr/share/fonts/jetbrains-mono-fonts/JetBrainsMono-Regular.otf",
            "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Medium.ttf",
            "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono-Medium.ttf",
            "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/adobe-source-code-pro-fonts/SourceCodePro-Medium.otf",
            "/usr/share/fonts/adobe-source-code-pro-fonts/SourceCodePro-Regular.otf",
            "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf",
            "/usr/share/fonts/liberation-mono-fonts/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            "/usr/share/fonts/dejavu-sans-mono-fonts/DejaVuSansMono.ttf",
            "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
            "/usr/share/fonts/google-noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
        ];
        for path in mono_candidates {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "linux_mono".to_owned(),
                    Arc::new(FontData::from_owned(data)),
                );
                if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                    family.insert(0, "linux_mono".to_owned());
                }
                if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                    family.insert(0, "linux_mono".to_owned());
                }
                break;
            }
        }

        // 2. Nạp fallback font chứa ký tự Symbols & Braille (Symbola hoặc Noto Sans Symbols 2)
        let symbol_candidates = [
            "/usr/share/fonts/gdouros-symbola/Symbola.ttf",
            "/usr/share/fonts/google-noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
            "/usr/share/fonts/truetype/ancient-scripts/Symbola.ttf",
            "/usr/share/fonts/google-noto-vf/NotoSansSymbols[wght].ttf",
        ];
        for path in symbol_candidates {
            if let Ok(data) = std::fs::read(path) {
                fonts.font_data.insert(
                    "linux_symbols".to_owned(),
                    Arc::new(FontData::from_owned(data)),
                );
                if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                    family.push("linux_symbols".to_owned());
                }
                if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                    family.push("linux_symbols".to_owned());
                }
                break;
            }
        }
    }

    ctx.set_fonts(fonts);
    ctx.set_visuals(create_visuals());

    // 3. Typography & Text Styles (Tối ưu độ nét và kích thước đọc thoải mái trên màn hình 1080p/2K/4K)
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(15.0, FontFamily::Monospace)),
            (TextStyle::Body, FontId::new(12.5, FontFamily::Monospace)),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
            (TextStyle::Button, FontId::new(12.0, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
        ]
        .into();

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(12);
        // Xóa interact_size mặc định để hitbox nút bấm chuẩn xác từng pixel
        style.spacing.interact_size = egui::Vec2::ZERO;
    });
}

/// Áp dụng theme (màu nền, màu chữ, Dark Mode) cho thanh tiêu đề gốc của Windows (DWM)
/// Giữ nguyên 3 nút điều khiển chuẩn (Minimize, Maximize, Close, Snap Layouts)
#[cfg(target_os = "windows")]
pub fn apply_windows_titlebar_theme(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Ok(window_handle) = cc.window_handle() {
        if let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw() {
            let hwnd = win32_handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            unsafe {
                use windows_sys::Win32::Graphics::Dwm::{
                    DwmSetWindowAttribute, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR,
                    DWMWA_USE_IMMERSIVE_DARK_MODE,
                };

                // 1. Kích hoạt Dark Mode cho Title Bar (Windows 10 1809+ & Windows 11)
                let dark_mode: i32 = 1;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                    &dark_mode as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                );

                // 2. Set màu nền thanh tiêu đề trùng khớp chính xác với BG_MANTLE (#1a1d28)
                // Windows DWM dùng định dạng COLORREF 0x00BBGGRR
                // RGB(0x1a, 0x1d, 0x28) -> BGR: 0x00281d1a
                let caption_color: u32 = 0x00281d1a;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_CAPTION_COLOR as u32,
                    &caption_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );

                // 3. Set màu chữ tiêu đề trùng với TEXT_PRIMARY (#c5cdd9) -> BGR: 0x00d9cdc5
                let text_color: u32 = 0x00d9cdc5;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_TEXT_COLOR as u32,
                    &text_color as *const _ as *const _,
                    std::mem::size_of::<u32>() as u32,
                );
            }
        }
    }
}

pub fn parse_ansi_segments(text: &str, default_color: Color32) -> Vec<(String, Color32)> {
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
                        match code {
                            0 | 39 => current_color = default_color,
                            30 => current_color = Color32::from_rgb(0x45, 0x47, 0x5a),
                            31 => current_color = Color32::from_rgb(0xf3, 0x8b, 0xa8),
                            32 => current_color = Color32::from_rgb(0xa6, 0xe3, 0xa1),
                            33 => current_color = Color32::from_rgb(0xf9, 0xe2, 0xaf),
                            34 => current_color = Color32::from_rgb(0x89, 0xb4, 0xfa),
                            35 => current_color = Color32::from_rgb(0xcb, 0xa6, 0xf7),
                            36 => current_color = Color32::from_rgb(0x89, 0xdc, 0xeb),
                            37 => current_color = Color32::from_rgb(0xcd, 0xd6, 0xf4),
                            90 => current_color = Color32::from_rgb(0x6c, 0x70, 0x86),
                            91 => current_color = Color32::from_rgb(0xf3, 0x8b, 0xa8),
                            92 => current_color = Color32::from_rgb(0xa6, 0xe3, 0xa1),
                            93 => current_color = Color32::from_rgb(0xf9, 0xe2, 0xaf),
                            94 => current_color = Color32::from_rgb(0x89, 0xb4, 0xfa),
                            95 => current_color = Color32::from_rgb(0xcb, 0xa6, 0xf7),
                            96 => current_color = Color32::from_rgb(0x89, 0xdc, 0xeb),
                            97 => current_color = Color32::from_rgb(0xff, 0xff, 0xff),
                            _ => {}
                        }
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

pub fn create_highlighted_layout_job(
    text: &str,
    default_color: Color32,
    font_id: FontId,
    highlighted_terms: &std::collections::HashSet<String>,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    if text.is_empty() {
        return job;
    }

    // Fast-path 0 heap allocation cho log thuần túy không có ANSI và không có Highlight
    if highlighted_terms.is_empty() && !text.contains('\x1b') {
        job.append(
            text,
            0.0,
            egui::TextFormat {
                font_id,
                color: default_color,
                ..Default::default()
            },
        );
        return job;
    }

    let segments = parse_ansi_segments(text, default_color);

    for (seg_text, seg_color) in segments {
        if highlighted_terms.is_empty() || seg_text.is_empty() {
            job.append(
                &seg_text,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: seg_color,
                    ..Default::default()
                },
            );
            continue;
        }

        let lower_text = seg_text.to_lowercase();
        let mut intervals: Vec<(usize, usize)> = Vec::new();

        for term in highlighted_terms {
            let term_clean = term.trim().to_lowercase();
            if term_clean.is_empty() {
                continue;
            }
            for (pos, _) in lower_text.match_indices(&term_clean) {
                let actual_end = pos + term_clean.len();
                intervals.push((pos, actual_end));
            }
        }

        if intervals.is_empty() {
            job.append(
                &seg_text,
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: seg_color,
                    ..Default::default()
                },
            );
            continue;
        }

        // Gộp các khoảng trùng nhau
        intervals.sort_by_key(|(s, _)| *s);
        let mut merged: Vec<(usize, usize)> = Vec::new();
        for (s, e) in intervals {
            if let Some(last) = merged.last_mut() {
                if s <= last.1 {
                    last.1 = last.1.max(e);
                } else {
                    merged.push((s, e));
                }
            } else {
                merged.push((s, e));
            }
        }

        let mut cur = 0;
        for (s, e) in merged {
            if s > cur {
                if let Some(slice) = seg_text.get(cur..s) {
                    job.append(
                        slice,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: seg_color,
                            ..Default::default()
                        },
                    );
                }
            }
            if let Some(slice) = seg_text.get(s..e) {
                job.append(
                    slice,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: TEXT_TERM_HIGHLIGHT,
                        background: BG_TERM_HIGHLIGHT,
                        ..Default::default()
                    },
                );
            }
            cur = e;
        }

        if cur < seg_text.len() {
            if let Some(slice) = seg_text.get(cur..) {
                job.append(
                    slice,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: seg_color,
                        ..Default::default()
                    },
                );
            }
        }
    }

    job
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

    #[test]
    fn test_highlighted_layout_job() {
        let mut terms = std::collections::HashSet::new();
        terms.insert("warn".to_string());
        terms.insert("error".to_string());

        let job = create_highlighted_layout_job(
            "this is a warning and ERROR message",
            TEXT_PRIMARY,
            FontId::monospace(11.5),
            &terms,
        );

        assert_eq!(job.text, "this is a warning and ERROR message");
        assert!(job.sections.len() > 1);
    }

    #[test]
    fn test_parse_ansi_segments() {
        let ansi_text = "\x1b[31mRed text\x1b[0m normal \x1b[32mGreen text\x1b[0m";
        let segments = parse_ansi_segments(ansi_text, TEXT_PRIMARY);
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].0, "Red text");
        assert_eq!(segments[0].1, Color32::from_rgb(0xf3, 0x8b, 0xa8));
        assert_eq!(segments[1].0, " normal ");
        assert_eq!(segments[1].1, TEXT_PRIMARY);
        assert_eq!(segments[2].0, "Green text");
        assert_eq!(segments[2].1, Color32::from_rgb(0xa6, 0xe3, 0xa1));
    }

    #[test]
    fn test_font_glyphs_and_drag_handle() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let mut output = ctx.run_ui(Default::default(), |_ui| {});
        output.textures_delta.clear();

        // Kiểm tra ký tự checkmark chuẩn '✔' (U+2714) hiển thị được trên mọi hệ thống
        ctx.fonts_mut(|f| {
            let mono = FontId::monospace(12.0);
            assert!(
                f.has_glyph(&mono, '✔'),
                "Char '✔' (U+2714) must have a valid glyph in theme monospace font"
            );
        });

        // Kiểm tra vector painter cho draw_drag_handle
        let painter = egui::Painter::new(
            ctx.clone(),
            egui::LayerId::new(egui::Order::Foreground, egui::Id::new("test_layer")),
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 100.0)),
        );
        draw_drag_handle(&painter, egui::pos2(12.0, 12.0), TEXT_KEY);
    }
}

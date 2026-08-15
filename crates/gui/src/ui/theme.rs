use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Rounding, Stroke, TextStyle,
    Visuals,
};

// Soft Eye-Pleasing Dark Palette (Nord / Tokyo Night Dimmed - Không chói mắt)
pub const BG_BASE: Color32 = Color32::from_rgb(0x14, 0x16, 0x1f); // #14161f - Nền tối êm mắt
pub const BG_MANTLE: Color32 = Color32::from_rgb(0x1a, 0x1d, 0x28); // #1a1d28 - Header, Sidebar
pub const BG_CRUST: Color32 = Color32::from_rgb(0x0e, 0x0f, 0x16); // #0e0f16 - Inputs, Code
pub const BG_SURFACE0: Color32 = Color32::from_rgb(0x28, 0x2c, 0x3c); // #282c3c - Viền phân cách
pub const BG_SURFACE1: Color32 = Color32::from_rgb(0x34, 0x3a, 0x4e); // #343a4e - Nút bấm / Hover
pub const BG_ROW_HOVER: Color32 = Color32::from_rgb(0x1e, 0x23, 0x33); // #1e2333 - Hover hàng log
pub const BG_ROW_SELECTED: Color32 = Color32::from_rgb(0x28, 0x32, 0x48); // #283248 - Chọn hàng log

// Màu chữ êm dịu, không chói lóa (Soft pastel & warm slate)
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(0xc5, 0xcd, 0xd9); // #c5cdd9 - Chữ xám ấm dịu mắt
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x72, 0x7a, 0x90); // #727a90 - Timestamp & nhãn phụ
pub const TEXT_KEY: Color32 = Color32::from_rgb(0x78, 0xa0, 0xd4); // #78a0d4 - Pastel Blue dịu

// Màu trạng thái mềm mại (Pastel Muted)
pub const COLOR_ERROR: Color32 = Color32::from_rgb(0xd9, 0x65, 0x70); // #d96570 - Đỏ san hô mềm (không chói)
pub const COLOR_WARN: Color32 = Color32::from_rgb(0xd4, 0xa3, 0x59); // #d4a359 - Vàng hổ phách ấm (không chói)
pub const COLOR_INFO: Color32 = Color32::from_rgb(0x7e, 0xc7, 0x87); // #7ec787 - Xanh lá pastel êm mắt
#[allow(dead_code)]
pub const COLOR_DEBUG: Color32 = TEXT_MUTED; // #727a90 - Đồng bộ dịu mắt

// Nút bấm mềm mại
pub const BTN_RESTART_BG: Color32 = Color32::from_rgb(0x1f, 0x42, 0x2e); // Xanh lục tối dịu
pub const BTN_RESTART_BORDER: Color32 = Color32::from_rgb(0x3a, 0x74, 0x52);
pub const BTN_STOP_BG: Color32 = Color32::from_rgb(0x4a, 0x1e, 0x24); // Đỏ tối dịu
pub const BTN_STOP_BORDER: Color32 = Color32::from_rgb(0x94, 0x38, 0x42);

// Nút bấm Latch / Auto-scroll
pub const BTN_LATCHED_BG: Color32 = Color32::from_rgb(0x1a, 0x33, 0x24); // Xanh lục tối dịu
pub const BTN_LATCHED_BORDER: Color32 = Color32::from_rgb(0x3a, 0x74, 0x52);
pub const BTN_UNLATCHED_BG: Color32 = Color32::from_rgb(0x38, 0x2b, 0x16); // Hổ phách tối dịu
pub const BTN_UNLATCHED_BORDER: Color32 = Color32::from_rgb(0x7a, 0x56, 0x25);

pub fn create_visuals() -> Visuals {
    let mut visuals = Visuals::dark();

    visuals.override_text_color = Some(TEXT_PRIMARY);
    visuals.window_fill = BG_MANTLE;
    visuals.panel_fill = BG_MANTLE;
    visuals.faint_bg_color = BG_BASE;
    visuals.extreme_bg_color = BG_CRUST;
    visuals.code_bg_color = BG_CRUST;

    // Window & Dialog
    visuals.window_rounding = Rounding::same(6.0);
    visuals.window_stroke = Stroke::new(1.0, BG_SURFACE0);

    // Non-interactive
    visuals.widgets.noninteractive.bg_fill = BG_BASE;
    visuals.widgets.noninteractive.weak_bg_fill = BG_BASE;
    visuals.widgets.noninteractive.rounding = Rounding::same(4.0);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BG_SURFACE0);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Inactive
    visuals.widgets.inactive.bg_fill = BG_SURFACE0;
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.rounding = Rounding::same(4.0);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BG_SURFACE0);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Hovered
    visuals.widgets.hovered.bg_fill = BG_SURFACE1;
    visuals.widgets.hovered.weak_bg_fill = BG_ROW_HOVER;
    visuals.widgets.hovered.rounding = Rounding::same(4.0);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, TEXT_KEY);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Active / Pressed
    visuals.widgets.active.bg_fill = BG_ROW_SELECTED;
    visuals.widgets.active.weak_bg_fill = BG_ROW_SELECTED;
    visuals.widgets.active.rounding = Rounding::same(4.0);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, TEXT_KEY);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT_PRIMARY);

    // Selection
    visuals.selection.bg_fill = BG_ROW_SELECTED;
    visuals.selection.stroke = Stroke::NONE;

    visuals
}

pub fn apply_theme(ctx: &egui::Context) {
    // 1. Setup Terminal Monospace Fonts
    let mut fonts = FontDefinitions::default();

    #[cfg(target_os = "windows")]
    {
        // Cascadia Code (Windows Terminal font) hoặc Consolas
        if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\CascadiaCode.ttf") {
            fonts
                .font_data
                .insert("terminal_mono".to_owned(), FontData::from_owned(data));
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "terminal_mono".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "terminal_mono".to_owned());
            }
        } else if let Ok(data) = std::fs::read("C:\\Windows\\Fonts\\consola.ttf") {
            fonts
                .font_data
                .insert("terminal_mono".to_owned(), FontData::from_owned(data));
            if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                family.insert(0, "terminal_mono".to_owned());
            }
            if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                family.insert(0, "terminal_mono".to_owned());
            }
        }
    }

    ctx.set_fonts(fonts);
    ctx.set_visuals(create_visuals());

    // 3. Typography & Text Styles
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Heading, FontId::new(14.5, FontFamily::Monospace)),
        (TextStyle::Body, FontId::new(12.0, FontFamily::Monospace)),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
        (TextStyle::Button, FontId::new(11.5, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(10.5, FontFamily::Monospace)),
    ]
    .into();

    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.window_margin = egui::Margin::same(12.0);
    ctx.set_style(style);
}

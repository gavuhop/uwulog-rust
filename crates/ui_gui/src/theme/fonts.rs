use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

pub const FONT_SEGOE_ICONS: &str = "segoe_icons";
pub const TABLE_FONT_SIZE: f32 = 14.0;

/// Cấu hình typography và nạp font Monospace sắc nét trên Windows và Linux
pub fn setup_fonts(ctx: &egui::Context) {
    // 1. Kích hoạt Pixel Snapping: làm tròn toạ độ text vào đúng lưới pixel vật lý, chống mờ do nội suy float
    ctx.tessellation_options_mut(|options| {
        options.round_text_to_pixels = true;
    });

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
                    FONT_SEGOE_ICONS.to_owned(),
                    Arc::new(FontData::from_owned(data)),
                );
                if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
                    family.push(FONT_SEGOE_ICONS.to_owned());
                }
                if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
                    family.push(FONT_SEGOE_ICONS.to_owned());
                }
                fonts.families.insert(
                    FontFamily::Name(FONT_SEGOE_ICONS.into()),
                    vec![FONT_SEGOE_ICONS.to_owned()],
                );
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 1. Cố gắng nạp các font Monospace sắc nét phổ biến trên Linux (ưu tiên JetBrains Mono, Source Code Pro, Adwaita Mono, Liberation)
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

    // Typography & Text Styles (Chuẩn hóa font size số nguyên: 13.0, 16.0, 11.0)
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontId::new(16.0, FontFamily::Monospace)),
            (TextStyle::Body, FontId::new(13.0, FontFamily::Monospace)),
            (
                TextStyle::Monospace,
                FontId::new(13.0, FontFamily::Monospace),
            ),
            (TextStyle::Button, FontId::new(13.0, FontFamily::Monospace)),
            (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
        ]
        .into();

        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(12);
        style.spacing.interact_size = egui::Vec2::ZERO;
    });
}

use super::color::{color_to_hex, hex_to_color};
use super::theme::{Appearance, Theme};
use super::tokens::*;
use anyhow::{Context, Result};
use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeJson {
    pub id: String,
    pub name: String,
    pub appearance: String, // "dark" hoặc "light"
    pub surfaces: SurfaceColorsJson,
    pub borders: BorderColorsJson,
    pub text: TextColorsJson,
    pub status: StatusColorsJson,
    pub log: LogTableColorsJson,
    #[serde(default)]
    pub selection: Option<SelectionColorsJson>,
    #[serde(default)]
    pub controls: Option<ControlColorsJson>,
    #[serde(default)]
    pub ansi: Option<AnsiColorsJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurfaceColorsJson {
    pub base: String,
    pub mantle: String,
    pub crust: String,
    pub surface0: String,
    pub surface1: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BorderColorsJson {
    pub border: String,
    pub border_subtle: String,
    pub border_focused: String,
    pub border_selected: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextColorsJson {
    pub primary: String,
    pub muted: String,
    pub placeholder: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusColorsJson {
    pub error: String,
    pub warning: String,
    pub info: String,
    pub debug: String,
    pub success: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogTableColorsJson {
    pub row_hover: String,
    pub row_selected: String,
    pub row_highlight: String,
    pub term_highlight_bg: String,
    pub term_highlight_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectionColorsJson {
    pub bg: Option<String>,
    pub stroke: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ControlColorsJson {
    pub restart_bg: Option<String>,
    pub restart_border: Option<String>,
    pub stop_bg: Option<String>,
    pub stop_border: Option<String>,
    pub latched_bg: Option<String>,
    pub latched_border: Option<String>,
    pub unlatched_bg: Option<String>,
    pub unlatched_border: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AnsiColorsJson {
    pub black: Option<String>,
    pub red: Option<String>,
    pub green: Option<String>,
    pub yellow: Option<String>,
    pub blue: Option<String>,
    pub magenta: Option<String>,
    pub cyan: Option<String>,
    pub white: Option<String>,
    pub bright_black: Option<String>,
    pub bright_red: Option<String>,
    pub bright_green: Option<String>,
    pub bright_yellow: Option<String>,
    pub bright_blue: Option<String>,
    pub bright_magenta: Option<String>,
    pub bright_cyan: Option<String>,
    pub bright_white: Option<String>,
}

impl Theme {
    /// Phân tích và nạp một Theme từ chuỗi JSON (tương thích định dạng hex #rrggbb hoặc #rrggbbaa)
    pub fn from_json(json_str: &str) -> Result<Self> {
        let raw: ThemeJson =
            serde_json::from_str(json_str).context("Không thể phân tích cú pháp Theme JSON")?;

        let parse_color = |hex: &str, field_name: &str| -> Result<Color32> {
            hex_to_color(hex)
                .with_context(|| format!("Mã màu hex không hợp lệ tại {}: '{}'", field_name, hex))
        };

        let appearance = match raw.appearance.to_lowercase().as_str() {
            "light" => Appearance::Light,
            _ => Appearance::Dark,
        };

        // Fallback sang theme cơ sở nếu các trường tùy chọn không được khai báo
        let fallback = if appearance.is_light() {
            super::presets::light()
        } else {
            super::presets::nord_dimmed()
        };

        let surfaces = SurfaceColors {
            base: parse_color(&raw.surfaces.base, "surfaces.base")?,
            mantle: parse_color(&raw.surfaces.mantle, "surfaces.mantle")?,
            crust: parse_color(&raw.surfaces.crust, "surfaces.crust")?,
            surface0: parse_color(&raw.surfaces.surface0, "surfaces.surface0")?,
            surface1: parse_color(&raw.surfaces.surface1, "surfaces.surface1")?,
        };

        let borders = BorderColors {
            border: parse_color(&raw.borders.border, "borders.border")?,
            border_subtle: parse_color(&raw.borders.border_subtle, "borders.border_subtle")?,
            border_focused: parse_color(&raw.borders.border_focused, "borders.border_focused")?,
            border_selected: parse_color(&raw.borders.border_selected, "borders.border_selected")?,
        };

        let text = TextColors {
            primary: parse_color(&raw.text.primary, "text.primary")?,
            muted: parse_color(&raw.text.muted, "text.muted")?,
            placeholder: parse_color(&raw.text.placeholder, "text.placeholder")?,
            accent: parse_color(&raw.text.accent, "text.accent")?,
        };

        let status = StatusColors {
            error: parse_color(&raw.status.error, "status.error")?,
            error_bg: fallback.status.error_bg,
            warning: parse_color(&raw.status.warning, "status.warning")?,
            warning_bg: fallback.status.warning_bg,
            info: parse_color(&raw.status.info, "status.info")?,
            info_bg: fallback.status.info_bg,
            debug: parse_color(&raw.status.debug, "status.debug")?,
            debug_bg: fallback.status.debug_bg,
            success: parse_color(&raw.status.success, "status.success")?,
            success_bg: fallback.status.success_bg,
        };

        let log = LogTableColors {
            row_hover: parse_color(&raw.log.row_hover, "log.row_hover")?,
            row_selected: parse_color(&raw.log.row_selected, "log.row_selected")?,
            row_highlight: parse_color(&raw.log.row_highlight, "log.row_highlight")?,
            row_highlight_hover: fallback.log.row_highlight_hover,
            term_highlight_bg: parse_color(&raw.log.term_highlight_bg, "log.term_highlight_bg")?,
            term_highlight_text: parse_color(
                &raw.log.term_highlight_text,
                "log.term_highlight_text",
            )?,
        };

        let mut controls = fallback.controls;
        if let Some(c) = raw.controls {
            if let Some(v) = c.restart_bg.as_deref().and_then(hex_to_color) {
                controls.restart_bg = v;
            }
            if let Some(v) = c.restart_border.as_deref().and_then(hex_to_color) {
                controls.restart_border = v;
            }
            if let Some(v) = c.stop_bg.as_deref().and_then(hex_to_color) {
                controls.stop_bg = v;
            }
            if let Some(v) = c.stop_border.as_deref().and_then(hex_to_color) {
                controls.stop_border = v;
            }
            if let Some(v) = c.latched_bg.as_deref().and_then(hex_to_color) {
                controls.latched_bg = v;
            }
            if let Some(v) = c.latched_border.as_deref().and_then(hex_to_color) {
                controls.latched_border = v;
            }
            if let Some(v) = c.unlatched_bg.as_deref().and_then(hex_to_color) {
                controls.unlatched_bg = v;
            }
            if let Some(v) = c.unlatched_border.as_deref().and_then(hex_to_color) {
                controls.unlatched_border = v;
            }
        }

        let mut ansi = fallback.ansi;
        if let Some(a) = raw.ansi {
            if let Some(v) = a.black.as_deref().and_then(hex_to_color) {
                ansi.black = v;
            }
            if let Some(v) = a.red.as_deref().and_then(hex_to_color) {
                ansi.red = v;
            }
            if let Some(v) = a.green.as_deref().and_then(hex_to_color) {
                ansi.green = v;
            }
            if let Some(v) = a.yellow.as_deref().and_then(hex_to_color) {
                ansi.yellow = v;
            }
            if let Some(v) = a.blue.as_deref().and_then(hex_to_color) {
                ansi.blue = v;
            }
            if let Some(v) = a.magenta.as_deref().and_then(hex_to_color) {
                ansi.magenta = v;
            }
            if let Some(v) = a.cyan.as_deref().and_then(hex_to_color) {
                ansi.cyan = v;
            }
            if let Some(v) = a.white.as_deref().and_then(hex_to_color) {
                ansi.white = v;
            }
            if let Some(v) = a.bright_black.as_deref().and_then(hex_to_color) {
                ansi.bright_black = v;
            }
            if let Some(v) = a.bright_red.as_deref().and_then(hex_to_color) {
                ansi.bright_red = v;
            }
            if let Some(v) = a.bright_green.as_deref().and_then(hex_to_color) {
                ansi.bright_green = v;
            }
            if let Some(v) = a.bright_yellow.as_deref().and_then(hex_to_color) {
                ansi.bright_yellow = v;
            }
            if let Some(v) = a.bright_blue.as_deref().and_then(hex_to_color) {
                ansi.bright_blue = v;
            }
            if let Some(v) = a.bright_magenta.as_deref().and_then(hex_to_color) {
                ansi.bright_magenta = v;
            }
            if let Some(v) = a.bright_cyan.as_deref().and_then(hex_to_color) {
                ansi.bright_cyan = v;
            }
            if let Some(v) = a.bright_white.as_deref().and_then(hex_to_color) {
                ansi.bright_white = v;
            }
        }

        let mut selection = fallback.selection;
        if let Some(s) = raw.selection {
            if let Some(v) = s.bg.as_deref().and_then(hex_to_color) {
                selection.bg = v;
            }
            if let Some(v) = s.stroke.as_deref().and_then(hex_to_color) {
                selection.stroke = v;
            }
        }

        Ok(Theme {
            id: raw.id,
            name: raw.name,
            appearance,
            surfaces,
            borders,
            text,
            selection,
            status,
            log,
            controls,
            ansi,
        })
    }

    /// Xuất thông số của Theme ra chuỗi định dạng JSON
    pub fn to_json(&self) -> Result<String> {
        let json_obj = ThemeJson {
            id: self.id.clone(),
            name: self.name.clone(),
            appearance: match self.appearance {
                Appearance::Dark => "dark".to_string(),
                Appearance::Light => "light".to_string(),
            },
            surfaces: SurfaceColorsJson {
                base: color_to_hex(self.surfaces.base),
                mantle: color_to_hex(self.surfaces.mantle),
                crust: color_to_hex(self.surfaces.crust),
                surface0: color_to_hex(self.surfaces.surface0),
                surface1: color_to_hex(self.surfaces.surface1),
            },
            borders: BorderColorsJson {
                border: color_to_hex(self.borders.border),
                border_subtle: color_to_hex(self.borders.border_subtle),
                border_focused: color_to_hex(self.borders.border_focused),
                border_selected: color_to_hex(self.borders.border_selected),
            },
            text: TextColorsJson {
                primary: color_to_hex(self.text.primary),
                muted: color_to_hex(self.text.muted),
                placeholder: color_to_hex(self.text.placeholder),
                accent: color_to_hex(self.text.accent),
            },
            status: StatusColorsJson {
                error: color_to_hex(self.status.error),
                warning: color_to_hex(self.status.warning),
                info: color_to_hex(self.status.info),
                debug: color_to_hex(self.status.debug),
                success: color_to_hex(self.status.success),
            },
            log: LogTableColorsJson {
                row_hover: color_to_hex(self.log.row_hover),
                row_selected: color_to_hex(self.log.row_selected),
                row_highlight: color_to_hex(self.log.row_highlight),
                term_highlight_bg: color_to_hex(self.log.term_highlight_bg),
                term_highlight_text: color_to_hex(self.log.term_highlight_text),
            },
            selection: Some(SelectionColorsJson {
                bg: Some(color_to_hex(self.selection.bg)),
                stroke: Some(color_to_hex(self.selection.stroke)),
            }),
            controls: Some(ControlColorsJson {
                restart_bg: Some(color_to_hex(self.controls.restart_bg)),
                restart_border: Some(color_to_hex(self.controls.restart_border)),
                stop_bg: Some(color_to_hex(self.controls.stop_bg)),
                stop_border: Some(color_to_hex(self.controls.stop_border)),
                latched_bg: Some(color_to_hex(self.controls.latched_bg)),
                latched_border: Some(color_to_hex(self.controls.latched_border)),
                unlatched_bg: Some(color_to_hex(self.controls.unlatched_bg)),
                unlatched_border: Some(color_to_hex(self.controls.unlatched_border)),
            }),
            ansi: Some(AnsiColorsJson {
                black: Some(color_to_hex(self.ansi.black)),
                red: Some(color_to_hex(self.ansi.red)),
                green: Some(color_to_hex(self.ansi.green)),
                yellow: Some(color_to_hex(self.ansi.yellow)),
                blue: Some(color_to_hex(self.ansi.blue)),
                magenta: Some(color_to_hex(self.ansi.magenta)),
                cyan: Some(color_to_hex(self.ansi.cyan)),
                white: Some(color_to_hex(self.ansi.white)),
                bright_black: Some(color_to_hex(self.ansi.bright_black)),
                bright_red: Some(color_to_hex(self.ansi.bright_red)),
                bright_green: Some(color_to_hex(self.ansi.bright_green)),
                bright_yellow: Some(color_to_hex(self.ansi.bright_yellow)),
                bright_blue: Some(color_to_hex(self.ansi.bright_blue)),
                bright_magenta: Some(color_to_hex(self.ansi.bright_magenta)),
                bright_cyan: Some(color_to_hex(self.ansi.bright_cyan)),
                bright_white: Some(color_to_hex(self.ansi.bright_white)),
            }),
        };

        serde_json::to_string_pretty(&json_obj).context("Lỗi serialize Theme sang JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::presets;

    #[test]
    fn test_theme_json_roundtrip() {
        let original = presets::nord_dimmed();
        let json_str = original.to_json().expect("Failed to export theme to JSON");
        assert!(json_str.contains("\"id\": \"nord-dimmed\""));

        let restored = Theme::from_json(&json_str).expect("Failed to parse theme from JSON");
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.surfaces.base, original.surfaces.base);
        assert_eq!(restored.text.primary, original.text.primary);
        assert_eq!(restored.selection.bg, original.selection.bg);
        assert_eq!(restored.selection.stroke, original.selection.stroke);
        assert_eq!(restored.status.error, original.status.error);
        assert_eq!(restored.log.row_selected, original.log.row_selected);
    }
}

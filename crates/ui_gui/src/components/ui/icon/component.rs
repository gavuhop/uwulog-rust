//! The reusable `Icon` component for egui layouts.
//!
//! Inspired by Zed's `crates/ui/src/components/icon.rs`.

use super::names::IconName;
use crate::theme;
use eframe::egui::{self, Color32, Response, Ui, Vec2};

/// Semantic sizing tokens for UI icons.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum IconSize {
    /// 10px - Micro badges & inline indicator dots
    Indicator,
    /// 12px - Dense tables & compact dropdowns
    XSmall,
    /// 14px - Secondary buttons, window controls & input prefixes
    Small,
    #[default]
    /// 16px - Standard toolbar buttons & primary UI icons
    Medium,
    /// 20px - Large headers & modal titles
    Large,
    /// 24px - Action buttons & hero icons
    XLarge,
    /// Custom pixel dimension
    Custom(f32),
}

impl IconSize {
    #[inline]
    pub fn px(self) -> f32 {
        match self {
            Self::Indicator => 10.0,
            Self::XSmall => 12.0,
            Self::Small => 14.0,
            Self::Medium => 16.0,
            Self::Large => 20.0,
            Self::XLarge => 24.0,
            Self::Custom(px) => px,
        }
    }
}

impl From<f32> for IconSize {
    fn from(px: f32) -> Self {
        Self::Custom(px)
    }
}

/// A fluent builder component for displaying an icon inside an `egui::Ui`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    name: IconName,
    size: IconSize,
    color: Option<Color32>,
}

impl Icon {
    pub fn new(name: IconName) -> Self {
        Self {
            name,
            size: IconSize::Medium,
            color: None,
        }
    }

    pub fn size(mut self, size: impl Into<IconSize>) -> Self {
        self.size = size.into();
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Renders the icon directly onto an existing `egui::Rect` using the current painter.
    pub fn paint(&self, ui: &mut Ui, rect: egui::Rect, fallback_color: Color32) {
        let draw_color = self.color.unwrap_or(fallback_color);
        self.name.paint(ui.painter(), rect, draw_color);
    }

    /// Allocates exact size for the icon and renders it within the `Ui`.
    pub fn show(self, ui: &mut Ui) -> Response {
        let px = self.size.px();
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(px), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let color = self.color.unwrap_or(theme::TEXT_PRIMARY);
            self.name.paint(ui.painter(), rect, color);
        }
        response
    }
}

impl From<IconName> for Icon {
    fn from(name: IconName) -> Self {
        Icon::new(name)
    }
}

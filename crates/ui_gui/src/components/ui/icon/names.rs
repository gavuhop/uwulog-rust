//! Strongly-typed icon identifiers and vector rendering routines for all UI icons.
//!
//! Directly inspired by Zed's `crates/icons/src/icons.rs`.

use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke};

/// All standardized in-app UI icons in uwulog-rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconName {
    // Actions & Feedback
    Check,
    Close,
    Dash,
    MagnifyingGlass,
    Copy,
    GripVertical,
    AlertTriangle,
    Rocket,

    // Navigation & Menus
    Menu,
    ChevronRight,
    ChevronDown,
    ExternalLink,

    // Stream & Engine Controls
    TableColumns,
    Settings,
    Stop,
    Restart,
    Camera,
    Anchor,
    Pause,
    Play,
    History,

    // Data Sources & Platforms
    Terminal,
    File,
    Linux,
}

impl IconName {
    /// Returns the file stem of the icon asset (e.g. "magnifying_glass").
    #[inline]
    pub const fn file_stem(&self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Close => "close",
            Self::Dash => "dash",
            Self::MagnifyingGlass => "magnifying_glass",
            Self::Copy => "copy",
            Self::GripVertical => "grip_vertical",
            Self::AlertTriangle => "alert_triangle",
            Self::Rocket => "rocket",
            Self::Menu => "menu",
            Self::ChevronRight => "chevron_right",
            Self::ChevronDown => "chevron_down",
            Self::ExternalLink => "external_link",
            Self::TableColumns => "table_columns",
            Self::Settings => "settings",
            Self::Stop => "stop",
            Self::Restart => "restart",
            Self::Camera => "camera",
            Self::Anchor => "anchor",
            Self::Pause => "pause",
            Self::Play => "play",
            Self::History => "history",
            Self::Terminal => "terminal",
            Self::File => "file",
            Self::Linux => "linux",
        }
    }

    /// All variants in the enum, useful for exhaustive testing and iteration.
    pub const ALL: &'static [IconName] = &[
        Self::Check,
        Self::Close,
        Self::Dash,
        Self::MagnifyingGlass,
        Self::Copy,
        Self::GripVertical,
        Self::AlertTriangle,
        Self::Rocket,
        Self::Menu,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::ExternalLink,
        Self::TableColumns,
        Self::Settings,
        Self::Stop,
        Self::Restart,
        Self::Camera,
        Self::Anchor,
        Self::Pause,
        Self::Play,
        Self::History,
        Self::Terminal,
        Self::File,
        Self::Linux,
    ];

    /// Returns the raw embedded SVG file contents compiled into the binary.
    pub const fn svg_content(&self) -> &'static str {
        match self {
            Self::Check => include_str!("../../../../assets/icons/check.svg"),
            Self::Close => include_str!("../../../../assets/icons/close.svg"),
            Self::Dash => include_str!("../../../../assets/icons/dash.svg"),
            Self::MagnifyingGlass => include_str!("../../../../assets/icons/magnifying_glass.svg"),
            Self::Copy => include_str!("../../../../assets/icons/copy.svg"),
            Self::GripVertical => include_str!("../../../../assets/icons/grip_vertical.svg"),
            Self::AlertTriangle => include_str!("../../../../assets/icons/alert_triangle.svg"),
            Self::Rocket => include_str!("../../../../assets/icons/rocket.svg"),
            Self::Menu => include_str!("../../../../assets/icons/menu.svg"),
            Self::ChevronRight => include_str!("../../../../assets/icons/chevron_right.svg"),
            Self::ChevronDown => include_str!("../../../../assets/icons/chevron_down.svg"),
            Self::ExternalLink => include_str!("../../../../assets/icons/external_link.svg"),
            Self::TableColumns => include_str!("../../../../assets/icons/table_columns.svg"),
            Self::Settings => include_str!("../../../../assets/icons/settings.svg"),
            Self::Stop => include_str!("../../../../assets/icons/stop.svg"),
            Self::Restart => include_str!("../../../../assets/icons/restart.svg"),
            Self::Camera => include_str!("../../../../assets/icons/camera.svg"),
            Self::Anchor => include_str!("../../../../assets/icons/anchor.svg"),
            Self::Pause => include_str!("../../../../assets/icons/pause.svg"),
            Self::Play => include_str!("../../../../assets/icons/play.svg"),
            Self::History => include_str!("../../../../assets/icons/history.svg"),
            Self::Terminal => include_str!("../../../../assets/icons/terminal.svg"),
            Self::File => include_str!("../../../../assets/icons/file.svg"),
            Self::Linux => include_str!("../../../../assets/icons/linux.svg"),
        }
    }

    /// Paints the vector geometry directly onto `painter` inside `rect`.
    ///
    /// This vector routine scales seamlessly and stays mathematically sharp at any DPI,
    /// with zero texture rasterization artifacts.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let w = rect.width();
        let h = rect.height();
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        let px = |val: f32| rect.min.x + (val / 16.0) * w;
        let py = |val: f32| rect.min.y + (val / 16.0) * h;
        let p = |x: f32, y: f32| Pos2::new(px(x), py(y));

        let stroke_w = ((1.3 / 16.0) * w).clamp(1.0, 2.5);
        let stroke = Stroke::new(stroke_w, color);

        match self {
            Self::Check => {
                let p1 = p(3.5, 8.5);
                let p2 = p(6.5, 11.5);
                let p3 = p(12.5, 4.5);
                painter.line_segment([p1, p2], stroke);
                painter.line_segment([p2, p3], stroke);
            }
            Self::Close => {
                let d = 4.0;
                let c = rect.center();
                let scale = w / 16.0;
                let offset = d * scale;
                painter.line_segment(
                    [
                        Pos2::new(c.x - offset, c.y - offset),
                        Pos2::new(c.x + offset, c.y + offset),
                    ],
                    stroke,
                );
                painter.line_segment(
                    [
                        Pos2::new(c.x + offset, c.y - offset),
                        Pos2::new(c.x - offset, c.y + offset),
                    ],
                    stroke,
                );
            }
            Self::Dash => {
                painter.line_segment([p(3.5, 8.0), p(12.5, 8.0)], stroke);
            }
            Self::MagnifyingGlass => {
                let center = p(6.8, 6.8);
                let r = (3.8 / 16.0) * w;
                painter.circle_stroke(center, r, stroke);
                painter.line_segment([p(9.6, 9.6), p(13.2, 13.2)], stroke);
            }
            Self::Settings => {
                let center = p(8.0, 8.0);
                let r = (2.4 / 16.0) * w;
                painter.circle_stroke(center, r, stroke);
                // 8 gear spokes
                let spokes = [
                    (p(8.0, 2.3), p(8.0, 3.8)),
                    (p(8.0, 12.2), p(8.0, 13.7)),
                    (p(2.3, 8.0), p(3.8, 8.0)),
                    (p(12.2, 8.0), p(13.7, 8.0)),
                    (p(4.0, 4.0), p(5.1, 5.1)),
                    (p(10.9, 10.9), p(12.0, 12.0)),
                    (p(4.0, 12.0), p(5.1, 10.9)),
                    (p(10.9, 5.1), p(12.0, 4.0)),
                ];
                for (a, b) in spokes {
                    painter.line_segment([a, b], stroke);
                }
            }
            Self::TableColumns => {
                let box_rect = Rect::from_min_max(p(2.5, 2.5), p(13.5, 13.5));
                let corner = CornerRadius::same(((1.5 / 16.0) * w) as u8);
                painter.rect_stroke(box_rect, corner, stroke, egui::StrokeKind::Inside);
                painter.line_segment([p(6.2, 2.5), p(6.2, 13.5)], stroke);
                painter.line_segment([p(9.8, 2.5), p(9.8, 13.5)], stroke);
            }
            Self::Stop => {
                let stop_rect = Rect::from_min_max(p(4.5, 4.5), p(11.5, 11.5));
                let corner = CornerRadius::same(((1.2 / 16.0) * w) as u8);
                painter.rect_filled(stop_rect, corner, color);
            }
            Self::Restart => {
                let center = p(8.0, 8.0);
                let r = (4.8 / 16.0) * w;
                let steps = 16;
                let start_angle = std::f32::consts::FRAC_PI_4;
                let sweep = std::f32::consts::PI * 1.6;
                for i in 0..steps {
                    let a1 = start_angle + (i as f32 / steps as f32) * sweep;
                    let a2 = start_angle + ((i + 1) as f32 / steps as f32) * sweep;
                    let p1 = Pos2::new(center.x + r * a1.cos(), center.y + r * a1.sin());
                    let p2 = Pos2::new(center.x + r * a2.cos(), center.y + r * a2.sin());
                    painter.line_segment([p1, p2], stroke);
                }
                // Arrow head at restart
                painter.line_segment([p(12.8, 3.2), p(12.8, 6.2)], stroke);
                painter.line_segment([p(12.8, 6.2), p(9.8, 6.2)], stroke);
            }
            Self::Camera => {
                let body = Rect::from_min_max(p(2.0, 4.5), p(14.0, 12.5));
                let corner = CornerRadius::same(((1.2 / 16.0) * w) as u8);
                painter.rect_stroke(body, corner, stroke, egui::StrokeKind::Inside);
                // Top bump
                painter.line_segment([p(5.0, 4.5), p(6.0, 3.0)], stroke);
                painter.line_segment([p(6.0, 3.0), p(10.0, 3.0)], stroke);
                painter.line_segment([p(10.0, 3.0), p(11.0, 4.5)], stroke);
                // Lens
                painter.circle_stroke(p(8.0, 8.5), (2.3 / 16.0) * w, stroke);
            }
            Self::Anchor => {
                painter.circle_stroke(p(8.0, 3.5), (1.5 / 16.0) * w, stroke);
                painter.line_segment([p(8.0, 5.0), p(8.0, 13.5)], stroke);
                painter.line_segment([p(5.5, 7.0), p(10.5, 7.0)], stroke);
                // Curved hook
                painter.line_segment([p(3.5, 9.5), p(3.5, 12.0)], stroke);
                painter.line_segment([p(3.5, 12.0), p(8.0, 13.5)], stroke);
                painter.line_segment([p(8.0, 13.5), p(12.5, 12.0)], stroke);
                painter.line_segment([p(12.5, 12.0), p(12.5, 9.5)], stroke);
            }
            Self::Pause => {
                let bar_w = ((2.2 / 16.0) * w).max(1.5);
                let bar_h = (9.0 / 16.0) * h;
                let corner = CornerRadius::same(((0.8 / 16.0) * w) as u8);

                let b1 = Rect::from_min_size(p(4.2, 3.5), egui::vec2(bar_w, bar_h));
                let b2 = Rect::from_min_size(p(9.6, 3.5), egui::vec2(bar_w, bar_h));
                painter.rect_filled(b1, corner, color);
                painter.rect_filled(b2, corner, color);
            }
            Self::Play => {
                let pts = [p(5.5, 3.8), p(12.2, 8.0), p(5.5, 12.2)];
                painter.add(egui::epaint::PathShape::convex_polygon(
                    pts.to_vec(),
                    color,
                    Stroke::NONE,
                ));
            }
            Self::History => {
                let center = p(8.0, 8.0);
                painter.circle_stroke(center, (5.2 / 16.0) * w, stroke);
                painter.line_segment([center, p(8.0, 4.8)], stroke);
                painter.line_segment([center, p(10.5, 9.2)], stroke);
            }
            Self::Copy => {
                let f1 = Rect::from_min_max(p(5.5, 5.5), p(13.2, 13.2));
                let corner = CornerRadius::same(((1.0 / 16.0) * w) as u8);
                painter.rect_stroke(f1, corner, stroke, egui::StrokeKind::Inside);
                painter.line_segment([p(3.5, 10.5), p(2.8, 10.5)], stroke);
                painter.line_segment([p(2.8, 10.5), p(2.8, 2.8)], stroke);
                painter.line_segment([p(2.8, 2.8), p(10.5, 2.8)], stroke);
                painter.line_segment([p(10.5, 2.8), p(10.5, 3.5)], stroke);
            }
            Self::Terminal => {
                let border = Rect::from_min_max(p(2.5, 2.5), p(13.5, 13.5));
                let corner = CornerRadius::same(((1.5 / 16.0) * w) as u8);
                painter.rect_stroke(border, corner, stroke, egui::StrokeKind::Inside);
                // > prompt
                painter.line_segment([p(5.0, 5.8), p(7.5, 8.0)], stroke);
                painter.line_segment([p(7.5, 8.0), p(5.0, 10.2)], stroke);
                // underscore
                painter.line_segment([p(8.5, 10.2), p(11.2, 10.2)], stroke);
            }
            Self::File => {
                let f = Rect::from_min_max(p(3.5, 2.5), p(12.5, 13.5));
                let corner = CornerRadius::same(((1.2 / 16.0) * w) as u8);
                painter.rect_stroke(f, corner, stroke, egui::StrokeKind::Inside);
                // folded corner indicator
                painter.line_segment([p(8.5, 2.5), p(8.5, 5.5)], stroke);
                painter.line_segment([p(8.5, 5.5), p(12.5, 5.5)], stroke);
            }
            Self::Menu => {
                painter.line_segment([p(2.8, 4.5), p(13.2, 4.5)], stroke);
                painter.line_segment([p(2.8, 8.0), p(13.2, 8.0)], stroke);
                painter.line_segment([p(2.8, 11.5), p(13.2, 11.5)], stroke);
            }
            Self::ChevronRight => {
                painter.line_segment([p(6.0, 4.0), p(10.0, 8.0)], stroke);
                painter.line_segment([p(10.0, 8.0), p(6.0, 12.0)], stroke);
            }
            Self::ChevronDown => {
                painter.line_segment([p(4.0, 6.0), p(8.0, 10.0)], stroke);
                painter.line_segment([p(8.0, 10.0), p(12.0, 6.0)], stroke);
            }
            Self::ExternalLink => {
                painter.line_segment([p(4.5, 11.5), p(11.5, 4.5)], stroke);
                painter.line_segment([p(7.0, 4.5), p(11.5, 4.5)], stroke);
                painter.line_segment([p(11.5, 4.5), p(11.5, 9.0)], stroke);
            }
            Self::GripVertical => {
                let r = ((1.1 / 16.0) * w).max(1.0);
                for &gx in &[p(6.0, 0.0).x, p(10.0, 0.0).x] {
                    for &gy in &[p(0.0, 4.0).y, p(0.0, 8.0).y, p(0.0, 12.0).y] {
                        painter.circle_filled(Pos2::new(gx, gy), r, color);
                    }
                }
            }
            Self::AlertTriangle => {
                painter.line_segment([p(8.0, 2.8), p(2.8, 12.8)], stroke);
                painter.line_segment([p(2.8, 12.8), p(13.2, 12.8)], stroke);
                painter.line_segment([p(13.2, 12.8), p(8.0, 2.8)], stroke);
                painter.line_segment([p(8.0, 6.2), p(8.0, 9.2)], stroke);
                painter.circle_filled(p(8.0, 11.0), ((0.9 / 16.0) * w).max(1.0), color);
            }
            Self::Rocket => {
                painter.line_segment([p(11.5, 2.5), p(7.0, 5.5)], stroke);
                painter.line_segment([p(7.0, 5.5), p(4.5, 8.0)], stroke);
                painter.line_segment([p(4.5, 8.0), p(4.0, 10.5)], stroke);
                painter.line_segment([p(4.0, 10.5), p(6.5, 10.0)], stroke);
                painter.line_segment([p(6.5, 10.0), p(9.0, 12.5)], stroke);
                painter.line_segment([p(9.0, 12.5), p(11.5, 9.0)], stroke);
                painter.line_segment([p(11.5, 9.0), p(13.5, 4.5)], stroke);
                painter.line_segment([p(13.5, 4.5), p(11.5, 2.5)], stroke);
                painter.circle_filled(p(10.0, 6.0), ((1.1 / 16.0) * w).max(1.0), color);
            }
            Self::Linux => {
                let r_head = ((2.0 / 16.0) * w).max(1.5);
                painter.circle_stroke(p(8.0, 5.0), r_head, stroke);
                painter.line_segment([p(7.0, 5.5), p(8.0, 6.5)], stroke);
                painter.line_segment([p(8.0, 6.5), p(9.0, 5.5)], stroke);
                painter.line_segment([p(6.0, 6.8), p(4.5, 11.0)], stroke);
                painter.line_segment([p(10.0, 6.8), p(11.5, 11.0)], stroke);
                painter.line_segment([p(4.5, 11.0), p(11.5, 11.0)], stroke);
                painter.line_segment([p(3.5, 13.0), p(6.5, 13.0)], stroke);
                painter.line_segment([p(9.5, 13.0), p(12.5, 13.0)], stroke);
            }
        }
    }
}

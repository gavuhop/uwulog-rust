//! Strongly-typed icon identifiers and vector rendering routines for all UI icons.
//!
//! Directly inspired by Zed's `crates/icons/src/icons.rs`.

use eframe::egui::{self, Color32, Rect, Stroke};

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
    Plus,
    Trash,
    Pencil,

    // Navigation & Menus
    Menu,
    ChevronRight,
    ChevronDown,
    ExternalLink,
    ArrowLeft,
    Return,
    ThisWindow,

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

    // Data Sources, Files & Platforms
    Terminal,
    File,
    Folder,
    FolderOpen,
    Linux,
    Server,
    Box,
    Screen,
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
            Self::Plus => "plus",
            Self::Trash => "trash",
            Self::Pencil => "pencil",
            Self::Menu => "menu",
            Self::ChevronRight => "chevron_right",
            Self::ChevronDown => "chevron_down",
            Self::ExternalLink => "external_link",
            Self::ArrowLeft => "arrow_left",
            Self::Return => "return",
            Self::ThisWindow => "this_window",
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
            Self::Folder => "folder",
            Self::FolderOpen => "folder_open",
            Self::Linux => "linux",
            Self::Server => "server",
            Self::Box => "box",
            Self::Screen => "screen",
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
        Self::Plus,
        Self::Trash,
        Self::Pencil,
        Self::Menu,
        Self::ChevronRight,
        Self::ChevronDown,
        Self::ExternalLink,
        Self::ArrowLeft,
        Self::Return,
        Self::ThisWindow,
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
        Self::Folder,
        Self::FolderOpen,
        Self::Linux,
        Self::Server,
        Self::Box,
        Self::Screen,
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
            Self::Plus => include_str!("../../../../assets/icons/plus.svg"),
            Self::Trash => include_str!("../../../../assets/icons/trash.svg"),
            Self::Pencil => include_str!("../../../../assets/icons/pencil.svg"),
            Self::Menu => include_str!("../../../../assets/icons/menu.svg"),
            Self::ChevronRight => include_str!("../../../../assets/icons/chevron_right.svg"),
            Self::ChevronDown => include_str!("../../../../assets/icons/chevron_down.svg"),
            Self::ExternalLink => include_str!("../../../../assets/icons/external_link.svg"),
            Self::ArrowLeft => include_str!("../../../../assets/icons/arrow_left.svg"),
            Self::Return => include_str!("../../../../assets/icons/return.svg"),
            Self::ThisWindow => include_str!("../../../../assets/icons/this_window.svg"),
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
            Self::Folder => include_str!("../../../../assets/icons/folder.svg"),
            Self::FolderOpen => include_str!("../../../../assets/icons/folder_open.svg"),
            Self::Linux => include_str!("../../../../assets/icons/linux.svg"),
            Self::Server => include_str!("../../../../assets/icons/server.svg"),
            Self::Box => include_str!("../../../../assets/icons/box.svg"),
            Self::Screen => include_str!("../../../../assets/icons/screen.svg"),
        }
    }

    /// Returns the static vector shapes compiled for this icon from its SVG asset.
    #[inline]
    pub const fn vector_shapes(&self) -> &'static [super::vector::VectorShape] {
        super::vector::get_icon_shapes(*self)
    }

    /// Paints the vector geometry directly onto `painter` inside `rect`.
    ///
    /// Vector geometry is pre-parsed from SVGs at compile-time by `build.rs`,
    /// guaranteeing zero runtime parsing overhead, zero GPU texture allocation,
    /// and mathematically sharp anti-aliased rendering at any DPI / scale.
    pub fn paint(&self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let w = rect.width();
        let h = rect.height();
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        let stroke_w = ((1.2 / 16.0) * w).clamp(1.0, 2.5);
        let stroke = Stroke::new(stroke_w, color);

        for shape in self.vector_shapes() {
            shape.paint(painter, rect, stroke, color);
        }
    }
}

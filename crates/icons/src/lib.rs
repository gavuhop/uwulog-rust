use serde::{Deserialize, Serialize};

pub mod component;
pub mod vector;

pub use component::{paint_icon, Icon, IconSize};
pub use vector::{get_icon_shapes, VectorShape};

/// All standardized in-app UI icons in uwulog-rust.
///
/// Directly inspired by Zed's `crates/icons/src/icons.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

    /// Returns relative asset path (e.g. "icons/magnifying_glass.svg").
    #[inline]
    pub const fn path(&self) -> &'static str {
        match self {
            Self::Check => "icons/check.svg",
            Self::Close => "icons/close.svg",
            Self::Dash => "icons/dash.svg",
            Self::MagnifyingGlass => "icons/magnifying_glass.svg",
            Self::Copy => "icons/copy.svg",
            Self::GripVertical => "icons/grip_vertical.svg",
            Self::AlertTriangle => "icons/alert_triangle.svg",
            Self::Rocket => "icons/rocket.svg",
            Self::Plus => "icons/plus.svg",
            Self::Trash => "icons/trash.svg",
            Self::Pencil => "icons/pencil.svg",

            Self::Menu => "icons/menu.svg",
            Self::ChevronRight => "icons/chevron_right.svg",
            Self::ChevronDown => "icons/chevron_down.svg",
            Self::ExternalLink => "icons/external_link.svg",
            Self::ArrowLeft => "icons/arrow_left.svg",
            Self::Return => "icons/return.svg",
            Self::ThisWindow => "icons/this_window.svg",

            Self::TableColumns => "icons/table_columns.svg",
            Self::Settings => "icons/settings.svg",
            Self::Stop => "icons/stop.svg",
            Self::Restart => "icons/restart.svg",
            Self::Camera => "icons/camera.svg",
            Self::Anchor => "icons/anchor.svg",
            Self::Pause => "icons/pause.svg",
            Self::Play => "icons/play.svg",
            Self::History => "icons/history.svg",

            Self::Terminal => "icons/terminal.svg",
            Self::File => "icons/file.svg",
            Self::Folder => "icons/folder.svg",
            Self::FolderOpen => "icons/folder_open.svg",
            Self::Linux => "icons/linux.svg",
            Self::Server => "icons/server.svg",
            Self::Box => "icons/box.svg",
            Self::Screen => "icons/screen.svg",
        }
    }

    /// Renders vector geometry of this icon directly onto `painter` inside `rect`.
    #[inline]
    pub fn paint(&self, painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
        paint_icon(*self, painter, rect, color);
    }

    /// Returns the static vector shape primitives for this icon.
    #[inline]
    pub fn vector_shapes(&self) -> &'static [VectorShape] {
        get_icon_shapes(*self)
    }

    /// All registered icon variants.
    pub const ALL: &'static [IconName] = &[
        IconName::Check,
        IconName::Close,
        IconName::Dash,
        IconName::MagnifyingGlass,
        IconName::Copy,
        IconName::GripVertical,
        IconName::AlertTriangle,
        IconName::Rocket,
        IconName::Plus,
        IconName::Trash,
        IconName::Pencil,
        IconName::Menu,
        IconName::ChevronRight,
        IconName::ChevronDown,
        IconName::ExternalLink,
        IconName::ArrowLeft,
        IconName::Return,
        IconName::ThisWindow,
        IconName::TableColumns,
        IconName::Settings,
        IconName::Stop,
        IconName::Restart,
        IconName::Camera,
        IconName::Anchor,
        IconName::Pause,
        IconName::Play,
        IconName::History,
        IconName::Terminal,
        IconName::File,
        IconName::Folder,
        IconName::FolderOpen,
        IconName::Linux,
        IconName::Server,
        IconName::Box,
        IconName::Screen,
    ];

    /// Find an icon by its file stem (e.g. "magnifying_glass").
    pub fn from_file_stem(stem: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|icon| icon.file_stem() == stem)
    }
}

impl std::fmt::Display for IconName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.file_stem())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_all_icons_exist_in_assets() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let assets_dir = PathBuf::from(manifest_dir).join("../../assets");

        for icon in IconName::ALL {
            let icon_path = assets_dir.join(icon.path());
            assert!(
                icon_path.exists(),
                "Icon {:?} does not exist at {:?}",
                icon,
                icon_path
            );
        }
    }

    #[test]
    fn test_no_dangling_icons_in_assets() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let icons_dir = PathBuf::from(manifest_dir).join("../../assets/icons");

        for entry in std::fs::read_dir(&icons_dir).expect("failed to read icons directory") {
            let entry = entry.expect("failed to read directory entry");
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "svg") {
                continue;
            }
            let file_stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("icon file name is not valid UTF-8");

            assert!(
                IconName::from_file_stem(file_stem).is_some(),
                "Found unmapped icon SVG in assets/icons: {}.svg",
                file_stem
            );
        }
    }

    #[test]
    fn test_icon_count() {
        assert_eq!(IconName::ALL.len(), 35);
    }

    #[test]
    fn test_icon_sizes() {
        assert_eq!(IconSize::Indicator.px(), 10.0);
        assert_eq!(IconSize::XSmall.px(), 12.0);
        assert_eq!(IconSize::Small.px(), 14.0);
        assert_eq!(IconSize::Medium.px(), 16.0);
        assert_eq!(IconSize::Large.px(), 20.0);
        assert_eq!(IconSize::XLarge.px(), 24.0);
        assert_eq!(IconSize::Custom(18.5).px(), 18.5);
    }

    #[test]
    fn test_all_icon_variants_have_compiled_vector_shapes() {
        for icon in IconName::ALL {
            let shapes = icon.vector_shapes();
            assert!(
                !shapes.is_empty(),
                "Icon {icon:?} must have at least one compiled VectorShape"
            );
        }
    }
}

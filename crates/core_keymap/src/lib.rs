//! Zed-Style 4-Layer Hierarchical Keybindings Manager for `uwulog`.
//!
//! Cung cấp engine quản lý phím tắt 4 tầng phân cấp độc lập:
//! - Tầng 1: Keystroke & Chord Parser (Độc lập UI framework, hỗ trợ adapter cho `egui`)
//! - Tầng 2: Context-Aware Resolution & Hierarchical Fall-through
//! - Tầng 3: Semantic Action Registry (Command Pattern)
//! - Tầng 4: User Configuration & Persistence (`keymap.json`)

pub mod action;
pub mod config;
pub mod context;
pub mod key;
pub mod keystroke;
pub mod manager;

pub use action::KeyAction;
pub use config::{
    default_config_path, ensure_sample_config_file, load_user_keymap, reset_user_keymap_file,
    save_user_keymap, KeymapConfigFile, KeymapSection,
};
pub use context::KeyContext;
pub use key::{format_key, parse_key, Key};
pub use keystroke::{Keystroke, KeystrokeParseError};
pub use manager::{KeyBinding, KeymapManager};

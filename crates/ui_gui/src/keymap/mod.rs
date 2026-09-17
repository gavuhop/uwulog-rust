//! Zed-Style 4-Layer Keybindings Manager for `uwulog`.

pub mod action;
pub mod config;
pub mod context;
pub mod keystroke;
pub mod manager;

pub use action::KeyAction;
pub use config::{
    default_config_path, ensure_sample_config_file, load_user_keymap, KeymapConfigFile,
};
pub use context::KeyContext;
pub use keystroke::{format_key, parse_key, Keystroke, KeystrokeParseError};
pub use manager::{KeyBinding, KeymapManager};

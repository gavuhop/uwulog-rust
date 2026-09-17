//! In-app UI Icon System for uwulog-rust.
//!
//! Inspired by Zed's UI icon architecture (`crates/icons` and `crates/ui/src/components/icon.rs`):
//! - Standardized SVG assets in `assets/icons/`.
//! - Strongly-typed `IconName` enum.
//! - Semantic sizing via `IconSize`.
//! - Reusable `Icon` builder component with crisp GPU vector rendering.

pub mod component;
pub mod names;
#[cfg(test)]
mod tests;
pub mod vector;

pub use component::{Icon, IconSize};
pub use names::IconName;
pub use vector::VectorShape;

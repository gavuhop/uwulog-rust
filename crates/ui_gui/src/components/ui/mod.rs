//! Reusable Zed-style UI Primitives: Button, Badge, Modal, Popover.

pub mod badge;
pub mod button;
pub mod card;
pub mod modal;
pub mod popover;

pub use badge::{CountBadge, StatusDot};
pub use button::{AppButton, ButtonVariant, IconButton, TabButton};
pub use card::render_card;
pub use modal::{ModalContainer, ModalResponse};
pub use popover::{PopoverContainer, PopoverResponse};

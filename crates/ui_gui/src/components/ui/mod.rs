//! Reusable Zed-style UI Primitives: Button, Badge, Modal, Popover.

pub mod badge;
pub mod button;
pub mod card;
pub mod icon;
pub mod input;
pub mod modal;
pub mod popover;

pub use badge::{CountBadge, StatusDot};
pub use button::{AppButton, ButtonIcon, ButtonVariant, IconButton, TabButton};
pub use card::render_card;
pub use icon::{Icon, IconName, IconSize};
pub use input::{AppInput, TextInput};
pub use modal::{ModalContainer, ModalResponse};
pub use popover::{PopoverContainer, PopoverResponse};

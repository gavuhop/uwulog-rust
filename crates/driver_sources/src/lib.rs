pub mod file_tailer;
pub mod journald;
pub mod normalizer;
pub mod process;
pub mod remote;
pub mod traits;
pub mod windows_event;
pub mod wsl;

pub use file_tailer::FileSource;
pub use journald::JournaldSource;
pub use normalizer::{extract_timestamp_from_text, LogNormalizer};
pub use process::ProcessSource;
pub use remote::RemoteSource;
pub use traits::LogSource;
pub use windows_event::WinEventSource;
pub use wsl::{WslSource, WslTargetMode};

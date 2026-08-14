pub mod file_tailer;
pub mod process;
pub mod traits;
pub mod windows_event;

pub use file_tailer::FileSource;
pub use process::ProcessSource;
pub use traits::{LogSource, RawLogEntry};
pub use windows_event::WinEventSource;

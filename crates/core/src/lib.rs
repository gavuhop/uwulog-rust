pub mod engine;
pub mod filter;
pub mod normalizer;
pub mod schema;
pub mod sources;

// Re-exports for convenient top-level usage
pub use engine::SystemEngine;
pub use filter::LogEngine;
pub use normalizer::{strip_ansi, LogNormalizer};
pub use schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};
pub use sources::{FileSource, JournaldSource, LogSource, ProcessSource, WinEventSource};

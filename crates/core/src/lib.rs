pub mod engine;
pub mod filter;
pub mod normalizer;
pub mod protocol;
pub mod schema;
pub mod sources;
pub mod transport;
pub mod workspace;

// Re-exports for convenient top-level usage
pub use engine::SystemEngine;
pub use filter::LogEngine;
pub use normalizer::{strip_ansi, LogNormalizer};
pub use protocol::{
    ClientEnvelope, FramedReader, FramedWriter, RemoteLogSourceSpec, ServerEnvelope,
};
pub use schema::{LogEvent, LogLevel, RawLogEntry, RawPayload};
pub use sources::{
    FileSource, JournaldSource, LogSource, ProcessSource, RemoteSource, WinEventSource, WslSource,
    WslTargetMode,
};
pub use transport::{ProcessTransport, RemoteTransport, SshTransport, WslTransport};
pub use workspace::{Workspace, WorkspaceLocation, WorkspaceStore};

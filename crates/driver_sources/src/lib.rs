pub mod file_tailer;
pub mod normalizer;
pub mod process;
pub mod remote;
pub mod traits;
pub mod wsl;

pub use file_tailer::FileSource;
pub use normalizer::LogNormalizer;
pub use process::ProcessSource;
pub use remote::RemoteSource;
pub use traits::LogSource;
pub use wsl::{WslSource, WslTargetMode};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;
use uwu_core_schema::{RawLogEntry, RawPayload};

pub(crate) fn spawn_line_reader<R: AsyncRead + Unpin + Send + 'static>(
    stream: Option<R>,
    tx: mpsc::Sender<RawLogEntry>,
) -> tokio::task::JoinHandle<()> {
    spawn_line_reader_mapped(stream, tx, |line| line)
}

pub(crate) fn spawn_line_reader_mapped<R, F>(
    stream: Option<R>,
    tx: mpsc::Sender<RawLogEntry>,
    mapper: F,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    F: Fn(String) -> String + Send + 'static,
{
    if let Some(s) = stream {
        tokio::spawn(async move {
            let mut reader = BufReader::new(s).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let entry = RawLogEntry {
                    payload: RawPayload::Text(mapper(line)),
                };
                if tx.send(entry).await.is_err() {
                    break;
                }
            }
        })
    } else {
        tokio::spawn(async {})
    }
}

use crate::schema::{RawLogEntry, RawPayload};
use crate::sources::traits::LogSource;
use anyhow::{Context, Result};
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

pub struct ProcessSource {
    command: String,
    args: Vec<String>,
    source_id: String,
}

impl ProcessSource {
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Self {
        let cmd = command.into();
        let source_id = if args.is_empty() {
            format!("proc:{}", cmd)
        } else {
            format!("proc:{} {}", cmd, args.join(" "))
        };

        Self {
            command: cmd,
            args,
            source_id,
        }
    }
}

#[async_trait]
impl LogSource for ProcessSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        let mut child = cmd.spawn().with_context(|| {
            format!("Failed to spawn process: {} {:?}", self.command, self.args)
        })?;

        // Trên Windows: Gán Child Process vào Windows JobObject để OS tự động Kill cả Cây Tiến Trình (Process Tree) khi uwu-log thoát
        #[cfg(target_os = "windows")]
        setup_windows_job_object(&child);

        let child_pid = child.id();
        let source_id_out = self.source_id.clone();
        let source_id_err = format!("{}:stderr", self.source_id);

        // Sử dụng WeakSender cho task đọc stdout/stderr để không làm tăng strong reference count của tx.
        // Nhờ đó, khi tx chính ở ngoài bị drop, tx.closed() ở task giám sát bên dưới sẽ trigger TỨC THÌ.
        let weak_tx_out = tx.downgrade();
        let weak_tx_err = tx.downgrade();

        // Task đọc stdout
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let entry = RawLogEntry {
                        source_id: source_id_out.clone(),
                        payload: RawPayload::Text(line),
                    };
                    if let Some(tx_out) = weak_tx_out.upgrade() {
                        if tx_out.send(entry).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });
        }

        // Task đọc stderr
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    let formatted_err = if line.to_uppercase().contains("ERROR") {
                        line
                    } else {
                        format!("[ERROR] {}", line)
                    };
                    let entry = RawLogEntry {
                        source_id: source_id_err.clone(),
                        payload: RawPayload::Text(formatted_err),
                    };
                    if let Some(tx_err) = weak_tx_err.upgrade() {
                        if tx_err.send(entry).await.is_err() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            });
        }

        // Task giám sát lifecycle: khi channel đóng (Stop/Restart hoăc thoát app), diệt TỨC THÌ Cây Tiến Trình (Process Tree)
        tokio::spawn(async move {
            tokio::select! {
                _ = child.wait() => {},
                _ = tx.closed() => {
                    #[cfg(target_os = "windows")]
                    if let Some(pid) = child_pid {
                        // Taskkill cưỡng chế diệt cả cây tiến trình con/cháu (Process Tree) trên Windows ngay lập tức
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/T", "/PID", &pid.to_string()])
                            .output();
                    }
                    #[cfg(not(target_os = "windows"))]
                    let _ = child_pid;
                    let _ = child.kill().await;
                }
            }
        });

        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn setup_windows_job_object(child: &tokio::process::Child) {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::*;

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if !job.is_null() {
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if let Some(raw_handle) = child.raw_handle() {
                AssignProcessToJobObject(job, raw_handle as HANDLE);
                let _ = job;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_source_echo() {
        let (tx, mut rx) = mpsc::channel(100);

        #[cfg(target_os = "windows")]
        let proc = ProcessSource::new(
            "cmd",
            vec!["/c".to_string(), "echo Hello ProcessSource".to_string()],
        );

        #[cfg(not(target_os = "windows"))]
        let proc = ProcessSource::new("echo", vec!["Hello ProcessSource".to_string()]);

        proc.start_stream(tx).await.unwrap();

        let mut received = Vec::new();
        while let Ok(Some(entry)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await
        {
            if let RawPayload::Text(text) = entry.payload {
                received.push(text);
                break;
            }
        }

        assert!(!received.is_empty());
        assert!(received[0].contains("Hello ProcessSource"));
    }

    #[tokio::test]
    async fn test_process_source_stderr_formatting() {
        let (tx, mut rx) = mpsc::channel(100);

        #[cfg(target_os = "windows")]
        let proc = ProcessSource::new(
            "cmd",
            vec![
                "/c".to_string(),
                "echo Critical stderr failure 1>&2".to_string(),
            ],
        );

        #[cfg(not(target_os = "windows"))]
        let proc = ProcessSource::new(
            "sh",
            vec![
                "-c".to_string(),
                "echo 'Critical stderr failure' >&2".to_string(),
            ],
        );

        proc.start_stream(tx).await.unwrap();

        let mut received = Vec::new();
        while let Ok(Some(entry)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await
        {
            if let RawPayload::Text(text) = entry.payload {
                received.push(text);
                break;
            }
        }

        assert!(!received.is_empty());
        assert!(received[0].contains("[ERROR]"));
        assert!(received[0].contains("Critical stderr failure"));
    }
}

use super::traits::LogSource;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use uwu_core_schema::RawLogEntry;
#[cfg(target_os = "windows")]
use uwu_core_schema::RawPayload;

pub struct WinEventSource {
    channel: String,
    source_id: String,
}

impl WinEventSource {
    pub fn new(channel: impl Into<String>) -> Self {
        let ch = channel.into();
        let source_id = format!("winevent:{}", ch);
        Self {
            channel: ch,
            source_id,
        }
    }
}

#[async_trait]
impl LogSource for WinEventSource {
    fn name(&self) -> &str {
        &self.source_id
    }

    async fn start_stream(&self, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            let channel = self.channel.clone();
            tokio::spawn(async move {
                if let Err(e) = run_win_event_stream(channel, tx).await {
                    log::error!("WinEventSource error: {:?}", e);
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (&tx, &self.channel, &self.source_id);
            log::warn!("WinEventSource is only supported natively on Windows.");
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
async fn run_win_event_stream(channel: String, tx: mpsc::Sender<RawLogEntry>) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::EventLog::*;

    let wide_channel: Vec<u16> = OsStr::new(&channel)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // 1. Đăng ký nhận sự kiện liên tục (live stream) qua EvtSubscribe
    let sub_handle = unsafe {
        EvtSubscribe(
            0,
            std::ptr::null_mut(),
            wide_channel.as_ptr(),
            std::ptr::null(),
            0,
            std::ptr::null_mut(),
            None,
            EvtSubscribeStartAtOldestRecord,
        )
    };

    let handle = if sub_handle != 0 {
        sub_handle
    } else {
        // Fallback sang EvtQuery nếu EvtSubscribe không khả dụng
        let query = unsafe {
            EvtQuery(
                0,
                wide_channel.as_ptr(),
                std::ptr::null(),
                EvtQueryChannelPath | EvtQueryReverseDirection,
            )
        };
        if query == 0 {
            anyhow::bail!(
                "Failed to subscribe or query Windows Event channel: {}",
                channel
            );
        }
        query
    };

    let mut events: [isize; 10] = [0; 10];
    let mut returned: u32 = 0;

    loop {
        let status = unsafe {
            EvtNext(
                handle,
                events.len() as u32,
                events.as_mut_ptr(),
                100, // short timeout ms
                0,
                &mut returned,
            )
        };

        if status != 0 && returned > 0 {
            for &evt_handle in events.iter().take(returned as usize) {
                if let Some(xml_str) = render_event_xml(evt_handle) {
                    let entry = RawLogEntry {
                        payload: RawPayload::Text(xml_str),
                    };
                    if tx.send(entry).await.is_err() {
                        unsafe {
                            EvtClose(evt_handle);
                            EvtClose(handle);
                        };
                        return Ok(());
                    }
                }
                unsafe { EvtClose(evt_handle) };
            }
        } else {
            // Nghỉ ngắn trước khi poll lượt tiếp theo
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }
}

#[cfg(target_os = "windows")]
fn render_event_xml(event_handle: isize) -> Option<String> {
    use windows_sys::Win32::System::EventLog::*;

    let mut buffer_used: u32 = 0;
    let mut property_count: u32 = 0;

    unsafe {
        EvtRender(
            0,
            event_handle,
            EvtRenderEventXml,
            0,
            std::ptr::null_mut(),
            &mut buffer_used,
            &mut property_count,
        );
    }

    if buffer_used == 0 {
        return None;
    }

    let mut buffer: Vec<u16> = vec![0; (buffer_used / 2) as usize];
    let status = unsafe {
        EvtRender(
            0,
            event_handle,
            EvtRenderEventXml,
            buffer_used,
            buffer.as_mut_ptr() as *mut _,
            &mut buffer_used,
            &mut property_count,
        )
    };

    if status != 0 {
        // Strip trailing nulls
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        String::from_utf16(&buffer).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_win_event_source_naming() {
        let src = WinEventSource::new("Application");
        assert_eq!(src.name(), "winevent:Application");

        let src_sec = WinEventSource::new("Security");
        assert_eq!(src_sec.name(), "winevent:Security");
    }
}

use crate::traits::LogSource;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;
use uwu_schema::{RawLogEntry, RawPayload};

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
        let channel = self.channel.clone();
        let source_id = self.source_id.clone();

        #[cfg(target_os = "windows")]
        {
            tokio::spawn(async move {
                if let Err(e) = run_win_event_stream(channel, source_id, tx).await {
                    log::error!("WinEventSource error: {:?}", e);
                }
            });
        }

        #[cfg(not(target_os = "windows"))]
        {
            log::warn!("WinEventSource is only supported natively on Windows.");
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
async fn run_win_event_stream(
    channel: String,
    source_id: String,
    tx: mpsc::Sender<RawLogEntry>,
) -> Result<()> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::System::EventLog::*;

    let wide_channel: Vec<u16> = OsStr::new(&channel)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Query các bản ghi hiện tại và lắng nghe sự kiện mới
    let query_handle = unsafe {
        EvtQuery(
            0,
            wide_channel.as_ptr(),
            std::ptr::null(),
            EvtQueryChannelPath | EvtQueryReverseDirection,
        )
    };

    if query_handle == 0 {
        anyhow::bail!("Failed to EvtQuery Windows Event channel: {}", channel);
    }

    let mut events: [isize; 10] = [0; 10];
    let mut returned: u32 = 0;

    loop {
        let status = unsafe {
            EvtNext(
                query_handle,
                events.len() as u32,
                events.as_mut_ptr(),
                50, // short timeout ms
                0,
                &mut returned,
            )
        };

        if status != 0 && returned > 0 {
            for i in 0..returned as usize {
                let evt_handle = events[i];
                if let Some(xml_str) = render_event_xml(evt_handle) {
                    let entry = RawLogEntry {
                        source_id: source_id.clone(),
                        payload: RawPayload::Text(xml_str),
                    };
                    if tx.send(entry).await.is_err() {
                        unsafe { EvtClose(query_handle) };
                        return Ok(());
                    }
                }
                unsafe { EvtClose(evt_handle) };
            }
        } else {
            // Nghỉ ngắn trước khi đọc lượt tiếp theo
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

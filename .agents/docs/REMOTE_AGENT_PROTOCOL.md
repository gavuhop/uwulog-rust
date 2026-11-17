# Giao thức Remote Agent & Tầng Transport (`REMOTE_AGENT_PROTOCOL.md`)

Tài liệu này mô tả đặc tả giao thức RPC đóng khung nhị phân (`uwu-core-protocol`), tiến trình headless `uwu-agent` (`uwu-cli-agent`), và các bộ chuyển tiếp (`uwu-driver-transport` + `uwu-driver-sources`).

---

## 1. Kiến trúc Phân lớp Remote Streaming

| Tầng | Crate / File | Trách nhiệm |
| :--- | :--- | :--- |
| **Tier 0** | [crates/core_protocol/src/lib.rs](../../crates/core_protocol/src/lib.rs) | Định nghĩa `AGENT_READY_MARKER`, `MAX_FRAME_SIZE`, `RemoteLogSourceSpec`, `ClientEnvelope`, `ServerEnvelope`, `FramedReader`, `FramedWriter`, và `wait_for_ready_marker`. |
| **Tier 1** | [crates/driver_transport/src/lib.rs](../../crates/driver_transport/src/lib.rs) | Trait `RemoteTransport` (`spawn_proxy()`), triển khai cho `ProcessTransport`, `WslTransport` (`WslArch`), `SshTransport` (interactive ask-pass session), và bộ phân tích `~/.ssh/config` (`ssh_config.rs`). |
| **Tier 1** | [crates/driver_sources/src/remote.rs](../../crates/driver_sources/src/remote.rs) | `RemoteSource` quản lý bắt tay marker, heartbeat `Ping` 5s, và chuyển tiếp `ServerEnvelope::LogBatch` vào kênh `mpsc::Sender<RawLogEntry>`. |
| **Tier 4** | [crates/cli_agent/src/main.rs](../../crates/cli_agent/src/main.rs) | Binary `uwu-agent` hỗ trợ `proxy`, `version`, và standalone mode (`-c`, `-f`, `-d`, `--`). |

---

## 2. Đặc tả Khung Truyền & Bắt tay (Framing & Handshake)

1. **Ready Handshake Marker**:
   - `AGENT_READY_MARKER = b"__UWU_AGENT_READY__\n"`
   - Khi chạy `uwu-agent proxy`, agent phát marker lên `stdout` ngay sau khi thiết lập xong diagnostic hook trên `stderr`.
   - Phía client gọi `wait_for_ready_marker(&mut reader)` (timeout `15s`, giới hạn preamble `MAX_PREAMBLE_BYTES = 1 MB`) để loại bỏ toàn bộ MOTD, banner SSH hoặc text từ `.bashrc` trước khi đọc frame nhị phân.
2. **Định dạng Frame**:
   - `[4-byte u32 Big-Endian Length][JSON UTF-8 Payload]`
   - Giới hạn tối đa: `MAX_FRAME_SIZE = 64 * 1024 * 1024` (`64 MB`).
3. **Thông điệp Điều khiển (`ClientEnvelope`)**:
   - `Ping { seq: u64 }`
   - `StartStream { spec: RemoteLogSourceSpec, filter_pushdown: Option<String> }` (`Command(String)` hoặc `File(String)`)
   - `PauseStream`, `ResumeStream`, `StopStream`
4. **Thông điệp Phản hồi (`ServerEnvelope`)**:
   - `Pong { seq: u64 }`
   - `LogBatch(Vec<RawLogEntry>)` (gom tối đa 256 entry hoặc tick mỗi `50ms`)
   - `Error { message: String }`
   - `Terminated { exit_code: Option<i32> }`

---

## 3. Kiểm thử Giao thức & Loopback

```bash
# Kiểm thử unit test giao thức (bao gồm SSH banner/MOTD preamble skip)
./.agents/scripts/harness.sh test-tier 0

# Kiểm thử trực tiếp binary `uwu-agent proxy` qua stdio pipe
python3 .agents/skills/uwu-remote-agent-debug/scripts/test_agent_loopback.py
```

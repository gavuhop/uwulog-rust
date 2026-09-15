use std::ffi::OsStr;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000_u32;

/// Tạo một `std::process::Command` với cấu hình chuẩn Zed UI:
/// Trên Windows, tự động gắn cờ `CREATE_NO_WINDOW` để không bật cửa sổ terminal đen khi gọi từ ứng dụng GUI.
#[inline]
pub fn new_std_command(program: impl AsRef<OsStr>) -> std::process::Command {
    let mut cmd = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Tạo một `tokio::process::Command` với cấu hình chuẩn Zed UI:
/// Trên Windows, tự động gắn cờ `CREATE_NO_WINDOW` để không bật cửa sổ terminal đen khi chạy lệnh lấy log/stream.
#[inline]
pub fn new_tokio_command(program: impl AsRef<OsStr>) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

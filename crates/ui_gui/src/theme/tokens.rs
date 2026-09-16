use eframe::egui::Color32;

/// Nhóm màu bề mặt phân tầng (Grounded / Elevated surfaces)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceColors {
    /// Nền cơ bản của ứng dụng và bảng log chính (ví dụ: #14161f)
    pub base: Color32,
    /// Nền thanh tiêu đề, sidebar, header, popups, modals (ví dụ: #1a1d28)
    pub mantle: Color32,
    /// Nền tối sâu nhất cho ô nhập liệu, console code block (ví dụ: #0e0f16)
    pub crust: Color32,
    /// Bề mặt nền phụ, viền thẻ hoặc phân cách cấp 1 (ví dụ: #282c3c)
    pub surface0: Color32,
    /// Bề mặt nổi (Elevated surface), hover nút bấm, context popup (ví dụ: #343a4e)
    pub surface1: Color32,
}

/// Nhóm màu đường viền (Borders & Dividers)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BorderColors {
    /// Viền phân cách thông thường
    pub border: Color32,
    /// Viền mờ/tế nhị phân cách các mục nhỏ
    pub border_subtle: Color32,
    /// Viền nổi bật khi một phần tử được focus hoặc đang thao tác
    pub border_focused: Color32,
    /// Viền khi phần tử được chọn (selected)
    pub border_selected: Color32,
}

/// Nhóm màu chữ (Typography)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextColors {
    /// Màu chữ nội dung chính, dễ đọc, êm mắt
    pub primary: Color32,
    /// Màu chữ thứ cấp, timestamp, nhãn phụ, đơn vị đo
    pub muted: Color32,
    /// Màu placeholder mờ trong ô tìm kiếm hoặc ô nhập liệu
    pub placeholder: Color32,
    /// Màu chữ điểm nhấn (Key accent), shortcut, tab active
    pub accent: Color32,
}

/// Nhóm màu bôi đen chọn văn bản (Text Selection)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionColors {
    pub bg: Color32,
    pub stroke: Color32,
}

/// Nhóm màu biểu thị trạng thái hệ thống (Status & Diagnostics)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusColors {
    pub error: Color32,
    pub error_bg: Color32,
    pub warning: Color32,
    pub warning_bg: Color32,
    pub info: Color32,
    pub info_bg: Color32,
    pub debug: Color32,
    pub debug_bg: Color32,
    pub success: Color32,
    pub success_bg: Color32,
}

/// Nhóm màu chuyên biệt cho bảng hiển thị log stream (Log Table & Search Highlights)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogTableColors {
    /// Nền khi rê chuột qua hàng log
    pub row_hover: Color32,
    /// Nền khi một hàng log đang được click chọn
    pub row_selected: Color32,
    /// Nền khi hàng log được đánh dấu / ghim / lọc
    pub row_highlight: Color32,
    /// Nền khi rê chuột qua hàng đang được ghim
    pub row_highlight_hover: Color32,
    /// Nền highlight từ khóa tìm kiếm
    pub term_highlight_bg: Color32,
    /// Màu chữ highlight từ khóa tìm kiếm
    pub term_highlight_text: Color32,
}

/// Nhóm màu các nút bấm hành động đặc thù (Action Controls)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlColors {
    pub restart_bg: Color32,
    pub restart_border: Color32,
    pub stop_bg: Color32,
    pub stop_border: Color32,
    pub latched_bg: Color32,
    pub latched_border: Color32,
    pub unlatched_bg: Color32,
    pub unlatched_border: Color32,
}

/// 16 màu ANSI tiêu chuẩn cho luồng terminal log
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnsiColors {
    pub black: Color32,
    pub red: Color32,
    pub green: Color32,
    pub yellow: Color32,
    pub blue: Color32,
    pub magenta: Color32,
    pub cyan: Color32,
    pub white: Color32,
    pub bright_black: Color32,
    pub bright_red: Color32,
    pub bright_green: Color32,
    pub bright_yellow: Color32,
    pub bright_blue: Color32,
    pub bright_magenta: Color32,
    pub bright_cyan: Color32,
    pub bright_white: Color32,
}

impl AnsiColors {
    /// Lấy màu tương ứng từ mã ANSI SGR (30..=37 hoặc 90..=97)
    pub fn color_for_code(&self, code: u32, default_color: Color32) -> Color32 {
        match code {
            0 | 39 => default_color,
            30 => self.black,
            31 => self.red,
            32 => self.green,
            33 => self.yellow,
            34 => self.blue,
            35 => self.magenta,
            36 => self.cyan,
            37 => self.white,
            90 => self.bright_black,
            91 => self.bright_red,
            92 => self.bright_green,
            93 => self.bright_yellow,
            94 => self.bright_blue,
            95 => self.bright_magenta,
            96 => self.bright_cyan,
            97 => self.bright_white,
            _ => default_color,
        }
    }
}

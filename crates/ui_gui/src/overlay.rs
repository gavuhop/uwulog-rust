/// Các tầng hiển thị nổi (Overlay / Modal / Popup) theo mô hình Navigation Stack & State Machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLayer {
    /// Menu chính (☰)
    MainMenu,
    /// Submenu chọn Theme bên phải
    ThemeSubmenu,
    /// Modal thông tin ứng dụng About
    AboutModal,
    /// Modal cấu hình nguồn dữ liệu khởi chạy
    LaunchModal,
    /// Bộ chọn Project / Workspace phong cách Zed
    ProjectPicker,
    /// Modal tùy biến cột và thứ tự hiển thị
    ColumnsModal,
    /// Modal chọn Remote Server / WSL Distro theo phong cách Zed
    RemoteServersModal,
}

/// Vị trí hiển thị của Remote Servers Modal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RemoteModalPlacement {
    /// Khi bấm vào server trên header bên trái -> hiển thị ở bên trái kế bên nút
    TopLeft,
    /// Khi bấm Open Remote (từ Project Picker hoặc Command Palette) -> hiển thị ở giữa trên màn hình
    #[default]
    TopCenter,
}

/// Ngăn xếp điều hướng tầng hiển thị nổi theo cơ chế LIFO
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct OverlayStack {
    layers: Vec<OverlayLayer>,
}

impl OverlayStack {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Kiểm tra xem một layer có đang mở trên stack hay không
    #[inline]
    pub fn is_open(&self, layer: OverlayLayer) -> bool {
        self.layers.contains(&layer)
    }

    /// Đẩy một layer mới vào đỉnh ngăn xếp nếu chưa có
    pub fn push(&mut self, layer: OverlayLayer) {
        if !self.is_open(layer) {
            self.layers.push(layer);
        }
    }

    /// Đóng một layer cụ thể khỏi ngăn xếp
    pub fn close(&mut self, layer: OverlayLayer) {
        self.layers.retain(|l| *l != layer);
    }

    /// Đóng toàn bộ hệ thống menu chính và submenu liên quan
    pub fn close_main_menu(&mut self) {
        self.close(OverlayLayer::MainMenu);
        self.close(OverlayLayer::ThemeSubmenu);
    }

    /// Lấy layer trên đỉnh ngăn xếp (LIFO) mà không gỡ bỏ
    #[inline]
    pub fn top(&self) -> Option<OverlayLayer> {
        self.layers.last().copied()
    }

    /// Lấy và gỡ layer trên đỉnh ngăn xếp (LIFO)
    pub fn pop(&mut self) -> Option<OverlayLayer> {
        self.layers.pop()
    }

    /// Kiểm tra ngăn xếp có rỗng không
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Xóa toàn bộ layer trong ngăn xếp
    pub fn clear(&mut self) {
        self.layers.clear();
    }
}

impl std::ops::Deref for OverlayStack {
    type Target = Vec<OverlayLayer>;

    fn deref(&self) -> &Self::Target {
        &self.layers
    }
}

impl std::ops::DerefMut for OverlayStack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.layers
    }
}

impl From<Vec<OverlayLayer>> for OverlayStack {
    fn from(layers: Vec<OverlayLayer>) -> Self {
        Self { layers }
    }
}

impl FromIterator<OverlayLayer> for OverlayStack {
    fn from_iter<I: IntoIterator<Item = OverlayLayer>>(iter: I) -> Self {
        Self {
            layers: iter.into_iter().collect(),
        }
    }
}

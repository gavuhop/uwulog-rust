//! Window resize border detection for frameless (borderless) windows.
//! Automatically coordinates with window controls on Windows, macOS, and Linux
//! to ensure grab handles never conflict with buttons or traffic lights.

use super::{linux, macos, windows, WindowControlsPlatform, TITLEBAR_HEIGHT};
use eframe::egui;
/// Độ dày viền kéo dãn (6px)
const BORDER_THICKNESS: f32 = 6.0;
/// Kích thước góc kéo dãn (14px)
const CORNER_SIZE: f32 = 14.0;

/// Hỗ trợ kéo dãn / thu nhỏ cửa sổ tùy ý từ 4 góc và 4 cạnh viền màn hình (Edge & Corner Resizing).
/// Tự động chừa vùng an toàn cho caption buttons (Windows/Linux) hoặc traffic lights (macOS).
pub fn render_window_resize_borders(ctx: &egui::Context) {
    let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if is_maximized {
        return;
    }

    let screen_rect = ctx.viewport_rect();
    let platform = WindowControlsPlatform::current();
    let is_mac = platform == WindowControlsPlatform::MacOs;

    let caption_controls_width = match platform {
        WindowControlsPlatform::Windows => windows::total_width(),
        WindowControlsPlatform::Linux => linux::total_width(),
        WindowControlsPlatform::MacOs => 0.0,
    };

    let mut corners_and_edges = Vec::with_capacity(8);

    // 1. Các góc
    if !is_mac {
        // Windows / Linux: Cho phép resize góc trên-trái (NorthWest)
        corners_and_edges.push((
            egui::Rect::from_min_max(
                screen_rect.min,
                screen_rect.min + egui::vec2(CORNER_SIZE, CORNER_SIZE),
            ),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNorthWest,
        ));
    } else {
        // macOS: Cho phép resize góc trên-phải (NorthEast) vì phía bên phải không có nút Close
        corners_and_edges.push((
            egui::Rect::from_min_max(
                egui::pos2(screen_rect.max.x - CORNER_SIZE, screen_rect.min.y),
                egui::pos2(screen_rect.max.x, screen_rect.min.y + CORNER_SIZE),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNorthEast,
        ));
    }

    // Hai góc dưới luôn resize được trên mọi hệ điều hành:
    corners_and_edges.push((
        egui::Rect::from_min_max(
            egui::pos2(screen_rect.min.x, screen_rect.max.y - CORNER_SIZE),
            egui::pos2(screen_rect.min.x + CORNER_SIZE, screen_rect.max.y),
        ),
        egui::ResizeDirection::SouthWest,
        egui::CursorIcon::ResizeSouthWest,
    ));
    corners_and_edges.push((
        egui::Rect::from_min_max(
            screen_rect.max - egui::vec2(CORNER_SIZE, CORNER_SIZE),
            screen_rect.max,
        ),
        egui::ResizeDirection::SouthEast,
        egui::CursorIcon::ResizeSouthEast,
    ));

    // 2. Bốn cạnh viền
    // Cạnh trên (North): dừng lại trước khu vực caption controls hoặc traffic lights
    let north_min_x = if is_mac {
        screen_rect.min.x + macos::safe_width() // Bỏ qua khu vực Traffic Lights ở góc trên bên trái
    } else {
        screen_rect.min.x + CORNER_SIZE
    };
    let north_max_x = if is_mac {
        screen_rect.max.x - CORNER_SIZE
    } else {
        screen_rect.max.x - caption_controls_width // Bỏ qua caption buttons ở góc trên bên phải
    };
    if north_max_x > north_min_x {
        corners_and_edges.push((
            egui::Rect::from_min_max(
                egui::pos2(north_min_x, screen_rect.min.y),
                egui::pos2(north_max_x, screen_rect.min.y + BORDER_THICKNESS),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeNorth,
        ));
    }

    // Cạnh dưới (South):
    corners_and_edges.push((
        egui::Rect::from_min_max(
            egui::pos2(
                screen_rect.min.x + CORNER_SIZE,
                screen_rect.max.y - BORDER_THICKNESS,
            ),
            egui::pos2(screen_rect.max.x - CORNER_SIZE, screen_rect.max.y),
        ),
        egui::ResizeDirection::South,
        egui::CursorIcon::ResizeSouth,
    ));

    // Cạnh trái (West):
    let west_min_y = if is_mac {
        screen_rect.min.y + TITLEBAR_HEIGHT // Bắt đầu từ dưới Traffic Lights
    } else {
        screen_rect.min.y + CORNER_SIZE
    };
    corners_and_edges.push((
        egui::Rect::from_min_max(
            egui::pos2(screen_rect.min.x, west_min_y),
            egui::pos2(
                screen_rect.min.x + BORDER_THICKNESS,
                screen_rect.max.y - CORNER_SIZE,
            ),
        ),
        egui::ResizeDirection::West,
        egui::CursorIcon::ResizeWest,
    ));

    // Cạnh phải (East):
    let east_min_y = if is_mac {
        screen_rect.min.y + CORNER_SIZE
    } else {
        screen_rect.min.y + TITLEBAR_HEIGHT // Bắt đầu từ dưới caption buttons
    };
    corners_and_edges.push((
        egui::Rect::from_min_max(
            egui::pos2(screen_rect.max.x - BORDER_THICKNESS, east_min_y),
            egui::pos2(screen_rect.max.x, screen_rect.max.y - CORNER_SIZE),
        ),
        egui::ResizeDirection::East,
        egui::CursorIcon::ResizeEast,
    ));

    let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
    let pointer_pressed = ctx.input(|i| i.pointer.primary_pressed());

    if let Some(pos) = pointer_pos {
        for (rect, direction, cursor) in corners_and_edges {
            if rect.contains(pos) {
                ctx.set_cursor_icon(cursor);
                if pointer_pressed {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
                break;
            }
        }
    }
}

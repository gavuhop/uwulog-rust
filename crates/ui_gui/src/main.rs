#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod actions;
pub mod app;
pub mod cli;
pub mod components;
pub mod overlay;
pub mod session;
pub mod state;
pub mod theme;
pub mod views;

use app::UwuGuiApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let handle = rt.handle().clone();
    let _guard = rt.enter();

    let icon_data = eframe::icon_data::from_png_bytes(include_bytes!("../../../avatar.png"))
        .unwrap_or_else(|_| egui::IconData {
            rgba: vec![0; 4],
            width: 1,
            height: 1,
        });

    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Uwu Log")
            .with_decorations(false)
            .with_resizable(true)
            .with_icon(std::sync::Arc::new(icon_data)),
        ..Default::default()
    };

    let _res = eframe::run_native(
        "Uwu Log Viewer",
        native_options,
        Box::new(move |cc| Ok(Box::new(UwuGuiApp::new(cc, handle)))),
    );

    // Tắt Tokio runtime ngay lập tức (không chờ task) để tránh treo UI khi đóng ứng dụng
    rt.shutdown_background();

    // Force exit: đảm bảo process ứng dụng thoát hoàn toàn, không bị treo bởi bất kỳ thread nào
    std::process::exit(0);
}

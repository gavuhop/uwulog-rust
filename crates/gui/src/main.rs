mod app;
mod ui;

use app::UwuGuiApp;
use eframe::NativeOptions;

fn main() -> eframe::Result<()> {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let handle = rt.handle().clone();
    let _guard = rt.enter();

    let native_options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("🐱 Uwu Log Viewer GUI"),
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

//! ImageView 主程序入口。

// release 构建为 GUI 子系统：双击启动不弹黑色控制台窗口；
// debug 构建保留控制台，方便看日志/诊断输出
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod loader;
mod render;
mod ui;

fn main() -> eframe::Result<()> {
    // 命令行参数：可选的初始文件路径（文件关联 / 拖到 exe 上打开）
    let initial_path = std::env::args()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file());

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 860.0])
        .with_min_inner_size([880.0, 560.0]);
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "LcL ImageView",
        options,
        Box::new(move |cc| Box::new(app::App::new(cc, initial_path))),
    )
}

/// 解码内置 PNG 图标（窗口标题栏 / Alt+Tab），失败时返回 None（使用系统默认图标）。
fn load_window_icon() -> Option<std::sync::Arc<eframe::egui::IconData>> {
    let png = include_bytes!("../assets/icon.png");
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .into_rgba8();
    let (width, height) = img.dimensions();
    Some(std::sync::Arc::new(eframe::egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    }))
}

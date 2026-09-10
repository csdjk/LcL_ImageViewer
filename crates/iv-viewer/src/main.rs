//! ImageView 主程序入口。

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

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 860.0])
            .with_min_inner_size([880.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "LcL ImageView",
        options,
        Box::new(move |cc| Box::new(app::App::new(cc, initial_path))),
    )
}

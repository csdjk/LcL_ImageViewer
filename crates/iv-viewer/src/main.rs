//! ImageView 主程序入口。

// release 构建为 GUI 子系统：双击启动不弹黑色控制台窗口；
// debug 构建保留控制台，方便看日志/诊断输出
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backdrop;
mod loader;
mod render;
mod ui;
mod winassoc;

fn main() -> eframe::Result<()> {
    // 命令行参数：可选的初始文件路径（文件关联 / 拖到 exe 上打开）
    let initial_path = std::env::args()
        .skip(1)
        .map(std::path::PathBuf::from)
        .find(|p| p.is_file());

    // 注：窗口保持不透明。wgpu flip-model swapchain 窗口上 DWM 磨砂材质
    // （Acrylic/Mica）均不生效，窗口磨砂背景由 backdrop.rs 自实现
    // （截取窗口背后画面 → 模糊 → 作画布背景）。
    // 无边框：去掉系统标题栏/边框，整个窗口由自绘 UI（悬浮胶囊工具栏 +
    // 全窗磨砂背景）接管；拖动/缩放/最小最大/关闭均在 app.rs 内自绘实现。
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_decorations(false)
        .with_inner_size([1280.0, 860.0])
        .with_min_inner_size([880.0, 560.0]);
    // 仅显式QA会话使用独立持久化命名空间；常规启动的用户设置路径不变。
    if let Some(id) = std::env::var("LCL_IV_QA_PROFILE").ok().and_then(|v| qa_app_id(&v)) {
        eprintln!("QA isolated profile: {id}");
        viewport = viewport.with_app_id(id);
    }
    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "LcL ImageViewer",
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

/// 只接受会话标识，不接受文件路径，避免测试入口覆盖正常用户配置。
fn qa_app_id(profile: &str) -> Option<String> {
    if !profile.is_empty() && profile.len() <= 64
        && profile.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
        Some(format!("LcL ImageViewer QA-{profile}"))
    } else {
        None
    }
}

#[cfg(test)]
mod qa_tests {
    #[test]
    fn qa_profiles_cannot_select_the_normal_app_or_a_path() {
        assert_eq!(super::qa_app_id("abc-123").as_deref(), Some("LcL ImageViewer QA-abc-123"));
        for invalid in ["", "..", "../LcL ImageViewer", "C:\\Users", "a/b", "a\\b"] {
            assert!(super::qa_app_id(invalid).is_none());
        }
        assert!(super::qa_app_id(&"a".repeat(65)).is_none());
    }
}

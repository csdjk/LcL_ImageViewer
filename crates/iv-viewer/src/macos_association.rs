//! No Windows registry/Explorer integration is exposed in the macOS preview.
pub const APP_NAME: &str = "LcL ImageViewer";
pub fn is_registered() -> bool { false }

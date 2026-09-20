//! macOS native window ownership. Desktop screen capture is intentionally unavailable.
//! Image rendering, the color palette and topmost level remain in the shared egui path.
#![allow(dead_code, unexpected_cfgs)]
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::Object;

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

/// Called only from the application's main/UI thread; never enumerates other apps.
pub fn find_own_window() -> Option<isize> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let mut window: *mut Object = msg_send![app, keyWindow];
        if window.is_null() { window = msg_send![app, mainWindow]; }
        if window.is_null() {
            let windows: *mut Object = msg_send![app, windows];
            let count: usize = msg_send![windows, count];
            if count > 0 { window = msg_send![windows, objectAtIndex: 0usize]; }
        }
        (!window.is_null()).then_some(window as isize)
    }
}

/// This NSWindow belongs to the running App and remains alive during the modal picker.
pub fn dialog_handle(window: isize) -> raw_window_handle::RawWindowHandle {
    let mut handle = raw_window_handle::AppKitWindowHandle::empty();
    handle.ns_window = window as *mut std::ffi::c_void;
    let view: *mut Object = unsafe { msg_send![window as *mut Object, contentView] };
    handle.ns_view = view.cast();
    raw_window_handle::RawWindowHandle::AppKit(handle)
}

// These Windows acrylic APIs are never enabled on macOS; no screenshot permission is requested.
pub fn set_exclude_from_capture(_: isize, _: bool) -> bool { false }
pub fn set_rounded_corners(_: isize) -> bool { false }
pub fn clamp_window_onscreen(_: isize) {}
pub struct Capture { pub rgba: Vec<u8>, pub width: u32, pub height: u32 }
pub fn capture_behind(_: isize, _: f32) -> Result<Capture, String> {
    Err("macOS 版本不启用 Windows 桌面磨砂捕获".into())
}
pub struct WindowDrag;
impl WindowDrag {
    pub fn begin(_: isize, _: [f32; 2], _: f32) -> Option<Self> { None }
    pub fn advance(&self) -> bool { false }
}

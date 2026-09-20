//! macOS recycle-only deletion via NSFileManager. No permanent-delete fallback.
#![allow(unexpected_cfgs)]
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::os::unix::fs::MetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::ffi::{CStr, CString};
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::{Object, BOOL, YES};

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

#[derive(Clone)]
pub struct Target {
    pub path: PathBuf,
    len: u64,
    modified: SystemTime,
    device: u64,
    inode: u64,
}
impl Target {
    pub fn new(path: &Path) -> Result<Self, String> {
        if path.as_os_str().is_empty() { return Err("没有选中的图片文件".into()); }
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let meta = std::fs::symlink_metadata(&path).map_err(|e| format!("无法读取文件：{e}"))?;
        if !meta.file_type().is_file() { return Err("只支持普通图片文件，不处理目录或符号链接".into()); }
        Ok(Self { path, len: meta.len(), modified: meta.modified().map_err(|e| e.to_string())?,
            device: meta.dev(), inode: meta.ino() })
    }
    pub fn verify_unchanged(&self) -> Result<(), String> {
        let now = Self::new(&self.path)?;
        if (self.len, self.modified, self.device, self.inode) != (now.len, now.modified, now.device, now.inode) {
            return Err("图片在确认期间已发生变化，请重新确认".into());
        }
        Ok(())
    }
}

fn recycle_to_url(target: &Target) -> Result<PathBuf, String> {
    target.verify_unchanged()?;
    let path = CString::new(target.path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    objc::rc::autoreleasepool(|| unsafe {
        let text: *mut Object = msg_send![class!(NSString), stringWithUTF8String: path.as_ptr()];
        if text.is_null() { return Err("macOS 无法表示该文件路径，未删除文件".into()); }
        let url: *mut Object = msg_send![class!(NSURL), fileURLWithPath: text];
        let manager: *mut Object = msg_send![class!(NSFileManager), defaultManager];
        let mut result: *mut Object = std::ptr::null_mut();
        let mut error: *mut Object = std::ptr::null_mut();
        let ok: BOOL = msg_send![manager, trashItemAtURL: url resultingItemURL: &mut result error: &mut error];
        if ok != YES {
            let detail = if error.is_null() { "未知系统错误".to_owned() } else {
                let desc: *mut Object = msg_send![error, localizedDescription];
                let ptr: *const std::ffi::c_char = msg_send![desc, UTF8String];
                if ptr.is_null() { "未知系统错误".into() } else { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
            };
            return Err(format!("移入废纸篓失败：{detail}；不会永久删除图片"));
        }
        if result.is_null() { return Err("系统未返回废纸篓位置，请检查废纸篓".into()); }
        let p: *mut Object = msg_send![result, path];
        let ptr: *const std::ffi::c_char = msg_send![p, UTF8String];
        if ptr.is_null() { return Err("无法读取废纸篓位置，请检查废纸篓".into()); }
        Ok(PathBuf::from(CStr::from_ptr(ptr).to_string_lossy().into_owned()))
    })
}

pub fn recycle(target: &Target) -> Result<(), String> { recycle_to_url(target).map(|_| ()) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_missing_and_directory_targets_are_rejected() {
        assert!(Target::new(Path::new("")).is_err());
        assert!(Target::new(&std::env::temp_dir()).is_err());
        assert!(Target::new(&std::env::temp_dir().join("iv-not-a-real-image-xyz.png")).is_err());
    }
    #[test]
    fn real_native_trash_preserves_confirmed_file_bytes() {
        let token = format!("iv-mac-trash-{}-{}",std::process::id(),SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos());
        let dir=std::env::temp_dir().join(token);std::fs::create_dir(&dir).unwrap();
        let file=dir.join("测试 spaces.png");std::fs::write(&file,b"unique test payload").unwrap();
        let target=Target::new(&file).unwrap();
        let trashed=recycle_to_url(&target).unwrap();
        assert!(!file.exists());assert_eq!(std::fs::read(&trashed).unwrap(),b"unique test payload");
        // Only remove the exact test-owned result URL after checking its contents.
        std::fs::remove_file(trashed).unwrap();std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn replacement_and_symlinks_require_reconfirmation() {
        let dir=std::env::temp_dir().join(format!("iv-mac-target-{}-{}",std::process::id(),SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let file=dir.join("image.png");std::fs::write(&file,b"before").unwrap();
        let target=Target::new(&file).unwrap();std::fs::write(&file,b"changed").unwrap();
        assert!(target.verify_unchanged().is_err());
        let link=dir.join("link.png");std::os::unix::fs::symlink(&file,&link).unwrap();assert!(Target::new(&link).is_err());
        std::fs::remove_file(link).unwrap();std::fs::remove_file(file).unwrap();std::fs::remove_dir(dir).unwrap();
    }
}

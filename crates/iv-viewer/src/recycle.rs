//! Recycle exactly one confirmed regular image file. Never fall back to permanent deletion.
//! Windows contract: IFileOperation::SetOperationFlags / IFileOperationProgressSink::PreDeleteItem.
#![allow(non_snake_case)]

use std::cell::Cell;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;
use windows::core::{implement, Error, Result as WinResult, HRESULT, PCWSTR};
use windows::Win32::Foundation::E_ABORT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IFileOperationProgressSink, IFileOperationProgressSink_Impl,
    IShellItem, SHCreateItemFromParsingName, FOFX_ADDUNDORECORD, FOFX_EARLYFAILURE,
    FOFX_RECYCLEONDELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI,
    FOF_NO_CONNECTED_ELEMENTS, FOF_SILENT, TSF_DELETE_RECYCLE_IF_POSSIBLE,
};

/// The confirmation binds to this exact path and observed file version, not a later selection.
#[derive(Clone)]
pub struct Target {
    pub path: PathBuf,
    len: u64,
    modified: SystemTime,
}

impl Target {
    pub fn new(path: &Path) -> Result<Self, String> {
        if path.as_os_str().is_empty() {
            return Err("没有选中的图片文件".into());
        }
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        let meta = std::fs::symlink_metadata(&path).map_err(|e| format!("无法读取文件：{e}"))?;
        if !meta.file_type().is_file() {
            return Err("只支持删除普通图片文件，不处理目录或符号链接".into());
        }
        Ok(Self {
            path,
            len: meta.len(),
            modified: meta.modified().map_err(|e| e.to_string())?,
        })
    }

    pub fn verify_unchanged(&self) -> Result<(), String> {
        let now = Self::new(&self.path)?;
        if self.len != now.len || self.modified != now.modified {
            return Err("图片在确认期间已被其他程序修改，请重新确认".into());
        }
        Ok(())
    }
}

fn recycling_allowed(flags: u32) -> bool {
    flags & TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32 != 0
}
fn cancelled() -> Error {
    Error::new(E_ABORT, "无法移入回收站，已取消；不会永久删除图片")
}

#[implement(IFileOperationProgressSink)]
struct RecycleOnly {
    outcome: Rc<Cell<Option<HRESULT>>>,
}
impl IFileOperationProgressSink_Impl for RecycleOnly_Impl {
    fn StartOperations(&self) -> WinResult<()> {
        Ok(())
    }
    fn FinishOperations(&self, hr: HRESULT) -> WinResult<()> {
        if hr.is_err() {
            self.outcome.set(Some(hr));
        }
        Ok(())
    }
    fn PreDeleteItem(&self, flags: u32, _item: Option<&IShellItem>) -> WinResult<()> {
        if recycling_allowed(flags) {
            Ok(())
        } else {
            self.outcome.set(Some(E_ABORT));
            Err(cancelled())
        }
    }
    fn PostDeleteItem(
        &self,
        _flags: u32,
        _item: Option<&IShellItem>,
        hr: HRESULT,
        _new: Option<&IShellItem>,
    ) -> WinResult<()> {
        self.outcome.set(Some(hr));
        Ok(())
    }
    fn PreRenameItem(&self, _: u32, _: Option<&IShellItem>, _: &PCWSTR) -> WinResult<()> {
        Err(cancelled())
    }
    fn PostRenameItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Option<&IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreMoveItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: Option<&IShellItem>,
        _: &PCWSTR,
    ) -> WinResult<()> {
        Err(cancelled())
    }
    fn PostMoveItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: Option<&IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Option<&IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreCopyItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: Option<&IShellItem>,
        _: &PCWSTR,
    ) -> WinResult<()> {
        Err(cancelled())
    }
    fn PostCopyItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: Option<&IShellItem>,
        _: &PCWSTR,
        _: HRESULT,
        _: Option<&IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn PreNewItem(&self, _: u32, _: Option<&IShellItem>, _: &PCWSTR) -> WinResult<()> {
        Err(cancelled())
    }
    fn PostNewItem(
        &self,
        _: u32,
        _: Option<&IShellItem>,
        _: &PCWSTR,
        _: &PCWSTR,
        _: u32,
        _: HRESULT,
        _: Option<&IShellItem>,
    ) -> WinResult<()> {
        Ok(())
    }
    fn UpdateProgress(&self, _: u32, _: u32) -> WinResult<()> {
        Ok(())
    }
    fn ResetTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn PauseTimer(&self) -> WinResult<()> {
        Ok(())
    }
    fn ResumeTimer(&self) -> WinResult<()> {
        Ok(())
    }
}

struct ComApartment;
impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

/// Run on a fresh worker thread (STA). Only called after explicit user confirmation.
pub fn recycle(target: &Target) -> Result<(), String> {
    target.verify_unchanged()?;
    let wide: Vec<u16> = target
        .path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let execute = || -> WinResult<()> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
            let _com = ComApartment;
            let operation: IFileOperation =
                CoCreateInstance(&FileOperation, None, CLSCTX_INPROC_SERVER)?;
            operation.SetOperationFlags(
                FOFX_RECYCLEONDELETE
                    | FOFX_ADDUNDORECORD
                    | FOF_ALLOWUNDO
                    | FOFX_EARLYFAILURE
                    | FOF_NOERRORUI
                    | FOF_SILENT
                    | FOF_NOCONFIRMATION
                    | FOF_NO_CONNECTED_ELEMENTS,
            )?;
            let outcome = Rc::new(Cell::new(None));
            let sink: IFileOperationProgressSink = RecycleOnly {
                outcome: outcome.clone(),
            }
            .into();
            let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
            operation.DeleteItem(&item, &sink)?;
            operation.PerformOperations()?;
            if operation.GetAnyOperationsAborted()?.as_bool() {
                return Err(cancelled());
            }
            match outcome.get() {
                Some(hr) => hr.ok(),
                None => Err(cancelled()),
            }
        }
    };
    execute().map_err(|e| format!("移入回收站失败或已取消：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn irreversible_delete_is_rejected() {
        assert!(!recycling_allowed(0));
        assert!(!recycling_allowed(0x10));
        assert!(recycling_allowed(TSF_DELETE_RECYCLE_IF_POSSIBLE.0 as u32));
        let outcome = Rc::new(Cell::new(None));
        let sink: IFileOperationProgressSink = RecycleOnly {
            outcome: outcome.clone(),
        }
        .into();
        let result = unsafe { sink.PreDeleteItem(0, None) };
        assert!(result.is_err());
        assert_eq!(outcome.get(), Some(E_ABORT));
    }

    #[test]
    fn directories_empty_and_missing_paths_are_not_deletion_targets() {
        assert!(Target::new(Path::new("")).is_err());
        assert!(Target::new(&std::env::temp_dir()).is_err());
        assert!(Target::new(
            &std::env::temp_dir().join(format!("iv-missing-{}-target.png", std::process::id()))
        )
        .is_err());
    }

    #[test]
    fn changed_file_must_be_confirmed_again() {
        let path = std::env::temp_dir().join(format!(
            "iv-delete-target-{}-{}.tmp",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        file.write_all(b"test").unwrap();
        drop(file);
        let target = Target::new(&path).unwrap();
        assert!(target.verify_unchanged().is_ok());
        std::fs::write(&path, b"changed test").unwrap();
        assert!(target.verify_unchanged().is_err());
        std::fs::remove_file(path).unwrap(); // only this test's unique temporary file
    }
}

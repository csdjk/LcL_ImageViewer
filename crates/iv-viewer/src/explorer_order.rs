//! Read the actual Explorer view, including column direction and grouping.
use std::path::{Path, PathBuf};
use windows::core::{Interface, VARIANT};
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::*;

struct Apartment;
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
fn path_of(item: &IShellItem) -> windows::core::Result<PathBuf> {
    unsafe {
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let value = raw.to_string();
        CoTaskMemFree(Some(raw.0.cast()));
        Ok(PathBuf::from(value?))
    }
}
pub fn read(root: &Path) -> Result<Option<Vec<PathBuf>>, String> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok().map_err(|e| e.to_string())?; }
    let _apartment = Apartment;
    unsafe { read_inner(root).map_err(|e| e.to_string()) }
}
unsafe fn read_inner(root: &Path) -> windows::core::Result<Option<Vec<PathBuf>>> {
    let shell: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL)?;
    let canonical = std::fs::canonicalize(root).map_err(|e| windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), e.to_string()))?;
    let mut found: Option<Vec<PathBuf>> = None;
    for i in 0..shell.Count()? {
        let view = (|| -> windows::core::Result<IFolderView> {
            let dispatch = shell.Item(&VARIANT::from(i))?;
            let service: IServiceProvider = dispatch.cast()?;
            let browser: IShellBrowser = service.QueryService(&SID_STopLevelBrowser)?;
            browser.QueryActiveShellView()?.cast()
        })();
        let Ok(view) = view else { continue }; // Non-folder shell windows are irrelevant.
        let folder: IPersistFolder2 = view.GetFolder()?;
        let pidl = folder.GetCurFolder()?;
        let name = SHGetNameFromIDList(pidl, SIGDN_FILESYSPATH);
        CoTaskMemFree(Some(pidl.cast()));
        let Ok(name) = name else { continue }; // Virtual shell folders have no filesystem path.
        let path = name.to_string();
        CoTaskMemFree(Some(name.0.cast()));
        let Ok(path) = path else { continue };
        if std::fs::canonicalize(path).ok().as_ref() != Some(&canonical) { continue; }
        let items: IShellItemArray = view.Items(SVGIO_ALLVIEW | SVGIO_FLAG_VIEWORDER)?;
        let count = items.GetCount()?;
        if count > 250_000 { return Err(windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Explorer视图超过25万项")); }
        let mut order = Vec::new();
        let mut charged = 0usize;
        for j in 0..count {
            let path = path_of(&items.GetItemAt(j)?)?;
            if iv_core::format::has_supported_ext(&path) && path.is_file() {
                charged = charged.saturating_add(path.as_os_str().len().saturating_mul(4).saturating_add(128));
                if order.len() >= 50_000 || charged > 32 * 1024 * 1024 {
                    return Err(windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "Explorer图片顺序超过5万项或32 MiB路径预算"));
                }
                order.push(path);
            }
        }
        if found.as_ref().is_some_and(|previous| previous != &order) {
            return Err(windows::core::Error::new(windows::core::HRESULT(0x80004005u32 as i32), "同一目录的多个Explorer窗口排序不同，请只保留目标窗口后刷新顺序"));
        }
        found = Some(order);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires an explicitly prepared Explorer fixture window"]
    fn live_explorer_view_order() {
        let root=std::env::var("IV_EXPLORER_TEST_ROOT").expect("explicit fixture root required");
        let expected=std::env::var("IV_EXPLORER_TEST_ORDER").expect("explicit expected order required");
        let order=super::read(std::path::Path::new(&root)).unwrap().expect("fixture Explorer window missing");
        let actual:Vec<_>=order.iter().map(|p|p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(actual,expected.split('|').map(str::to_string).collect::<Vec<_>>());
    }
}

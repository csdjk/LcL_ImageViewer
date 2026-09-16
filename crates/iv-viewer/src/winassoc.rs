//! Windows 文件关联（全部走 HKCU，免管理员）：
//! - `Applications\imageview.exe`：让应用出现在"打开方式"列表（带图标与友好名称），
//!   且"始终"按钮可用（未注册时 Win11 只允许"仅一次"）
//! - ProgId `LcL.ImageViewer.Image` + 各扩展名 `OpenWithProgids`：丰富"打开方式"入口
//! - `Capabilities` + `RegisteredApplications`：让应用出现在系统"默认应用"设置页，
//!   配合 `ms-settings:defaultapps` 引导用户设为默认（Win10+ 的 UserChoice 有哈希保护，
//!   程序无法直接写入默认关联，只能引导）

use std::path::Path;

use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

/// 应用显示名（打开方式列表 / 默认应用页中可见）
pub const APP_NAME: &str = "LcL ImageViewer";
/// 文件类型 ProgId
pub const PROG_ID: &str = "LcL.ImageViewer.Image";
/// 注册为可打开的扩展名（不含点）
pub const EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "gif", "webp", "avif", "ico", "tif", "tiff", "hdr", "dds", "psd",
    "qoi", "tga", "ppm", "pgm", "pbm",
];

fn classes_key() -> std::io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
        r"Software\Classes",
        winreg::enums::KEY_ALL_ACCESS,
    )
}

/// 当前 exe 的绝对路径（注册命令行用）。
fn exe_path() -> Option<String> {
    std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .filter(|p| Path::new(p).is_file())
}

/// 是否已注册到"打开方式"（以 Applications 键的 open command 存在且指向当前 exe 为准）。
pub fn is_registered() -> bool {
    let Some(exe) = exe_path() else { return false };
    let Ok(classes) = classes_key() else { return false };
    let Ok(cmd) = classes.open_subkey(r"Applications\imageview.exe\shell\open\command") else {
        return false;
    };
    let Ok(registered) = cmd.get_value::<String, _>("") else {
        return false;
    };
    registered.to_lowercase().contains(&exe.to_lowercase())
}

/// 注册到"打开方式" + "默认应用"候选。返回错误信息（成功为 Ok）。
pub fn register() -> Result<(), String> {
    let exe = exe_path().ok_or("无法获取当前程序路径")?;
    let classes = classes_key().map_err(|e| format!("打开注册表失败：{e}"))?;
    let reg = |sub: &str| -> Result<RegKey, String> {
        classes
            .create_subkey(sub)
            .map(|(k, _)| k)
            .map_err(|e| format!("写入 {sub} 失败：{e}"))
    };

    // Applications\imageview.exe：打开方式列表的图标/名称/命令/支持的类型
    let app = reg(r"Applications\imageview.exe")?;
    app.set_value("FriendlyAppName", &APP_NAME)
        .map_err(|e| e.to_string())?;
    reg(r"Applications\imageview.exe\DefaultIcon")?
        .set_value("", &format!("{exe},0"))
        .map_err(|e| e.to_string())?;
    reg(r"Applications\imageview.exe\shell\open\command")?
        .set_value("", &format!("\"{exe}\" \"%1\""))
        .map_err(|e| e.to_string())?;
    let types = reg(r"Applications\imageview.exe\SupportedTypes")?;
    for ext in EXTS {
        types
            .set_value(format!(".{ext}"), &"")
            .map_err(|e| e.to_string())?;
    }

    // ProgId + OpenWithProgids：丰富每个扩展名的打开方式入口
    let prog = reg(PROG_ID)?;
    prog.set_value("", &format!("{APP_NAME} Image"))
        .map_err(|e| e.to_string())?;
    reg(&format!(r"{PROG_ID}\DefaultIcon"))?
        .set_value("", &format!("{exe},0"))
        .map_err(|e| e.to_string())?;
    reg(&format!(r"{PROG_ID}\shell\open\command"))?
        .set_value("", &format!("\"{exe}\" \"%1\""))
        .map_err(|e| e.to_string())?;
    for ext in EXTS {
        let k = reg(&format!(r".{ext}\OpenWithProgids"))?;
        k.set_value(PROG_ID, &"").map_err(|e| e.to_string())?;
    }

    // Capabilities + RegisteredApplications：出现在系统"默认应用"设置页
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (cap, _) = hkcu
        .create_subkey(r"Software\LcL\ImageViewer\Capabilities")
        .map_err(|e| e.to_string())?;
    cap.set_value("ApplicationName", &APP_NAME)
        .map_err(|e| e.to_string())?;
    cap.set_value(
        "ApplicationDescription",
        &"轻量级游戏美术看图工具：DDS/PSD/TGA/QOI/HDR/GIF/WebP/APNG/AVIF",
    )
    .map_err(|e| e.to_string())?;
    let (fa, _) = hkcu
        .create_subkey(r"Software\LcL\ImageViewer\Capabilities\FileAssociations")
        .map_err(|e| e.to_string())?;
    for ext in EXTS {
        fa.set_value(format!(".{ext}"), &PROG_ID)
            .map_err(|e| e.to_string())?;
    }
    let (ra, _) = hkcu
        .create_subkey(r"Software\RegisteredApplications")
        .map_err(|e| e.to_string())?;
    ra.set_value(APP_NAME, &r"Software\LcL\ImageViewer\Capabilities")
        .map_err(|e| e.to_string())?;

    // 通知 shell 关联已变化（刷新打开方式列表缓存）
    notify_shell();
    Ok(())
}

/// 解除注册（删除本应用写入的所有键；不碰用户可能手动设置的 UserChoice）。
pub fn unregister() -> Result<(), String> {
    let classes = classes_key().map_err(|e| format!("打开注册表失败：{e}"))?;
    let _ = classes.delete_subkey_all(r"Applications\imageview.exe");
    let _ = classes.delete_subkey_all(PROG_ID);
    for ext in EXTS {
        if let Ok(k) = classes.open_subkey_with_flags(
            format!(r".{ext}\OpenWithProgids"),
            winreg::enums::KEY_SET_VALUE,
        ) {
            let _ = k.delete_value(PROG_ID);
        }
    }
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = hkcu.delete_subkey_all(r"Software\LcL\ImageViewer");
    if let Ok(ra) = hkcu.open_subkey_with_flags(
        r"Software\RegisteredApplications",
        winreg::enums::KEY_SET_VALUE,
    ) {
        let _ = ra.delete_value(APP_NAME);
    }
    notify_shell();
    Ok(())
}

/// 打开系统"默认应用"设置页（默认关联需用户在系统设置里确认）。
pub fn open_default_apps_settings() {
    let _ = std::process::Command::new("explorer")
        .arg("ms-settings:defaultapps")
        .spawn();
}

/// SHChangeNotify(SHCNE_ASSOCCHANGED)：让资源管理器立即感知关联变化。
fn notify_shell() {
    #[link(name = "shell32")]
    extern "system" {
        fn SHChangeNotify(event_id: i32, flags: u32, item1: *const u8, item2: *const u8);
    }
    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0;
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, std::ptr::null(), std::ptr::null());
    }
}

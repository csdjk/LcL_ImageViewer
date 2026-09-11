//! 构建脚本：把 assets/icon.ico 嵌入 exe 资源（资源管理器 / 任务栏 / 安装包图标）。

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // MSVC：winres 走 Windows SDK 的 rc.exe，链接正常
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed icon resource: {e}");
        }
        return;
    }

    // GNU：winres 的“windres → ar 静态库”方式会被 ld 整体丢弃
    //（纯资源 .o 无符号，archive 成员不会被拉入链接）。
    // 改为手动调 windres 生成 .o，通过 link-arg-bin 直接进入链接命令行。
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let rc = Path::new(&out_dir).join("icon.rc");
    let obj = Path::new(&out_dir).join("icon.o");
    let ico = Path::new(&manifest).join("assets").join("icon.ico");
    // rc 内用正斜杠绝对路径，避免转义与相对目录歧义
    let ico_fwd = ico.display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 ICON \"{ico_fwd}\"\n")).unwrap();

    let mingw = std::env::var("MINGW_BIN").unwrap_or_else(|_| r"C:\mingw64\mingw64\bin".into());
    let windres = if Path::new(&mingw).join("windres.exe").exists() {
        format!(r"{mingw}\windres.exe")
    } else {
        "windres".to_string() // 退化为 PATH 查找
    };
    match Command::new(&windres)
        .arg(&rc)
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .status()
    {
        Ok(s) if s.success() && obj.exists() => {
            println!("cargo:rustc-link-arg-bin=imageview={}", obj.display());
        }
        Ok(s) => println!("cargo:warning=windres exited with {s}"),
        Err(e) => println!("cargo:warning=failed to run windres: {e}"),
    }
}

//! 缩略图 COM 全链路自检：不依赖 Explorer，直接激活 iv_shell.dll 的类厂，
//! 走 IInitializeWithStream + IThumbnailProvider::GetThumbnail 验证返回的 HBITMAP。
//!
//! 运行：`cargo run -p iv-shell --release --example test_thumbnail`

use windows::core::{Interface, GUID, HSTRING};
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP, HGDIOBJ};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, IStream, CLSCTX, COINIT_APARTMENTTHREADED, STGM_READ,
};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream;
use windows::Win32::UI::Shell::{IThumbnailProvider, SHCreateStreamOnFileEx, WTS_ALPHATYPE};

const CLSID_IV_THUMBNAIL: GUID = GUID::from_u128(0x7a3e9b21_4c5d_4e8f_9a6b_1d2c3e4f5a6b);

type DllGetClassObjectFn =
    unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut core::ffi::c_void) -> i32;

fn main() -> windows::core::Result<()> {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    // 不依赖注册表：直接 LoadLibrary + DllGetClassObject，验证 DLL 本身的 COM 链路
    let dll_path = concat!(env!("CARGO_MANIFEST_DIR"), "\\..\\..\\target\\release\\iv_shell.dll");
    let h: HMODULE = unsafe { LoadLibraryW(&HSTRING::from(dll_path))? };
    let sym = unsafe { GetProcAddress(h, windows::core::s!("DllGetClassObject")) }
        .expect("DllGetClassObject export missing");
    let get_class_object: DllGetClassObjectFn = unsafe { std::mem::transmute(sym) };

    let mut factory_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    let hr = unsafe {
        get_class_object(
            &CLSID_IV_THUMBNAIL,
            &windows::Win32::System::Com::IClassFactory::IID,
            &mut factory_ptr,
        )
    };
    assert_eq!(hr, 0, "DllGetClassObject failed: 0x{hr:08X}");
    let factory = unsafe { windows::Win32::System::Com::IClassFactory::from_raw(factory_ptr) };

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "\\..\\..\\test_images\\");
    for name in ["bc1_mips.dds", "checker.psd", "anim_test.gif"] {
        let path = format!("{root}{name}");
        if !std::path::Path::new(&path).exists() {
            println!("[skip] {name} (file missing)");
            continue;
        }
        match probe(&factory, &path, 128) {
            Ok(info) => println!("[ok]   {name}: {info}"),
            Err(e) => println!("[FAIL] {name}: {e:?}"),
        }
    }

    // 顺带验证 CoCreateInstance 走注册表的路径（explorer 实际走的就是这条）
    let via_registry: windows::core::Result<IThumbnailProvider> =
        unsafe { CoCreateInstance(&CLSID_IV_THUMBNAIL, None, CLSCTX(0x1)) };
    match via_registry {
        Ok(_) => println!("[ok]   CoCreateInstance via registry succeeded"),
        Err(e) => println!("[warn] CoCreateInstance via registry failed: {e:?}"),
    }
    Ok(())
}

fn probe(
    factory: &windows::Win32::System::Com::IClassFactory,
    path: &str,
    cx: u32,
) -> windows::core::Result<String> {
    let provider: IThumbnailProvider = unsafe { factory.CreateInstance(None)? };
    let init: IInitializeWithStream = provider.cast()?;
    let stream: IStream = unsafe {
        SHCreateStreamOnFileEx(&HSTRING::from(path), STGM_READ.0, 0, BOOL(0), None)?
    };
    unsafe { init.Initialize(&stream, 0)? };

    let mut hbmp = HBITMAP::default();
    let mut alpha = WTS_ALPHATYPE(0);
    unsafe { provider.GetThumbnail(cx, &mut hbmp, &mut alpha)? };

    let mut bm = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            HGDIOBJ(hbmp.0),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bm as *mut _ as *mut _),
        )
    };
    let _ = unsafe { DeleteObject(HGDIOBJ(hbmp.0)) };
    if got == 0 {
        return Err(windows::core::Error::from_win32());
    }
    Ok(format!(
        "{}x{} alpha={} (requested cx={cx})",
        bm.bmWidth, bm.bmHeight, alpha.0
    ))
}

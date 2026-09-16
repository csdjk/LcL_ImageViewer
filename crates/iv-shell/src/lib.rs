//! iv-shell：Windows 资源管理器缩略图扩展（IThumbnailProvider COM）。
//!
//! - 解码全部复用 iv-core（DDS/TGA/PSD/QOI/HDR/Netpbm/GIF/WebP/APNG 等）
//! - 动画格式返回第一帧；HDR 做 Reinhard tonemap 到 sRGB
//! - 本 DLL 运行在 explorer.exe 等宿主进程内：所有错误转为 HRESULT，
//!   入口全部 catch_unwind，绝不 panic 跨 FFI
//!
//! 注册（HKCU，免管理员）：tools/register_thumbnail.ps1 / unregister_thumbnail.ps1

mod resample;

use std::sync::Mutex;

use iv_core::{decode::decode_preview_bytes, PixelData};
use windows::core::{implement, Error, Interface, Result, GUID, HRESULT};
use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_INVALIDARG, E_POINTER,
    E_UNEXPECTED, S_FALSE,
};
use windows::Win32::Graphics::Gdi::{
    CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl, IStream};
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, WTS_ALPHATYPE, WTSAT_ARGB, WTSAT_RGB,
};

/// 本扩展的 CLSID（注册脚本同步使用，保持稳定勿改）。
pub const CLSID_IV_THUMBNAIL: GUID = GUID::from_u128(0x7a3e9b21_4c5d_4e8f_9a6b_1d2c3e4f5a6b);

/// 读取上限：拒绝超大文件（缩略图场景无意义，防内存被拖爆）。
const MAX_FILE_BYTES: usize = 512 * 1024 * 1024;

#[implement(IThumbnailProvider, IInitializeWithStream)]
struct ThumbnailProvider {
    /// Initialize 收到的文件内容
    bytes: Mutex<Vec<u8>>,
}

impl ThumbnailProvider {
    fn new() -> Self {
        Self {
            bytes: Mutex::new(Vec::new()),
        }
    }

    /// 解码 → 缩放 → HBITMAP。
    fn make_thumbnail(&self, cx: u32) -> Result<(HBITMAP, WTS_ALPHATYPE)> {
        if cx == 0 {
            return Err(Error::from(E_INVALIDARG));
        }
        let bytes = self.bytes.lock().map_err(|_| Error::from(E_UNEXPECTED))?;
        if bytes.is_empty() {
            return Err(Error::from(E_INVALIDARG));
        }
        let img = decode_preview_bytes(&bytes).map_err(|_| Error::from(E_FAIL))?;
        let Some(mip) = img.mips.first() else {
            return Err(Error::from(E_FAIL));
        };

        // 统一成 RGBA8（HDR 先 tonemap）
        let rgba8: Vec<u8> = match &mip.data {
            PixelData::Rgba8(v) => v.clone(),
            PixelData::RgbaF32(v) => v.chunks_exact(4).flat_map(|p| [
                tonemap_u8(p[0]), tonemap_u8(p[1]), tonemap_u8(p[2]),
                (p[3].clamp(0.0, 1.0) * 255.0).round() as u8,
            ]).collect(),
        };

        // Shell's 32-bit alpha bitmap is premultiplied; do this BEFORE resampling
        // to avoid fringe colours from fully transparent texels.
        let mut rgba8 = rgba8;
        premultiply(&mut rgba8);

        // 目标尺寸：等比缩到最长边 cx；小图不放大（Shell 会居中显示）
        let (sw, sh) = (img.width as usize, img.height as usize);
        let (dw, dh) = if sw.max(sh) as u32 <= cx {
            (sw, sh)
        } else {
            let s = cx as f32 / sw.max(sh) as f32;
            (
                ((sw as f32) * s).round().max(1.0) as usize,
                ((sh as f32) * s).round().max(1.0) as usize,
            )
        };
        let rgba = if (dw, dh) == (sw, sh) {
            rgba8
        } else {
            resample::downscale_box(&rgba8, sw, sh, dw, dh)
        };

        // DIB 内存序是 BGRA
        let mut bgra = rgba;
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        let hbmp = build_hbitmap(&bgra, dw as i32, dh as i32)?;
        let alpha = if img.has_alpha { WTSAT_ARGB } else { WTSAT_RGB };
        Ok((hbmp, alpha))
    }
}

// 注意：windows 0.58 的 #[implement] 宏要求 _Impl trait 实现于宏生成的
// box 类型（<Name>_Impl）上；字段/方法经 Deref 自动落到内部类型。
impl IInitializeWithStream_Impl for ThumbnailProvider_Impl {
    /// Shell 通过 IStream 给文件内容（比文件路径更通用，支持虚拟文件）。
    fn Initialize(&self, pstream: Option<&IStream>, _grfmode: u32) -> Result<()> {
        let Some(stream) = pstream else {
            return Err(Error::from(E_INVALIDARG));
        };
        let read_all = || {
            let mut buf = Vec::with_capacity(1 << 20);
            let mut chunk = [0u8; 65536];
            loop {
                let mut read = 0u32;
                let hr = unsafe {
                    stream.Read(chunk.as_mut_ptr() as *mut _, chunk.len() as u32, Some(&mut read))
                };
                if hr.is_err() {
                    return Err(Error::from(hr));
                }
                if read == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..read as usize]);
                if buf.len() > MAX_FILE_BYTES {
                    return Err(Error::from(E_FAIL));
                }
            }
            Ok(buf)
        };
        let buf = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read_all))
            .map_err(|_| Error::from(E_UNEXPECTED))??;
        *self.bytes.lock().map_err(|_| Error::from(E_UNEXPECTED))? = buf;
        Ok(())
    }
}

impl IThumbnailProvider_Impl for ThumbnailProvider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> Result<()> {
        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(Error::from(E_POINTER));
        }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.make_thumbnail(cx)));
        match r {
            Ok(Ok((hbmp, alpha))) => unsafe {
                *phbmp = hbmp;
                *pdwalpha = alpha;
                Ok(())
            },
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Error::from(E_UNEXPECTED)),
        }
    }
}

#[implement(IClassFactory)]
struct ThumbnailClassFactory;

impl IClassFactory_Impl for ThumbnailClassFactory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Option<&windows::core::IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> Result<()> {
        if punkouter.is_some() {
            return Err(Error::from(CLASS_E_NOAGGREGATION));
        }
        if riid.is_null() || ppvobject.is_null() {
            return Err(Error::from(E_POINTER));
        }
        unsafe { *ppvobject = std::ptr::null_mut() };
        let provider: windows::core::IUnknown = ThumbnailProvider::new().into();
        // query 成功即 AddRef；provider 离开作用域 Release，净增 1，所有权移交调用方
        unsafe { provider.query(riid, ppvobject) }.ok()
    }

    fn LockServer(&self, _flock: windows::Win32::Foundation::BOOL) -> Result<()> {
        Ok(())
    }
}

/// COM 入口：创建类厂。
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    unsafe {
        *ppv = std::ptr::null_mut();
        if *rclsid != CLSID_IV_THUMBNAIL {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = ThumbnailClassFactory.into();
        factory.query(riid, ppv)
    }
}

/// 简化生命周期管理：常驻不卸载（explorer 进程中开销可忽略）。
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    S_FALSE
}

/// HDR → sRGB u8（与查看器 shader 相同的 Reinhard + gamma）。
fn tonemap_u8(v: f32) -> u8 {
    if !v.is_finite() {
        return 0;
    }
    let v = v.max(0.0);
    let v = v / (1.0 + v);
    (v.powf(1.0 / 2.2) * 255.0 + 0.5) as u8
}

/// 32bpp 顶向下 DIB（调用方负责 DeleteObject）。
fn build_hbitmap(bgra: &[u8], w: i32, h: i32) -> Result<HBITMAP> {
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -h, // 负值 = 顶向下行序
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let hbmp = unsafe { CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits, None, 0)? };
    if bits.is_null() {
        return Err(Error::from(E_UNEXPECTED));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bgra.as_ptr(), bits as *mut u8, bgra.len());
    }
    Ok(hbmp)
}

fn premultiply(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = pixel[3] as u16;
        for c in &mut pixel[..3] { *c = ((*c as u16 * alpha + 127) / 255) as u8; }
    }
}

#[cfg(test)]
mod alpha_tests {
    #[test]
    fn thumbnail_alpha_has_no_hidden_color_fringe() {
        let mut px = vec![255,128,64,0, 200,100,50,128, 20,40,60,255];
        super::premultiply(&mut px);
        assert_eq!(px, [0,0,0,0,100,50,25,128,20,40,60,255]);
    }
    #[test]
    fn thumbnail_resampling_preserves_premultiplied_alpha() {
        let mut px = vec![255,0,0,255,0,255,0,0];
        super::premultiply(&mut px);
        assert_eq!(super::resample::downscale_box(&px,2,1,1,1),[127,0,0,127]);
    }
}

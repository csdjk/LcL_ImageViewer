//! Persist a full-resolution static snapshot; never use the rendered viewport.
use iv_core::decode::{DecodedImage, PixelData};
use std::path::PathBuf;
use windows::core::PCWSTR;
use windows::Win32::{System::Com::*, UI::Shell::*};

fn rgb(image: &DecodedImage, frame: usize) -> Result<(u32, u32, Vec<u8>), String> {
    if image.is_hdr { return Err("HDR壁纸暂不支持，请先转换为普通图片".into()); }
    let (w,h,data) = if image.frames.len()>1 {
        let f=image.frames.get(frame).ok_or("动画帧已失效")?;
        (image.width,image.height,&f.data)
    } else {
        let m=image.mips.first().ok_or("图片没有像素数据")?;
        (m.width,m.height,&m.data)
    };
    let bytes = match data { PixelData::Rgba8(v)=>v.as_slice(),PixelData::Rgba8Mapped(v)=>v.as_slice(),_=>return Err("壁纸暂不支持浮点像素".into()) };
    let expected=u64::from(w)*u64::from(h)*4;
    if expected==0 || expected>256*1024*1024 || expected!=bytes.len() as u64 { return Err("壁纸像素超出256 MiB预算或尺寸无效".into()); }
    // Desktop wallpaper is opaque. Composite transparent pixels on black explicitly.
    let mut out=Vec::with_capacity(bytes.len()/4*3);
    for p in bytes.chunks_exact(4) { for c in &p[..3] { out.push(((*c as u16*p[3] as u16+127)/255) as u8); } }
    Ok((w,h,out))
}
struct Apartment;
impl Drop for Apartment { fn drop(&mut self) { unsafe { CoUninitialize(); } } }
pub fn set(image: &DecodedImage, frame: usize) -> Result<(),String> {
    let (w,h,pixels)=rgb(image,frame)?;
    unsafe { CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok().map_err(|e|e.to_string())?; }
    let _apartment=Apartment;
    let wallpaper: IDesktopWallpaper=unsafe { CoCreateInstance(&DesktopWallpaper,None,CLSCTX_ALL) }.map_err(|e|e.to_string())?;
    let root=PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("无法获取本地应用数据目录")?).join("LcL ImageViewer/wallpaper");
    std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;
    let previous = unsafe {
        let raw=wallpaper.GetWallpaper(PCWSTR::null()).map_err(|e|e.to_string())?;
        let value=raw.to_string(); CoTaskMemFree(Some(raw.0.cast()));
        PathBuf::from(value.map_err(|e|e.to_string())?)
    };
    // Unique file means a failed or concurrent operation cannot overwrite an active wallpaper.
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
    let path=root.join(format!("wallpaper-{}-{stamp}.png",std::process::id()));
    let file=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e|e.to_string())?;
    use image::ImageEncoder;
    let result=(|| {
        image::codecs::png::PngEncoder::new(file).write_image(&pixels,w,h,image::ExtendedColorType::Rgb8).map_err(|e|e.to_string())?;
        use std::os::windows::ffi::OsStrExt;
        let wide:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe { wallpaper.SetWallpaper(PCWSTR::null(),PCWSTR(wide.as_ptr())) }.map_err(|e|e.to_string())
    })();
    if result.is_err() { if let Err(e)=std::fs::remove_file(&path) { return Err(format!("{}；清理失败：{e}",result.unwrap_err())); } }
    if result.is_ok() && previous.parent()==Some(root.as_path())
        && previous.file_name().is_some_and(|n|n.to_string_lossy().starts_with("wallpaper-") && n.to_string_lossy().ends_with(".png")) {
        // Only the prior uniform, app-owned snapshot; never delete source images or a multi-monitor set.
        if let Err(e)=std::fs::remove_file(&previous) {
            if e.kind()!=std::io::ErrorKind::NotFound { return Err(format!("壁纸已设置，但旧壁纸缓存清理失败：{e}")); }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_uses_original_pixels_and_composites_alpha() {
        use image::ImageEncoder;
        let mut encoded=Vec::new();
        image::codecs::png::PngEncoder::new(&mut encoded).write_image(&[255,20,30,255,100,200,50,128,255,255,255,0],3,1,image::ExtendedColorType::Rgba8).unwrap();
        let image=iv_core::decode::decode_bytes(&encoded).unwrap();
        let (w,h,pixels)=rgb(&image,0).unwrap();
        assert_eq!((w,h),(3,1)); assert_eq!(pixels,vec![255,20,30,50,100,25,0,0,0]);
    }
}

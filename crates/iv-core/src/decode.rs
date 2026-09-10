//! 统一解码入口：所有格式 → `DecodedImage`（RGBA，含 mip 链）。

use std::fmt;
use std::path::Path;

use crate::format::{detect_format, ImageFormat};

/// 解码后的图像。所有格式的像素统一为 RGBA。
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    /// mip 链（mips[0] 为最高分辨率）。非 DDS 格式恒为 1 个元素。
    pub mips: Vec<MipLevel>,
    /// 文件格式标识（用于 UI 显示）。
    pub kind: ImageKind,
    /// 块压缩类型（如 "BC7"），仅 DDS。
    pub compression: Option<String>,
    pub has_alpha: bool,
    pub is_hdr: bool,
    /// 附加说明（如 cubemap、PSB 降级信息等）。
    pub extra_meta: Option<String>,
}

/// 单个 mip 层级。
#[derive(Debug, Clone)]
pub struct MipLevel {
    pub width: u32,
    pub height: u32,
    pub data: PixelData,
}

impl MipLevel {
    /// 取 (x, y) 处的 RGBA（u8 形式；HDR 值会被 clamp 到 0..255）。
    /// 越界坐标返回 None。
    pub fn rgba8_at(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        match &self.data {
            PixelData::Rgba8(v) => v.get(i..i + 4).map(|s| [s[0], s[1], s[2], s[3]]),
            PixelData::RgbaF32(v) => {
                let r = v.get(i)?;
                let g = v.get(i + 1)?;
                let b = v.get(i + 2)?;
                let a = v.get(i + 3)?;
                Some([
                    (r.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (g.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (b.clamp(0.0, 1.0) * 255.0).round() as u8,
                    (a.clamp(0.0, 1.0) * 255.0).round() as u8,
                ])
            }
        }
    }

    /// 取 (x, y) 处的 RGBA f32（0..1 线性；LDR 值按 sRGB 数值直接除以 255）。
    pub fn rgba_f32_at(&self, x: u32, y: u32) -> Option<[f32; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        match &self.data {
            PixelData::Rgba8(v) => {
                let s = v.get(i..i + 4)?;
                Some([
                    s[0] as f32 / 255.0,
                    s[1] as f32 / 255.0,
                    s[2] as f32 / 255.0,
                    s[3] as f32 / 255.0,
                ])
            }
            PixelData::RgbaF32(v) => Some([
                *v.get(i)?,
                *v.get(i + 1)?,
                *v.get(i + 2)?,
                *v.get(i + 3)?,
            ]),
        }
    }
}

/// 像素数据。
#[derive(Debug, Clone)]
pub enum PixelData {
    /// RGBA，每像素 4 字节
    Rgba8(Vec<u8>),
    /// RGBA f32，每像素 16 字节（线性空间）
    RgbaF32(Vec<f32>),
}

/// 文件格式标识（UI 显示用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Bmp,
    Gif,
    WebP,
    Ico,
    Tiff,
    Hdr,
    Qoi,
    Netpbm,
    Tga,
    Dds,
    DdsCube,
    DdsVolume,
    Psd,
}

impl ImageKind {
    pub fn label(self) -> &'static str {
        match self {
            ImageKind::Png => "PNG",
            ImageKind::Jpeg => "JPEG",
            ImageKind::Bmp => "BMP",
            ImageKind::Gif => "GIF",
            ImageKind::WebP => "WebP",
            ImageKind::Ico => "ICO",
            ImageKind::Tiff => "TIFF",
            ImageKind::Hdr => "Radiance HDR",
            ImageKind::Qoi => "QOI",
            ImageKind::Netpbm => "Netpbm",
            ImageKind::Tga => "TGA",
            ImageKind::Dds => "DDS",
            ImageKind::DdsCube => "DDS (Cubemap)",
            ImageKind::DdsVolume => "DDS (Volume)",
            ImageKind::Psd => "PSD",
        }
    }
}

/// 解码错误。
#[derive(Debug, Clone)]
pub enum DecodeError {
    /// 文件头不是该格式 / 结构损坏
    NotAnImage(String),
    /// 识别出格式但暂不支持该变体
    UnsupportedFormat(String),
    /// 文件被截断
    Truncated,
    /// 解码过程中失败
    Decode(String),
    /// IO 错误
    Io(String),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::NotAnImage(s) => write!(f, "不是有效的图像文件: {s}"),
            DecodeError::UnsupportedFormat(s) => write!(f, "暂不支持的格式: {s}"),
            DecodeError::Truncated => write!(f, "文件数据不完整（截断）"),
            DecodeError::Decode(s) => write!(f, "解码失败: {s}"),
            DecodeError::Io(s) => write!(f, "读取失败: {s}"),
        }
    }
}

impl std::error::Error for DecodeError {}

impl From<std::io::Error> for DecodeError {
    fn from(e: std::io::Error) -> Self {
        DecodeError::Io(e.to_string())
    }
}

/// 解码文件路径。
pub fn decode_path(path: &Path) -> Result<DecodedImage, DecodeError> {
    let bytes = std::fs::read(path).map_err(|e| DecodeError::Io(format!("{path:?}: {e}")))?;
    decode_bytes(&bytes)
}

/// 解码内存中的图像数据（按内容探测格式）。
pub fn decode_bytes(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    let fmt = detect_format(bytes);
    decode_with_format(bytes, fmt)
}

fn decode_with_format(bytes: &[u8], fmt: ImageFormat) -> Result<DecodedImage, DecodeError> {
    match fmt {
        ImageFormat::Dds => crate::dds::decode_dds(bytes),
        ImageFormat::Psd => crate::psd_composite::decode_psd(bytes),
        ImageFormat::Tga => decode_image_crate(bytes, ImageFormat::Tga),
        ImageFormat::Hdr => decode_hdr(bytes),
        ImageFormat::Unknown => Err(DecodeError::NotAnImage(
            "无法识别的图像格式".into(),
        )),
        other => decode_image_crate(bytes, other),
    }
}

fn img_format_of(fmt: ImageFormat) -> Option<image::ImageFormat> {
    Some(match fmt {
        ImageFormat::Png => image::ImageFormat::Png,
        ImageFormat::Jpeg => image::ImageFormat::Jpeg,
        ImageFormat::Bmp => image::ImageFormat::Bmp,
        ImageFormat::Gif => image::ImageFormat::Gif,
        ImageFormat::WebP => image::ImageFormat::WebP,
        ImageFormat::Ico => image::ImageFormat::Ico,
        ImageFormat::Tiff => image::ImageFormat::Tiff,
        ImageFormat::Qoi => image::ImageFormat::Qoi,
        ImageFormat::Netpbm => image::ImageFormat::Pnm,
        ImageFormat::Tga => image::ImageFormat::Tga,
        _ => return None,
    })
}

/// 走 image crate 的常规格式。
fn decode_image_crate(bytes: &[u8], fmt: ImageFormat) -> Result<DecodedImage, DecodeError> {
    let img_fmt = img_format_of(fmt).ok_or_else(|| {
        DecodeError::UnsupportedFormat(format!("{fmt:?} 不由 image crate 处理"))
    })?;
    let kind = match fmt {
        ImageFormat::Png => ImageKind::Png,
        ImageFormat::Jpeg => ImageKind::Jpeg,
        ImageFormat::Bmp => ImageKind::Bmp,
        ImageFormat::Gif => ImageKind::Gif,
        ImageFormat::WebP => ImageKind::WebP,
        ImageFormat::Ico => ImageKind::Ico,
        ImageFormat::Tiff => ImageKind::Tiff,
        ImageFormat::Qoi => ImageKind::Qoi,
        ImageFormat::Netpbm => ImageKind::Netpbm,
        ImageFormat::Tga => ImageKind::Tga,
        _ => unreachable!(),
    };
    let img = image::load_from_memory_with_format(bytes, img_fmt)
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    let (w, h) = (img.width(), img.height());
    let has_alpha = img.color().has_alpha();
    let rgba = img.to_rgba8().into_raw();
    Ok(DecodedImage {
        width: w,
        height: h,
        mips: vec![MipLevel {
            width: w,
            height: h,
            data: PixelData::Rgba8(rgba),
        }],
        kind,
        compression: None,
        has_alpha,
        is_hdr: false,
        extra_meta: None,
    })
}

/// Radiance HDR：保留 f32 线性数据（查看器做曝光/tonemap）。
fn decode_hdr(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    use image::ImageDecoder;
    use image::codecs::hdr::HdrDecoder;

    let decoder =
        HdrDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| DecodeError::Decode(e.to_string()))?;
    let (w, h) = {
        let m = decoder.metadata();
        (m.width, m.height)
    };
    // ColorType::Rgb32F：每像素 3 个 f32
    let total = decoder.total_bytes() as usize;
    let mut buf = vec![0u8; total];
    decoder
        .read_image(&mut buf)
        .map_err(|e| DecodeError::Decode(e.to_string()))?;

    let mut rgba = Vec::with_capacity((w as usize * h as usize) * 4);
    for px in buf.chunks_exact(12) {
        let r = f32::from_le_bytes([px[0], px[1], px[2], px[3]]);
        let g = f32::from_le_bytes([px[4], px[5], px[6], px[7]]);
        let b = f32::from_le_bytes([px[8], px[9], px[10], px[11]]);
        rgba.extend_from_slice(&[r, g, b, 1.0]);
    }
    Ok(DecodedImage {
        width: w,
        height: h,
        mips: vec![MipLevel {
            width: w,
            height: h,
            data: PixelData::RgbaF32(rgba),
        }],
        kind: ImageKind::Hdr,
        compression: None,
        has_alpha: false,
        is_hdr: true,
        extra_meta: None,
    })
}

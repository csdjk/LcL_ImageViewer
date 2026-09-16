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
    /// 动画帧序列（GIF / WebP / APNG / AVIF；静态图为空，>1 帧即为动画）。
    /// 帧均为合成后的完整帧，尺寸与 width/height 一致；mips[0] 与 frames[0] 内容相同。
    pub frames: Vec<AnimatedFrame>,
}

/// 一个动画帧。
#[derive(Debug, Clone)]
pub struct AnimatedFrame {
    pub data: PixelData,
    /// 显示时长（毫秒；GIF/WebP/APNG 的过短延时修正为 100ms，AVIF 最小 1ms）
    pub delay_ms: u32,
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
        self.data.rgba8_at(self.width, self.height, x, y)
    }

    /// 取 (x, y) 处的 RGBA f32（0..1 线性；LDR 值按 sRGB 数值直接除以 255）。
    pub fn rgba_f32_at(&self, x: u32, y: u32) -> Option<[f32; 4]> {
        self.data.rgba_f32_at(self.width, self.height, x, y)
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

impl PixelData {
    /// 取 (x, y) 处的 RGBA（u8 形式；HDR clamp）。`width/height` 为图像尺寸。
    pub fn rgba8_at(&self, width: u32, height: u32, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= width || y >= height {
            return None;
        }
        let i = (y as usize * width as usize + x as usize) * 4;
        match self {
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

    /// 取 (x, y) 处的 RGBA f32（LDR 按 /255）。
    pub fn rgba_f32_at(&self, width: u32, height: u32, x: u32, y: u32) -> Option<[f32; 4]> {
        if x >= width || y >= height {
            return None;
        }
        let i = (y as usize * width as usize + x as usize) * 4;
        match self {
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

/// 文件格式标识（UI 显示用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Bmp,
    Gif,
    WebP,
    Avif,
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
            ImageKind::Avif => "AVIF",
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

/// Thumbnail-only decoding: animated files contribute just the first composed frame.
/// Full viewer decoding remains unchanged, preserving all frames and delays.
pub fn decode_preview_bytes(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    use image::AnimationDecoder;
    use std::io::Cursor;
    let err = |e: image::ImageError| DecodeError::Decode(e.to_string());
    match detect_format(bytes) {
        ImageFormat::Avif => crate::avif::decode_avif_preview(bytes),
        ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(err)?;
            if decoder.has_animation() { first_preview_frame(decoder.into_frames(), ImageKind::WebP) }
            else { static_decoder(decoder, ImageKind::WebP) }
        }
        ImageFormat::Gif => {
            let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(err)?;
            first_preview_frame(decoder.into_frames(), ImageKind::Gif)
        }
        ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(Cursor::new(bytes)).map_err(err)?;
            if decoder.is_apng().map_err(err)? {
                first_preview_frame(decoder.apng().map_err(err)?.into_frames(), ImageKind::Png)
            } else { static_decoder(decoder, ImageKind::Png) }
        }
        other => decode_with_format(bytes, other),
    }
}

fn first_preview_frame(mut frames: image::Frames<'_>, kind: ImageKind) -> Result<DecodedImage, DecodeError> {
    let frame = frames.next().ok_or_else(|| DecodeError::Decode("图像无有效预览帧".into()))?
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    Ok(static_image(image::DynamicImage::ImageRgba8(frame.into_buffer()), kind))
}

fn decode_with_format(bytes: &[u8], fmt: ImageFormat) -> Result<DecodedImage, DecodeError> {
    match fmt {
        ImageFormat::Avif => crate::avif::decode_avif(bytes),
        ImageFormat::Dds => crate::dds::decode_dds(bytes),
        ImageFormat::Psd => crate::psd_composite::decode_psd(bytes),
        ImageFormat::Tga => decode_image_crate(bytes, ImageFormat::Tga),
        ImageFormat::Hdr => decode_hdr(bytes),
        // 动画感知格式：GIF / WebP / APNG
        ImageFormat::Gif => decode_gif(bytes),
        ImageFormat::WebP => decode_webp(bytes),
        ImageFormat::Png => decode_png(bytes),
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
    Ok(static_image(img, kind))
}

fn static_image(img: image::DynamicImage, kind: ImageKind) -> DecodedImage {
    let (w, h) = (img.width(), img.height());
    let has_alpha = img.color().has_alpha();
    let rgba = img.into_rgba8().into_raw();
    DecodedImage {
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
        frames: Vec::new(),
    }
}

/// 收集 image crate 的动画帧（GIF / WebP / APNG 统一走这里）。
/// 单帧时自动退化为静态图（frames 为空）。
fn collect_animation(
    frames: image::Frames,
    kind: ImageKind,
) -> Result<DecodedImage, DecodeError> {
    const MAX_FRAMES: usize = 4096; // 防恶意文件撑爆内存
    let mut list: Vec<AnimatedFrame> = Vec::new();
    let mut size: Option<(u32, u32)> = None;
    for fr in frames.into_iter() {
        let fr = fr.map_err(|e| DecodeError::Decode(e.to_string()))?;
        let buf = fr.buffer();
        let (fw, fh) = (buf.width(), buf.height());
        match size {
            None => size = Some((fw, fh)),
            Some((w, h)) => {
                if fw != w || fh != h {
                    continue; // 防御：尺寸不一致的帧跳过
                }
            }
        }
        let (num, den) = fr.delay().numer_denom_ms();
        // numer_denom_ms 返回的就是毫秒分数：delay_ms = num / den（不要再乘 1000）
        let ms = if den == 0 {
            0
        } else {
            (num as u64 / den as u64) as u32
        };
        // 浏览器惯例：过短延时（GIF 里常见 0）按 100ms
        let delay_ms = if ms < 10 { 100 } else { ms };
        list.push(AnimatedFrame {
            data: PixelData::Rgba8(fr.into_buffer().into_raw()),
            delay_ms,
        });
        if list.len() >= MAX_FRAMES {
            break;
        }
    }
    let (w, h) = size.ok_or_else(|| DecodeError::Decode("动画无有效帧".into()))?;
    let Some(first) = list.first().map(|f| f.data.clone()) else {
        return Err(DecodeError::Decode("动画无有效帧".into()));
    };
    // 第一帧扫描是否真有透明像素
    let has_alpha = match &first {
        PixelData::Rgba8(v) => v.chunks_exact(4).any(|p| p[3] != 255),
        PixelData::RgbaF32(v) => v.chunks_exact(4).any(|p| p[3] < 1.0),
    };
    let animated = list.len() > 1;
    Ok(DecodedImage {
        width: w,
        height: h,
        mips: vec![MipLevel {
            width: w,
            height: h,
            data: first,
        }],
        kind,
        compression: None,
        has_alpha,
        is_hdr: false,
        extra_meta: if animated {
            Some(format!("动画 · {} 帧", list.len()))
        } else {
            None
        },
        frames: if animated { list } else { Vec::new() },
    })
}

/// GIF：统一走动画收集（静态 GIF 自动退化为单帧静态图）。
fn decode_gif(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    use image::codecs::gif::GifDecoder;
    use image::AnimationDecoder;
    let dec = GifDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    collect_animation(dec.into_frames(), ImageKind::Gif)
}

/// WebP：静态图直接解码；仅动画进入帧收集。
fn decode_webp(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    use image::codecs::webp::WebPDecoder;
    use image::AnimationDecoder;
    let dec = WebPDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    if dec.has_animation() {
        collect_animation(dec.into_frames(), ImageKind::WebP)
    } else {
        static_decoder(dec, ImageKind::WebP)
    }
}

/// PNG：静态走常规路径；APNG 转 ApngDecoder 收帧。
fn decode_png(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    use image::codecs::png::PngDecoder;
    use image::AnimationDecoder;
    let dec = PngDecoder::new(std::io::Cursor::new(bytes))
        .map_err(|e| DecodeError::Decode(e.to_string()))?;
    if dec.is_apng().unwrap_or(false) {
        let apng = dec.apng().map_err(|e| DecodeError::Decode(e.to_string()))?;
        return collect_animation(apng.into_frames(), ImageKind::Png);
    }
    static_decoder(dec, ImageKind::Png)
}

fn static_decoder(mut decoder: impl image::ImageDecoder, kind: ImageKind) -> Result<DecodedImage, DecodeError> {
    // Reuse the parsed decoder; preserve ImageReader's default allocation checks.
    let mut limits = image::Limits::default();
    limits.reserve(decoder.total_bytes()).map_err(|e| DecodeError::Decode(e.to_string()))?;
    decoder.set_limits(limits).map_err(|e| DecodeError::Decode(e.to_string()))?;
    let image = image::DynamicImage::from_decoder(decoder).map_err(|e| DecodeError::Decode(e.to_string()))?;
    Ok(static_image(image, kind))
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
        frames: Vec::new(),
    })
}

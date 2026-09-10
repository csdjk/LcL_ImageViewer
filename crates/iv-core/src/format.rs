//! 按文件内容（magic bytes）探测图像格式，不信任扩展名。

/// 识别出的图像格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
    Gif,
    WebP,
    Ico,
    Tiff,
    /// Radiance RGBE (.hdr)
    Hdr,
    Dds,
    Psd,
    Qoi,
    /// Netpbm（P1..P7）
    Netpbm,
    /// TGA 无 magic，由 footer 或启发式判断（作为兑底格式）
    Tga,
    Unknown,
}

/// 探测 `bytes` 开头数据所属的图像格式。
pub fn detect_format(bytes: &[u8]) -> ImageFormat {
    if bytes.len() < 12 {
        return ImageFormat::Unknown;
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return ImageFormat::Png;
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return ImageFormat::Jpeg;
    }
    if bytes.starts_with(b"BM") {
        return ImageFormat::Bmp;
    }
    if bytes.starts_with(b"GIF8") {
        return ImageFormat::Gif;
    }
    if bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return ImageFormat::WebP;
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return ImageFormat::Ico;
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return ImageFormat::Tiff;
    }
    if bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE") {
        return ImageFormat::Hdr;
    }
    if bytes.starts_with(b"DDS ") {
        return ImageFormat::Dds;
    }
    if bytes.starts_with(b"8BPS") {
        return ImageFormat::Psd;
    }
    if bytes.starts_with(b"qoif") {
        return ImageFormat::Qoi;
    }
    // Netpbm: "P1".."P7" + 空白
    if bytes[0] == b'P'
        && (b'1'..=b'7').contains(&bytes[1])
        && matches!(bytes[2], b' ' | b'\t' | b'\r' | b'\n')
    {
        return ImageFormat::Netpbm;
    }
    // TGA：新版本有 footer "TRUEVISION-XFILE."
    if bytes.len() > 18 && bytes[bytes.len() - 18..].starts_with(b"TRUEVISION-XFILE.") {
        return ImageFormat::Tga;
    }
    // TGA 启发式：colormap type(0/1) + 合法 image type + 合理尺寸
    if is_probably_tga(bytes) {
        return ImageFormat::Tga;
    }
    ImageFormat::Unknown
}

/// 判断查看器支持的文件扩展名（用于目录过滤、文件关联）。
pub fn has_supported_ext(path: &std::path::Path) -> bool {
    supported_ext_of(path).is_some()
}

/// 返回该路径扩展名对应的格式描述；不支持则返回 None。
pub fn supported_ext_of(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" | "apng" => "PNG",
        "jpg" | "jpeg" | "jpe" => "JPEG",
        "bmp" | "dib" => "BMP",
        "gif" => "GIF",
        "webp" => "WebP",
        "ico" => "ICO",
        "tif" | "tiff" => "TIFF",
        "hdr" => "Radiance HDR",
        "dds" => "DDS",
        "psd" => "PSD",
        "psb" => "PSB",
        "qoi" => "QOI",
        "tga" => "TGA",
        "ppm" | "pgm" | "pbm" | "pnm" => "Netpbm",
        _ => return None,
    })
}

fn is_probably_tga(b: &[u8]) -> bool {
    if b.len() < 18 {
        return false;
    }
    let colormap_type = b[1];
    let image_type = b[2];
    let cm_ok = colormap_type == 0 || colormap_type == 1;
    let it_ok = matches!(image_type, 0 | 1 | 2 | 3 | 9 | 10 | 11);
    let w = u16::from_le_bytes([b[12], b[13]]) as u32;
    let h = u16::from_le_bytes([b[14], b[15]]) as u32;
    let depth = b[16];
    let dim_ok = w > 0 && h > 0 && w <= 65535 && h <= 65535 && w * h < 512 * 1024 * 1024;
    let depth_ok = matches!(depth, 8 | 16 | 24 | 32);
    cm_ok && it_ok && dim_ok && depth_ok
}

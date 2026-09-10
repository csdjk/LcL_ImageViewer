//! PSD 合成图（composite image）提取。
//!
//! 直接读取 PSD 文件末尾的 Image Data Section（Photoshop 保存的整图合成结果），
//! 不解析图层，速度快且所见即所得。这也是缩略图扩展提取 PSD 预览的同一条路径。
//!
//! 支持：version 1（PSD）、8bit 深度、RGB/灰度模式、RLE 或 RAW 压缩。

use crate::decode::{DecodeError, DecodedImage, ImageKind, MipLevel, PixelData};

fn read_u16(b: &[u8], off: usize) -> Result<u16, DecodeError> {
    let s = b.get(off..off + 2).ok_or(DecodeError::Truncated)?;
    Ok(u16::from_be_bytes([s[0], s[1]]))
}

fn read_u32(b: &[u8], off: usize) -> Result<u32, DecodeError> {
    let s = b.get(off..off + 4).ok_or(DecodeError::Truncated)?;
    Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

/// 提取 PSD 合成图为 RGBA。
pub fn decode_psd(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    if bytes.len() < 26 || !bytes.starts_with(b"8BPS") {
        return Err(DecodeError::NotAnImage("PSD 头无效".into()));
    }
    let version = read_u16(bytes, 4)?;
    if version != 1 {
        // PSB 大文件：结构差异较大，暂不支持
        return Err(DecodeError::UnsupportedFormat("PSB（version 2）暂不支持".into()));
    }
    let channels = read_u16(bytes, 12)?;
    let height = read_u32(bytes, 14)?;
    let width = read_u32(bytes, 18)?;
    let depth = read_u16(bytes, 22)?;
    let mode = read_u16(bytes, 24)?;

    if width == 0 || height == 0 || width > 30000 || height > 30000 {
        return Err(DecodeError::NotAnImage(format!("非法尺寸 {width}x{height}")));
    }
    if depth != 8 && depth != 16 {
        return Err(DecodeError::UnsupportedFormat(format!("PSD 深度 {depth} 暂不支持")));
    }
    let (n_color, has_alpha) = match mode {
        3 => (3usize, channels >= 4),  // RGB
        1 => (1usize, channels >= 2),  // Grayscale
        _ => {
            return Err(DecodeError::UnsupportedFormat(format!(
                "PSD 颜色模式 {mode}（CMYK/Indexed 等）暂不支持"
            )))
        }
    };

    // 跳过 Color Mode Data / Image Resources / Layer and Mask 三段
    let mut pos = 26usize;
    for _ in 0..3 {
        let len = read_u32(bytes, pos)? as usize;
        pos = pos
            .checked_add(4)
            .and_then(|p| p.checked_add(len))
            .ok_or(DecodeError::Truncated)?;
        if pos > bytes.len() {
            return Err(DecodeError::Truncated);
        }
    }

    // Image Data Section
    let compression = read_u16(bytes, pos)?;
    pos += 2;

    let px = width as usize * height as usize;

    let n_read_channels = n_color + if has_alpha { 1 } else { 0 };
    // 每通道平面数据
    let mut planes: Vec<Vec<u8>> = Vec::with_capacity(n_read_channels);

    match compression {
        0 => {
            // RAW：按通道平面顺序
            let plane_len = width as usize * (depth as usize / 8);
            for _ in 0..n_read_channels {
                let data = bytes
                    .get(pos..pos + plane_len * height as usize)
                    .ok_or(DecodeError::Truncated)?;
                planes.push(data.to_vec());
                pos += plane_len * height as usize;
            }
        }
        1 => {
            // RLE (PackBits)：先是每行每通道的 byte count 表，再是压缩数据
            let total_counts = height as usize * n_read_channels;
            let counts: Vec<usize> = (0..total_counts)
                .map(|i| read_u16(bytes, pos + i * 2).map(|v| v as usize))
                .collect::<Result<_, _>>()?;
            pos += total_counts * 2;
            let plane_len = width as usize * (depth as usize / 8);
            for (i, &cnt) in counts.iter().enumerate() {
                let src = bytes
                    .get(pos..pos + cnt)
                    .ok_or(DecodeError::Truncated)?;
                pos += cnt;
                let row = packbits_decode(src, plane_len)?;
                let plane_idx = i / height as usize;
                if plane_idx < n_read_channels {
                    while planes.len() <= plane_idx {
                        planes.push(Vec::with_capacity(plane_len * height as usize));
                    }
                    planes[plane_idx].extend_from_slice(&row);
                }
            }
        }
        other => {
            return Err(DecodeError::UnsupportedFormat(format!(
                "PSD 压缩方式 {other}（ZIP/ZIP prediction）暂不支持"
            )))
        }
    }

    // 平面 → 交错 RGBA
    let sample = |plane: &[u8], idx: usize| -> u8 {
        if depth == 8 {
            *plane.get(idx).unwrap_or(&0)
        } else {
            // 16bit：取高字节显示
            *plane.get(idx * 2).unwrap_or(&0)
        }
    };
    let mut rgba = Vec::with_capacity(px * 4);
    for i in 0..px {
        let (r, g, b) = if n_color == 3 {
            (
                sample(&planes[0], i),
                sample(&planes[1], i),
                sample(&planes[2], i),
            )
        } else {
            let l = sample(&planes[0], i);
            (l, l, l)
        };
        let a = if has_alpha {
            sample(&planes[n_color], i)
        } else {
            255
        };
        rgba.extend_from_slice(&[r, g, b, a]);
    }

    Ok(DecodedImage {
        width,
        height,
        mips: vec![MipLevel {
            width,
            height,
            data: PixelData::Rgba8(rgba),
        }],
        kind: ImageKind::Psd,
        compression: None,
        has_alpha,
        is_hdr: false,
        extra_meta: None,
        frames: Vec::new(),
    })
}

/// PackBits (RLE) 解码，输出恰好 `expected` 字节。
fn packbits_decode(src: &[u8], expected: usize) -> Result<Vec<u8>, DecodeError> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0usize;
    while i < src.len() && out.len() < expected {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            // 字面量：后跟 n+1 个字节
            let count = n as usize + 1;
            let end = i + count;
            if end > src.len() {
                return Err(DecodeError::Truncated);
            }
            out.extend_from_slice(&src[i..end]);
            i = end;
        } else if n != -128 {
            // 重复：下一字节重复 1-n 次
            let count = (1 - n as isize) as usize;
            let b = *src.get(i).ok_or(DecodeError::Truncated)?;
            out.extend(std::iter::repeat(b).take(count));
            i += 1;
        }
    }
    if out.len() != expected {
        return Err(DecodeError::Truncated);
    }
    Ok(out)
}

//! DDS 解码：手写 header 解析 + texture2ddecoder 的 BC 块解码。
//!
//! 支持：
//! - 传统 FourCC：DXT1~DXT5 / ATI1 / ATI2 / BC4U / BC5U
//! - DX10 扩展头：BC1~BC7 全系列、R8G8B8A8、B8G8R8A8、R16G16B16A16_FLOAT、R32G32B32A32_FLOAT
//! - 未压缩 32bpp（按位掩码识别 ARGB 变体）与 24bpp
//! - mip 链完整解码；cubemap / texture array 只解第 0 面
//!
//! 解码失败的非法数据一律返回 `Err`，不允许 panic。

use crate::decode::{DecodeError, DecodedImage, ImageKind, MipLevel, PixelData};

const DDSD_MIPMAPCOUNT: u32 = 0x20_0000;
const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const DDSCAPS2_CUBEMAP: u32 = 0x200;
const DDSCAPS2_VOLUME: u32 = 0x20_0000;

/// 块压缩格式种类（block compressed formats）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockFormat {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc6,
    Bc7,
}

impl BlockFormat {
    fn name(self) -> &'static str {
        match self {
            BlockFormat::Bc1 => "BC1 (DXT1)",
            BlockFormat::Bc2 => "BC2 (DXT3)",
            BlockFormat::Bc3 => "BC3 (DXT5)",
            BlockFormat::Bc4 => "BC4",
            BlockFormat::Bc5 => "BC5",
            BlockFormat::Bc6 => "BC6H",
            BlockFormat::Bc7 => "BC7",
        }
    }

    fn bytes_per_block(self) -> usize {
        match self {
            BlockFormat::Bc1 | BlockFormat::Bc4 => 8,
            _ => 16,
        }
    }

    fn has_alpha(self) -> bool {
        matches!(self, BlockFormat::Bc2 | BlockFormat::Bc3 | BlockFormat::Bc7)
    }
}

/// DX10 头中 DXGI_FORMAT 枚举值（仅列出本解码器支持的）。
#[derive(Debug, Clone, Copy)]
enum DxgiFormat {
    Rgba8 { bgra: bool, alpha: bool },
    Rgba16F,
    Rgba32F,
    Bc(BlockFormat),
    /// BC6H HDR 块压缩（signed 区分 SF16/UF16）
    Bc6 { signed: bool },
}

fn dxgi_from_u32(v: u32) -> Option<DxgiFormat> {
    Some(match v {
        28 | 29 => DxgiFormat::Rgba8 { bgra: false, alpha: true },
        87 | 91 => DxgiFormat::Rgba8 { bgra: true, alpha: true },
        88 | 93 => DxgiFormat::Rgba8 { bgra: true, alpha: false },
        10 => DxgiFormat::Rgba16F,
        2 => DxgiFormat::Rgba32F,
        71 | 72 => DxgiFormat::Bc(BlockFormat::Bc1),
        74 | 75 => DxgiFormat::Bc(BlockFormat::Bc2),
        77 | 78 => DxgiFormat::Bc(BlockFormat::Bc3),
        // The current BC4/5 decoder is unsigned; do not misread SNORM as UNORM.
        80 => DxgiFormat::Bc(BlockFormat::Bc4),
        83 => DxgiFormat::Bc(BlockFormat::Bc5),
        95 => DxgiFormat::Bc6 { signed: false }, // BC6H_UF16
        96 => DxgiFormat::Bc6 { signed: true },  // BC6H_SF16
        98 | 99 => DxgiFormat::Bc(BlockFormat::Bc7),
        _ => return None,
    })
}

fn read_u32(b: &[u8], off: usize) -> Result<u32, DecodeError> {
    let end = off.checked_add(4).ok_or(DecodeError::Truncated)?;
    let s = b.get(off..end).ok_or(DecodeError::Truncated)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// f16 → f32 位级转换（无需 half crate）。
fn f16_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) & 1) as u32;
    let exp = ((h >> 10) & 0x1F) as u32;
    let frac = (h & 0x3FF) as u32;
    let bits = match (exp, frac) {
        (0, 0) => sign << 31,
        (0, _) => {
            // 次正规数：归一化
            let mut e: i32 = -1;
            let mut f = frac;
            while f & 0x400 == 0 {
                f <<= 1;
                e -= 1;
            }
            f &= 0x3FF;
            ((sign << 31) | (((112 + e) as u32) << 23) | (f << 13)) as u32
        }
        (0x1F, 0) => (sign << 31) | (0xFF << 23),
        (0x1F, _) => (sign << 31) | (0xFF << 23) | (frac << 13),
        _ => (sign << 31) | ((exp + 112) << 23) | (frac << 13),
    };
    f32::from_bits(bits)
}

/// 解码整个 DDS 文件字节流。
pub fn decode_dds(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    if bytes.len() < 128 || !bytes.starts_with(b"DDS ") {
        return Err(DecodeError::NotAnImage("DDS 头无效".into()));
    }
    let height = read_u32(bytes, 12)?;
    let width = read_u32(bytes, 16)?;
    if width == 0 || height == 0 || width > 32768 || height > 32768 {
        return Err(DecodeError::NotAnImage(format!("非法尺寸 {width}x{height}")));
    }
    let flags = read_u32(bytes, 8)?;
    let mip_count = if flags & DDSD_MIPMAPCOUNT != 0 {
        read_u32(bytes, 28)?.max(1).min(32)
    } else {
        1
    };
    let caps2 = read_u32(bytes, 112)?;
    let is_cubemap = caps2 & DDSCAPS2_CUBEMAP != 0;
    let is_volume = caps2 & DDSCAPS2_VOLUME != 0;

    // 像素格式（DDSPF，偏移 76）
    let pf_size = read_u32(bytes, 76)?;
    if pf_size < 32 {
        return Err(DecodeError::NotAnImage("DDSPF 尺寸无效".into()));
    }
    let pf_flags = read_u32(bytes, 80)?;
    let fourcc = &bytes[84..88];
    let rgb_bit_count = read_u32(bytes, 88)?;
    let r_mask = read_u32(bytes, 92)?;
    let g_mask = read_u32(bytes, 96)?;
    let b_mask = read_u32(bytes, 100)?;
    let a_mask = read_u32(bytes, 104)?;

    let mut data_offset = 128usize;
    let (dxgi, compression, has_alpha, bpp, bgr_order): (
        Option<DxgiFormat>,
        Option<&'static str>,
        bool,
        u32,
        bool,
    ) = if pf_flags & DDPF_FOURCC != 0 {
            match fourcc {
                b"DXT1" | b"BC1U" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc1)),
                    Some(BlockFormat::Bc1.name()),
                    false,
                    0,
                    false,
                ),
                b"DXT2" | b"DXT3" | b"BC2U" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc2)),
                    Some(BlockFormat::Bc2.name()),
                    true,
                    0,
                    false,
                ),
                b"DXT4" | b"DXT5" | b"BC3U" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc3)),
                    Some(BlockFormat::Bc3.name()),
                    true,
                    0,
                    false,
                ),
                b"ATI1" | b"BC4U" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc4)),
                    Some(BlockFormat::Bc4.name()),
                    false,
                    0,
                    false,
                ),
                b"ATI2" | b"BC5U" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc5)),
                    Some(BlockFormat::Bc5.name()),
                    false,
                    0,
                    false,
                ),
                b"BC6H" | b"BC6U" => (
                    Some(DxgiFormat::Bc6 { signed: false }),
                    Some(BlockFormat::Bc6.name()),
                    false,
                    0,
                    false,
                ),
                b"BC7L" | b"BC7U" | b"BC7_" => (
                    Some(DxgiFormat::Bc(BlockFormat::Bc7)),
                    Some(BlockFormat::Bc7.name()),
                    true,
                    0,
                    false,
                ),
                b"DX10" => {
                    // 解析 DX10 扩展头（20 字节）
                    let dxgi_format = read_u32(bytes, data_offset)?;
                    data_offset += 20;
                    let dxgi = dxgi_from_u32(dxgi_format).ok_or_else(|| {
                        DecodeError::UnsupportedFormat(format!(
                            "DXGI_FORMAT {dxgi_format} 暂不支持"
                        ))
                    })?;
                    let (name, alpha) = match dxgi {
                        DxgiFormat::Bc(bc) => (Some(bc.name()), bc.has_alpha()),
                        DxgiFormat::Bc6 { .. } => (Some("BC6H"), false),
                        DxgiFormat::Rgba8 { alpha, .. } => (None, alpha),
                        DxgiFormat::Rgba16F | DxgiFormat::Rgba32F => (None, true),
                    };
                    (Some(dxgi), name, alpha, 0, false)
                }
                other => {
                    return Err(DecodeError::UnsupportedFormat(format!(
                        "未知 FourCC {:?}",
                        String::from_utf8_lossy(other)
                    )))
                }
            }
        } else if pf_flags & DDPF_RGB != 0 {
            // 未压缩 RGB(A)，按掩码识别
            let alpha = pf_flags & DDPF_ALPHAPIXELS != 0 && a_mask != 0;
            match (rgb_bit_count, r_mask, g_mask, b_mask) {
                (32, 0x00FF_0000, 0x0000_FF00, 0x0000_00FF) => {
                    (None, None, alpha, 32, true) // A8R8G8B8 / X8R8G8B8（字节序 B,G,R,A）
                }
                (32, 0x0000_00FF, 0x0000_FF00, 0x00FF_0000) => {
                    (None, None, alpha, 32, false) // A8B8G8R8（字节序 R,G,B,A）
                }
                (24, 0x00FF_0000, 0x0000_FF00, 0x0000_00FF) => (None, None, false, 24, true),
                (8, 0xFF, 0, 0) => (None, Some("Luminance8"), false, 8, false),
                _ => {
                    return Err(DecodeError::UnsupportedFormat(format!(
                        "未压缩 DDS 格式 bpp={rgb_bit_count} mask={r_mask:08X}/{g_mask:08X}/{b_mask:08X} 暂不支持"
                    )))
                }
            }
        } else {
            return Err(DecodeError::UnsupportedFormat(
                "DDS 像素格式无 FOURCC/RGB 标志".into(),
            ))
        };

    // 逐 mip 解码
    let mut mips = Vec::with_capacity(mip_count as usize);
    let mut mip_w = width;
    let mut mip_h = height;
    for _ in 0..mip_count {
        let pixels = decode_one_mip(
            bytes,
            data_offset,
            mip_w,
            mip_h,
            dxgi,
            bpp,
            has_alpha,
            bgr_order,
        )?;
        mips.push(MipLevel {
            width: mip_w,
            height: mip_h,
            data: pixels,
        });
        // 计算本 mip 占用的字节数，推进偏移
        data_offset += mip_size(mip_w, mip_h, dxgi, bpp)?;
        mip_w = (mip_w / 2).max(1);
        mip_h = (mip_h / 2).max(1);
        if data_offset > bytes.len() && mips.len() < mip_count as usize {
            // 数据不足，保留已解出的 mip
            break;
        }
    }
    if mips.is_empty() {
        return Err(DecodeError::Truncated);
    }

    // BC6H 由 texture2ddecoder 输出 8bit 像素，不标记 HDR（真实 HDR 仅 Rgba16F/32F）
    let is_hdr = matches!(dxgi, Some(DxgiFormat::Rgba16F) | Some(DxgiFormat::Rgba32F));
    let kind = if is_cubemap {
        ImageKind::DdsCube
    } else if is_volume {
        ImageKind::DdsVolume
    } else {
        ImageKind::Dds
    };

    let mut notes = Vec::new();
    if is_cubemap { notes.push("cubemap（显示第 1 面）"); }
    if is_volume { notes.push("volume（显示第 1 层）"); }
    if matches!(dxgi, Some(DxgiFormat::Bc6 { .. })) {
        notes.push("BC6H：8 位预览，未保留 HDR 浮点值");
    }
    Ok(DecodedImage {
        width,
        height,
        mips,
        kind,
        compression: compression.map(|s| s.to_string()),
        has_alpha,
        is_hdr,
        extra_meta: (!notes.is_empty()).then(|| notes.join("；")),
        frames: Vec::new(),
    })
}

/// 单个 mip 的字节尺寸。
fn mip_size(w: u32, h: u32, dxgi: Option<DxgiFormat>, bpp: u32) -> Result<usize, DecodeError> {
    match dxgi {
        Some(DxgiFormat::Bc(bc)) => {
            let blocks = ((w as usize + 3) / 4) * ((h as usize + 3) / 4);
            Ok(blocks * bc.bytes_per_block())
        }
        Some(DxgiFormat::Bc6 { .. }) => {
            let blocks = ((w as usize + 3) / 4) * ((h as usize + 3) / 4);
            Ok(blocks * 16)
        }
        _ => {
            let w = w as usize;
            let h = h as usize;
            let bpp = if bpp == 0 {
                match dxgi {
                    Some(DxgiFormat::Rgba16F) => 64,
                    Some(DxgiFormat::Rgba32F) => 128,
                    Some(DxgiFormat::Rgba8 { .. }) => 32,
                    _ => 32,
                }
            } else {
                bpp
            };
            Ok(w
                .checked_mul(h)
                .and_then(|v| v.checked_mul(bpp as usize / 8))
                .ok_or(DecodeError::Truncated)?)
        }
    }
}

/// 解码一个 mip 的像素数据。
fn decode_one_mip(
    bytes: &[u8],
    offset: usize,
    w: u32,
    h: u32,
    dxgi: Option<DxgiFormat>,
    bpp: u32,
    has_alpha: bool,
    bgr_order: bool,
) -> Result<PixelData, DecodeError> {
    let w_us = w as usize;
    let h_us = h as usize;
    let px_count = w_us.checked_mul(h_us).ok_or(DecodeError::Truncated)?;

    match dxgi {
        Some(DxgiFormat::Bc(bc)) => {
            let block_bytes = mip_size(w, h, dxgi, 0)?;
            let data = bytes
                .get(offset..offset + block_bytes)
                .ok_or(DecodeError::Truncated)?;
            let mut out = vec![0u32; px_count];
            decode_bc_blocks(bc, data, w_us, h_us, &mut out)?;
            Ok(PixelData::Rgba8(u32s_to_rgba8(&out)))
        }
        Some(DxgiFormat::Bc6 { signed }) => {
            let block_bytes = mip_size(w, h, dxgi, 0)?;
            let data = bytes
                .get(offset..offset + block_bytes)
                .ok_or(DecodeError::Truncated)?;
            let mut out = vec![0u32; px_count];
            texture2ddecoder::decode_bc6(data, w_us, h_us, &mut out, signed)
                .map_err(|e| DecodeError::Decode(format!("BC6H 解码失败: {e}")))?;
            Ok(PixelData::Rgba8(u32s_to_rgba8(&out)))
        }
        Some(DxgiFormat::Rgba8 { bgra, alpha }) => {
            let byte_len = px_count * 4;
            let data = bytes
                .get(offset..offset + byte_len)
                .ok_or(DecodeError::Truncated)?;
            let mut rgba = Vec::with_capacity(byte_len);
            if bgra {
                for px in data.chunks_exact(4) {
                    rgba.extend_from_slice(&[px[2], px[1], px[0], if alpha { px[3] } else { 255 }]);
                }
            } else {
                rgba.extend_from_slice(data);
            }
            Ok(PixelData::Rgba8(rgba))
        }
        Some(DxgiFormat::Rgba16F) => {
            let byte_len = px_count * 8;
            let data = bytes
                .get(offset..offset + byte_len)
                .ok_or(DecodeError::Truncated)?;
            let mut rgba = Vec::with_capacity(px_count * 4);
            for px in data.chunks_exact(8) {
                rgba.push(f16_to_f32(u16::from_le_bytes([px[0], px[1]])));
                rgba.push(f16_to_f32(u16::from_le_bytes([px[2], px[3]])));
                rgba.push(f16_to_f32(u16::from_le_bytes([px[4], px[5]])));
                rgba.push(f16_to_f32(u16::from_le_bytes([px[6], px[7]])));
            }
            Ok(PixelData::RgbaF32(rgba))
        }
        Some(DxgiFormat::Rgba32F) => {
            let byte_len = px_count * 16;
            let data = bytes
                .get(offset..offset + byte_len)
                .ok_or(DecodeError::Truncated)?;
            let mut rgba = Vec::with_capacity(px_count * 4);
            for px in data.chunks_exact(16) {
                rgba.push(f32::from_le_bytes([px[0], px[1], px[2], px[3]]));
                rgba.push(f32::from_le_bytes([px[4], px[5], px[6], px[7]]));
                rgba.push(f32::from_le_bytes([px[8], px[9], px[10], px[11]]));
                rgba.push(f32::from_le_bytes([px[12], px[13], px[14], px[15]]));
            }
            Ok(PixelData::RgbaF32(rgba))
        }
        None => {
            // 传统未压缩格式
            match bpp {
                32 => {
                    let data = bytes
                        .get(offset..offset + px_count * 4)
                        .ok_or(DecodeError::Truncated)?;
                    let mut rgba = Vec::with_capacity(px_count * 4);
                    for px in data.chunks_exact(4) {
                        // bgr_order: 字节序 [B, G, R, A/X]；否则 [R, G, B, A/X]
                        // X8R8G8B8 等无 alpha 格式强制不透明
                        let a = if has_alpha { px[3] } else { 255 };
                        if bgr_order {
                            rgba.extend_from_slice(&[px[2], px[1], px[0], a]);
                        } else {
                            rgba.extend_from_slice(&[px[0], px[1], px[2], a]);
                        }
                    }
                    Ok(PixelData::Rgba8(rgba))
                }
                24 => {
                    let data = bytes
                        .get(offset..offset + px_count * 3)
                        .ok_or(DecodeError::Truncated)?;
                    let mut rgba = Vec::with_capacity(px_count * 4);
                    for px in data.chunks_exact(3) {
                        rgba.extend_from_slice(&[px[2], px[1], px[0], 255]);
                    }
                    Ok(PixelData::Rgba8(rgba))
                }
                8 => {
                    let data = bytes
                        .get(offset..offset + px_count)
                        .ok_or(DecodeError::Truncated)?;
                    let mut rgba = Vec::with_capacity(px_count * 4);
                    for &l in data {
                        rgba.extend_from_slice(&[l, l, l, 255]);
                    }
                    Ok(PixelData::Rgba8(rgba))
                }
                other => Err(DecodeError::UnsupportedFormat(format!(
                    "未压缩 bpp={other} 暂不支持"
                ))),
            }
        }
    }
}

/// texture2ddecoder 输出的 u32（color() = from_le_bytes([b,g,r,a])，低字节 B）→ RGBA8 字节。
fn u32s_to_rgba8(px: &[u32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(px.len() * 4);
    for &p in px {
        v.push((p >> 16) as u8); // R
        v.push((p >> 8) as u8);  // G
        v.push(p as u8);         // B
        v.push((p >> 24) as u8); // A
    }
    v
}

/// 调用 texture2ddecoder 解码 BC 块。
fn decode_bc_blocks(
    bc: BlockFormat,
    data: &[u8],
    w: usize,
    h: usize,
    out: &mut [u32],
) -> Result<(), DecodeError> {
    let need = ((w + 3) / 4) * ((h + 3) / 4) * bc.bytes_per_block();
    if data.len() < need {
        return Err(DecodeError::Truncated);
    }
    let r = match bc {
        BlockFormat::Bc1 => texture2ddecoder::decode_bc1(data, w, h, out),
        BlockFormat::Bc2 => texture2ddecoder::decode_bc2(data, w, h, out),
        BlockFormat::Bc3 => texture2ddecoder::decode_bc3(data, w, h, out),
        BlockFormat::Bc4 => texture2ddecoder::decode_bc4(data, w, h, out),
        BlockFormat::Bc5 => texture2ddecoder::decode_bc5(data, w, h, out),
        BlockFormat::Bc7 => texture2ddecoder::decode_bc7(data, w, h, out),
        BlockFormat::Bc6 => unreachable!("BC6 由 decode_bc6 单独处理"),
    };
    r.map_err(|e| DecodeError::Decode(format!("BC 解码失败: {e}")))
}

/// 读 DDS 头部（用于快速报尺寸等元信息，不解码像素）。
pub fn dds_header_size(bytes: &[u8]) -> Option<(u32, u32, u32)> {
    if bytes.len() < 128 || !bytes.starts_with(b"DDS ") {
        return None;
    }
    let h = read_u32(bytes, 12).ok()?;
    let w = read_u32(bytes, 16).ok()?;
    let flags = read_u32(bytes, 8).ok()?;
    let mips = if flags & DDSD_MIPMAPCOUNT != 0 {
        read_u32(bytes, 28).unwrap_or(1).max(1)
    } else {
        1
    };
    Some((w, h, mips))
}

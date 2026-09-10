//! 生成一套测试图片到 test_images/，用于查看器的实际打开验证。
//!
//! 运行：cargo run -p iv-core --example gen_test_images

use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("test_images");
    std::fs::create_dir_all(dir)?;

    // ---------- PNG / JPG / TGA（带 alpha 渐变） ----------
    let w = 256u32;
    let h = 256u32;
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        image::Rgba([
            (x * 255 / 255) as u8,
            (y) as u8,
            ((x + y) % 256) as u8,
            (x as f32 / w as f32 * 255.0) as u8,
        ])
    });
    let dynimg = image::DynamicImage::ImageRgba8(img);
    dynimg.save(dir.join("gradient.png"))?;
    {
        let rgb = dynimg.to_rgb8();
        image::DynamicImage::ImageRgb8(rgb)
            .save_with_format(dir.join("gradient.jpg"), image::ImageFormat::Jpeg)?;
    }
    dynimg.save_with_format(dir.join("gradient.tga"), image::ImageFormat::Tga)?;
    println!("PNG/JPG/TGA 完成");

    // ---------- BC1 DDS（8x8 色块网格 + mip 链） ----------
    let bc1 = make_bc1_mips();
    std::fs::write(dir.join("bc1_mips.dds"), bc1)?;
    println!("BC1 mip 链完成");

    // ---------- BC3 DDS（蓝色 + alpha 渐变） ----------
    let bc3 = make_bc3_alpha();
    std::fs::write(dir.join("bc3_alpha.dds"), bc3)?;
    println!("BC3 完成");

    // ---------- BC5 DDS（法线图风格：R 水平梯度 / G 垂直梯度） ----------
    let bc5 = make_bc5();
    std::fs::write(dir.join("bc5_normal.dds"), bc5)?;
    println!("BC5 完成");

    // ---------- 未压缩 RGBA8 DDS ----------
    let raw = make_uncompressed_dds();
    std::fs::write(dir.join("rgba8888.dds"), raw)?;
    println!("未压缩 DDS 完成");

    // ---------- PSD（RAW 合成图，64x64 棋盘） ----------
    let psd = make_psd();
    std::fs::write(dir.join("checker.psd"), psd)?;
    println!("PSD 完成");

    println!("\n全部测试图已生成到 {}", dir.display());
    Ok(())
}

fn le_u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// RGB → RGB565
fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)
}

/// 写传统 DDS 头（128 字节，DXT fourcc，含 mip 数）
fn dds_header_fourcc(fourcc: &[u8; 4], w: u32, h: u32, mips: u32, linear_size: u32) -> Vec<u8> {
    let mut d = Vec::with_capacity(128);
    d.extend_from_slice(b"DDS ");
    d.extend_from_slice(&le_u32(124));
    let flags = 0x4 | 0x2 | 0x1000 | 0x80000 | 0x20000;
    d.extend_from_slice(&le_u32(flags));
    d.extend_from_slice(&le_u32(h));
    d.extend_from_slice(&le_u32(w));
    d.extend_from_slice(&le_u32(linear_size));
    d.extend_from_slice(&le_u32(0));
    d.extend_from_slice(&le_u32(mips));
    d.extend_from_slice(&[0u8; 44]);
    d.extend_from_slice(&le_u32(32));
    d.extend_from_slice(&le_u32(0x4));
    d.extend_from_slice(fourcc);
    d.extend_from_slice(&[0u8; 20]);
    d.extend_from_slice(&le_u32(0x1000));
    d.extend_from_slice(&[0u8; 16]);
    debug_assert_eq!(d.len(), 128);
    d
}

/// 32x32、3 mip 的 BC1 DDS，每 mip 是 4 色象限块网格
fn make_bc1_mips() -> Vec<u8> {
    let total_blocks_mips: u32 = (8 * 8) + (4 * 4) + (2 * 2);
    let mut d = dds_header_fourcc(b"DXT1", 32, 32, 3, total_blocks_mips * 8);

    let palette = [
        (255u8, 0u8, 0u8),
        (0, 255, 0),
        (0, 0, 255),
        (255, 255, 255),
    ];
    let mut w = 32u32;
    let mut h = 32u32;
    let mut mip = 0;
    while w > 0 && h > 0 && mip < 3 {
        let blocks_x = (w + 3) / 4;
        let blocks_y = (h + 3) / 4;
        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let (r, g, b) = palette[(((bx / 2) + (by / 2) * 2) as usize) % 4];
                let c0 = rgb565(r, g, b);
                let c1 = rgb565(r, g, b);
                d.extend_from_slice(&c0.to_le_bytes());
                d.extend_from_slice(&c1.to_le_bytes());
                d.extend_from_slice(&[0, 0, 0, 0]); // 全 idx0 → c0
            }
        }
        w /= 2;
        h /= 2;
        mip += 1;
    }
    d
}

/// 32x32 BC3：纯蓝 RGB + alpha 水平渐变（每列 alpha 不同）
fn make_bc3_alpha() -> Vec<u8> {
    let mut d = dds_header_fourcc(b"DXT5", 32, 32, 1, 8 * 8 * 16);
    // alpha 块索引：8-index 模式（a0 > a1），alpha_i = a0 - i*(a0-a1)/7
    // 列 x 的 alpha 目标 = x*85 → idx ≈ (255 - x*85)*7/255
    let alpha_idx = |x: usize| -> u32 { ((255 - (x * 85) as u32) * 7 / 255) as u32 };
    for _block in 0..8 * 8 {
        // alpha block (8 bytes)
        d.extend_from_slice(&[255u8, 0u8]);
        // 16 像素 3bit 索引：48bit = 6 字节（u64 小端的低 6 字节）
        let mut bits: u64 = 0;
        for py in 0..4usize {
            for px in 0..4usize {
                let idx = alpha_idx(px);
                let bit_pos = (py * 4 + px) * 3;
                bits |= (idx as u64) << bit_pos;
            }
        }
        d.extend_from_slice(&bits.to_le_bytes()[..6]);
        // color block (8 bytes)：纯蓝
        let c = rgb565(0, 0, 255);
        d.extend_from_slice(&c.to_le_bytes());
        d.extend_from_slice(&c.to_le_bytes());
        d.extend_from_slice(&[0, 0, 0, 0]);
    }
    d
}

/// 32x32 BC5（法线图风格）：R 块水平渐变、G 块垂直渐变
fn make_bc5() -> Vec<u8> {
    let mut d = dds_header_fourcc(b"ATI2", 32, 32, 1, 8 * 8 * 16);
    for _block in 0..8 * 8 {
        // R 通道块（BC4 格式）：水平渐变。16 × 3bit 索引 = 6 字节
        d.extend_from_slice(&[255u8, 0u8]);
        let mut bits: u64 = 0;
        for py in 0..4usize {
            for px in 0..4usize {
                let idx = ((255 - (px * 85) as u32) * 7 / 255) as u64;
                bits |= idx << ((py * 4 + px) * 3);
            }
        }
        d.extend_from_slice(&bits.to_le_bytes()[..6]);
        // G 通道块：垂直渐变
        d.extend_from_slice(&[255u8, 0u8]);
        let mut bits: u64 = 0;
        for py in 0..4usize {
            for px in 0..4usize {
                let idx = ((255 - (py * 85) as u32) * 7 / 255) as u64;
                bits |= idx << ((py * 4 + px) * 3);
            }
        }
        d.extend_from_slice(&bits.to_le_bytes()[..6]);
    }
    d
}

/// 64x64 未压缩 RGBA8 DDS（对角渐变）
fn make_uncompressed_dds() -> Vec<u8> {
    let w = 64u32;
    let h = 64u32;
    let mut d = Vec::new();
    d.extend_from_slice(b"DDS ");
    d.extend_from_slice(&le_u32(124));
    let flags = 0x4 | 0x2 | 0x1000 | 0x8 | 0x20000;
    d.extend_from_slice(&le_u32(flags));
    d.extend_from_slice(&le_u32(h));
    d.extend_from_slice(&le_u32(w));
    d.extend_from_slice(&le_u32(w * 4));
    d.extend_from_slice(&le_u32(0));
    d.extend_from_slice(&le_u32(1));
    d.extend_from_slice(&[0u8; 44]);
    d.extend_from_slice(&le_u32(32));
    d.extend_from_slice(&le_u32(0x1 | 0x40)); // ALPHAPIXELS | RGB
    d.extend_from_slice(&[0u8; 4]); // fourcc 空
    d.extend_from_slice(&le_u32(32));
    d.extend_from_slice(&le_u32(0x00FF_0000));
    d.extend_from_slice(&le_u32(0x0000_FF00));
    d.extend_from_slice(&le_u32(0x0000_00FF));
    d.extend_from_slice(&le_u32(0xFF00_0000));
    d.extend_from_slice(&le_u32(0x1000));
    d.extend_from_slice(&[0u8; 16]);
    for y in 0..h {
        for x in 0..w {
            let r = (x * 4) as u8;
            let g = (y * 4) as u8;
            let b = 128;
            let a = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 128 };
            // 字节序 [B, G, R, A]
            d.extend_from_slice(&[b, g, r, a]);
        }
    }
    d
}

/// 64x64 RAW PSD：红绿蓝白 + 棋盘 alpha
fn make_psd() -> Vec<u8> {
    let w = 64u32;
    let h = 64u32;
    let mut d = Vec::new();
    d.extend_from_slice(b"8BPS");
    d.extend_from_slice(&1u16.to_be_bytes());
    d.extend_from_slice(&[0u8; 6]);
    d.extend_from_slice(&4u16.to_be_bytes()); // channels: R G B A
    d.extend_from_slice(&h.to_be_bytes());
    d.extend_from_slice(&w.to_be_bytes());
    d.extend_from_slice(&8u16.to_be_bytes());
    d.extend_from_slice(&3u16.to_be_bytes()); // RGB
    d.extend_from_slice(&0u32.to_be_bytes()); // color mode
    d.extend_from_slice(&0u32.to_be_bytes()); // resources
    d.extend_from_slice(&0u32.to_be_bytes()); // layer & mask
    d.extend_from_slice(&0u16.to_be_bytes()); // compression RAW
    for plane in 0..4usize {
        for y in 0..h as usize {
            for x in 0..w as usize {
                let v = match plane {
                    0 => (x * 4) as u8, // R
                    1 => (y * 4) as u8, // G
                    2 => 200,           // B
                    _ => {
                        if (x / 8 + y / 8) % 2 == 0 {
                            255
                        } else {
                            96
                        }
                    } // A 棋盘
                };
                let _ = d.write(&[v]);
            }
        }
    }
    d
}

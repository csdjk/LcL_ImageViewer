//! iv-core 解码测试：手工构造最小合法文件验证各格式路径。

use iv_core::decode::{decode_bytes, ImageKind, PixelData};
use iv_core::format::detect_format;
use iv_core::format::ImageFormat;

fn le_u32(v: u32) -> [u8; 4] {
    v.to_le_bytes()
}

/// 构造一个传统头 DDS（DXT1，4x4，1 mip，单一色块）。
fn make_bc1_dds() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"DDS ");
    d.extend_from_slice(&le_u32(124)); // header size
    let flags = 0x4 | 0x2 | 0x1000 | 0x80000 | 0x20000; // WIDTH|HEIGHT|PIXELFORMAT|LINEARSIZE|MIPMAPCOUNT
    d.extend_from_slice(&le_u32(flags));
    d.extend_from_slice(&le_u32(4)); // height
    d.extend_from_slice(&le_u32(4)); // width
    d.extend_from_slice(&le_u32(8)); // pitch/linear size
    d.extend_from_slice(&le_u32(0)); // depth
    d.extend_from_slice(&le_u32(1)); // mip count
    d.extend_from_slice(&[0u8; 44]); // reserved1[11]
    // DDSPF
    d.extend_from_slice(&le_u32(32)); // pf size
    d.extend_from_slice(&le_u32(0x4)); // DDPF_FOURCC
    d.extend_from_slice(b"DXT1");
    d.extend_from_slice(&le_u32(0)); // rgb bit count
    d.extend_from_slice(&le_u32(0)); // r mask
    d.extend_from_slice(&le_u32(0)); // g mask
    d.extend_from_slice(&le_u32(0)); // b mask
    d.extend_from_slice(&le_u32(0)); // a mask
    d.extend_from_slice(&le_u32(0x1000)); // caps
    d.extend_from_slice(&le_u32(0)); // caps2
    d.extend_from_slice(&le_u32(0)); // caps3
    d.extend_from_slice(&le_u32(0)); // caps4
    d.extend_from_slice(&le_u32(0)); // reserved2
    assert_eq!(d.len(), 128);
    // 一个 BC1 块：c0=0xF800(红) c1=0x07E0(绿)，索引全 0 → 每像素取 c0（纯红）
    // c0 > c1 → 4 色模式，idx=0 表示颜色 c0；选红色可验证 R/B 通道顺序
    d.extend_from_slice(&0xF800u16.to_le_bytes());
    d.extend_from_slice(&0x07E0u16.to_le_bytes());
    d.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]);
    d
}

/// 构造未压缩 32bpp RGBA DDS（2x1 两像素：红、半透明绿）。
fn make_uncompressed_dds() -> Vec<u8> {
    let mut d = Vec::new();
    d.extend_from_slice(b"DDS ");
    d.extend_from_slice(&le_u32(124));
    let flags = 0x4 | 0x2 | 0x1000 | 0x8 | 0x20000; // + PITCH
    d.extend_from_slice(&le_u32(flags));
    d.extend_from_slice(&le_u32(1)); // height
    d.extend_from_slice(&le_u32(2)); // width
    d.extend_from_slice(&le_u32(8)); // pitch
    d.extend_from_slice(&le_u32(0));
    d.extend_from_slice(&le_u32(1));
    d.extend_from_slice(&[0u8; 44]);
    d.extend_from_slice(&le_u32(32));
    d.extend_from_slice(&le_u32(0x1 | 0x40)); // ALPHAPIXELS | RGB
    d.extend_from_slice(&[0, 0, 0, 0]); // fourcc 空
    d.extend_from_slice(&le_u32(32)); // bpp
    d.extend_from_slice(&le_u32(0x00FF_0000)); // r
    d.extend_from_slice(&le_u32(0x0000_FF00)); // g
    d.extend_from_slice(&le_u32(0x0000_00FF)); // b
    d.extend_from_slice(&le_u32(0xFF00_0000)); // a
    d.extend_from_slice(&le_u32(0x1000));
    d.extend_from_slice(&[0u8; 16]);
    // 像素按 [B, G, R, A]
    d.extend_from_slice(&[0, 0, 255, 255]); // 红
    d.extend_from_slice(&[0, 255, 0, 128]); // 半透明绿
    d
}

/// 构造最小 RAW 压缩 PSD（2x2 RGB）。
fn make_psd_raw() -> Vec<u8> {
    use std::io::Write;
    let mut d = Vec::new();
    d.extend_from_slice(b"8BPS");
    d.extend_from_slice(&1u16.to_be_bytes()); // version
    d.extend_from_slice(&[0u8; 6]); // reserved
    d.extend_from_slice(&3u16.to_be_bytes()); // channels
    d.extend_from_slice(&2u32.to_be_bytes()); // height
    d.extend_from_slice(&2u32.to_be_bytes()); // width
    d.extend_from_slice(&8u16.to_be_bytes()); // depth
    d.extend_from_slice(&3u16.to_be_bytes()); // mode RGB
    d.extend_from_slice(&0u32.to_be_bytes()); // color mode data len
    d.extend_from_slice(&0u32.to_be_bytes()); // image resources len
    d.extend_from_slice(&0u32.to_be_bytes()); // layer&mask len
    d.extend_from_slice(&0u16.to_be_bytes()); // compression: RAW
    // planar 数据，行序：R 平面（4 像素）、G、B
    let _ = d.write(&[255, 0, 0, 255]); // R: 红 绿 蓝 白
    let _ = d.write(&[0, 255, 0, 255]); // G
    let _ = d.write(&[0, 0, 255, 255]); // B
    d
}

#[test]
fn detect_formats() {
    assert_eq!(detect_format(&make_bc1_dds()), ImageFormat::Dds);
    assert_eq!(detect_format(&make_psd_raw()), ImageFormat::Psd);
    assert_eq!(
        detect_format(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0]),
        ImageFormat::Png
    );
    assert_eq!(
        detect_format(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0, 0, 0, 0, 0]),
        ImageFormat::Jpeg
    );
}

#[test]
fn decode_dds_bc1() {
    let img = decode_bytes(&make_bc1_dds()).expect("BC1 DDS 解码失败");
    assert_eq!((img.width, img.height), (4, 4));
    assert_eq!(img.kind, ImageKind::Dds);
    assert_eq!(img.compression.as_deref(), Some("BC1 (DXT1)"));
    assert_eq!(img.mips.len(), 1);
    // 验证 BC1 解码像素布局：所有像素应为纯红 [255,0,0,255]
    // （红色块可验证 texture2ddecoder 输出的 u32 通道顺序是否被正确转换）
    let mip = &img.mips[0];
    match &mip.data {
        PixelData::Rgba8(v) => {
            for px in v.chunks_exact(4) {
                assert_eq!(px, &[255, 0, 0, 255], "BC1 像素应为纯红，实际 {px:?}");
            }
        }
        _ => panic!("BC1 应输出 Rgba8"),
    }
}

#[test]
fn decode_dds_uncompressed() {
    let img = decode_bytes(&make_uncompressed_dds()).expect("未压缩 DDS 解码失败");
    let mip = &img.mips[0];
    assert_eq!(mip.rgba8_at(0, 0), Some([255, 0, 0, 255]));
    assert_eq!(mip.rgba8_at(1, 0), Some([0, 255, 0, 128]));
    assert_eq!(mip.rgba8_at(2, 0), None); // 越界
}

#[test]
fn decode_psd_minimal() {
    let img = decode_bytes(&make_psd_raw()).expect("PSD 解码失败");
    assert_eq!((img.width, img.height), (2, 2));
    assert_eq!(img.kind, ImageKind::Psd);
    let mip = &img.mips[0];
    assert_eq!(mip.rgba8_at(0, 0), Some([255, 0, 0, 255])); // 红
    assert_eq!(mip.rgba8_at(1, 0), Some([0, 255, 0, 255])); // 绿
    assert_eq!(mip.rgba8_at(0, 1), Some([0, 0, 255, 255])); // 蓝
    assert_eq!(mip.rgba8_at(1, 1), Some([255, 255, 255, 255])); // 白
}

#[test]
fn png_roundtrip() {
    // 用 image crate 编码一张带 alpha 的 PNG，再走 decode_bytes
    let img = image::RgbaImage::from_fn(3, 2, |x, y| {
        image::Rgba([(x * 80) as u8, (y * 120) as u8, 7, 200])
    });
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    let decoded = decode_bytes(&png).expect("PNG 解码失败");
    assert_eq!((decoded.width, decoded.height), (3, 2));
    assert!(decoded.has_alpha);
    assert_eq!(decoded.mips[0].rgba8_at(2, 1), Some([160, 120, 7, 200]));
}

#[test]
fn tga_roundtrip() {
    let img = image::RgbaImage::from_fn(2, 2, |x, _| image::Rgba([x as u8 * 100, 10, 20, 255]));
    let mut tga = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut tga), image::ImageFormat::Tga)
        .unwrap();
    let decoded = decode_bytes(&tga).expect("TGA 解码失败");
    assert_eq!(decoded.kind, ImageKind::Tga);
    assert_eq!((decoded.width, decoded.height), (2, 2));
}

#[test]
fn truncated_files_do_not_panic() {
    // 各种截断输入都必须返回 Err，不允许 panic
    let dds = make_bc1_dds();
    for cut in [4usize, 60, 128, 130] {
        let r = decode_bytes(&dds[..cut]);
        assert!(r.is_err(), "截断到 {cut} 应报错");
    }
    let psd = make_psd_raw();
    for cut in [4usize, 30, 40] {
        assert!(decode_bytes(&psd[..cut]).is_err());
    }
    // 纯垃圾
    assert!(decode_bytes(&[0u8; 64]).is_err());
    assert!(decode_bytes(b"").is_err());
}

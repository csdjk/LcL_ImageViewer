//! Standard DXGI identifiers must select the correct pixel layout.
use iv_core::dds::dds_header_size;
use iv_core::decode::{decode_bytes, DecodeError};

fn dds(format: u32, width: u32, height: u32, payload: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 148];
    bytes[..4].copy_from_slice(b"DDS ");
    for (offset, value) in [
        (4, 124u32),
        (8, 0x1007),
        (12, height),
        (16, width),
        (76, 32),
        (80, 4),
        (108, 0x1000),
        (128, format),
        (132, 3),
        (140, 1),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[84..88].copy_from_slice(b"DX10");
    bytes.extend_from_slice(payload);
    bytes
}

#[test]
fn bc6h_standard_unsigned_and_signed_headers_decode() {
    // Mode 0, zero endpoints/indices: a black block in either representation.
    for format in [95, 96] {
        let image = decode_bytes(&dds(format, 4, 4, &[0; 16])).unwrap();
        assert_eq!(image.compression.as_deref(), Some("BC6H"));
        assert_eq!(image.mips[0].rgba8_at(0, 0), Some([0, 0, 0, 255]));
        assert!(!image.is_hdr, "8-bit preview must not claim float HDR");
        assert!(image.extra_meta.unwrap().contains("8 位预览"));
    }
}

#[test]
fn bgra_and_bgrx_do_not_enter_the_bc6h_decoder() {
    for format in [87, 91, 88, 93] {
        let image = decode_bytes(&dds(format, 1, 1, &[10, 20, 30, 7])).unwrap();
        let alpha = if matches!(format, 88 | 93) { 255 } else { 7 };
        assert_eq!(image.mips[0].rgba8_at(0, 0), Some([30, 20, 10, alpha]));
        assert_eq!(image.has_alpha, alpha == 7);
        assert!(image.compression.is_none());
    }
}

#[test]
fn typeless_and_signed_bc4_bc5_are_not_silently_reinterpreted() {
    for format in [90, 92, 94, 81, 84, 999] {
        assert!(
            matches!(
                decode_bytes(&dds(format, 4, 4, &[0; 64])),
                Err(DecodeError::UnsupportedFormat(_))
            ),
            "format {format}"
        );
    }
}

#[test]
fn dx10_truncated_payload_is_rejected() {
    for format in [95, 96, 91, 93] {
        assert!(decode_bytes(&dds(format, 4, 4, &[0; 3])).is_err());
    }
}

#[test]
fn standard_mipmap_flag_exposes_and_decodes_all_levels() {
    let mut pixels = vec![20, 30, 40, 255].repeat(16);
    pixels.extend(vec![100, 120, 140, 255].repeat(4));
    let mut bytes = dds(28, 4, 4, &pixels);
    bytes[8..12].copy_from_slice(&0x2100Fu32.to_le_bytes());
    bytes[28..32].copy_from_slice(&2u32.to_le_bytes());
    assert_eq!(dds_header_size(&bytes), Some((4, 4, 2)));
    let image = decode_bytes(&bytes).unwrap();
    assert_eq!(image.mips.len(), 2);
    assert_eq!(image.mips[0].rgba8_at(0, 0), Some([20, 30, 40, 255]));
    assert_eq!(image.mips[1].rgba8_at(0, 0), Some([100, 120, 140, 255]));
}

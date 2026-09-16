use iv_core::{
    decode::decode_preview_bytes,
    decode_bytes, decode_path,
    format::{detect_format, has_supported_ext, ImageFormat},
    ImageKind, PixelData,
};
use std::path::Path;

const ALPHA: &[u8] = include_bytes!("fixtures/avif/alpha.avif");
const OPAQUE: &[u8] = include_bytes!("fixtures/avif/opaque.avif");
const ANIMATED: &[u8] = include_bytes!("fixtures/avif/animated.avif");

fn rgba(image: &iv_core::DecodedImage) -> &[u8] {
    match &image.mips[0].data {
        PixelData::Rgba8(p) => p,
        _ => panic!("expected RGBA8"),
    }
}
fn reference(name: &str) -> Vec<u8> {
    image::open(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/avif")
            .join(name),
    )
    .unwrap()
    .into_rgba8()
    .into_raw()
}
fn compare(bytes: &[u8], name: &str) -> iv_core::DecodedImage {
    let decoded = decode_bytes(bytes).unwrap();
    assert_eq!(decoded.kind, ImageKind::Avif);
    assert_eq!(decoded.kind.label(), "AVIF");
    assert_eq!((decoded.width, decoded.height), (160, 96));
    assert_eq!(decoded.mips.len(), 1);
    assert!(!decoded.is_hdr);
    let expected = reference(name);
    assert_eq!(rgba(&decoded).len(), expected.len());
    for (actual, expected) in rgba(&decoded).chunks_exact(4).zip(expected.chunks_exact(4)) {
        assert_eq!(actual[3], expected[3], "alpha must be exact");
        if actual[3] != 0 {
            for channel in 0..3 {
                assert!(
                    actual[channel].abs_diff(expected[channel]) <= 2,
                    "{actual:?} != {expected:?}"
                );
            }
        }
    }
    decoded
}
#[test]
fn opaque_avif_matches_independent_reference() {
    let decoded = compare(OPAQUE, "opaque-reference.png");
    assert!(!decoded.has_alpha);
    assert!(rgba(&decoded).chunks_exact(4).all(|p| p[3] == 255));
}
#[test]
fn alpha_avif_preserves_source_transparency_and_straight_rgb() {
    let decoded = compare(ALPHA, "alpha-reference.png");
    assert!(decoded.has_alpha);
    let source = reference("alpha-source.png");
    assert!(rgba(&decoded)
        .chunks_exact(4)
        .zip(source.chunks_exact(4))
        .all(|(a, b)| a[3] == b[3]));
    // Partial transparency is not multiplied into the stored RGB values.
    assert!(rgba(&decoded)
        .chunks_exact(4)
        .any(|p| p[3] == 64 && p[2] > 200));
    assert_eq!(decoded.mips[0].rgba8_at(160, 96), None);
}
#[test]
fn animated_avif_decodes_all_frames_with_explicit_metadata() {
    let decoded = compare(ANIMATED, "animated-reference.png");
    assert_eq!(decoded.frames.len(), 2);
    let (PixelData::Rgba8(a), PixelData::Rgba8(b)) =
        (&decoded.frames[0].data, &decoded.frames[1].data)
    else {
        panic!("expected RGBA8");
    };
    assert_ne!(a, b);
    let metadata = decoded.extra_meta.as_deref().unwrap();
    assert!(metadata.contains("2 帧") && metadata.contains("循环播放"));
}
#[test]
fn thumbnail_decoding_reuses_exact_avif_pixels() {
    for bytes in [ALPHA, OPAQUE, ANIMATED] {
        let full = decode_bytes(bytes).unwrap();
        let thumb = decode_preview_bytes(bytes).unwrap();
        assert!(thumb.frames.is_empty());
        assert_eq!(rgba(&full), rgba(&thumb));
        assert_eq!(full.has_alpha, thumb.has_alpha);
    }
}
#[test]
fn avif_extension_matching_is_case_insensitive() {
    for name in ["image.avif", "IMAGE.AVIF", "中文 路径/图片.AvIf"] {
        assert!(has_supported_ext(Path::new(name)));
    }
    for name in ["image.avif.tmp", "image.heic", "image.mp4"] {
        assert!(!has_supported_ext(Path::new(name)));
    }
}
fn ftyp(major: &[u8; 4], compatible: &[u8]) -> Vec<u8> {
    let mut data = ((16 + compatible.len()) as u32).to_be_bytes().to_vec();
    data.extend_from_slice(b"ftyp");
    data.extend_from_slice(major);
    data.extend_from_slice(&[0; 4]);
    data.extend_from_slice(compatible);
    data
}
#[test]
fn bmff_brand_detection_accepts_major_and_compatible_avif_only() {
    for data in [
        ftyp(b"avif", b"mif1"),
        ftyp(b"mif1", b"avif"),
        ftyp(b"avis", b"msf1"),
    ] {
        assert_eq!(detect_format(&data), ImageFormat::Avif);
    }
    for data in [
        ftyp(b"heic", b"mif1"),
        ftyp(b"mp42", b"isom"),
        b"arbitrary text containing avif".to_vec(),
    ] {
        assert_ne!(detect_format(&data), ImageFormat::Avif);
    }
    let mut extended = 1u32.to_be_bytes().to_vec();
    extended.extend_from_slice(b"ftyp");
    extended.extend_from_slice(&28u64.to_be_bytes());
    extended.extend_from_slice(b"avif\0\0\0\0mif1");
    assert_eq!(detect_format(&extended), ImageFormat::Avif);
}
#[test]
fn content_detection_does_not_depend_on_filename() {
    let path = std::env::temp_dir().join(format!(
        "iv-avif-{}-{}.dat",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, ALPHA).unwrap();
    let result = decode_path(&path);
    std::fs::remove_file(path).unwrap();
    assert_eq!(result.unwrap().kind, ImageKind::Avif);
}
#[test]
fn truncated_and_corrupt_avif_returns_error_without_panicking() {
    for end in [0, 12, 16, 24, 32, 64, ALPHA.len() / 2, ALPHA.len() - 8] {
        let result = std::panic::catch_unwind(|| decode_bytes(&ALPHA[..end]));
        assert!(result.is_ok(), "panic for {end} bytes");
        assert!(result.unwrap().is_err(), "accepted {end} bytes");
    }
    let mut invalid = ftyp(b"avif", b"mif1");
    invalid.extend_from_slice(&u32::MAX.to_be_bytes());
    invalid.extend_from_slice(b"meta");
    assert!(decode_bytes(&invalid).is_err());
}
#[test]
fn native_parser_rejects_oversized_ispe_before_pixel_allocation() {
    let mut bad = ALPHA.to_vec();
    let offset = bad.windows(4).position(|b| b == b"ispe").unwrap();
    bad[offset + 8..offset + 12].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(decode_bytes(&bad).is_err());
}

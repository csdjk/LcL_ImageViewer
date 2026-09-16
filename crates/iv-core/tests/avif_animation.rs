//! Independently encoded RGBA animation: per-frame colors, alpha and variable delays.
use iv_core::{decode::decode_preview_bytes, decode_bytes, ImageKind, PixelData};

const ANIMATION: &[u8] = include_bytes!("fixtures/avif/animated-alpha.avif");
const REFERENCES: [&[u8]; 4] = [
    include_bytes!("fixtures/avif/animated-alpha-0.png"),
    include_bytes!("fixtures/avif/animated-alpha-1.png"),
    include_bytes!("fixtures/avif/animated-alpha-2.png"),
    include_bytes!("fixtures/avif/animated-alpha-3.png"),
];
fn pixels(data: &PixelData) -> &[u8] {
    match data {
        PixelData::Rgba8(v) => v,
        _ => panic!("expected RGBA8"),
    }
}
#[test]
fn all_animation_frames_match_independent_rgba_references() {
    let decoded = decode_bytes(ANIMATION).unwrap();
    assert_eq!(decoded.kind, ImageKind::Avif);
    assert_eq!((decoded.width, decoded.height), (160, 96));
    assert!(decoded.has_alpha);
    assert_eq!(decoded.frames.len(), 4);
    for (i, frame) in decoded.frames.iter().enumerate() {
        let expected = image::load_from_memory(REFERENCES[i]).unwrap().into_rgba8();
        assert_eq!(pixels(&frame.data).len(), expected.as_raw().len());
        for (a, b) in pixels(&frame.data)
            .chunks_exact(4)
            .zip(expected.as_raw().chunks_exact(4))
        {
            assert_eq!(a[3], b[3], "frame {i} alpha");
            if a[3] > 0 {
                assert!(
                    (0..3).all(|c| a[c].abs_diff(b[c]) <= 2),
                    "frame {i}: {a:?} != {b:?}"
                );
            }
        }
    }
    assert_eq!(
        pixels(&decoded.mips[0].data),
        pixels(&decoded.frames[0].data)
    );
    assert_ne!(
        pixels(&decoded.frames[0].data),
        pixels(&decoded.frames[3].data)
    );
}
#[test]
fn animation_preserves_variable_frame_durations_and_straight_alpha() {
    let decoded = decode_bytes(ANIMATION).unwrap();
    assert_eq!(
        decoded
            .frames
            .iter()
            .map(|f| f.delay_ms)
            .collect::<Vec<_>>(),
        [80, 160, 240, 320]
    );
    assert_eq!(decoded.frames.iter().map(|f| f.delay_ms).sum::<u32>(), 800);
    for frame in &decoded.frames {
        let values: std::collections::BTreeSet<_> =
            pixels(&frame.data).chunks_exact(4).map(|p| p[3]).collect();
        assert_eq!(values, [0, 64, 128, 192, 255].into_iter().collect());
        assert!(
            pixels(&frame.data)
                .chunks_exact(4)
                .any(|p| p[3] == 64 && p[..3].iter().any(|c| *c > 200)),
            "straight alpha must not darken RGB"
        );
    }
}
#[test]
fn animated_thumbnail_stays_static_and_matches_first_frame() {
    let full = decode_bytes(ANIMATION).unwrap();
    let preview = decode_preview_bytes(ANIMATION).unwrap();
    assert!(preview.frames.is_empty());
    assert!(preview.has_alpha);
    assert_eq!(pixels(&preview.mips[0].data), pixels(&full.frames[0].data));
}
#[test]
fn a_damaged_final_sample_is_not_accepted_as_a_shortened_animation() {
    // Corrupt the final AV1 sample without changing the BMFF sample offsets/header.
    let mut bytes = ANIMATION.to_vec();
    let n = bytes.len();
    bytes[n - 24..].fill(0);
    assert!(std::panic::catch_unwind(|| decode_bytes(&bytes))
        .unwrap()
        .is_err());
    // The first-frame path must not attempt to decode a later corrupt sample.
    assert!(decode_preview_bytes(&bytes).is_ok());
}

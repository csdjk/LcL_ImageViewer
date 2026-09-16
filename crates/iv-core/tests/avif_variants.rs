//! Tiny generated AVIF variants cover high bit depth and actual container transforms.
use iv_core::{decode_bytes, PixelData};
use libavif_sys as ffi;
use std::ptr::NonNull;

struct NativeImage(NonNull<ffi::avifImage>);
impl Drop for NativeImage {
    fn drop(&mut self) {
        unsafe { ffi::avifImageDestroy(self.0.as_ptr()) };
    }
}
struct Encoder(NonNull<ffi::avifEncoder>);
impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe { ffi::avifEncoderDestroy(self.0.as_ptr()) };
    }
}
struct Output(ffi::avifRWData);
impl Drop for Output {
    fn drop(&mut self) {
        unsafe { ffi::avifRWDataFree(&mut self.0) };
    }
}

fn original() -> image::RgbaImage {
    image::RgbaImage::from_fn(16, 12, |x, y| {
        image::Rgba([
            (32 + x * 12) as u8,
            (32 + y * 16) as u8,
            128,
            if x < 8 { 128 } else { 255 },
        ])
    })
}
fn encoded(depth: u32, rotation: u8, mirror: Option<u8>, crop: bool) -> Vec<u8> {
    // All FFI pointers below refer to these owned objects or the live input pixel Vec.
    unsafe {
        let image = NativeImage(
            NonNull::new(ffi::avifImageCreate(
                16,
                12,
                depth,
                ffi::AVIF_PIXEL_FORMAT_YUV444,
            ))
            .unwrap(),
        );
        let p = image.0.as_ptr();
        (*p).yuvRange = ffi::AVIF_RANGE_FULL;
        (*p).matrixCoefficients = 0; // lossless RGB identity conversion (GBR planes)
        (*p).colorPrimaries = 1;
        (*p).transferCharacteristics = 13;
        let mut pixels = original().into_raw();
        let mut rgb = ffi::avifRGBImage::default();
        ffi::avifRGBImageSetDefaults(&mut rgb, p);
        rgb.depth = 8;
        rgb.format = ffi::AVIF_RGB_FORMAT_RGBA;
        rgb.rowBytes = 16 * 4;
        rgb.pixels = pixels.as_mut_ptr();
        assert_eq!(ffi::avifImageRGBToYUV(p, &rgb), ffi::AVIF_RESULT_OK);
        if rotation != 0 {
            (*p).transformFlags |= ffi::AVIF_TRANSFORM_IROT;
            (*p).irot.angle = rotation;
        }
        if let Some(axis) = mirror {
            (*p).transformFlags |= ffi::AVIF_TRANSFORM_IMIR;
            (*p).imir.axis = axis;
        }
        if crop {
            (*p).transformFlags |= ffi::AVIF_TRANSFORM_CLAP;
            (*p).clap = ffi::avifCleanApertureBox {
                widthN: 8,
                widthD: 1,
                heightN: 4,
                heightD: 1,
                horizOffN: 0,
                horizOffD: 1,
                vertOffN: 0,
                vertOffD: 1,
            };
        }
        let encoder = Encoder(NonNull::new(ffi::avifEncoderCreate()).unwrap());
        let e = encoder.0.as_ptr();
        (*e).maxThreads = 1;
        (*e).speed = 10;
        (*e).quality = 100;
        (*e).qualityAlpha = 100;
        (*e).minQuantizer = 0;
        (*e).maxQuantizer = 0;
        (*e).minQuantizerAlpha = 0;
        (*e).maxQuantizerAlpha = 0;
        let mut output = Output(ffi::avifRWData {
            data: std::ptr::null_mut(),
            size: 0,
        });
        assert_eq!(
            ffi::avifEncoderWrite(e, p, &mut output.0),
            ffi::AVIF_RESULT_OK
        );
        assert!(!output.0.data.is_null());
        std::slice::from_raw_parts(output.0.data, output.0.size).to_vec()
    }
}
fn pixels(image: &iv_core::DecodedImage) -> &[u8] {
    match &image.mips[0].data {
        PixelData::Rgba8(p) => p,
        _ => panic!("expected RGBA8"),
    }
}
#[test]
fn avif_8_10_and_12_bit_decode_to_documented_rgba8() {
    for depth in [8, 10, 12] {
        let decoded = decode_bytes(&encoded(depth, 0, None, false)).unwrap();
        assert_eq!((decoded.width, decoded.height), (16, 12));
        assert!(decoded.has_alpha);
        for (a, b) in pixels(&decoded).iter().zip(original().as_raw()) {
            assert!(a.abs_diff(*b) <= 1, "depth {depth}: {a} vs {b}");
        }
        assert!(decoded
            .extra_meta
            .as_deref()
            .unwrap()
            .contains(&format!("{depth} 位")));
        assert!(!decoded.is_hdr);
    }
}
#[test]
fn container_crop_rotation_and_mirroring_match_reference_pixels() {
    for crop in [false, true] {
        for rotation in 0..4 {
            for mirror in [None, Some(0), Some(1)] {
                let decoded = decode_bytes(&encoded(8, rotation, mirror, crop)).unwrap();
                let mut expected = if crop {
                    image::imageops::crop_imm(&original(), 4, 4, 8, 4).to_image()
                } else {
                    original()
                };
                expected = match rotation {
                    1 => image::imageops::rotate270(&expected),
                    2 => image::imageops::rotate180(&expected),
                    3 => image::imageops::rotate90(&expected),
                    _ => expected,
                };
                if mirror == Some(0) {
                    image::imageops::flip_vertical_in_place(&mut expected);
                }
                if mirror == Some(1) {
                    image::imageops::flip_horizontal_in_place(&mut expected);
                }
                assert_eq!((decoded.width, decoded.height), expected.dimensions());
                assert_eq!(
                    pixels(&decoded),
                    expected.as_raw(),
                    "crop={crop}, rotation={rotation}, mirror={mirror:?}"
                );
            }
        }
    }
}

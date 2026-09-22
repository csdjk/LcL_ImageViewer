//! Bundled libavif/libaom decoding. No Windows codec installation is required.
//! Native ownership stays inside this module; output is straight (not premultiplied) RGBA8.
use std::{ffi::CStr, mem::MaybeUninit, ptr::NonNull};

use libavif_sys as ffi;

use crate::decode::{AnimatedFrame, DecodeError, DecodedImage, ImageKind, MipLevel, PixelData};

const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;
const MAX_PIXELS: u32 = 64 * 1024 * 1024;
const MAX_DIMENSION: u32 = 32768;
const MAX_IMAGES: u32 = 4096;
// Retained frame pixels plus the separate first-frame mip. Native scratch memory is additional.
use crate::decode::{check_animation_budget, MAX_ANIMATION_BYTES};

struct Decoder(NonNull<ffi::avifDecoder>);

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: this pointer came from avifDecoderCreate and has exactly one owner.
        unsafe { ffi::avifDecoderDestroy(self.0.as_ptr()) };
    }
}

pub(crate) fn is_avif(bytes: &[u8]) -> bool {
    if bytes.len() < 16 {
        return false;
    }
    let input = ffi::avifROData {
        data: bytes.as_ptr(),
        size: bytes.len(),
    };
    // SAFETY: Peek only reads the provided slice; it validates the ftyp box and compatible brands.
    unsafe { ffi::avifPeekCompatibleFileType(&input) != 0 }
}

fn checked_length(width: u32, height: u32) -> Result<usize, DecodeError> {
    let pixels = width.checked_mul(height);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || pixels.is_none()
        || pixels.unwrap_or(u32::MAX) > MAX_PIXELS
    {
        return Err(DecodeError::UnsupportedFormat(
            "AVIF 尺寸超过安全限制（单边 32768，最多 67108864 像素）".into(),
        ));
    }
    (pixels.unwrap_or(0) as usize)
        .checked_mul(4)
        .ok_or_else(|| DecodeError::Decode("AVIF 像素缓冲区大小溢出".into()))
}

fn result(code: ffi::avifResult) -> Result<(), DecodeError> {
    if code == ffi::AVIF_RESULT_OK {
        return Ok(());
    }
    if code == ffi::AVIF_RESULT_TRUNCATED_DATA {
        return Err(DecodeError::Truncated);
    }
    // SAFETY: libavif returns a static, null-terminated message for every result code.
    let message = unsafe { CStr::from_ptr(ffi::avifResultToString(code)) }.to_string_lossy();
    Err(DecodeError::Decode(format!("AVIF: {message}")))
}

pub(crate) fn decode_avif(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    decode(bytes, false, MAX_ANIMATION_BYTES)
}

pub(crate) fn decode_avif_preview(bytes: &[u8]) -> Result<DecodedImage, DecodeError> {
    decode(bytes, true, MAX_ANIMATION_BYTES)
}

// Round timestamps, not each interval independently, so 60 fps remains 1000 ms/60 frames.
// A zero/sub-millisecond interval is one millisecond; never apply GIF's 100 ms fallback.
fn frame_delay_ms(timing: &ffi::avifImageTiming) -> Result<u32, DecodeError> {
    let scale = u128::from(timing.timescale);
    if scale == 0 {
        return Err(DecodeError::Decode("AVIF 动画时间基无效".into()));
    }
    let start = u128::from(timing.ptsInTimescales);
    let end = start + u128::from(timing.durationInTimescales);
    let round = |ticks: u128| (ticks * 1000 + scale / 2) / scale;
    u32::try_from((round(end) - round(start)).max(1))
        .map_err(|_| DecodeError::UnsupportedFormat("AVIF 单帧播放时长超过支持范围".into()))
}

fn decode(
    bytes: &[u8],
    preview_only: bool,
    animation_budget: usize,
) -> Result<DecodedImage, DecodeError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(DecodeError::UnsupportedFormat(
            "AVIF 文件超过 128 MiB 解码限制".into(),
        ));
    }
    // SAFETY: creation returns either a valid owned decoder or null, checked immediately.
    let decoder = Decoder(
        NonNull::new(unsafe { ffi::avifDecoderCreate() })
            .ok_or_else(|| DecodeError::Decode("无法创建 AVIF 解码器".into()))?,
    );
    let ptr = decoder.0.as_ptr();
    // SAFETY: decoder and input outlive all synchronous calls below. Apply native limits
    // BEFORE parsing/decoding, rather than checking only after native pixel allocation.
    unsafe {
        (*ptr).maxThreads = 4;
        (*ptr).imageSizeLimit = MAX_PIXELS;
        (*ptr).imageDimensionLimit = MAX_DIMENSION;
        (*ptr).imageCountLimit = MAX_IMAGES;
        (*ptr).ignoreExif = 1;
        (*ptr).ignoreXMP = 1;
        (*ptr).allowProgressive = 0;
        (*ptr).allowIncremental = 0;
        result(ffi::avifDecoderSetIOMemory(
            ptr,
            bytes.as_ptr(),
            bytes.len(),
        ))?;
        result(ffi::avifDecoderParse(ptr))?;
        if (*ptr).image.is_null() {
            return Err(DecodeError::Decode("AVIF 缺少有效图像".into()));
        }
    }
    // SAFETY: Parse succeeded and the image pointer was checked above. Copy metadata before
    // advancing the native decoder; no Rust reference to its image is held across NextImage.
    let (count, raw_bytes) = unsafe {
        (
            (*ptr).imageCount,
            checked_length((*(*ptr).image).width, (*(*ptr).image).height)?,
        )
    };
    if count < 1 || count > MAX_IMAGES as i32 {
        return Err(DecodeError::Decode("AVIF 没有有效的帧序列".into()));
    }
    let frame_count = if preview_only { 1 } else { count as usize };
    if frame_count > 1 {
        check_animation_budget(raw_bytes, frame_count, animation_budget)?;
    }
    let mut frames = Vec::new();
    if frame_count > 1 {
        frames
            .try_reserve_exact(frame_count)
            .map_err(|_| DecodeError::Decode("AVIF 动画帧列表内存不足".into()))?;
    }
    let mut first_mip = None;
    let mut notes = Vec::new();
    let mut has_alpha = false;
    let mut dimensions = None;
    for index in 0..frame_count {
        // SAFETY: decoder and bytes remain alive. Native decoding owns AV1 reference frames.
        unsafe { result(ffi::avifDecoderNextImage(ptr))? };
        let source = unsafe { (*ptr).image.as_ref() }
            .ok_or_else(|| DecodeError::Decode("AVIF 缺少有效帧".into()))?;
        if checked_length(source.width, source.height)? != raw_bytes {
            return Err(DecodeError::Decode("AVIF 动画编码尺寸发生变化".into()));
        }
        let image = convert_frame(source)?;
        let size = image.dimensions();
        if dimensions.is_some_and(|expected| expected != size) {
            return Err(DecodeError::Decode("AVIF 动画帧尺寸不一致".into()));
        }
        dimensions = Some(size);
        has_alpha |= unsafe { (*ptr).alphaPresent != 0 } || !source.alphaPlane.is_null();
        if index == 0 {
            notes = image_notes(source);
            if count > 1 {
                notes.push(if preview_only {
                    format!("动态 AVIF · {count} 帧 · 缩略图仅显示首帧")
                } else {
                    format!("动态 AVIF · {count} 帧 · 循环播放")
                });
            }
        }
        let pixels = image.into_raw();
        if frame_count > 1 {
            if index == 0 {
                // Accounted for in the budget, retained for initial upload and thumbnail parity.
                let mut copy = Vec::new();
                copy.try_reserve_exact(pixels.len())
                    .map_err(|_| DecodeError::Decode("AVIF 首帧缓存内存不足".into()))?;
                copy.extend_from_slice(&pixels);
                first_mip = Some(MipLevel {
                    width: size.0,
                    height: size.1,
                    data: PixelData::Rgba8(copy),
                });
            }
            let timing = unsafe { (*ptr).imageTiming };
            frames.push(AnimatedFrame {
                data: PixelData::Rgba8(pixels),
                delay_ms: frame_delay_ms(&timing)?,
            });
        } else {
            first_mip = Some(MipLevel {
                width: size.0,
                height: size.1,
                data: PixelData::Rgba8(pixels),
            });
        }
    }
    let mip = first_mip.ok_or_else(|| DecodeError::Decode("AVIF 缺少首帧".into()))?;
    Ok(DecodedImage {
        width: mip.width,
        height: mip.height,
        mips: vec![mip],
        kind: ImageKind::Avif,
        compression: None,
        has_alpha,
        is_hdr: false,
        extra_meta: Some(notes.join(" · ")),
        frames,
    })
}

fn convert_frame(source: &ffi::avifImage) -> Result<image::RgbaImage, DecodeError> {
    let len = checked_length(source.width, source.height)?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(len)
        .map_err(|_| DecodeError::Decode("AVIF 像素缓冲区内存不足".into()))?;
    pixels.resize(len, 0);
    let mut rgb = MaybeUninit::<ffi::avifRGBImage>::zeroed();
    // SAFETY: defaults initializes this POD struct; RGB output is a checked, owned Vec.
    let mut rgb = unsafe {
        ffi::avifRGBImageSetDefaults(rgb.as_mut_ptr(), source);
        rgb.assume_init()
    };
    rgb.depth = 8;
    rgb.format = ffi::AVIF_RGB_FORMAT_RGBA;
    rgb.alphaPremultiplied = 0;
    rgb.ignoreAlpha = 0;
    rgb.maxThreads = 4;
    rgb.pixels = pixels.as_mut_ptr();
    rgb.rowBytes = source.width * 4;
    // SAFETY: YUV buffers are decoder-owned; output is width*height*4 writable bytes.
    unsafe { result(ffi::avifImageYUVToRGB(source, &mut rgb))? };
    let mut image = image::RgbaImage::from_raw(source.width, source.height, pixels)
        .ok_or_else(|| DecodeError::Decode("AVIF 像素长度与尺寸不一致".into()))?;

    // ISO BMFF transforms are metadata: libavif intentionally does not apply them to RGB.
    // Apply clean aperture, then anti-clockwise rotation, then mirroring to displayed pixels.
    if source.transformFlags & ffi::AVIF_TRANSFORM_CLAP != 0 {
        let mut crop = MaybeUninit::<ffi::avifCropRect>::zeroed();
        let mut diagnostics = MaybeUninit::<ffi::avifDiagnostics>::zeroed();
        let ok = unsafe {
            ffi::avifCropRectConvertCleanApertureBox(
                crop.as_mut_ptr(),
                &source.clap,
                source.width,
                source.height,
                source.yuvFormat,
                diagnostics.as_mut_ptr(),
            )
        };
        if ok == 0 {
            return Err(DecodeError::Decode("AVIF 裁剪区域无效".into()));
        }
        let crop = unsafe { crop.assume_init() };
        checked_length(crop.width, crop.height)?;
        if crop
            .x
            .checked_add(crop.width)
            .is_none_or(|v| v > source.width)
            || crop
                .y
                .checked_add(crop.height)
                .is_none_or(|v| v > source.height)
        {
            return Err(DecodeError::Decode("AVIF 裁剪区域越界".into()));
        }
        image =
            image::imageops::crop_imm(&image, crop.x, crop.y, crop.width, crop.height).to_image();
    }
    let rotation = if source.transformFlags & ffi::AVIF_TRANSFORM_IROT != 0 {
        source.irot.angle
    } else {
        0
    };
    let mirror = if source.transformFlags & ffi::AVIF_TRANSFORM_IMIR != 0 {
        Some(source.imir.axis)
    } else {
        None
    };
    orient(image, rotation, mirror)
}

fn image_notes(source: &ffi::avifImage) -> Vec<String> {
    let mut notes = vec![format!("AV1 · {} 位", source.depth)];
    if source.depth > 8 {
        notes.push("转换为 8 位显示".into());
    }
    if source.icc.size != 0 {
        notes.push("未应用 ICC 色彩配置".into());
    }
    if matches!(source.transferCharacteristics, 16 | 18) {
        notes.push("HDR 色彩映射暂不支持".into());
    }
    if source.transformFlags & ffi::AVIF_TRANSFORM_PASP != 0
        && source.pasp.hSpacing != source.pasp.vSpacing
    {
        notes.push("按原始像素比例显示".into());
    }
    notes
}

fn orient(
    mut image: image::RgbaImage,
    rotation: u8,
    mirror: Option<u8>,
) -> Result<image::RgbaImage, DecodeError> {
    image = match rotation {
        0 => image,
        1 => image::imageops::rotate270(&image),
        2 => image::imageops::rotate180(&image),
        3 => image::imageops::rotate90(&image),
        _ => return Err(DecodeError::Decode("AVIF 旋转角度无效".into())),
    };
    match mirror {
        None => {}
        Some(0) => image::imageops::flip_vertical_in_place(&mut image),
        Some(1) => image::imageops::flip_horizontal_in_place(&mut image),
        _ => return Err(DecodeError::Decode("AVIF 镜像轴无效".into())),
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_limits_reject_zero_overflow_and_excessive_images() {
        for (w, h) in [(0, 1), (1, 0), (u32::MAX, 2), (32769, 1), (8193, 8192)] {
            assert!(checked_length(w, h).is_err(), "{w}x{h}");
        }
        assert_eq!(checked_length(8192, 8192).unwrap(), 256 * 1024 * 1024);
    }

    #[test]
    fn transforms_use_counterclockwise_rotation_then_mirror() {
        let im =
            image::RgbaImage::from_fn(3, 2, |x, y| image::Rgba([(y * 3 + x + 1) as u8, 0, 0, 255]));
        let rotated = orient(im.clone(), 1, None).unwrap();
        assert_eq!(rotated.dimensions(), (2, 3));
        assert_eq!(
            rotated.pixels().map(|p| p[0]).collect::<Vec<_>>(),
            [3, 6, 2, 5, 1, 4]
        );
        let mirrored = orient(im, 1, Some(1)).unwrap();
        assert_eq!(
            mirrored.pixels().map(|p| p[0]).collect::<Vec<_>>(),
            [6, 3, 5, 2, 4, 1]
        );
    }

    #[test]
    fn fractional_frame_timestamps_preserve_cycle_duration() {
        let delays: Vec<_> = (0..60)
            .map(|i| {
                frame_delay_ms(&ffi::avifImageTiming {
                    timescale: 60,
                    ptsInTimescales: i,
                    durationInTimescales: 1,
                    ..Default::default()
                })
                .unwrap()
            })
            .collect();
        assert_eq!(delays.iter().sum::<u32>(), 1000);
        assert!(delays.iter().all(|d| *d == 16 || *d == 17));
        let timing = ffi::avifImageTiming {
            timescale: 1000,
            durationInTimescales: 5,
            ..Default::default()
        };
        assert_eq!(frame_delay_ms(&timing).unwrap(), 5);
    }

    #[test]
    fn malformed_and_extreme_timing_is_bounded() {
        assert!(frame_delay_ms(&ffi::avifImageTiming::default()).is_err());
        assert_eq!(
            frame_delay_ms(&ffi::avifImageTiming {
                timescale: 1000,
                ..Default::default()
            })
            .unwrap(),
            1
        );
        assert_eq!(
            frame_delay_ms(&ffi::avifImageTiming {
                timescale: u64::MAX,
                ptsInTimescales: u64::MAX,
                durationInTimescales: u64::MAX,
                ..Default::default()
            })
            .unwrap(),
            1000
        );
        assert!(frame_delay_ms(&ffi::avifImageTiming {
            timescale: 1,
            durationInTimescales: u64::MAX,
            ..Default::default()
        })
        .is_err());
    }

    #[test]
    fn total_animation_budget_includes_first_mip_and_rejects_overflow() {
        assert!(check_animation_budget(100, 2, 300).is_ok());
        assert!(check_animation_budget(100, 2, 299).is_err());
        assert!(check_animation_budget(usize::MAX, 2, usize::MAX).is_err());
        assert!(check_animation_budget(4, 4097, usize::MAX).is_err());
        assert!(check_animation_budget(4, 0, usize::MAX).is_err());
    }

    #[test]
    fn preview_does_not_reserve_or_decode_the_complete_animation() {
        let bytes = include_bytes!("../tests/fixtures/avif/animated.avif");
        assert!(decode(bytes, false, 1).is_err());
        let preview = decode(bytes, true, 1).unwrap();
        assert!(preview.frames.is_empty());
        assert_eq!((preview.width, preview.height), (160, 96));
    }
}

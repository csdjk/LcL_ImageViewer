//! Bundled libavif/libaom decoding. No Windows codec installation is required.
//! Native ownership stays inside this module; output is straight (not premultiplied) RGBA8.
use std::{ffi::CStr, mem::MaybeUninit, ptr::NonNull};

use libavif_sys as ffi;

use crate::decode::{DecodeError, DecodedImage, ImageKind, MipLevel, PixelData};

const MAX_FILE_BYTES: usize = 128 * 1024 * 1024;
const MAX_PIXELS: u32 = 64 * 1024 * 1024;
const MAX_DIMENSION: u32 = 32768;
const MAX_IMAGES: u32 = 4096;

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
        checked_length((*(*ptr).image).width, (*(*ptr).image).height)?;
        // Deliberately decode only the first image, including animated AVIF previews.
        result(ffi::avifDecoderNextImage(ptr))?;
    }
    // SAFETY: the image belongs to decoder and remains valid until its Drop at function exit.
    let source = unsafe { &*(*ptr).image };
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
    image = orient(image, rotation, mirror)?;
    let (width, height) = image.dimensions();
    let mut notes = vec![format!("AV1 · {} 位", source.depth)];
    if source.depth > 8 {
        notes.push("转换为 8 位显示".into());
    }
    let count = unsafe { (*ptr).imageCount };
    if count > 1 {
        notes.push(format!("动态 AVIF · {count} 帧 · 仅显示首帧"));
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
    Ok(DecodedImage {
        width,
        height,
        mips: vec![MipLevel {
            width,
            height,
            data: PixelData::Rgba8(image.into_raw()),
        }],
        kind: ImageKind::Avif,
        compression: None,
        has_alpha: unsafe { (*ptr).alphaPresent != 0 },
        is_hdr: false,
        extra_meta: Some(notes.join(" · ")),
        frames: Vec::new(),
    })
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
}

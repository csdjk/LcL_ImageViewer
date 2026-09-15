//! Pixel-for-pixel comparison with the pre-optimization image-crate conversion path.
use std::io::Cursor;
use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba, Luma, LumaA, Rgb};
use iv_core::{decode_bytes, PixelData};

fn compare(image: DynamicImage, format: ImageFormat) {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    let reference = image::load_from_memory_with_format(bytes.get_ref(), format).unwrap();
    let expected = reference.to_rgba8();
    let decoded = decode_bytes(bytes.get_ref()).unwrap();
    assert_eq!((decoded.width, decoded.height), expected.dimensions());
    assert_eq!(decoded.mips.len(), 1);
    let PixelData::Rgba8(actual) = &decoded.mips[0].data else { panic!("LDR output changed type") };
    assert_eq!(actual, expected.as_raw());
    assert_eq!(decoded.frames.len(), 0);
}

#[test]
fn png_rgba_preserves_rgb_under_zero_alpha_and_partial_alpha() {
    compare(DynamicImage::ImageRgba8(ImageBuffer::from_fn(31,17,|x,y|
        Rgba([(x*7) as u8,(y*13) as u8,173,((x+y)*11) as u8]))), ImageFormat::Png);
}
#[test]
fn png_rgb_luma_and_luma_alpha_match_previous_conversion() {
    compare(DynamicImage::ImageRgb8(ImageBuffer::from_fn(31,17,|x,y|Rgb([(x*7) as u8,(y*13) as u8,92]))),ImageFormat::Png);
    compare(DynamicImage::ImageLuma8(ImageBuffer::from_fn(31,17,|x,y|Luma([((x+y)*3) as u8]))),ImageFormat::Png);
    compare(DynamicImage::ImageLumaA8(ImageBuffer::from_fn(31,17,|x,y|LumaA([(x*7) as u8,(y*13) as u8]))),ImageFormat::Png);
}
#[test]
fn png_16bit_and_jpeg_match_previous_conversion() {
    compare(DynamicImage::ImageRgba16(ImageBuffer::from_fn(31,17,|x,y|Rgba([(x*2000) as u16,(y*4000) as u16,60000,((x+y)*1300) as u16]))),ImageFormat::Png);
    compare(DynamicImage::ImageLuma16(ImageBuffer::from_fn(31,17,|x,y|Luma([((x+y)*1300) as u16]))),ImageFormat::Png);
    compare(DynamicImage::ImageRgb8(ImageBuffer::from_fn(31,17,|x,y|Rgb([(x*7) as u8,(y*13) as u8,92]))),ImageFormat::Jpeg);
}
#[test]
fn gif_moving_frame_buffers_keeps_pixels_and_delays() {
    use image::{AnimationDecoder, Frame, Delay};
    let mut bytes=Vec::new();
    {
        let mut encoder=image::codecs::gif::GifEncoder::new(&mut bytes);
        for (rgb,ms) in [([220,35,40],80),([30,200,50],120),([15,60,220],250)] {
            encoder.encode_frame(Frame::from_parts(ImageBuffer::from_pixel(16,16,Rgba([rgb[0],rgb[1],rgb[2],255])),0,0,Delay::from_numer_denom_ms(ms,1))).unwrap();
        }
    }
    let reference=image::codecs::gif::GifDecoder::new(Cursor::new(&bytes)).unwrap().into_frames().collect_frames().unwrap();
    let actual=decode_bytes(&bytes).unwrap();
    assert_eq!(actual.frames.len(),reference.len());
    for (expected,frame) in reference.iter().zip(&actual.frames) {
        let PixelData::Rgba8(pixels)=&frame.data else {panic!()};
        assert_eq!(pixels,expected.buffer().as_raw());
        let (n,d)=expected.delay().numer_denom_ms();
        assert_eq!(frame.delay_ms,n/d);
    }
}

#[test]
fn apng_frames_alpha_and_delays_are_unchanged() {
    use image::AnimationDecoder;
    let bytes: &[u8] = &[137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0, 114, 182, 13, 36, 0, 0, 0, 8, 97, 99, 84, 76, 0, 0, 0, 2, 0, 0, 0, 0, 243, 141, 147, 112, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 25, 0, 0, 177, 148, 187, 249, 0, 0, 0, 20, 73, 68, 65, 84, 120, 156, 99, 108, 146, 244, 97, 0, 1, 38, 48, 201, 192, 192, 0, 0, 14, 220, 0, 235, 241, 49, 40, 13, 0, 0, 0, 26, 102, 99, 84, 76, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 25, 0, 0, 23, 135, 120, 157, 0, 0, 0, 25, 102, 100, 65, 84, 0, 0, 0, 2, 120, 156, 99, 20, 217, 103, 190, 133, 129, 129, 129, 129, 9, 68, 128, 48, 0, 26, 123, 1, 193, 98, 191, 251, 161, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];
    let reference=image::codecs::png::PngDecoder::new(Cursor::new(bytes)).unwrap().apng().unwrap().into_frames().collect_frames().unwrap();
    let actual=decode_bytes(bytes).unwrap();
    assert_eq!(actual.frames.len(),reference.len());
    for (expected,frame) in reference.iter().zip(&actual.frames) {
        let PixelData::Rgba8(pixels)=&frame.data else {panic!()};
        assert_eq!(pixels,expected.buffer().as_raw());
        let (n,d)=expected.delay().numer_denom_ms();
        assert_eq!(frame.delay_ms,n/d);
    }
}

#[test]
fn static_webp_preserves_pixels() {
    compare(DynamicImage::ImageRgba8(ImageBuffer::from_fn(31,17,|x,y|
        Rgba([(x*7) as u8,(y*13) as u8,173,((x+y)*11) as u8]))),ImageFormat::WebP);
}

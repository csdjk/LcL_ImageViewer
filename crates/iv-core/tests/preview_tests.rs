use iv_core::decode::{decode_bytes, decode_preview_bytes, PixelData};
use std::io::Cursor;

fn rgba(image: &iv_core::DecodedImage) -> &[u8] {
    let PixelData::Rgba8(data) = &image.mips[0].data else { panic!("Expected RGBA8") };
    data
}

#[test]
fn static_png_preview_matches_full_decode() {
    let img = image::RgbaImage::from_raw(2, 1, vec![90,80,70,0, 200,100,50,128]).unwrap();
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img).write_to(&mut bytes,image::ImageFormat::Png).unwrap();
    assert_eq!(rgba(&decode_bytes(bytes.get_ref()).unwrap()), rgba(&decode_preview_bytes(bytes.get_ref()).unwrap()));
}

#[test]
fn animated_preview_contains_only_first_composed_frame() {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for color in [[200,30,60,255],[20,180,50,255]] {
            encoder.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(4,2,image::Rgba(color)))).unwrap();
        }
    }
    let full = decode_bytes(&bytes).unwrap();
    let preview = decode_preview_bytes(&bytes).unwrap();
    assert_eq!(full.frames.len(),2);
    assert!(preview.frames.is_empty());
    assert_eq!(rgba(&preview),rgba(&full));
}

fn psd(extra_mask: bool, compression: u16) -> Vec<u8> {
    let channels: u16 = if extra_mask {5} else {4};
    let mut bytes = b"8BPS".to_vec();
    bytes.extend(1u16.to_be_bytes()); bytes.extend([0;6]);
    bytes.extend(channels.to_be_bytes());
    bytes.extend(1u32.to_be_bytes()); bytes.extend(2u32.to_be_bytes());
    bytes.extend(8u16.to_be_bytes()); bytes.extend(3u16.to_be_bytes());
    bytes.extend([0;12]); bytes.extend(compression.to_be_bytes());
    let planes = [[200,100],[80,40],[50,25],[128,0],[12,34]];
    if compression == 1 { for _ in 0..channels { bytes.extend(3u16.to_be_bytes()); } }
    for plane in planes.iter().take(channels as usize) {
        if compression == 1 { bytes.push(1); }
        bytes.extend(plane);
    }
    bytes
}

#[test]
fn psd_raw_and_rle_with_extra_mask_produce_the_same_preview() {
    let expected = [200,80,50,128,100,40,25,0];
    for compression in [0,1] {
        for extra in [false,true] {
            assert_eq!(rgba(&decode_preview_bytes(&psd(extra,compression)).unwrap()),expected);
        }
    }
}

#[test]
fn malformed_psd_channels_and_truncation_return_errors() {
    let mut bad = psd(false,0); bad[12..14].copy_from_slice(&2u16.to_be_bytes());
    assert!(decode_preview_bytes(&bad).is_err());
    let original=psd(true,1);
    for length in 0..original.len()-3 {
        assert!(std::panic::catch_unwind(||decode_preview_bytes(&original[..length])).is_ok());
    }
}

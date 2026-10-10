//! Explicit large-fixture regression; normal unit tests force spilling at tiny budgets.
use iv_core::decode::{decode_path, decode_preview_bytes};
use std::{path::PathBuf, time::Instant};

#[test]
#[ignore = "generate fixtures with tools/ui-qa/generate_large_animations.py and set LCL_IV_LARGE_ANIMATIONS"]
fn gif_apng_webp_over_old_budget_keep_every_frame_and_clean_up() {
    let root =
        PathBuf::from(std::env::var_os("LCL_IV_LARGE_ANIMATIONS").expect("fixture directory"));
    for ext in ["gif", "png", "webp"] {
        let path = root.join(ext).join(format!("large-animation.{ext}"));
        // Pillow reads the encoded file as an independent oracle; GIF palette
        // quantization must not be mistaken for a storage/decoder pixel error.
        let oracle = std::fs::read(root.join(ext).join("expected.bin")).unwrap();
        assert_eq!(oracle.len(), 72 * 9);
        let started = Instant::now();
        let image = decode_path(&path).unwrap();
        assert_eq!((image.width, image.height), (1024, 1024));
        assert_eq!(image.frames.len(), 72);
        assert_eq!(
            image
                .frames
                .iter()
                .map(|f| f.data.byte_len())
                .sum::<usize>(),
            288 * 1024 * 1024
        );
        assert!(image.frames.iter().all(|f| f.data.is_disk_backed()));
        assert!(image.has_alpha);
        for i in (0..72).rev() {
            let frame = &image.frames[i];
            let expected: [u8; 4] = oracle[i * 9..i * 9 + 4].try_into().unwrap();
            let sample = frame.data.rgba8_at(1024, 1024, 512, 512).unwrap();
            if ext == "webp" {
                // Existing image-webp and libwebp/Pillow compositing differ by one
                // level on this animation. Exact native frame parity is tested below.
                assert!(sample.iter().zip(expected).all(|(a,b)| a.abs_diff(b)<=2));
            } else { assert_eq!(sample, expected, "{ext} frame {i}"); }
            assert_eq!(
                frame.data.rgba8_at(1024, 1024, 950, 60).unwrap()[3],
                oracle[i * 9 + 4]
            );
            assert_eq!(
                frame.delay_ms,
                u32::from_le_bytes(oracle[i * 9 + 5..i * 9 + 9].try_into().unwrap())
            );
        }
        // Stream the original decoder (no AnimationBuilder/cache) and compare every
        // byte of all 72 composed frames. This isolates storage from codec rounding.
        use ::image::AnimationDecoder;
        let bytes=std::fs::read(&path).unwrap();
        let cursor=std::io::Cursor::new(bytes.as_slice());
        let reference=match ext {
            "gif" => ::image::codecs::gif::GifDecoder::new(cursor).unwrap().into_frames(),
            "png" => ::image::codecs::png::PngDecoder::new(cursor).unwrap().apng().unwrap().into_frames(),
            _ => ::image::codecs::webp::WebPDecoder::new(cursor).unwrap().into_frames(),
        };
        let mut compared=0;
        for (i,frame) in reference.enumerate() {
            let frame=frame.unwrap();
            assert_eq!(image.frames[i].data.as_rgba8().unwrap(),frame.buffer().as_raw(),"{ext} frame {i} storage mismatch");
            compared+=1;
        }
        assert_eq!(compared,72);
        let first = image.mips[0].data.clone();
        assert_eq!(first.as_rgba8(), image.frames[0].data.as_rgba8());
        let preview = decode_preview_bytes(&std::fs::read(&path).unwrap()).unwrap();
        assert!(preview.frames.is_empty());
        assert!(!preview.mips[0].data.is_disk_backed());
        assert_eq!(preview.mips[0].data.as_rgba8(), first.as_rgba8());
        println!(
            "LARGE_PASS {ext}: 72 full frames, 288 MiB logical pixels, {} ms",
            started.elapsed().as_millis()
        );
        let first_pixel=first.rgba8_at(1024,1024,512,512);
        drop(image);
        assert_eq!(
            first.rgba8_at(1024, 1024, 512, 512),
            first_pixel
        );
        drop(first);
    }
    #[cfg(windows)]
    {
        let prefix = format!("lcl-iv-animation-{}-", std::process::id());
        assert!(
            !std::fs::read_dir(std::env::temp_dir())
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().starts_with(&prefix)),
            "Temporary file leaked"
        );
    }
}

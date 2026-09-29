//! Non-destructive crop/resize of an immutable decoded snapshot. No display-channel data.
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use image::ImageEncoder;
use iv_core::decode::{DecodedImage, PixelData};

pub const MAX_SIDE: u32 = 16_384;
pub const MAX_PIXELS: u64 = 16_777_216;

pub fn pixel_bytes(w: u32, h: u32) -> Result<usize, String> {
    if w == 0 || h == 0 || w > MAX_SIDE || h > MAX_SIDE || u64::from(w) * u64::from(h) > MAX_PIXELS
    {
        return Err("尺寸需大于 0，单边不超过 16384，像素总数不超过 16777216".into());
    }
    Ok(w as usize * h as usize * 4)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl Crop {
    pub fn full(w: u32, h: u32) -> Self {
        Self { x: 0, y: 0, w, h }
    }
    pub fn validate(self, size: (u32, u32)) -> Result<(), String> {
        if self.w == 0
            || self.h == 0
            || self.x.checked_add(self.w).is_none_or(|n| n > size.0)
            || self.y.checked_add(self.h).is_none_or(|n| n > size.1)
        {
            return Err("裁剪区域必须位于图片内，宽高至少为 1 像素".into());
        }
        Ok(())
    }
    pub fn from_points(a: [f32; 2], b: [f32; 2], size: (u32, u32)) -> Self {
        let x0 = a[0]
            .min(b[0])
            .floor()
            .clamp(0.0, size.0.saturating_sub(1) as f32) as u32;
        let y0 = a[1]
            .min(b[1])
            .floor()
            .clamp(0.0, size.1.saturating_sub(1) as f32) as u32;
        let x1 = a[0].max(b[0]).ceil().clamp((x0 + 1) as f32, size.0 as f32) as u32;
        let y1 = a[1].max(b[1]).ceil().clamp((y0 + 1) as f32, size.1 as f32) as u32;
        Self {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
    pub fn translated(self, dx: i64, dy: i64, size: (u32, u32)) -> Self {
        Self {
            x: (i64::from(self.x) + dx).clamp(0, i64::from(size.0 - self.w)) as u32,
            y: (i64::from(self.y) + dy).clamp(0, i64::from(size.1 - self.h)) as u32,
            ..self
        }
    }
    pub fn centered_ratio(size: (u32, u32), ratio: (u32, u32)) -> Self {
        let mut w = size.0;
        let mut h = (u64::from(w) * u64::from(ratio.1) / u64::from(ratio.0)).max(1) as u32;
        if h > size.1 {
            h = size.1;
            w = (u64::from(h) * u64::from(ratio.0) / u64::from(ratio.1)).max(1) as u32;
        }
        Self {
            x: (size.0 - w) / 2,
            y: (size.1 - h) / 2,
            w,
            h,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Smooth,
    Nearest,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub crop: Crop,
    pub width: u32,
    pub height: u32,
    pub filter: Filter,
}
impl Plan {
    pub fn full(size: (u32, u32)) -> Self {
        Self {
            crop: Crop::full(size.0, size.1),
            width: size.0,
            height: size.1,
            filter: Filter::Smooth,
        }
    }
    pub fn set_crop(&mut self, crop: Crop) {
        self.crop = crop;
        self.width = crop.w;
        self.height = crop.h;
    }
    pub fn validate(self, size: (u32, u32)) -> Result<(), String> {
        self.crop.validate(size)?;
        pixel_bytes(self.width, self.height).map(|_| ())
    }
    pub fn paired_dimension(value: u32, numerator: u32, denominator: u32) -> u32 {
        ((u64::from(value) * u64::from(numerator) + u64::from(denominator) / 2)
            / u64::from(denominator.max(1)))
        .clamp(1, u64::from(u32::MAX)) as u32
    }
}

#[derive(Clone)]
pub struct Source {
    image: Arc<DecodedImage>,
    frame: Option<usize>,
    pub size: (u32, u32),
    pub note: String,
}
impl Source {
    pub fn new(image: Arc<DecodedImage>, frame_index: usize) -> Result<Self, String> {
        let animated = image.frames.len() > 1;
        if animated && frame_index >= image.frames.len() {
            return Err("当前动画帧已失效，请重新打开图片".into());
        }
        let size = if animated {
            (image.width, image.height)
        } else {
            let mip = image.mips.first().ok_or("图片没有可编辑的像素")?;
            (mip.width, mip.height)
        };
        pixel_bytes(size.0, size.1)?;
        if image.is_hdr {
            return Err("暂不编辑 HDR 浮点图像，以免损失亮度信息".into());
        }
        let note = if animated {
            format!(
                "当前动画帧 {}/{} → 静态 PNG（不导出整段动画）",
                frame_index + 1,
                image.frames.len()
            )
        } else if image.mips.len() > 1 {
            "编辑最高分辨率 Mip 0 → 单张 PNG（不保留 Mipmap）".into()
        } else {
            "原始 RGBA · 另存 8 位 PNG（不保留图层、ICC 等元数据）".into()
        };
        let source = Self {
            image,
            frame: animated.then_some(frame_index),
            size,
            note,
        };
        if source.pixels()?.len() != pixel_bytes(size.0, size.1)? {
            return Err("图像像素长度与尺寸不匹配".into());
        }
        Ok(source)
    }
    fn pixels(&self) -> Result<&[u8], String> {
        let data = match self.frame {
            Some(i) => &self.image.frames.get(i).ok_or("动画帧已失效")?.data,
            None => &self.image.mips.first().ok_or("图像数据为空")?.data,
        };
        match data {
            PixelData::Rgba8(v) => Ok(v),
            _ => Err("暂不编辑浮点像素数据".into()),
        }
    }
    pub fn render(&self, plan: Plan) -> Result<Pixels, String> {
        plan.validate(self.size)?;
        let len = pixel_bytes(plan.width, plan.height)?;
        let mut data = Vec::new();
        data.try_reserve_exact(len)
            .map_err(|_| "编辑内存不足，请缩小输出尺寸")?;
        data.resize(len, 0);
        let src = self.pixels()?;
        let crop = plan.crop;
        let at = |x: u32, y: u32| -> &[u8] {
            let i = (((crop.y + y) as usize * self.size.0 as usize) + (crop.x + x) as usize) * 4;
            &src[i..i + 4]
        };
        for y in 0..plan.height {
            for x in 0..plan.width {
                let i = ((y as usize * plan.width as usize) + x as usize) * 4;
                let out = &mut data[i..i + 4];
                // Cropping without resizing is byte-exact, including hidden RGB at Alpha=0.
                if plan.width == crop.w && plan.height == crop.h {
                    out.copy_from_slice(at(x, y));
                    continue;
                }
                if plan.filter == Filter::Nearest {
                    let sx = ((u64::from(x) * 2 + 1) * u64::from(crop.w)
                        / (u64::from(plan.width) * 2)) as u32;
                    let sy = ((u64::from(y) * 2 + 1) * u64::from(crop.h)
                        / (u64::from(plan.height) * 2)) as u32;
                    out.copy_from_slice(at(sx.min(crop.w - 1), sy.min(crop.h - 1)));
                } else {
                    let fx = ((x as f64 + 0.5) * crop.w as f64 / plan.width as f64 - 0.5)
                        .clamp(0.0, (crop.w - 1) as f64);
                    let fy = ((y as f64 + 0.5) * crop.h as f64 / plan.height as f64 - 0.5)
                        .clamp(0.0, (crop.h - 1) as f64);
                    let (x0, y0) = (fx.floor() as u32, fy.floor() as u32);
                    let (dx, dy) = (fx - x0 as f64, fy - y0 as f64);
                    let mut sums = [0.0; 4];
                    for (sx, sy, weight) in [
                        (x0, y0, (1.0 - dx) * (1.0 - dy)),
                        ((x0 + 1).min(crop.w - 1), y0, dx * (1.0 - dy)),
                        (x0, (y0 + 1).min(crop.h - 1), (1.0 - dx) * dy),
                        ((x0 + 1).min(crop.w - 1), (y0 + 1).min(crop.h - 1), dx * dy),
                    ] {
                        let p = at(sx, sy);
                        let aw = f64::from(p[3]) * weight;
                        sums[3] += aw;
                        for c in 0..3 {
                            sums[c] += f64::from(p[c]) * aw;
                        }
                    }
                    out[3] = sums[3].round().clamp(0.0, 255.0) as u8;
                    for c in 0..3 {
                        out[c] = if sums[3] > 1e-8 {
                            (sums[c] / sums[3]).round().clamp(0.0, 255.0) as u8
                        } else {
                            0
                        };
                    }
                }
            }
        }
        Ok(Pixels {
            width: plan.width,
            height: plan.height,
            data,
        })
    }
    pub fn thumbnail(&self) -> Result<Pixels, String> {
        let mut plan = Plan::full(self.size);
        let scale = (1024.0 / self.size.0.max(self.size.1) as f64).min(1.0);
        plan.width = (self.size.0 as f64 * scale).round().max(1.0) as u32;
        plan.height = (self.size.1 as f64 * scale).round().max(1.0) as u32;
        self.render(plan)
    }
}
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

fn resolved_name(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let dir = parent
        .canonicalize()
        .map_err(|e| format!("目标目录不可用：{e}"))?;
    Ok(dir.join(path.file_name().ok_or("请选择文件名")?))
}

/// Never replace an existing file. Encode completely to an owned temporary file first.
pub fn save_new_png(
    source: &Source,
    plan: Plan,
    original: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    if !destination
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        return Err("输出文件扩展名必须为 .png".into());
    }
    let dest = resolved_name(destination)?;
    let orig = resolved_name(original)?;
    let same = if cfg!(windows) {
        dest.to_string_lossy().to_lowercase() == orig.to_string_lossy().to_lowercase()
    } else {
        dest == orig
    };
    if same || dest.symlink_metadata().is_ok() {
        return Err("不会覆盖原图或已有文件，请使用新的文件名".into());
    }
    let pixels = source.render(plan)?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = dest.with_file_name(format!(
        ".lcl-edit-{}-{stamp}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| format!("无法写入目标目录：{e}"))?;
    struct RemoveOnDrop(PathBuf);
    impl Drop for RemoveOnDrop {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }
    let _guard = RemoveOnDrop(temp.clone());
    let mut writer = BufWriter::new(file);
    image::codecs::png::PngEncoder::new(&mut writer)
        .write_image(
            &pixels.data,
            pixels.width,
            pixels.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("PNG 编码失败：{e}"))?;
    writer.flush().map_err(|e| format!("写入失败：{e}"))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|e| format!("保存失败：{e}"))?;
    drop(writer);
    // Hard-link publication is complete-file and no-replace on NTFS and Unix.
    if let Err(link_error) = fs::hard_link(&temp, &dest) {
        if dest.symlink_metadata().is_ok() {
            return Err("文件名已被占用，请使用新的文件名".into());
        }
        // FAT / some network disks do not support links. create_new remains no-replace.
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)
            .map_err(|e| format!("无法新建文件：{e}（{link_error}）"))?;
        let result = (|| -> std::io::Result<()> {
            let mut input = fs::File::open(&temp)?;
            std::io::copy(&mut input, &mut output)?;
            output.flush()?;
            output.sync_all()
        })();
        drop(output);
        if let Err(e) = result {
            let _ = fs::remove_file(&dest);
            return Err(format!("写入失败，已撤销本次新文件：{e}"));
        }
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iv_core::decode::{AnimatedFrame, ImageKind, MipLevel};
    fn decoded(w: u32, h: u32, data: Vec<u8>) -> DecodedImage {
        DecodedImage {
            width: w,
            height: h,
            mips: vec![MipLevel {
                width: w,
                height: h,
                data: PixelData::Rgba8(data),
            }],
            kind: ImageKind::Png,
            compression: None,
            has_alpha: true,
            is_hdr: false,
            extra_meta: None,
            frames: vec![],
        }
    }
    fn source(w: u32, h: u32, data: Vec<u8>) -> Source {
        Source::new(Arc::new(decoded(w, h, data)), 0).unwrap()
    }
    #[test]
    fn dimensions_and_crop_reject_zero_overflow_and_budget() {
        for s in [(0, 1), (1, 0), (u32::MAX, 2), (MAX_SIDE, MAX_SIDE)] {
            assert!(pixel_bytes(s.0, s.1).is_err());
        }
        assert!(Crop {
            x: u32::MAX,
            y: 0,
            w: 2,
            h: 1
        }
        .validate((8, 8))
        .is_err());
        assert!(Crop {
            x: 0,
            y: 0,
            w: 0,
            h: 1
        }
        .validate((8, 8))
        .is_err());
        assert!(pixel_bytes(4096, 4096).is_ok());
    }
    #[test]
    fn crop_is_exact_including_transparent_hidden_rgb() {
        let s = source(
            2,
            2,
            vec![1, 2, 3, 0, 4, 5, 6, 64, 7, 8, 9, 128, 10, 11, 12, 255],
        );
        let mut p = Plan::full(s.size);
        p.set_crop(Crop {
            x: 1,
            y: 0,
            w: 1,
            h: 2,
        });
        assert_eq!(
            s.render(p).unwrap().data,
            vec![4, 5, 6, 64, 10, 11, 12, 255]
        );
        assert_eq!(
            s.render(Plan::full(s.size)).unwrap().data,
            s.pixels().unwrap()
        );
    }
    #[test]
    fn nearest_preserves_raw_channel_bytes() {
        let s = source(2, 1, vec![17, 88, 199, 0, 220, 80, 100, 128]);
        let mut p = Plan::full(s.size);
        p.width = 4;
        p.filter = Filter::Nearest;
        assert_eq!(
            s.render(p).unwrap().data,
            vec![17, 88, 199, 0, 17, 88, 199, 0, 220, 80, 100, 128, 220, 80, 100, 128]
        );
    }
    #[test]
    fn smooth_resamples_premultiplied_alpha_without_hidden_color_bleed() {
        let s = source(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 0]);
        let mut p = Plan::full(s.size);
        p.width = 1;
        assert_eq!(s.render(p).unwrap().data, vec![255, 0, 0, 128]);
    }
    #[test]
    fn smooth_does_not_sample_outside_crop() {
        let s = source(3, 1, vec![255, 0, 0, 255, 0, 255, 0, 127, 0, 0, 255, 255]);
        let mut p = Plan::full(s.size);
        p.set_crop(Crop {
            x: 1,
            y: 0,
            w: 1,
            h: 1,
        });
        p.width = 9;
        for px in s.render(p).unwrap().data.chunks_exact(4) {
            assert_eq!(px, &[0, 255, 0, 127]);
        }
    }
    #[test]
    fn reversed_drag_and_moving_stay_inside_source() {
        let c = Crop::from_points([20.0, 40.0], [-5.0, 9.0], (30, 30));
        assert_eq!(
            c,
            Crop {
                x: 0,
                y: 9,
                w: 20,
                h: 21
            }
        );
        assert_eq!(
            c.translated(100, -100, (30, 30)),
            Crop {
                x: 10,
                y: 0,
                w: 20,
                h: 21
            }
        );
        assert_eq!(
            Crop::centered_ratio((400, 200), (1, 1)),
            Crop {
                x: 100,
                y: 0,
                w: 200,
                h: 200
            }
        );
    }
    #[test]
    fn resolution_lock_rounds_using_crop_ratio() {
        assert_eq!(Plan::paired_dimension(800, 240, 320), 600);
        assert_eq!(Plan::paired_dimension(1, 1, 100), 1);
        assert_eq!(Plan::paired_dimension(127, 9, 16), 71);
    }
    #[test]
    fn animation_uses_selected_frame_and_static_always_uses_mip_zero() {
        let mut d = decoded(1, 1, vec![1, 2, 3, 4]);
        d.frames = vec![
            AnimatedFrame {
                data: PixelData::Rgba8(vec![1, 2, 3, 4]),
                delay_ms: 100,
            },
            AnimatedFrame {
                data: PixelData::Rgba8(vec![5, 6, 7, 8]),
                delay_ms: 100,
            },
        ];
        let s = Source::new(Arc::new(d), 1).unwrap();
        assert_eq!(s.render(Plan::full(s.size)).unwrap().data, vec![5, 6, 7, 8]);
        assert!(s.note.contains("2/2"));
        let mut d = decoded(2, 2, vec![23; 16]);
        d.mips.push(MipLevel {
            width: 1,
            height: 1,
            data: PixelData::Rgba8(vec![99; 4]),
        });
        let s = Source::new(Arc::new(d), 0).unwrap();
        assert_eq!(s.size, (2, 2));
        assert!(s.note.contains("Mip 0"));
    }
    #[test]
    fn hdr_invalid_frame_and_truncated_pixels_are_rejected() {
        let mut d = decoded(1, 1, vec![0; 4]);
        d.is_hdr = true;
        assert!(Source::new(Arc::new(d), 0).is_err());
        assert!(Source::new(Arc::new(decoded(2, 2, vec![0; 4])), 0).is_err());
        let mut d = decoded(1, 1, vec![0; 4]);
        d.frames = vec![
            AnimatedFrame {
                data: PixelData::Rgba8(vec![0; 4]),
                delay_ms: 100
            };
            2
        ];
        assert!(Source::new(Arc::new(d), usize::MAX).is_err());
    }
    #[test]
    fn png_export_roundtrips_and_never_overwrites_any_existing_file() {
        let dir = std::env::temp_dir().join(format!(
            "iv-edit-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let orig = dir.join("原图.png");
        fs::write(&orig, b"original sentinel").unwrap();
        let out = dir.join("裁剪结果.png");
        let s = source(2, 1, vec![17, 88, 199, 0, 1, 2, 3, 128]);
        let p = Plan::full(s.size);
        assert!(save_new_png(&s, p, &orig, &orig).is_err());
        assert!(save_new_png(&s, p, &orig, &dir.join("bad.jpg")).is_err());
        save_new_png(&s, p, &orig, &out).unwrap();
        let bytes = fs::read(&out).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(decoded.as_raw().as_slice(), s.pixels().unwrap());
        assert!(save_new_png(&s, p, &orig, &out).is_err());
        assert_eq!(fs::read(&out).unwrap(), bytes);
        assert_eq!(fs::read(&orig).unwrap(), b"original sentinel");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_file(&orig).unwrap();
        assert!(save_new_png(&s, p, &orig, &orig).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}

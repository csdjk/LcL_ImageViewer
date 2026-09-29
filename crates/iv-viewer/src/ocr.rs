//! On-demand, offline OCR. Only immutable decoded pixels are passed to the OS.
//! No shell, network, screenshot, disk scratch image or clipboard writes in this module.
use iv_core::decode::{DecodedImage, PixelData};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const MAX_WORK_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct Input {
    pub image: Arc<DecodedImage>,
    pub path: PathBuf,
    pub frame: usize,
    pub size: (u32, u32),
    pub note: String,
}
impl Input {
    pub fn new(image: Arc<DecodedImage>, path: PathBuf, frame: usize) -> Result<Self, String> {
        let animated = image.frames.len() > 1;
        if animated && frame >= image.frames.len() {
            return Err("当前动画帧已失效".into());
        }
        let size = if animated {
            (image.width, image.height)
        } else {
            image
                .mips
                .first()
                .map(|m| (m.width, m.height))
                .ok_or("没有可识别的图片")?
        };
        if image.is_hdr {
            return Err("OCR 暂不识别 HDR 浮点图片，请先另存为普通图片".into());
        }
        let note = if animated {
            format!(
                "当前动画帧 {}/{} · 只识别这一帧",
                frame + 1,
                image.frames.len()
            )
        } else if image.mips.len() > 1 {
            "原图 Mip 0 · 不使用低分辨率预览".into()
        } else {
            "识别原图 · 不包含查看器界面、单通道效果或棋盘格".into()
        };
        let input = Self {
            image,
            path,
            frame: if animated { frame } else { 0 },
            size,
            note,
        };
        let bytes = u64::from(size.0)
            .checked_mul(u64::from(size.1))
            .and_then(|n| n.checked_mul(4));
        if size.0 == 0 || size.1 == 0 || bytes != Some(input.pixels()?.len() as u64) {
            return Err("图像尺寸与像素数据不匹配".into());
        }
        Ok(input)
    }
    pub fn matches(&self, image: &Arc<DecodedImage>, frame: usize) -> bool {
        Arc::ptr_eq(&self.image, image) && (image.frames.len() <= 1 || self.frame == frame)
    }
    fn pixels(&self) -> Result<&[u8], String> {
        let p = if self.image.frames.len() > 1 {
            &self
                .image
                .frames
                .get(self.frame)
                .ok_or("动画帧已失效")?
                .data
        } else {
            &self.image.mips.first().ok_or("没有图像数据")?.data
        };
        match p {
            PixelData::Rgba8(v) => Ok(v),
            _ => Err("OCR 暂不支持浮点像素".into()),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Language {
    pub tag: String,
    pub name: String,
}
pub struct Report {
    pub text: String,
    pub language: String,
    pub line_count: usize,
    pub source_size: (u32, u32),
    pub working_size: (u32, u32),
    pub dark_matte: bool,
}
pub enum Message {
    Languages(Vec<Language>),
    Finished(Result<Report, String>),
}
pub fn copy_text(text: &str, keep_lines: bool) -> Option<String> {
    if text.trim().is_empty() {
        return None;
    }
    Some(if keep_lines {
        text.trim().replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    })
}
fn joined_lines(lines: impl IntoIterator<Item = String>) -> String {
    let mut text = String::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if text.len() + line.len() + 1 > MAX_TEXT_BYTES {
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(line);
    }
    text
}
fn work_size((w, h): (u32, u32), native_limit: u32) -> Result<(u32, u32), String> {
    if w == 0 || h == 0 || native_limit == 0 {
        return Err("无效的 OCR 图片尺寸".into());
    }
    let limit = native_limit.min(4096) as f64;
    let scale = (limit / w.max(h) as f64)
        .min((MAX_WORK_PIXELS as f64 / (w as f64 * h as f64)).sqrt())
        .min(1.0);
    Ok((
        (w as f64 * scale).floor().max(1.0) as u32,
        (h as f64 * scale).floor().max(1.0) as u32,
    ))
}
struct Prepared {
    size: (u32, u32),
    gray: Vec<u8>,
    dark_matte: bool,
}
fn prepare(input: &Input, native_limit: u32, cancel: &AtomicBool) -> Result<Prepared, String> {
    let size = work_size(input.size, native_limit)?;
    let src = input.pixels()?;
    // Transparent light lettering needs a dark matte; hidden RGB (A=0) never influences it.
    let step = (src.len() / 4 / 4096).max(1);
    let mut lum = 0u64;
    let mut weight = 0u64;
    for p in src.chunks_exact(4).step_by(step) {
        let a = u64::from(p[3]);
        lum += (u64::from(p[0]) * 77 + u64::from(p[1]) * 150 + u64::from(p[2]) * 29) / 256 * a;
        weight += a;
    }
    let dark = weight > 0 && lum / weight > 160;
    let matte = if dark { 0.0 } else { 255.0 };
    let len = size.0 as usize * size.1 as usize;
    let mut gray = Vec::new();
    gray.try_reserve_exact(len)
        .map_err(|_| "OCR 内存不足，请裁剪后重试")?;
    gray.resize(len, 0);
    let at = |x: u32, y: u32| {
        let i = (y as usize * input.size.0 as usize + x as usize) * 4;
        let p = &src[i..i + 4];
        let l = (p[0] as f64 * 77.0 + p[1] as f64 * 150.0 + p[2] as f64 * 29.0) / 256.0;
        (l * p[3] as f64 + matte * (255 - p[3]) as f64) / 255.0
    };
    for y in 0..size.1 {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消识别".into());
        }
        let fy = ((y as f64 + 0.5) * input.size.1 as f64 / size.1 as f64 - 0.5)
            .clamp(0.0, (input.size.1 - 1) as f64);
        let y0 = fy.floor() as u32;
        let dy = fy - y0 as f64;
        for x in 0..size.0 {
            let fx = ((x as f64 + 0.5) * input.size.0 as f64 / size.0 as f64 - 0.5)
                .clamp(0.0, (input.size.0 - 1) as f64);
            let x0 = fx.floor() as u32;
            let dx = fx - x0 as f64;
            let x1 = (x0 + 1).min(input.size.0 - 1);
            let y1 = (y0 + 1).min(input.size.1 - 1);
            gray[y as usize * size.0 as usize + x as usize] = (at(x0, y0) * (1.0 - dx) * (1.0 - dy)
                + at(x1, y0) * dx * (1.0 - dy)
                + at(x0, y1) * (1.0 - dx) * dy
                + at(x1, y1) * dx * dy)
                .round() as u8;
        }
    }
    Ok(Prepared {
        size,
        gray,
        dark_matte: dark,
    })
}
#[cfg(not(windows))]
pub fn recognize(
    _input: &Input,
    _language: &str,
    _cancel: &AtomicBool,
    _languages: impl FnOnce(Vec<Language>),
) -> Result<Report, String> {
    Err("当前平台尚未接入 OCR；此功能目前使用 Windows 本机文字识别组件".into())
}
#[cfg(windows)]
pub fn recognize(
    input: &Input,
    language: &str,
    cancel: &AtomicBool,
    languages: impl FnOnce(Vec<Language>),
) -> Result<Report, String> {
    use std::time::{Duration, Instant};
    use windows::{
        core::HSTRING,
        Foundation::AsyncStatus,
        Globalization::Language as WinLanguage,
        Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    };
    // This function is called on a dedicated worker, never on the GUI thread.
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|e| format!("无法初始化本机 OCR：{e}"))?;
    struct Runtime;
    impl Drop for Runtime {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _runtime = Runtime;
    let os_error = |e: windows::core::Error| {
        format!("本机 OCR 不可用：{e}。请检查 Windows 的语言/文字识别组件；部分系统可能需要带应用标识的安装方式。不会上传图片。")
    };
    let available = OcrEngine::AvailableRecognizerLanguages().map_err(os_error)?;
    let mut choices = vec![];
    for i in 0..available.Size().map_err(os_error)? {
        let l = available.GetAt(i).map_err(os_error)?;
        choices.push(Language {
            tag: l.LanguageTag().map_err(os_error)?.to_string(),
            name: l.NativeName().map_err(os_error)?.to_string(),
        });
    }
    let tags: Vec<_> = choices.iter().map(|l| l.tag.clone()).collect();
    languages(choices);
    if tags.is_empty() {
        return Err("未安装 OCR 识别语言。请在 Windows 设置 → 时间和语言 → 语言选项中安装所需语言的文字识别组件，然后重试。程序不会自行安装。".into());
    }
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消识别".into());
    }
    let engine = if language.is_empty() {
        OcrEngine::TryCreateFromUserProfileLanguages().or_else(|_| {
            let l = WinLanguage::CreateLanguage(&HSTRING::from(&tags[0]))?;
            OcrEngine::TryCreateFromLanguage(&l)
        })
    } else if tags.iter().any(|t| t.eq_ignore_ascii_case(language)) {
        WinLanguage::CreateLanguage(&HSTRING::from(language))
            .and_then(|l| OcrEngine::TryCreateFromLanguage(&l))
    } else {
        return Err(format!(
            "当前电脑未安装 {language} 的 OCR 语言组件，请从列表中选择已安装的语言"
        ));
    };
    let engine = engine.map_err(os_error)?;
    let recognized_language = engine
        .RecognizerLanguage()
        .and_then(|l| l.LanguageTag())
        .map_err(os_error)?
        .to_string();
    let prepared = prepare(
        input,
        OcrEngine::MaxImageDimension().map_err(os_error)?,
        cancel,
    )?;
    let writer = DataWriter::new().map_err(os_error)?;
    writer.WriteBytes(&prepared.gray).map_err(os_error)?;
    let buffer = writer.DetachBuffer().map_err(os_error)?;
    let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
        &buffer,
        BitmapPixelFormat::Gray8,
        prepared.size.0 as i32,
        prepared.size.1 as i32,
    )
    .map_err(os_error)?;
    let operation = engine.RecognizeAsync(&bitmap).map_err(os_error)?;
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) || start.elapsed() > Duration::from_secs(45) {
            let _ = operation.Cancel();
            return Err(if cancel.load(Ordering::Relaxed) {
                "已取消识别"
            } else {
                "识别超时，请缩小或裁剪图片后重试"
            }
            .into());
        }
        if operation.Status().map_err(os_error)? != AsyncStatus::Started {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let result = operation.GetResults().map_err(os_error)?;
    let lines = result.Lines().map_err(os_error)?;
    let mut text_lines = vec![];
    let count = lines.Size().map_err(os_error)? as usize;
    if count > 10000 {
        return Err("识别文字过长，请裁剪后分次识别".into());
    }
    let mut text_bytes = 0;
    for i in 0..count {
        let line = lines
            .GetAt(i as u32)
            .and_then(|l| l.Text())
            .map_err(os_error)?
            .to_string();
        text_bytes += line.len() + 1;
        if text_bytes > MAX_TEXT_BYTES {
            return Err("识别文字过长，请裁剪后分次识别".into());
        }
        text_lines.push(line);
    }
    let text = joined_lines(text_lines);
    let _ = bitmap.Close();
    let _ = writer.Close();
    Ok(Report {
        text,
        language: recognized_language,
        line_count: count,
        source_size: input.size,
        working_size: prepared.size,
        dark_matte: prepared.dark_matte,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iv_core::decode::{AnimatedFrame, ImageKind, MipLevel};
    fn image(w: u32, h: u32, p: Vec<u8>) -> DecodedImage {
        DecodedImage {
            width: w,
            height: h,
            mips: vec![MipLevel {
                width: w,
                height: h,
                data: PixelData::Rgba8(p),
            }],
            kind: ImageKind::Png,
            compression: None,
            has_alpha: true,
            is_hdr: false,
            extra_meta: None,
            frames: vec![],
        }
    }
    #[test]
    fn dimensions_are_bounded_without_upscaling() {
        for s in [
            (1, 1),
            (160, 80),
            (3200, 1800),
            (64000, 32000),
            (u32::MAX, u32::MAX),
        ] {
            let t = work_size(s, 10000).unwrap();
            assert!(
                t.0 <= 4096 && t.1 <= 4096 && u64::from(t.0) * u64::from(t.1) <= MAX_WORK_PIXELS
            );
            assert!(t.0 <= s.0 && t.1 <= s.1);
        }
        assert_eq!(work_size((32, 64), 10000).unwrap(), (32, 64));
        assert!(work_size((0, 3), 5).is_err());
    }
    #[test]
    fn transparent_hidden_colors_are_not_recognized() {
        let input = Input::new(
            Arc::new(image(2, 1, vec![255, 0, 0, 0, 0, 0, 0, 255])),
            "a.png".into(),
            0,
        )
        .unwrap();
        let original = input.pixels().unwrap().to_vec();
        let p = prepare(&input, 1000, &AtomicBool::new(false)).unwrap();
        assert_eq!(p.gray, vec![255, 0]);
        assert_eq!(input.pixels().unwrap(), original);
        let bright = Input::new(
            Arc::new(image(2, 1, vec![25, 50, 80, 0, 255, 255, 255, 255])),
            "b.png".into(),
            0,
        )
        .unwrap();
        let p = prepare(&bright, 1000, &AtomicBool::new(false)).unwrap();
        assert!(p.dark_matte);
        assert_eq!(p.gray, vec![0, 255]);
    }
    #[test]
    fn current_frame_and_mip_zero_use_original_bytes() {
        let mut d = image(1, 1, vec![1, 2, 3, 255]);
        d.frames = vec![
            AnimatedFrame {
                data: PixelData::Rgba8(vec![1, 2, 3, 255]),
                delay_ms: 100,
            },
            AnimatedFrame {
                data: PixelData::Rgba8(vec![9, 8, 7, 255]),
                delay_ms: 100,
            },
        ];
        let a = Arc::new(d);
        let input = Input::new(a.clone(), "a.avif".into(), 1).unwrap();
        assert_eq!(input.pixels().unwrap(), &[9, 8, 7, 255]);
        assert!(input.matches(&a, 1));
        assert!(!input.matches(&a, 0));
        assert!(Input::new(a, "a".into(), 2).is_err());
        let mut d = image(2, 1, vec![3; 8]);
        d.mips.push(MipLevel {
            width: 1,
            height: 1,
            data: PixelData::Rgba8(vec![99; 4]),
        });
        let input = Input::new(Arc::new(d), "a.dds".into(), 0).unwrap();
        assert_eq!(input.size, (2, 1));
        assert_eq!(input.pixels().unwrap(), &[3; 8]);
    }
    #[test]
    fn rejects_hdr_bad_data_and_cancellation() {
        let mut d = image(1, 1, vec![1; 4]);
        d.is_hdr = true;
        assert!(Input::new(Arc::new(d), "a".into(), 0).is_err());
        assert!(Input::new(Arc::new(image(2, 2, vec![1; 4])), "a".into(), 0).is_err());
        let s = Input::new(Arc::new(image(1, 1, vec![1; 4])), "a".into(), 0).unwrap();
        assert!(prepare(&s, 1000, &AtomicBool::new(true)).is_err());
    }
    #[test]
    fn text_copy_preserves_lines_and_never_clears_on_empty() {
        assert_eq!(copy_text(" \n", true), None);
        assert_eq!(copy_text("中文\r\nGame UI", true).unwrap(), "中文\nGame UI");
        assert_eq!(copy_text("中文\nGame  UI", false).unwrap(), "中文 Game UI");
        assert_eq!(
            joined_lines(["  中文 ".into(), "".into(), "Game UI".into()]),
            "中文\nGame UI"
        );
    }
}

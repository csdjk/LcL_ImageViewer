//! Bounded OCR job and a selectable, editable result card using shared UI controls.
use crate::{
    ocr::{self, Input, Language, Message, Report},
    ui::{self, editor_controls as controls, Icon, Palette},
};
use eframe::egui::{self, RichText, Vec2};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, TryRecvError},
    Arc,
};
use std::time::{Duration, Instant};

#[cfg(windows)]
fn clipboard_sequence() -> u32 {
    #[link(name = "user32")]
    extern "system" {
        fn GetClipboardSequenceNumber() -> u32;
    }
    unsafe { GetClipboardSequenceNumber() }
}
#[cfg(not(windows))]
fn clipboard_sequence() -> u32 {
    0
}

pub struct Panel {
    pub visible: bool,
    input: Option<Input>,
    job: Option<Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    languages: Vec<Language>,
    language: String,
    text: String,
    report: Option<Report>,
    error: Option<String>,
    auto_copy: bool,
    clipboard_at_start: u32,
    keep_lines: bool,
    copied_at: Option<Instant>,
    copy_skipped: bool,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            visible: false,
            input: None,
            job: None,
            cancel: Arc::new(AtomicBool::new(false)),
            languages: vec![],
            language: String::new(),
            text: String::new(),
            report: None,
            error: None,
            auto_copy: false,
            clipboard_at_start: 0,
            keep_lines: true,
            copied_at: None,
            copy_skipped: false,
        }
    }
}
impl Drop for Panel {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Panel {
    pub fn new(language: Option<String>, keep_lines: bool) -> Self {
        let mut panel = Self::default();
        panel.language = language.unwrap_or_default();
        panel.keep_lines = keep_lines;
        panel
    }
    pub fn language_key(&self) -> String {
        self.language.clone()
    }
    pub fn keep_lines(&self) -> bool {
        self.keep_lines
    }
    pub fn is_current(&self, image: &Arc<iv_core::decode::DecodedImage>, frame: usize) -> bool {
        self.input.as_ref().is_none_or(|i| i.matches(image, frame))
    }
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.auto_copy = false;
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn invalidate(&mut self) {
        self.dismiss();
        self.input = None;
        self.text.clear();
        self.report = None;
        self.error = None;
    }
    pub fn open(&mut self, ctx: &egui::Context, input: Input, auto_copy: bool) {
        let cached = self
            .input
            .as_ref()
            .is_some_and(|i| i.matches(&input.image, input.frame))
            && self.report.is_some();
        self.visible = true;
        if cached {
            if auto_copy {
                self.copy(ctx);
            }
            return;
        }
        if self.job.is_some() {
            self.error = Some("上次识别正在结束，请稍后点击重新识别".into());
            return;
        }
        self.input = Some(input);
        self.start(ctx, auto_copy);
    }
    fn start(&mut self, ctx: &egui::Context, auto_copy: bool) {
        if self.job.is_some() {
            return;
        }
        let Some(input) = self.input.clone() else {
            return;
        };
        self.text.clear();
        self.report = None;
        self.error = None;
        self.copied_at = None;
        self.copy_skipped = false;
        self.auto_copy = auto_copy;
        self.clipboard_at_start = clipboard_sequence();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let language = self.language.clone();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        match std::thread::Builder::new()
            .name("image-ocr".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    ocr::recognize(&input, &language, &cancel, |ls| {
                        let _ = tx.send(Message::Languages(ls));
                        ctx.request_repaint();
                    })
                }))
                .unwrap_or_else(|_| Err("识别任务异常，请重新识别或裁剪图片后重试".into()));
                let _ = tx.send(Message::Finished(result));
                ctx.request_repaint();
            }) {
            Ok(_) => self.job = Some(rx),
            Err(e) => {
                self.auto_copy = false;
                self.error = Some(format!("无法启动文字识别：{e}"));
            }
        }
    }
    fn copy(&mut self, ctx: &egui::Context) {
        if let Some(text) = ocr::copy_text(&self.text, self.keep_lines) {
            ctx.output_mut(|o| o.copied_text = text);
            self.copied_at = Some(Instant::now());
            self.copy_skipped = false;
        }
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.job.take() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(Message::Languages(ls)) => self.languages = ls,
                Ok(Message::Finished(result)) => {
                    if self.cancel.load(Ordering::Relaxed) {
                        self.auto_copy = false;
                        return;
                    }
                    match result {
                        Ok(r) => {
                            self.text = r.text.clone();
                            self.report = Some(r);
                            if self.visible && self.auto_copy {
                                if self.clipboard_at_start == clipboard_sequence() {
                                    self.copy(ctx);
                                } else {
                                    self.copy_skipped = true;
                                }
                            }
                        }
                        Err(e) => self.error = Some(e),
                    }
                    self.auto_copy = false;
                    return;
                }
                Err(TryRecvError::Empty) => {
                    self.job = Some(rx);
                    ctx.request_repaint_after(Duration::from_millis(80));
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    if !self.cancel.load(Ordering::Relaxed) {
                        self.error = Some("识别任务已中断，请重试".into());
                    }
                    self.auto_copy = false;
                    return;
                }
            }
        }
    }
    pub fn show(&mut self, ctx: &egui::Context, pal: &Palette) {
        if !self.visible {
            return;
        }
        let screen = ctx.screen_rect();
        let width = (screen.width() - 64.0).min(480.0);
        let body = (screen.height() - 400.0).clamp(120.0, 380.0);
        let mut close = false;
        let mut rerun = false;
        let mut copy = false;
        let area=egui::Area::new(egui::Id::new("iv-ocr-result"))
            .order(egui::Order::Foreground).fixed_pos(egui::pos2(screen.right()-width-32.0,screen.top()+76.0)).movable(false)
            .show(ctx,|ui|{
                ui::menu_frame(pal).show(ui,|ui|{
                    controls::configure(ui);ui.set_width(width);
                    ui.horizontal(|ui|{
                        ui.label(RichText::new("文字识别 OCR").size(17.0).strong().color(pal.text));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui|{
                            if ui::icon_btn(ui,Icon::Close,false,pal).on_hover_text("关闭结果，不修改原图 (Esc)").clicked(){close=true;}
                        });
                    });
                    if let Some(input)=&self.input {
                        let name=input.path.file_name().unwrap_or_default().to_string_lossy();
                        ui.add(egui::Label::new(RichText::new(format!("{} · {} × {}",name,input.size.0,input.size.1)).color(pal.dim)).truncate(true))
                            .on_hover_text(format!("{}\n{}",input.path.display(),input.note));
                    }
                    ui.add_enabled_ui(self.job.is_none(),|ui|{
                        ui.horizontal(|ui|{
                            ui.label("识别语言");
                            let selected=if self.language.is_empty(){"自动（系统语言）".to_owned()}
                                else{self.languages.iter().find(|l|l.tag==self.language).map(|l|format!("{} · {}",l.name,l.tag)).unwrap_or_else(||self.language.clone())};
                            controls::combo(ui,"iv-ocr-language",&selected,width-95.0,|ui|{
                                ui.selectable_value(&mut self.language,String::new(),"自动（系统语言）");
                                for l in &self.languages{ui.selectable_value(&mut self.language,l.tag.clone(),format!("{} · {}",l.name,l.tag));}
                            });
                        });
                    });
                    egui::Frame::none().fill(pal.extreme).rounding(12.0).inner_margin(10.0).show(ui,|ui|{
                        let w=width-20.0;
                        if self.job.is_some(){ui.allocate_ui_with_layout(Vec2::new(w,body),egui::Layout::top_down(egui::Align::Center),|ui|{ui.add_space(body/3.0);ui.spinner();ui.label("正在本机识别…");ui.label(RichText::new("关闭面板可取消；图片不会上传").color(pal.dim));});}
                        else if let Some(error)=&self.error{
                            egui::ScrollArea::vertical().max_height(body).min_scrolled_height(body).show(ui,|ui|{ui.set_width(w);ui.label(RichText::new(error).color(pal.err_text));});
                        } else if self.report.is_some() && self.text.is_empty(){
                            ui.allocate_ui_with_layout(Vec2::new(w,body),egui::Layout::top_down(egui::Align::Min),|ui|{ui.add_space(12.0);ui.label("没有识别到文字");ui.label("可以切换语言后重新识别，或先裁剪出文字区域。剪贴板未改变。");});
                        } else {
                            egui::ScrollArea::vertical().id_source("iv-ocr-scroll").max_height(body).min_scrolled_height(body).show(ui,|ui|{
                                ui.add_sized([w,body],egui::TextEdit::multiline(&mut self.text).id_source("iv-ocr-text").frame(false).desired_width(w)
                                    .font(egui::FontId::proportional(15.0)).text_color(pal.text).hint_text("识别结果会显示在这里，可选择文字或直接校对"));
                            });
                        }
                    });
                    ui.horizontal(|ui|{
                        if controls::button(ui,"复制全文",self.job.is_none() && ocr::copy_text(&self.text,true).is_some(),controls::Role::Primary).clicked(){copy=true;}
                        controls::toggle(ui,"保留换行",&mut self.keep_lines);
                        if controls::button(ui,"重新识别",self.job.is_none() && self.input.is_some(),controls::Role::Secondary).clicked(){rerun=true;}
                    });
                    let status=if self.copy_skipped{"剪贴板已被其他操作更新，未自动覆盖；可点击复制全文".into()}
                        else if self.copied_at.is_some_and(|t|t.elapsed()<Duration::from_secs(4)){"已复制到剪贴板".into()}
                        else if let Some(r)=&self.report{format!("{} · {} 行 · {} 字符{}",r.language,r.line_count,self.text.chars().count(),
                            if r.working_size!=r.source_size {format!(" · 识别尺寸 {}×{}",r.working_size.0,r.working_size.1)}else{String::new()})}
                        else {"本机离线识别 · 仅使用已安装的语言组件".into()};
                    ui.add(egui::Label::new(RichText::new(status).size(12.0).color(pal.dim)).truncate(true));
                    let note=self.report.as_ref().map(|r|format!("结果可能有误，可直接校对后复制。透明区使用{}底。",if r.dark_matte{"黑"}else{"白"})).unwrap_or_else(||"不上传图片，不将识别文本保存到磁盘。".into());
                    ui.label(RichText::new(note).size(11.5).color(pal.dim));
                });
            });
        let _ = area;
        if copy {
            self.copy(ctx);
        }
        if rerun {
            self.start(ctx, false);
        }
        if close {
            self.dismiss();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canceled_or_closed_job_cannot_replace_clipboard() {
        let ctx = egui::Context::default();
        let mut p = Panel::default();
        let (tx, rx) = mpsc::channel();
        p.job = Some(rx);
        p.visible = true;
        p.auto_copy = true;
        p.dismiss();
        tx.send(Message::Finished(Ok(Report {
            text: "stale".into(),
            language: "en-US".into(),
            line_count: 1,
            source_size: (1, 1),
            working_size: (1, 1),
            dark_matte: false,
        })))
        .unwrap();
        p.poll(&ctx);
        assert!(p.text.is_empty() && p.job.is_none());
        assert!(ctx.output(|o| o.copied_text.is_empty()));
    }
    #[test]
    fn empty_result_leaves_clipboard_untouched() {
        let ctx = egui::Context::default();
        let mut p = Panel::default();
        p.text = " \n ".into();
        p.copy(&ctx);
        assert!(p.copied_at.is_none());
        assert!(ctx.output(|o| o.copied_text.is_empty()));
    }
}

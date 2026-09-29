"""Offline OCR integration contracts; native recognition is a separate opt-in GUI test."""
from pathlib import Path
import unittest
import re
R=Path(__file__).resolve().parents[2]
APP=(R/'crates/iv-viewer/src/app.rs').read_text('utf-8')
OCR=(R/'crates/iv-viewer/src/ocr.rs').read_text('utf-8')
OCR_COMPACT=re.sub(r'\s+','',OCR)
PANEL=(R/'crates/iv-viewer/src/ocr_panel.rs').read_text('utf-8')
CARGO=(R/'crates/iv-viewer/Cargo.toml').read_text('utf-8')
class OcrContracts(unittest.TestCase):
    def test_original_rgba_not_the_rendered_screen(self):
        source=APP.split('fn begin_ocr(',1)[1].split('fn close_ocr(',1)[0]
        self.assertIn('cur.img.clone(), cur.path.clone(), self.frame_index',source)
        self.assertNotIn('self.channel',source);self.assertNotIn('self.mip_index',source)
        self.assertIn('self.playing=false',source.replace(' ',''))
        self.assertIn('image.mips.first()',OCR_COMPACT)
    def test_platform_specific_os_dependencies_only(self):
        block=CARGO.split("[target.'cfg(windows)'.dependencies]",1)[1].split('[target.',1)[0]
        for flag in ['Media_Ocr','Graphics_Imaging','Storage_Streams','Win32_System_WinRT']:self.assertIn(flag,block)
        self.assertIn('#[cfg(not(windows))]',OCR)
        for forbidden in ['reqwest','https://','std::process::Command','File::create','save_buffer','fs::write']:self.assertNotIn(forbidden,OCR+PANEL)
    def test_on_demand_bounded_cancellable_worker(self):
        self.assertIn('if self.job.is_some()',PANEL)
        self.assertIn('image-ocr',PANEL);self.assertIn('AtomicBool',PANEL)
        self.assertIn('self.cancel.load',PANEL);self.assertIn('operation.Cancel()',OCR)
        self.assertIn('MAX_WORK_PIXELS',OCR);self.assertIn('from_secs(45)',OCR)
        self.assertIn('self.ocr.invalidate()',APP)
    def test_nonempty_clipboard_write_and_conflict_guard(self):
        self.assertIn('ifletSome(text)=ocr::copy_text',re.sub(r'\s+','',PANEL))
        self.assertIn('clipboard_at_start==clipboard_sequence()',re.sub(r'\s+','',PANEL))
        self.assertIn('self.visible && self.auto_copy',PANEL)
        self.assertNotIn('copied_text',OCR)
    def test_real_shared_controls_and_three_entry_routes(self):
        for word in ['Icon::Ocr','识别并复制图片文字','Key::C']:self.assertIn(word,APP)
        for word in ['controls::button','controls::combo','controls::toggle','TextEdit::multiline','保留换行','重新识别']:self.assertIn(word,PANEL)
        self.assertIn('iv-ocr-language',APP);self.assertIn('iv-ocr-keep-lines',APP)

if __name__=='__main__':unittest.main()

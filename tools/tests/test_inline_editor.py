"""Main-canvas integration contracts; executable geometry/pixel tests live in Rust."""
from pathlib import Path
import unittest
R = Path(__file__).resolve().parents[2]
APP = (R/'crates/iv-viewer/src/app.rs').read_text('utf-8')
ED = (R/'crates/iv-viewer/src/editor.rs').read_text('utf-8')
OPS = (R/'crates/iv-viewer/src/edit_ops.rs').read_text('utf-8')

class InlineEditorTests(unittest.TestCase):
    def test_editor_has_main_canvas_not_preview_window(self):
        self.assertNotIn('egui::Window', ED)
        for key in ['TopBottomPanel::top', 'TopBottomPanel::bottom', 'CentralPanel::default', 'draw_canvas']:
            self.assertIn(key, ED)
        update = APP[APP.index('fn update(&mut self, ctx:'):]
        self.assertLess(update.index('self.draw_editor(ctx, &pal)'), update.index('CentralPanel::default'))
    def test_transform_operations_and_bounded_document_history(self):
        for key in ['FlipHorizontal','FlipVertical','Quarter','Rotate','Resize','Crop','HISTORY_BUDGET','trim_history']:
            self.assertIn(key,OPS)
        self.assertIn('self.document.commit(source)',ED)
        self.assertIn('resize_crop', ED)
        self.assertIn('center_top()', ED)
        self.assertIn('left_center()', ED)
    def test_save_not_view_zoom_and_pending_changes_not_silently_exported(self):
        save = ED[ED.index('pub fn save_to('):ED.index('fn draft_key(')]
        self.assertIn('Plan::full(source.size)',save)
        self.assertNotIn('self.view.scale', save)
        self.assertIn('self.pending()', save)
        self.assertIn('pending_mode',ED)
        self.assertIn('应用并切换', ED)
        self.assertIn('放弃编辑', ED)
    def test_draft_preview_and_export_use_the_same_pixel_operations(self):
        self.assertIn('edit_ops::apply(&source, op)',ED)
        self.assertIn('image-edit-draft',ED)
        self.assertIn('image-edit-operation',ED)
        self.assertIn('draft_changed.elapsed()',ED)
        self.assertIn('180',ED)

if __name__=='__main__':unittest.main()

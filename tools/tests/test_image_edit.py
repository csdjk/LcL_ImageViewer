"""Editing scope and integration contracts; pixels and real GUI tested separately."""
from pathlib import Path
import unittest
R = Path(__file__).resolve().parents[2]
APP = (R/'crates/iv-viewer/src/app.rs').read_text('utf-8')
CORE = (R/'crates/iv-viewer/src/image_edit.rs').read_text('utf-8')
EDITOR = (R/'crates/iv-viewer/src/editor.rs').read_text('utf-8')

class EditorWiring(unittest.TestCase):
    def test_editor_uses_original_decoded_snapshot_not_display_or_mip_selection(self):
        section = APP[APP.index('fn begin_editor('):APP.index('fn draw_editor(')]
        self.assertIn('cur.img.clone(), self.frame_index', section)
        self.assertNotIn('self.channel', section)
        self.assertNotIn('self.mip_index', section)
        self.assertIn('self.playing = false;', section)
        self.assertIn('image.mips.first()', CORE)
    def test_editor_mode_blocks_file_drop_navigation_and_auto_refresh(self):
        body = APP[APP.index('fn handle_global_input('):APP.index('fn request_delete(')]
        self.assertLess(body.index('if let Some(editor)'), body.index('dropped_files'))
        self.assertIn('!self.delete_active() && self.editor.is_none()', APP)
        self.assertIn('if self.delete_active() || self.editor.is_some() { return; }', APP)
        self.assertIn('ViewportCommand::CancelClose', APP)
    def test_only_explicit_save_dispatches_output_without_install_or_publish(self):
        self.assertIn('save_file()', APP)
        self.assertIn('set_parent(&owner)', APP)
        self.assertIn('editor.save_to(ctx, path)', APP)
        self.assertIn('create_new(true)', CORE)
        self.assertNotIn('truncate(true)', CORE)
        self.assertIn('image-edit-save', EDITOR)
        self.assertIn('if self.save_job.is_some()', EDITOR)
    def test_ui_has_both_tools_and_retains_prior_readout(self):
        for term in ['裁剪','分辨率','锁定比例','另存为 PNG','撤销','重做','重置全部']:
            self.assertIn(term, EDITOR)
        self.assertIn('self.pixel_format.text(position, rgba, self.channel)', APP)
        self.assertNotIn('Icon::Fit', APP)
        self.assertIn('Icon::Actual', APP)

if __name__ == '__main__': unittest.main()

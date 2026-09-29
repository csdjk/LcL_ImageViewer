"""Guard the toolbar-only scope of the zoom-entry cleanup.

These source contracts supplement (not replace) native screenshot/interaction QA.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
APP = (ROOT / 'crates/iv-viewer/src/app.rs').read_text(encoding='utf-8')
UI = (ROOT / 'crates/iv-viewer/src/ui.rs').read_text(encoding='utf-8')


class TopbarZoomControlsTests(unittest.TestCase):
    def test_topbar_keeps_actual_size_without_a_fit_slot(self):
        start = APP.index('fn draw_top_overlay(')
        stop = APP.index('fn draw_side_navigation(', start)
        toolbar = APP[start:stop]
        self.assertEqual(toolbar.count('Icon::Actual'), 1)
        self.assertNotIn('Icon::Fit', toolbar)
        controls = toolbar[toolbar.index('// —— 视图控制 ——'):toolbar.index('// —— 动画播放控件')]
        self.assertEqual(re.findall(r'Icon::(\w+)', controls), ['Grid', 'Actual', 'Bounds', 'Edit'])
        self.assertIn('self.pending_actual = true;', controls)
        self.assertNotIn('add_space', controls)

    def test_keyboard_and_automatic_fit_are_preserved(self):
        self.assertRegex(APP, r'if key\(Key::F\)\s*\{\s*self\.fit\(canvas\);')
        self.assertRegex(APP, r'if key\(Key::Num0\)\s*\{\s*self\.actual_size\(canvas\);')
        self.assertIn('self.pending_fit = !preserve_view;', APP)
        self.assertRegex(APP, r'if self\.pending_fit\s*\{\s*self\.fit\(canvas_size\);')
        start = APP.index('fn actual_size(')
        self.assertIn('self.view.scale = 1.0;', APP[start:APP.index('\n    }', start)])

    def test_unused_fit_icon_removed_and_readme_matches(self):
        self.assertNotIn('Icon::Fit', UI)
        icon_enum = UI[UI.index('pub enum Icon {'):UI.index('\n}', UI.index('pub enum Icon {'))]
        self.assertNotRegex(icon_enum, r'\bFit\s*,')
        self.assertIn('Actual,', icon_enum)
        self.assertIn('顶部保留 **实际大小** 按钮', (ROOT / 'README.md').read_text(encoding='utf-8'))


if __name__ == '__main__':
    unittest.main()

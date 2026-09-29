"""Editor control wiring. Runtime geometry and original pixel tests run separately."""
from pathlib import Path
import re
import unittest
R = Path(__file__).resolve().parents[2]
ED = (R/'crates/iv-viewer/src/editor.rs').read_text('utf-8')
UI = (R/'crates/iv-viewer/src/editor_controls.rs').read_text('utf-8')

class EditorControlsTests(unittest.TestCase):
    def test_every_editor_action_uses_the_shared_controls(self):
        for old in ['egui::Button::new', 'ui.button(', '.button("', 'egui::SelectableLabel', 'ui.checkbox(', 'egui::ComboBox::']:
            self.assertNotIn(old, ED)
        for new in ['controls::tab(', 'controls::toggle(', 'controls::field(', 'controls::combo(', 'controls::notice(']:
            self.assertIn(new, ED)
    def test_primary_secondary_and_discard_roles_are_explicit(self):
        for label in ['应用裁剪', '应用旋转', '应用尺寸', '另存为 PNG…', '应用并切换']:
            self.assertRegex(ED, r'primary_command\(\s*ui,\s*"'+re.escape(label)+'"')
        for label in ['放弃编辑','丢弃调整']:
            self.assertRegex(ED, r'danger_command\(\s*ui,\s*"'+label+'"')
    def test_style_is_local_and_reuses_existing_theme(self):
        self.assertIn('button_face', UI)
        self.assertIn('surface_shapes', UI)
        self.assertIn('CONTROL_RADIUS', UI)
        self.assertNotIn('ctx.set_style', UI)
        self.assertNotIn('ctx.set_visuals', UI)
    def test_field_behavior_and_main_canvas_are_retained(self):
        self.assertIn('egui::DragValue', ED)
        self.assertIn('.show_ui(ui,', UI)
        self.assertNotIn('egui::Window', ED)
        self.assertIn('self.draw_canvas(', ED)
        self.assertIn('NOTICE_HEIGHT: f32 = 40.0', UI)
        self.assertIn('HEIGHT: f32 = 32.0', UI)

if __name__ == '__main__': unittest.main()

"""Source wiring checks; native screenshots and Rust value tests are separate."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
APP = (ROOT / 'crates/iv-viewer/src/app.rs').read_text('utf-8')


class PixelReadoutWiringTests(unittest.TestCase):
    def test_setting_load_and_save_use_the_same_key(self):
        self.assertIn('get_string(PixelFormat::STORAGE_KEY)', APP)
        self.assertIn('storage.set_string(PixelFormat::STORAGE_KEY, self.pixel_format.key().into());', APP)
        self.assertIn('"颜色显示格式"', APP)
        self.assertIn('for format in PixelFormat::ALL', APP)

    def test_bottom_formats_live_raw_values_and_channel_not_a_cached_hex_string(self):
        bottom = APP[APP.index('fn draw_bottom_overlay('):APP.index('fn draw_ctx_menu_overlay(')]
        self.assertIn('self.pixel_format.text(position, rgba, self.channel)', bottom)
        self.assertIn('self.probe_position.zip(self.probe_color)', bottom)
        self.assertIn('pixel_readout::swatch(rgba, self.channel)', bottom)
        self.assertNotIn('self.probe_text.clone()', bottom)
        self.assertIn('self.pixel_format.width_sample(dimensions, self.channel)', bottom)
        self.assertIn('self.current_mip()', bottom)
        self.assertIn('with_clip_rect(slot)', bottom)

    def test_sampling_precedes_overlay_and_keeps_current_frame_and_mip(self):
        update = APP[APP.index('impl eframe::App for App'):]
        self.assertLess(update.index('self.update_probe(ctx, canvas_hover)'), update.index('self.draw_bottom_overlay'))
        sample = APP[APP.index('fn update_probe('):APP.index('fn draw_bottom_overlay(')]
        self.assertIn('cur.img.frames.get(self.frame_index)', sample)
        self.assertIn('cur.img.mips.get(self.mip_index)', sample)
        self.assertIn('rgba_f32_at', sample)
        self.assertIn('self.probe_position = Some((x, y));', sample)
        self.assertEqual(sample.count('self.probe_position = None;'), 2)

    def test_raw_full_inspector_is_not_replaced_by_grayscale_or_normalized_values(self):
        sample = APP[APP.index('fn update_probe('):APP.index('fn draw_bottom_overlay(')]
        self.assertIn('self.probe_color = Some([r, g, b, a]);', sample)
        self.assertIn('PixelFormat::Hex.text((x, y), [r, g, b, a], ChannelMode::Rgb)', sample)
        self.assertIn('probe_detail = format!', sample)
        self.assertNotIn('pixel_readout::swatch', sample)


if __name__ == '__main__':
    unittest.main()

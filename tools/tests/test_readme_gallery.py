"""Read-only checks for the user-facing feature gallery and published media links."""
from pathlib import Path
import hashlib, json, re, unittest
from PIL import Image

ROOT=Path(__file__).resolve().parents[2]


class ReadmeGalleryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.readme=(ROOT/'README.md').read_text('utf-8')
        cls.media=ROOT/'docs/screenshots/features/v0.5.0'
        cls.manifest=json.loads((cls.media/'manifest.json').read_text('utf-8'))

    def test_each_major_feature_has_a_corresponding_illustration(self):
        section=self.readme.split('## 功能图解\n',1)[1].split('\n## 支持格式',1)[0]
        blocks=re.split(r'(?m)^### ',section)[1:]
        self.assertEqual(len(blocks),13)
        for block in blocks:
            self.assertRegex(block,r'!\[[^\]]+\]\((?:docs/screenshots/features/v0\.5\.0/[^)]+|docs/ui-qa/(?:editor|ocr)-preview\.jpg)\)')

    def test_readme_images_exist_and_have_descriptive_alt_text(self):
        sources=re.findall(r'!\[([^\]]*)\]\(([^)]+)\)',self.readme)
        for alt,source in sources:
            if source.startswith('http'):continue
            self.assertGreater(len(alt),5)
            path=ROOT/source;self.assertTrue(path.is_file(),source)
            with Image.open(path) as image:image.verify()

    def test_animation_is_a_real_multiframe_file_with_verified_actions(self):
        path=self.media/'animation-controls.gif'
        with Image.open(path) as image:
            self.assertGreaterEqual(image.n_frames,40)
            duration=0
            for i in range(image.n_frames):image.seek(i);duration+=image.info.get('duration',0)
            self.assertGreaterEqual(duration,6000)
        self.assertTrue(self.manifest['illustrations'][path.name]['recorded_play_pause_step_resume'])

    def test_screenshots_have_provenance_and_compact_payloads(self):
        self.assertEqual(self.manifest['version'],'0.5.0')
        self.assertRegex(self.manifest['capture_source_commit'],r'^[0-9a-f]{40}$')
        files=self.manifest['illustrations'];self.assertEqual(len(files),11)
        self.assertLess(sum((self.media/n).stat().st_size for n in files),3*1024*1024)
        for name,record in files.items():
            self.assertTrue(record['sources'],name)
            self.assertEqual((self.media/name).stat().st_size,record['bytes'])

    def test_readme_does_not_leak_machine_paths_or_temporal_test_logs(self):
        self.assertNotRegex(self.readme,r'[CEH]:\\(?:Users|LiChangLong|Work)')
        self.assertNotIn('ui-verify-shots/',self.readme)
        self.assertNotIn('target/',self.readme)

    def test_editor_illustration_has_current_native_provenance(self):
        p=ROOT/'docs/ui-qa/editor-preview.jpg'
        meta=json.loads(p.with_suffix('.json').read_text('utf-8'))
        self.assertRegex(meta['capture_source_commit'],r'^[0-9a-f]{40}$')
        self.assertEqual(len(meta['sources']),2)
        self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(),meta['sha256'])
        self.assertLess(p.stat().st_size,400000)
        self.assertEqual(p.stat().st_size,meta['bytes'])

    def test_ocr_illustration_comes_from_the_native_recognition_run(self):
        p=ROOT/'docs/ui-qa/ocr-preview.jpg'
        meta=json.loads(p.with_suffix('.json').read_text('utf-8'))
        self.assertRegex(meta['capture_source_commit'],r'^[0-9a-f]{40}$')
        self.assertTrue(meta['real_ocr_and_clipboard_verified'])
        self.assertEqual(hashlib.sha256(p.read_bytes()).hexdigest(),meta['sha256'])
        self.assertLess(p.stat().st_size,400000)

    def test_channel_and_animation_instructions_remain_present(self):
        for term in ['R、G、B、Alpha','忽略透明度','Space','逐帧','拖动帧进度条','图钉','HEX']:
            self.assertIn(term,self.readme)


if __name__=='__main__':unittest.main()

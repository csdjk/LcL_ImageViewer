"""Read-only checks for platform boundaries and macOS packaging intent."""
from pathlib import Path
import re, tomllib, unittest

ROOT=Path(__file__).resolve().parents[2]


class MacPackagingTests(unittest.TestCase):
    def test_winreg_and_win32_dependencies_are_windows_only(self):
        data=tomllib.loads((ROOT/'crates/iv-viewer/Cargo.toml').read_text('utf-8'))
        for name in ['winreg','windows','windows-core']:
            self.assertNotIn(name,data['dependencies'])
            self.assertIn(name,data['target']['cfg(windows)']['dependencies'])

    def test_mac_deletion_has_native_trash_and_no_unlink_fallback(self):
        source=(ROOT/'crates/iv-viewer/src/macos_recycle.rs').read_text('utf-8').split('#[cfg(test)]')[0]
        self.assertIn('trashItemAtURL',source)
        self.assertIn('verify_unchanged',source)
        self.assertNotIn('remove_file',source)
        self.assertNotIn('remove_dir',source)

    def test_workflow_permissions_are_read_only_and_no_release_upload(self):
        source=(ROOT/'.github/workflows/macos-package.yml').read_text('utf-8')
        for value in ['contents: read','aarch64-apple-darwin','x86_64-apple-darwin','retention-days: 30']:
            self.assertIn(value,source)
        self.assertNotIn('contents: write',source)
        self.assertNotIn('secrets.',source)
        self.assertNotIn('gh release',source)
        self.assertNotIn('pull_request_target',source)

    def test_installer_contains_notices_and_explicit_signing_limit(self):
        source=(ROOT/'tools/macos/package.py').read_text('utf-8')
        for value in ['AVIF-third-party-notices.txt', 'LSMinimumSystemVersion', 'ad-hoc', "'notarized':False", "'/Applications'", "'hdiutil','verify'"]:
            self.assertIn(value,source)
        self.assertNotIn('spctl --master-disable',source)
        self.assertNotIn('xattr -cr',source)

    def test_core_decoding_is_not_disabled_on_mac(self):
        source=(ROOT/'crates/iv-core/Cargo.toml').read_text('utf-8')
        self.assertIn('codec-aom',source)
        self.assertNotIn('cfg(windows)',source)


if __name__=='__main__':unittest.main(verbosity=2)

"""Read-only checks for installer destination policy."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DefaultDirectoryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = (ROOT / 'tools/setup.iss').read_text(encoding='utf-8-sig')
        cls.setup = cls.source.split('[Setup]', 1)[1].split('\n[', 1)[0]
        cls.code = cls.source.split('[Code]', 1)[1]

    def test_directory_is_resolved_at_install_time(self):
        self.assertRegex(self.setup, r'(?m)^DefaultDirName=\{code:GetDefaultInstallDir\}$')
        self.assertIn("DirExists('D:\\')", self.code)

    def test_d_drive_default_and_user_directory_fallback(self):
        self.assertIn("Result := 'D:\\Program Files\\{#MyAppName}'", self.code)
        self.assertIn("ExpandConstant('{localappdata}\\Programs\\{#MyAppName}')", self.code)
        self.assertIn('Result := UserDir;', self.code)

    def test_upgrades_keep_the_previous_location(self):
        self.assertRegex(self.setup, r'(?m)^UsePreviousAppDir=yes$')
        self.assertRegex(self.setup, r'(?m)^AppId=\{\{B8F2C1A0-3E4D-4F5A-9B6C-7D8E9F0A1B2C\}$')

    def test_destination_remains_editable(self):
        self.assertRegex(self.setup, r'(?m)^DisableDirPage=no$')
        self.assertNotIn('DirEdit.Text :=', self.code)

    def test_selection_has_no_installation_or_privilege_side_effects(self):
        self.assertRegex(self.setup, r'(?m)^PrivilegesRequired=lowest$')
        self.assertIsNone(re.search(r'(?i)\b(CreateDir|ForceDirectories|RegWrite\w*|Exec|SetACL)\s*\(', self.code))


if __name__ == '__main__':
    unittest.main(verbosity=2)

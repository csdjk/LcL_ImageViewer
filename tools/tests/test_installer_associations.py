"""Regression checks for installer registration. No registry writes in these tests."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


def registry_rows(source: str) -> list[dict[str, str]]:
    section = source.split('[Registry]', 1)[1].split('\n[', 1)[0]
    rows = []
    for line in section.splitlines():
        if not line.strip().startswith('Root:'):
            continue
        fields = {}
        for match in re.finditer(r'(\w+):\s*("(?:[^"]|"")*"|[^;]*)(?:;|$)', line):
            name, value = match.groups()
            value = value.strip()
            if value.startswith('"'):
                value = value[1:-1].replace('""', '"')
            fields[name] = value
        if not {'Root', 'Subkey'} <= fields.keys():
            raise ValueError(f'Unsupported registry syntax: {line}')
        rows.append(fields)
    return rows


class InstallerAssociationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.source = (ROOT / 'tools/setup.iss').read_text(encoding='utf-8')
        cls.rows = registry_rows(cls.source)

    def test_all_values_have_an_explicit_registry_type(self):
        self.assertEqual(len(self.rows), 71)
        for row in self.rows:
            self.assertEqual(row.get('ValueType'), 'string', row)

    def test_explorer_refresh_is_enabled(self):
        self.assertRegex(self.source, r'(?m)^ChangesAssociations=yes$')

    def test_supported_formats_match_runtime_registration(self):
        runtime = (ROOT / 'crates/iv-viewer/src/winassoc.rs').read_text(encoding='utf-8')
        extensions = {'.' + x for x in re.findall(r'"([a-z0-9]+)"', runtime.split('pub const EXTS:', 1)[1].split('];', 1)[0])}
        supported = {r['ValueName'] for r in self.rows if r['Subkey'].endswith('\\SupportedTypes')}
        associated = {r['Subkey'].split('\\')[2] for r in self.rows if r['Subkey'].endswith('\\OpenWithProgids')}
        capabilities = {r['ValueName'] for r in self.rows if r['Subkey'].endswith('\\FileAssociations')}
        self.assertEqual(supported, extensions)
        self.assertEqual(associated, extensions)
        self.assertEqual(capabilities, extensions)

    def test_open_commands_preserve_spaces_and_file_argument(self):
        commands = [r for r in self.rows if r['Subkey'].endswith('\\shell\\open\\command')]
        self.assertEqual(len(commands), 2)
        for row in commands:
            self.assertEqual(row['ValueData'], '"{app}\\{#MyAppExe}" "%1"')
            self.assertEqual(row.get('Tasks'), 'assoc')

    def test_shared_association_keys_only_remove_our_values(self):
        for row in self.rows:
            if row['Subkey'].endswith('\\OpenWithProgids') or row['Subkey'] == 'Software\\RegisteredApplications':
                self.assertEqual(row.get('Flags'), 'uninsdeletevalue')
                self.assertIn(row['ValueName'], ('{#MyProgId}', '{#MyAppName}'))

    def test_no_default_app_or_userchoice_write(self):
        for row in self.rows:
            self.assertEqual(row['Root'], 'HKCU')
            self.assertNotIn('userchoice', row['Subkey'].lower())
            self.assertFalse(re.fullmatch(r'Software\\Classes\\\.[^\\]+', row['Subkey']))
            self.assertNotIn('deletekey', row.get('Flags', '').split())
            self.assertIn(row.get('Tasks'), ('assoc', 'thumbs'))


if __name__ == '__main__':
    unittest.main(verbosity=2)

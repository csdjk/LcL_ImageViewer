"""Read-only packaging regressions: payload allowlist, versions and PE runtime imports."""
from pathlib import Path
import importlib.util
import struct
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('package_release', ROOT/'tools/package_release.py')
package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(package)


def fixture_pe(imports: list[str]) -> bytes:
    data = bytearray(4096)
    data[:2] = b'MZ'
    struct.pack_into('<I', data, 60, 0x80)
    data[0x80:0x84] = b'PE\0\0'
    struct.pack_into('<HH', data, 0x84, 0x8664, 1)
    struct.pack_into('<H', data, 0x94, 240)
    optional = 0x98
    struct.pack_into('<H', data, optional, 0x20b)
    struct.pack_into('<Q', data, optional+24, 0x140000000)
    struct.pack_into('<II', data, optional+120, 0x1000, 20*(len(imports)+1))
    struct.pack_into('<IIII', data, optional+240+8, 0x800, 0x1000, 0x800, 0x200)
    for i, name in enumerate(imports):
        name_offset = 0x400+i*64
        encoded = name.encode('ascii')+b'\0'
        data[name_offset:name_offset+len(encoded)] = encoded
        struct.pack_into('<IIIII', data, 0x200+i*20, 0, 0, 0, name_offset+0xe00, 1)
    return bytes(data)


class ReleasePackagingTests(unittest.TestCase):
    def test_portable_payload_has_native_notices_and_only_expected_files(self):
        sources = package.portable_sources(ROOT, Path('compiled'))
        self.assertEqual(set(sources), {'imageview.exe', 'iv_shell.dll', 'LICENSE', 'CHANGELOG.md',
                         'register_thumbnail.ps1', 'unregister_thumbnail.ps1',
                         'licenses/AVIF-third-party-notices.txt'})
        notice = sources['licenses/AVIF-third-party-notices.txt'].read_text(encoding='utf-8')
        self.assertIn('Alliance for Open Media', notice)
        self.assertIn('PATENTS', notice)

    def test_workspace_lock_and_installer_versions_match(self):
        version = tomllib.loads((ROOT/'Cargo.toml').read_text('utf-8'))['workspace']['package']['version']
        lock = tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))
        for crate in lock['package']:
            if crate['name'] in ['iv-core','iv-shell','iv-viewer']:
                self.assertEqual(crate['version'], version)
        self.assertIn(f'#define MyAppVersion "{version}"', (ROOT/'tools/setup.iss').read_text('utf-8'))
        self.assertTrue((ROOT/'docs/releases'/f'v{version}.md').is_file())

    def test_windows_api_imports_are_allowed(self):
        with tempfile.TemporaryDirectory() as folder:
            p = Path(folder)/'sample.exe'; p.write_bytes(fixture_pe(['KERNEL32.dll','USER32.dll']))
            self.assertEqual(package.assert_portable_runtime(p), ['KERNEL32.dll','USER32.dll'])

    def test_external_crt_and_codec_imports_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            p = Path(folder)/'sample.exe'
            for name in ['VCRUNTIME140.dll','MSVCP140.dll','ucrtbase.dll','avif.dll','libaom.dll','dav1d.dll','api-ms-win-crt-runtime-l1-1-0.dll']:
                p.write_bytes(fixture_pe(['KERNEL32.dll',name]))
                with self.assertRaises(RuntimeError, msg=name):
                    package.assert_portable_runtime(p)

    def test_invalid_or_non_x64_pe_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            p = Path(folder)/'sample.exe'
            bad_arch = bytearray(fixture_pe(['KERNEL32.dll']))
            struct.pack_into('<H', bad_arch, 0x84, 0x14c)
            for content in [b'', b'MZ', bytes(bad_arch), fixture_pe(['KERNEL32.dll'])[:256]]:
                p.write_bytes(content)
                with self.assertRaises(ValueError):
                    package.pe_imports(p)


if __name__ == '__main__':
    unittest.main(verbosity=2)

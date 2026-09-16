"""Exercise real Inno registry writes in an inert, unique HKCU test namespace.

All source registry keys are prefixed with Software\\LcL\\InstallerQA\\<UUID>.
No live Software\\Classes, default associations, application installs, shortcuts,
COM registration, or Explorer settings are modified. Optional EXE/DLL payload is
installed only below the new output directory, hash-checked and uninstalled. The generated test installers
have no Run/Icons entries and do not register an uninstall entry. Logs are retained.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import uuid
import tomllib
import winreg as reg
from test_installer_associations import ROOT, registry_rows

DEFINES = {'MyAppName': 'LcL ImageViewer', 'MyProgId': 'LcL.ImageViewer.Image',
           'MyAppExe': 'imageview.exe', 'ThumbClsid': '7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B'}
ACCESS = reg.KEY_READ | reg.KEY_WOW64_64KEY


def tree(key):
    try:
        with reg.OpenKey(reg.HKEY_CURRENT_USER, key, 0, ACCESS) as handle:
            children, count, _ = reg.QueryInfoKey(handle)
            values = []
            for i in range(count):
                name, data, kind = reg.EnumValue(handle, i)
                values.append((name, data.hex() if isinstance(data, bytes) else data, kind))
            return {'values': sorted(values), 'children': {
                reg.EnumKey(handle, i): tree(key + '\\' + reg.EnumKey(handle, i)) for i in range(children)}}
    except FileNotFoundError:
        return None


def read(key, name=''):
    try:
        with reg.OpenKey(reg.HKEY_CURRENT_USER, key, 0, ACCESS) as handle:
            return reg.QueryValueEx(handle, name)
    except FileNotFoundError:
        return None


def write_test(key, name, data):
    assert key.startswith('Software\\LcL\\InstallerQA\\')
    with reg.CreateKeyEx(reg.HKEY_CURRENT_USER, key, 0, reg.KEY_WRITE | reg.KEY_WOW64_64KEY) as h:
        reg.SetValueEx(h, name, 0, reg.REG_SZ, data)


def expand(value, install):
    for name, data in DEFINES.items():
        value = value.replace('{#' + name + '}', data)
    return value.replace('{app}', str(install)).replace('{{', '{')


def run(args):
    fixed = (ROOT / 'tools/setup.iss').read_text(encoding='utf-8')
    assert re.fullmatch(r'[0-9a-f]{7,40}', args.baseline)
    old = subprocess.check_output(['git', 'show', args.baseline + ':tools/setup.iss'],
                                   cwd=ROOT, text=True, encoding='utf-8')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    prefix = 'Software\\LcL\\InstallerQA\\' + uuid.uuid4().hex
    install = out / '安装 path & spaces'
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    payload = {}
    if args.payload_dir:
        for filename in ('imageview.exe', 'iv_shell.dll'):
            path = args.payload_dir.resolve() / filename
            if not path.is_file():
                raise FileNotFoundError(path)
            payload[filename] = hashlib.sha256(path.read_bytes()).hexdigest()
    rows = registry_rows(fixed)
    assert len(rows) == 72 and all(row['Root'] == 'HKCU' for row in rows)
    actual_keys = sorted({expand(row['Subkey'], install) for row in rows})
    actual_keys += ['Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\FileExts\\' + ext + '\\UserChoice'
                    for ext in ('.png', '.jpg', '.dds', '.tga')]
    actual_keys += ['Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{B8F2C1A0-3E4D-4F5A-9B6C-7D8E9F0A1B2C}_is1']
    before = {key: tree(key) for key in actual_keys}
    fingerprint = hashlib.sha256(json.dumps(before, sort_keys=True).encode()).hexdigest()
    command_key = prefix + '\\Software\\Classes\\Applications\\imageview.exe\\shell\\open\\command'
    old_command = '"Z:\\Old Viewer\\imageview.exe" "%1"'
    write_test(command_key, '', old_command)
    # Sentinels prove shared OpenWithProgids and RegisteredApplications survive uninstall.
    for suffix in ('Software\\Classes\\.png\\OpenWithProgids', 'Software\\RegisteredApplications'):
        write_test(prefix + '\\' + suffix, 'OtherApp.Sentinel', 'preserve')

    def compile_test(source, name):
        definitions = source.split('[Setup]', 1)[0]
        registry = source.split('[Registry]', 1)[1].split('\n[', 1)[0]
        registry = registry.replace('Subkey: "', 'Subkey: "' + prefix + '\\')
        transformed = registry_rows('[Registry]\n' + registry)
        assert len(transformed) == (72 if name == 'corrected' else 71)
        assert all(row['Root'] == 'HKCU' and row['Subkey'].startswith(prefix + '\\') for row in transformed)
        script = definitions + f'''
[Setup]
AppId=LcL-Registry-QA-{prefix.rsplit(chr(92), 1)[1]}
AppName=LcL Registry QA (isolated)
AppVersion={version}
DefaultDirName={install}
UsePreviousAppDir=no
UsePreviousTasks=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CreateUninstallRegKey=no
Uninstallable=yes
ChangesAssociations=no
CloseApplications=no
RestartApplications=no
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputDir={out}
OutputBaseFilename={name}
[Tasks]
Name: "assoc"; Description: "Isolated associations"
Name: "thumbs"; Description: "Isolated thumbnail values"
[Registry]
''' + registry
        if payload and name == 'corrected':
            script += '\n[Files]\n'
            for filename in payload:
                source_file = str(args.payload_dir.resolve() / filename)
                assert '"' not in source_file
                script += f'Source: "{source_file}"; DestDir: "{{app}}"; Flags: ignoreversion\n'
        path = out / (name + '.iss')
        path.write_text(script.lstrip('\ufeff'), encoding='utf-8-sig')
        with (out / (name + '-compile.log')).open('wb') as log:
            subprocess.run([str(args.iscc.resolve()), str(path)], stdout=log, stderr=subprocess.STDOUT,
                           check=True, timeout=60)
        return out / (name + '.exe')

    def execute(binary, label, tasks):
        subprocess.run([str(binary), '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/SP-',
                        '/TASKS=' + tasks, '/LOG=' + str(out / (label + '.log'))], check=True, timeout=45)

    try:
        legacy = compile_test(old, 'legacy')
        execute(legacy, 'legacy-install', 'assoc,thumbs')
        assert read(command_key) == (old_command, reg.REG_SZ), 'Old installer unexpectedly updated the command'
        assert tree(prefix + '\\Software\\Classes\\Applications\\imageview.exe\\SupportedTypes')['values'] == []
        assert read(prefix + '\\Software\\Classes\\Applications\\imageview.exe', 'FriendlyAppName') is None
        corrected = compile_test(fixed, 'corrected')
        execute(corrected, 'corrected-upgrade', 'assoc,thumbs')
        for row in rows:
            key = prefix + '\\' + expand(row['Subkey'], install)
            name = expand(row.get('ValueName', ''), install)
            expected = (expand(row.get('ValueData', ''), install), reg.REG_SZ)
            assert read(key, name) == expected, (key, name, read(key, name), expected)
        for filename, digest in payload.items():
            assert hashlib.sha256((install / filename).read_bytes()).hexdigest() == digest, filename
        # Independent Windows contract: previous tests mirrored the malformed source
        # and accepted a double closing brace. Never derive this expected slot from .iss.
        for ext in ('.psd', '.webp'):
            key = prefix + '\\Software\\Classes\\' + ext + '\\shellex\\{E357FCCD-A995-4576-B01F-234630154E96}'
            assert read(key) == ('{7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B}', reg.REG_SZ)
        uninstaller = install / 'unins000.exe'
        subprocess.run([str(uninstaller), '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART',
                        '/LOG=' + str(out / 'uninstall.log')], check=True, timeout=45)
        for suffix in ('Software\\Classes\\.png\\OpenWithProgids', 'Software\\RegisteredApplications'):
            assert read(prefix + '\\' + suffix, 'OtherApp.Sentinel') == ('preserve', reg.REG_SZ)
        assert read(prefix + '\\Software\\Classes\\.png\\OpenWithProgids', 'LcL.ImageViewer.Image') is None
        assert read(prefix + '\\Software\\RegisteredApplications', 'LcL ImageViewer') is None
        assert tree(prefix + '\\Software\\Classes\\Applications\\imageview.exe') is None
        for filename in payload:
            assert not (install / filename).exists(), 'Payload survived uninstall: ' + filename
        report = {'result': 'PASS', 'baseline': args.baseline, 'test_namespace': prefix,
                  'package_version': version, 'payload_install_hashes': payload,
                  'payload_uninstall_verified': bool(payload),
                  'legacy_empty_values_reproduced': True, 'stale_path_upgrade_fixed': True,
                  'verified_typed_values': len(rows), 'shared_value_uninstall_preserves_other_app': True,
                  'real_registry_snapshot_sha256': fingerprint,
                  'limitation': 'Namespace-remapped registry installation, not a live Open With menu test.'}
    finally:
        after = {key: tree(key) for key in actual_keys}
        assert before == after, 'Live registration changed during test; do not claim it stayed untouched'
    report['live_file_associations_unchanged'] = True
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--iscc', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--baseline', required=True)
    p.add_argument('--payload-dir', type=Path, help='Optional built EXE/DLL copied only into the isolated test install')
    run(p.parse_args())

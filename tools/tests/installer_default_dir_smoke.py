"""Run the production Pascal directory resolver in a real Inno executable.

This inert probe has no payload, registry/shortcut/run entries and always stops
in InitializeSetup, before installation. It writes only a report in the supplied
new output directory. No drives, live associations or current installs change.
The missing-D branch is injected into the pure selector; disks are not unmounted.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import uuid

ROOT = Path(__file__).resolve().parents[2]


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = (ROOT / 'tools/setup.iss').read_text(encoding='utf-8-sig')
    code = source.split('[Code]', 1)[1]
    results = out / 'resolver.txt'
    q = lambda p: str(p).replace("'", "''")
    script = '\n'.join([
        '#define MyAppName "LcL ImageViewer"', '[Setup]',
        'AppId=LcL-Destination-QA-' + uuid.uuid4().hex,
        'AppName=LcL Destination Logic Test', 'AppVersion=0.3.0',
        'PrivilegesRequired=lowest', 'DefaultDirName={code:GetDefaultInstallDir}',
        'UsePreviousAppDir=no', 'Uninstallable=no', 'CreateUninstallRegKey=no',
        'CloseApplications=no', 'RestartApplications=no', 'ChangesAssociations=no',
        'OutputDir=' + str(out), 'OutputBaseFilename=directory-resolver-test', '[Code]', code,
        f'''
function InitializeSetup(): Boolean;
var
  Fallback, WithD, WithoutD, Actual: String;
begin
  Fallback := 'C:\\QA fallback\\Programs\\LcL ImageViewer';
  WithD := SelectDefaultInstallDir(True, Fallback);
  WithoutD := SelectDefaultInstallDir(False, Fallback);
  Actual := GetDefaultInstallDir('');
  if WithD <> 'D:\\Program Files\\LcL ImageViewer' then
    RaiseException('D-drive selection failed');
  if WithoutD <> Fallback then
    RaiseException('Missing-drive fallback failed');
  if not SaveStringToFile('{q(results)}',
    'with_d=' + WithD + #13#10 +
    'without_d=' + WithoutD + #13#10 +
    'actual=' + Actual + #13#10 +
    'result=PASS' + #13#10, False) then
    RaiseException('Could not save probe result');
  Result := False; {{ Intentionally stop BEFORE installation. }}
end;
'''])
    path = out / 'directory-resolver-test.iss'
    path.write_text(script, encoding='utf-8-sig')
    with (out / 'compile.log').open('wb') as log:
        subprocess.run([str(args.iscc.resolve()), str(path)], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=60)
    exe = out / 'directory-resolver-test.exe'
    process = subprocess.run([str(exe), '/SP-', '/VERYSILENT', '/SUPPRESSMSGBOXES',
                              '/NORESTART', '/LOG=' + str(out / 'probe.log')], timeout=30, check=False)
    values = dict(line.split('=', 1) for line in results.read_text(encoding='utf-8-sig').splitlines() if '=' in line)
    assert values['result'] == 'PASS'
    expected = r'D:\Program Files\LcL ImageViewer' if Path('D:/').is_dir() else str(Path(os.environ['LOCALAPPDATA']) / 'Programs/LcL ImageViewer')
    assert values['actual'].casefold() == expected.casefold(), (values['actual'], expected)
    report = {'result': 'PASS', 'values': values, 'intentional_exit_code': process.returncode,
              'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_sha256': hashlib.sha256(source.encode('utf-8')).hexdigest(),
              'probe_sha256': hashlib.sha256(exe.read_bytes()).hexdigest(),
              'installation_executed': False, 'live_associations_changed': False,
              'scope': 'Real Inno resolver execution; missing-drive branch injected; no wizard/upgrade installation.'}
    (out / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iscc', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    run(parser.parse_args())

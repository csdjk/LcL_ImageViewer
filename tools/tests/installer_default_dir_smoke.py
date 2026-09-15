"""Preview Inno's real directory page; never install or change live associations.

The probe uses the production directory code/settings with a fresh AppId, no
payload, no registry/shortcut/run entries, and a disabled Next action. Tests
D-present/fallback selection and an explicit /DIR override. Captures are real
wizard windows; only this probe's own windows are closed.
"""
from __future__ import annotations
import argparse
import ctypes as c
from ctypes import wintypes as w
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/ui-qa'))
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = (ROOT / 'tools/setup.iss').read_text(encoding='utf-8-sig')
    setup = source.split('[Setup]', 1)[1].split('\n[', 1)[0]
    directives = dict(line.split('=', 1) for line in setup.splitlines() if '=' in line and not line.startswith(';'))
    code = source.split('[Code]', 1)[1]
    report_path = out / 'wizard.txt'
    uid = uuid.uuid4().hex
    q = lambda p: str(p).replace("'", "''")
    script = '\n'.join([
        '#define MyAppName "LcL ImageViewer"', '[Setup]',
        f'AppId=LcL-Destination-QA-{uid}', f'AppName=LcL Destination Preview {uid[:8]}',
        'AppVersion=0.3.0', 'PrivilegesRequired=lowest',
        *[key + '=' + directives[key] for key in ('DefaultDirName', 'UsePreviousAppDir', 'DisableDirPage')],
        'Uninstallable=no', 'CreateUninstallRegKey=no', 'CloseApplications=no',
        'RestartApplications=no', 'ChangesAssociations=no', 'DisableWelcomePage=yes',
        'DisableProgramGroupPage=yes', 'ArchitecturesAllowed=x64compatible',
        'ArchitecturesInstallIn64BitMode=x64compatible', 'WizardStyle=modern',
        'OutputDir=' + str(out), 'OutputBaseFilename=directory-preview', '[Code]', code,
        f'''
procedure InitializeWizard();
var
  Fallback: String;
begin
  Fallback := 'C:\\QA fallback\\Programs\\LcL ImageViewer';
  if SelectDefaultInstallDir(True, Fallback) <> 'D:\\Program Files\\LcL ImageViewer' then
    RaiseException('D-drive selection failed');
  if SelectDefaultInstallDir(False, Fallback) <> Fallback then
    RaiseException('Missing-drive fallback failed');
  if not SaveStringToFile('{q(report_path)}',
    'selected=' + WizardDirValue() + #13#10 +
    'has_d=' + IntToStr(Ord(DirExists('D:\\'))) + #13#10 +
    'hwnd=' + IntToStr(WizardForm.Handle) + #13#10 +
    'branch_checks=PASS' + #13#10, False) then
    RaiseException('Could not save probe result');
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := False; {{ Preview only: installation cannot be reached. }}
end;

procedure CancelButtonClick(CurPageID: Integer; var Cancel, Confirm: Boolean);
begin
  Cancel := True;
  Confirm := False;
end;
'''])
    path = out / 'directory-preview.iss'
    path.write_text(script, encoding='utf-8-sig')
    with (out / 'compile.log').open('wb') as log:
        subprocess.run([str(args.iscc.resolve()), str(path)], stdout=log, stderr=subprocess.STDOUT, check=True, timeout=90)
    binary = out / 'directory-preview.exe'
    report = {'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
              'source_sha256': hashlib.sha256(source.encode('utf-8')).hexdigest(),
              'probe_sha256': qa.sha(binary), 'cases': [], 'installation_executed': False,
              'live_associations_changed': False}
    for name, manual in [('fresh', None), ('manual', out / 'manually chosen folder')]:
        if report_path.exists():
            report_path.unlink()  # Only this newly-created probe's report.
        cmd = [str(binary), '/SP-', '/NORESTART', '/LOG=' + str(out / (name + '.log'))]
        if manual:
            cmd.append('/DIR=' + str(manual))
        proc = subprocess.Popen(cmd)
        hwnd = None
        child_pid = w.DWORD()
        try:
            deadline = time.monotonic() + 20
            while not report_path.exists():
                if time.monotonic() >= deadline or proc.poll() is not None:
                    raise RuntimeError('Preview did not initialize; inspect its log')
                time.sleep(0.1)
            values = dict(line.split('=', 1) for line in report_path.read_text(encoding='utf-8-sig').splitlines() if '=' in line)
            hwnd = int(values['hwnd'])
            qa.u.GetWindowThreadProcessId(hwnd, c.byref(child_pid))
            time.sleep(0.8)
            wanted = str(manual) if manual else (r'D:\Program Files\LcL ImageViewer' if Path('D:/').is_dir() else str(Path(os.environ['LOCALAPPDATA']) / 'Programs/LcL ImageViewer'))
            assert values['selected'].casefold() == wanted.casefold(), (name, values['selected'], wanted)
            assert values['branch_checks'] == 'PASS'
            meta = qa.capture(hwnd, out / (name + '.png'))
            meta.update({'source_commit': report['source_commit'], 'state': name,
                         'selected_directory': values['selected'], 'probe_sha256': report['probe_sha256'],
                         'capture_target': 'WizardForm.Handle reported by newly launched inert probe'})
            (out / (name + '.json')).write_text(json.dumps(meta, indent=2), encoding='utf-8')
            report['cases'].append({'case': name, 'selected': values['selected'], 'result': 'PASS'})
        finally:
            if hwnd:
                qa.u.PostMessageW(hwnd, 0x0010, 0, 0)
            try:
                proc.wait(timeout=8)
            except subprocess.TimeoutExpired:
                if child_pid.value:
                    os.kill(child_pid.value, signal.SIGTERM)
                proc.terminate()
                proc.wait(timeout=5)
    report['result'] = 'PASS'
    (out / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iscc', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    run(parser.parse_args())

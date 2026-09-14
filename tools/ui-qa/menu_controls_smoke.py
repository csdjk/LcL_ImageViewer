"""Real Windows menu/key/recycle regression. Only recycles this run's NEW fixture images.

Requires Python 3.10+, Pillow, Windows Explorer and a binary supporting isolated QA profiles.
Never selects/deletes a user image; never empties the recycle bin or closes existing viewers.
Recycled fixtures are left recoverable in the Windows recycle bin as evidence.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import neumorphic_smoke as qa


def recycle_evidence(folder: Path) -> list[dict]:
    env = dict(os.environ, LCL_QA_RECYCLE_FROM=str(folder.resolve()))
    script = r'''
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$ErrorActionPreference='Stop'
$shell = New-Object -ComObject Shell.Application
$matches = @()
foreach($item in $shell.Namespace(10).Items()) {
    $from = [string]$item.ExtendedProperty('System.Recycle.DeletedFrom')
    if($from.TrimEnd('\') -ieq $env:LCL_QA_RECYCLE_FROM.TrimEnd('\')) {
        $matches += [pscustomobject]@{
            name=[string]$item.Name
            original_directory=$from
            sha256=[BitConverter]::ToString([Security.Cryptography.SHA256]::Create().ComputeHash([IO.File]::ReadAllBytes([string]$item.Path))).Replace('-','').ToLowerInvariant()
        }
    }
}
ConvertTo-Json -InputObject @($matches) -Depth 4 -Compress
'''
    result = subprocess.run(['powershell', '-NoProfile', '-Command', script],
                            env=env, capture_output=True, encoding='utf-8', check=True)
    return json.loads(result.stdout)


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    subprocess.run([sys.executable, '-B', str(Path(__file__).with_name('generate_navigation_fixtures.py')),
                    str(fixtures)], check=True)
    files = sorted(fixtures.glob('*.png'))
    assert [p.name for p in files] == ['01_first.png', '02_middle.png', '03_last.png']
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    actions = []
    expected = {}
    file_counts = {}

    def key(code, modifiers=()):
        actions.append({'kind': 'key', 'code': code, 'modifiers': list(modifiers)})
    def click(x, y):
        actions.append({'kind': 'click', 'x': x, 'y': y})
    def shot(name, title='02_middle.png', count=3):
        actions.append({'kind': 'wait', 'seconds': 0.25})
        actions.append({'kind': 'shot', 'name': name})
        expected[name] = title
        file_counts[name] = count
    def menu():
        actions.append({'kind': 'right-click', 'x': 300, 'y': 100})
    def confirm():
        click(args.width / 2 + 85, args.height / 2 + 64)
        actions.append({'kind': 'wait', 'seconds': 1.2})
        actions.append({'kind': 'move', 'x': args.width / 2, 'y': args.height / 2})

    key('D'); shot('d-next', '03_last.png')
    key('D'); shot('last-boundary', '03_last.png')
    key('A'); shot('a-back')
    key('A'); shot('a-first', '01_first.png')
    key('A'); shot('first-boundary', '01_first.png')
    key('D'); shot('middle')
    key('A', ['ctrl']); key('D', ['ctrl']); key(46, ['shift']); shot('modified-keys-ignored')
    menu(); shot('compact-menu')
    key('D'); shot('menu-blocks-navigation'); key(27)
    menu(); click(360, 318); shot('properties')
    key('D'); key(46); shot('properties-block-keys'); key(27)
    menu(); click(360, 435); shot('settings')
    key('A'); key(46); shot('settings-block-keys'); key(27)
    key(46); shot('delete-confirm')
    key('D'); key(46); shot('confirm-blocks-other-keys')
    click(args.width / 2 - 110, args.height / 2 + 64); shot('cancel-preserves-files')
    key(46); shot('delete-middle-confirm'); confirm(); shot('deleted-middle', '03_last.png', 2)
    key(46); shot('delete-last-confirm', '03_last.png', 2); key(27); shot('escape-cancels', '03_last.png', 2)
    key(46); confirm(); shot('deleted-last-fallback', '01_first.png', 1)
    menu(); click(360, 245); shot('menu-delete-confirm', '01_first.png', 1)
    confirm(); shot('empty-after-delete', 'LcL ImageViewer', 0)
    key('A'); key('D'); key(46); shot('empty-keys-noop', 'LcL ImageViewer', 0)

    action_file = out / 'actions.json'
    action_file.write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
    snapshots = {}
    capture = qa.capture
    def checked_capture(hwnd, destination):
        name = destination.stem
        remaining = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p.exists()}
        assert all(hashes[n] == h for n, h in remaining.items()), 'A fixture was modified rather than recycled'
        snapshots[name] = list(remaining)
        if name in file_counts:
            assert len(remaining) == file_counts[name], (name, remaining)
        return capture(hwnd, destination)
    qa.capture = checked_capture
    try:
        qa.run(SimpleNamespace(binary=args.binary, output=out / 'viewer', input=files[1],
                               commit=args.commit, theme=args.theme, width=args.width, height=args.height,
                               actions=action_file, isolated_profile=True))
    finally:
        qa.capture = capture
        (out / 'file-snapshots.json').write_text(json.dumps(snapshots, indent=2), encoding='utf-8')
    viewer_run = json.loads((out / 'viewer/run.json').read_text(encoding='utf-8'))
    for name, title in expected.items():
        meta = json.loads((out / f'viewer/{name}.json').read_text(encoding='utf-8'))
        assert meta['window_title'].startswith(title), (name, title, meta['window_title'])
    assert viewer_run['isolated_profile'] and viewer_run['preferences_restored']
    recycled = recycle_evidence(fixtures)
    assert len(recycled) == 3 and sorted(i['sha256'] for i in recycled) == sorted(hashes.values()), recycled
    report = {'result': 'PASS', 'fixture_directory': str(fixtures), 'fixture_hashes': hashes,
              'recycled_fixtures': recycled, 'captures': len(viewer_run['captures']),
              'commit': args.commit, 'binary_sha256': viewer_run['binary_sha256'],
              'checks': ['A/D navigation and boundaries', 'Ctrl/Shift combinations ignored',
                         'menu/properties/settings/confirmation isolation', 'cancel and Escape preserve files',
                         'middle deletion advances', 'last deletion falls back', 'single image deletion clears view',
                         'all 3 original byte hashes found in Recycle Bin'],
              'scope': 'only newly generated fixture PNGs; all user images/viewers untouched'}
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=['light', 'dark'], default='light')
    parser.add_argument('--width', type=int, default=880)
    parser.add_argument('--height', type=int, default=560)
    run(parser.parse_args())

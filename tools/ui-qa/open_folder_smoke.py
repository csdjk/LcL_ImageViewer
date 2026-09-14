"""Verify the real context menu opens the current image's parent in Explorer.

Requires Windows, Python 3.10+ and Pillow. Uses a fresh QA viewer profile and a
new fixture directory. Reads only Explorer window IDs and the exact test folder;
closes only new Explorer windows still showing that folder, never existing ones.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import time

import neumorphic_smoke as qa


def explorer_state(folder: Path, close_ids: set[int] | None = None) -> dict:
    env = dict(os.environ)
    env['LCL_IV_QA_MATCH_FOLDER'] = str(folder.resolve())
    env['LCL_IV_QA_CLOSE_IDS'] = ','.join(str(n) for n in (close_ids or set()))
    script = r'''
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$ErrorActionPreference = 'Stop'
$target = $env:LCL_IV_QA_MATCH_FOLDER
$closeIds = @($env:LCL_IV_QA_CLOSE_IDS -split ',' | Where-Object { $_ })
$shell = New-Object -ComObject Shell.Application
$ids = @(); $found = @()
foreach ($window in @($shell.Windows())) {
    try {
        if ([System.IO.Path]::GetFileName([string]$window.FullName) -ine 'explorer.exe') { continue }
        $id = [long]$window.HWND
        $ids += $id
        $path = [string]$window.Document.Folder.Self.Path
        if ($path.TrimEnd('\') -ieq $target.TrimEnd('\')) {
            $found += @{ hwnd = $id; directory = $path; location_url = [string]$window.LocationURL }
            if ($closeIds -contains [string]$id) { $window.Quit() }
        }
    } catch { }
}
@{ ids = @($ids); matching = @($found) } | ConvertTo-Json -Depth 4 -Compress
'''
    result = subprocess.run(
        ['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', script],
        env=env, capture_output=True, encoding='utf-8', check=True, timeout=15,
    )
    return json.loads(result.stdout)


def run(args: argparse.Namespace) -> None:
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    fixture = root / 'fixtures' / '中文 素材, & (验证)'
    fixture.mkdir(parents=True)
    image = fixture / '图片 - 副本.png'
    image.write_bytes(args.source.read_bytes())
    actions = root / 'actions.json'
    actions.write_text(json.dumps([
        {'kind': 'right-click', 'x': 300, 'y': 100},
        {'kind': 'shot', 'name': 'folder-menu'},
        {'kind': 'move', 'x': 390, 'y': 205},
        {'kind': 'shot', 'name': 'hover-folder-menu'},
        {'kind': 'click', 'x': 390, 'y': 205},
        {'kind': 'wait', 'seconds': 0.5},
        {'kind': 'shot', 'name': 'after-open-folder'},
    ]), encoding='utf-8')
    before = explorer_state(fixture)
    assert not before['matching'], 'Fresh fixture directory unexpectedly already open'
    original_ids = set(before['ids'])
    result = {'expected_directory': str(fixture), 'input': str(image), 'result': 'FAILED'}
    try:
        qa.run(argparse.Namespace(
            binary=args.binary, output=root / 'viewer', input=image,
            commit=args.commit, theme=args.theme, width=args.width, height=args.height,
            isolated_profile=True, actions=actions,
        ))
        deadline = time.monotonic() + 10.0
        while True:
            state = explorer_state(fixture)
            if state['matching']:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError('Menu did not open the exact current-image directory in Explorer')
            time.sleep(0.3)
        result['explorer'] = state['matching']
        new_windows = [w for w in state['matching'] if w['hwnd'] not in original_ids]
        if new_windows:
            result['explorer_capture'] = qa.capture(new_windows[0]['hwnd'], root / 'explorer.png')
        else:
            result['explorer_capture'] = 'Existing Explorer window reused; left unchanged'
        result['viewer_run'] = json.loads((root / 'viewer/run.json').read_text(encoding='utf-8'))
        assert result['viewer_run']['preferences_restored']
        result['result'] = 'PASS'
        print(json.dumps(result, ensure_ascii=False, indent=2), flush=True)
    finally:
        state = explorer_state(fixture)
        new_ids = {w['hwnd'] for w in state['matching']} - original_ids
        if new_ids:
            explorer_state(fixture, new_ids)
        (root / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--source', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=('light', 'dark'), default='light')
    parser.add_argument('--width', type=int, default=1280)
    parser.add_argument('--height', type=int, default=860)
    run(parser.parse_args())

"""Read-only pixel/geometry assertions for mouse-gestures-*.json desktop runs."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops, ImageStat


def check_run(folder: Path) -> dict:
    run = json.loads((folder / 'run.json').read_text(encoding='utf-8'))
    assert run['isolated_profile'] and run['preferences_restored']
    names = [Path(n).stem for n in run['captures']]
    meta = {n: json.loads((folder / f'{n}.json').read_text(encoding='utf-8')) for n in names}
    images = {n: Image.open(folder / f'{n}.png').convert('RGB') for n in names}
    w, h = images['baseline'].size
    assert meta['baseline']['dpi'] == 96, 'Pixel checks currently cover DPI96 only'
    roi = (260, 130, w - 190, h - 130)
    def diff(a, b, ra=roi, rb=roi):
        return max(ImageStat.Stat(ImageChops.difference(images[a].crop(ra), images[b].crop(rb))).mean)
    # Left pan must translate image content by the entire input delta, without moving the HWND.
    shifted = (roi[0] + 80, roi[1] + 45, roi[2] + 80, roi[3] + 45)
    pan_error = diff('baseline', 'left-pan', roi, shifted)
    assert pan_error < 0.05, ('Left pan did not follow 80x45 pixels', pan_error)
    assert diff('baseline', 'left-pan') > 0.05, 'Left pan had no visible effect'
    assert meta['baseline']['physical_window'] == meta['left-pan']['physical_window']
    # Right drag moves only the window, including an out-and-back gesture ending at the same position.
    for name in ('right-window', 'right-out-and-back', 'menu-closed'):
        assert diff('left-pan', name) == 0, (name, 'Moved image or left a menu open')
    start = meta['left-pan']['physical_window']
    moved = meta['right-window']['physical_window']
    assert [moved[i] - start[i] for i in range(4)] == [72, 36, 72, 36]
    assert moved == meta['right-out-and-back']['physical_window']
    assert diff('right-window', 'right-click-menu') > 1, 'Right click menu did not open'
    assert diff('baseline', 'middle-pan') < 0.05, 'Middle pan compatibility failed'
    assert meta['next-image']['window_title'].startswith('03_last.png'), 'Side navigation broken'
    # Drag actions also assert native resize deltas during the actual desktop run.
    final_actions = meta['restored']['actions']
    drags = [a for a in final_actions if a['kind'] == 'drag']
    assert len(drags) == 6
    for action in drags:
        actual = action['window_delta_physical']
        assert all(abs(a-b) <= 3 for a,b in zip(actual, action['expect_window_delta']))
    bar = (190 if w > 1000 else 140, 14, w - 190 if w > 1000 else w - 140, 55)
    assert diff('resize-restored', 'hidden', bar, bar) > 3, 'Auto-hide failed'
    assert diff('resize-restored', 'restored', bar, bar) < 0.05, 'Auto-hide restore failed'
    return {'run': folder.name, 'result': 'PASS', 'captures': len(names),
            'image_pan_error': pan_error, 'window_drag_physical': [72, 36],
            'right_drag_image_error': diff('left-pan', 'right-window'),
            'commit': run['commit'], 'binary_sha256': run['binary_sha256']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('runs', nargs='+', type=Path)
    args = parser.parse_args()
    print(json.dumps([check_run(p) for p in args.runs], ensure_ascii=False, indent=2))

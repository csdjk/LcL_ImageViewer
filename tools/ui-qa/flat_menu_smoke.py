"""Check the ungrouped context menu using a fresh viewer profile and NEW images.
Only opens/cancels the delete confirmation; never confirms deletion.
Requires Windows, Python 3.10+ and Pillow. Leaves existing viewers untouched.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageDraw
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    fixtures.mkdir()
    for i, name in enumerate(('01_first.png', '02_middle.png', '03_last.png'), 1):
        image = Image.new('RGB', (512, 512), (210, 220, 232))
        draw = ImageDraw.Draw(image)
        for y in range(512):
            draw.line((0, y, 511, y), fill=(192 + y // 12, 203 + y // 16, 218 + y // 20))
        draw.rectangle((210, 210, 302, 302), outline=(110, 130, 158), width=2)
        draw.text((234, 250), f'FRAME {i}', fill=(55, 72, 95))
        image.save(fixtures / name)
    before = {p.name: qa.sha(p) for p in fixtures.iterdir()}
    actions = [
        {'kind': 'right-click', 'x': 100, 'y': 100},
        {'kind': 'shot', 'name': 'menu-no-pixel'},
        {'kind': 'move', 'x': 200, 'y': 229},
        {'kind': 'shot', 'name': 'hover-properties'},
        {'kind': 'click', 'x': 200, 'y': 229},
        {'kind': 'shot', 'name': 'properties'},
        {'kind': 'key', 'code': 27},
        {'kind': 'right-click', 'x': 100, 'y': 100},
        {'kind': 'click', 'x': 200, 'y': 194},
        {'kind': 'shot', 'name': 'delete-confirm-only'},
        {'kind': 'key', 'code': 27},
        {'kind': 'right-click', 'x': args.width / 2, 'y': 100},
        {'kind': 'shot', 'name': 'menu-with-pixel'},
        {'kind': 'key', 'code': 27},
        {'kind': 'key', 'code': 'D'},
        {'kind': 'wait', 'seconds': 0.25},
        {'kind': 'shot', 'name': 'd-next'},
        {'kind': 'key', 'code': 'A'},
        {'kind': 'wait', 'seconds': 0.25},
        {'kind': 'shot', 'name': 'a-back'},
        {'kind': 'move', 'x': 100, 'y': 100},
        {'kind': 'wait', 'seconds': 1.4},
        {'kind': 'shot', 'name': 'hidden'},
        {'kind': 'move', 'x': 101, 'y': 100},
        {'kind': 'shot', 'name': 'restored'},
    ]
    action_path = out / 'actions.json'
    action_path.write_text(json.dumps(actions, indent=2), encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary, output=out / 'viewer', input=fixtures / '02_middle.png',
                           commit=args.commit, theme=args.theme, width=args.width, height=args.height,
                           actions=action_path, isolated_profile=True))
    after = {p.name: qa.sha(p) for p in fixtures.iterdir()}
    assert before == after, 'Test images must remain unchanged: no deletion is permitted'
    result = json.loads((out / 'viewer/run.json').read_text(encoding='utf-8'))
    assert result['preferences_restored'] and result['isolated_profile']
    assert len(result['captures']) == 10
    for file in result['captures']:
        meta = json.loads((out / 'viewer' / Path(file).with_suffix('.json')).read_text(encoding='utf-8'))
        assert meta['logical_client'] == [args.width, args.height]
        expected = '03_last.png' if file == 'd-next.png' else '02_middle.png'
        assert meta['window_title'].startswith(expected), (file, meta['window_title'])
    report = {'result': 'PASS; visual menu review required', 'captures': len(result['captures']),
              'commit': args.commit, 'binary_sha256': qa.sha(args.binary), 'files_unchanged': True,
              'preferences_restored': True, 'dpi': meta['dpi'], 'theme': args.theme,
              'logical_client': [args.width, args.height]}
    (out / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=('light', 'dark'), default='light')
    parser.add_argument('--width', type=int, default=1280)
    parser.add_argument('--height', type=int, default=860)
    run(parser.parse_args())

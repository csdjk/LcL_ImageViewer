"""Native toolbar-only regression using the existing isolated Windows QA harness.

Button centers are supplied from the current build's screenshots, not inferred
from an old release. Image-pixel assertions reject a wrong-coordinate click.
Never modifies installed viewers, the normal profile, or source image files.
"""
from __future__ import annotations
import argparse
import json
import time
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageChops, ImageDraw, ImageStat
import neumorphic_smoke as qa


def mark(name):
    return {'kind': 'shot', 'name': name}


def wait(seconds=0.25):
    return {'kind': 'wait', 'seconds': seconds}


def key(code):
    return {'kind': 'key', 'code': code}


def red_bounds(path: Path):
    with Image.open(path).convert('RGB') as im:
        r, g, b = im.split()
        mask = ImageChops.multiply(r.point(lambda v: 255 if abs(v - 220) <= 3 else 0),
                                  g.point(lambda v: 255 if abs(v - 70) <= 3 else 0))
        mask = ImageChops.multiply(mask, b.point(lambda v: 255 if abs(v - 110) <= 3 else 0))
        bounds = mask.getbbox()
        if bounds is None:
            raise AssertionError(f'Fixture image not visible: {path}')
        return bounds


def difference(a: Path, b: Path, roi):
    with Image.open(a).convert('RGB') as x, Image.open(b).convert('RGB') as y:
        return max(ImageStat.Stat(ImageChops.difference(x.crop(roi), y.crop(roi))).mean)


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    fixtures.mkdir()
    for name, size in [('01-check.png', (400, 240)), ('02-check.png', (280, 180))]:
        image = Image.new('RGBA', size, (220, 70, 110, 255))
        x, y = size[0] // 2, size[1] // 2
        ImageDraw.Draw(image).rectangle((x-40, y-30, x+40, y+30), fill=(45, 128, 220, 255))
        image.save(fixtures / name)
    results = []
    for theme in ('light', 'dark'):
        for width, height, actual_x, settings_x in [
            (880, 560, args.actual_x_880, args.settings_x_880),
            (1280, 860, args.actual_x_1280, args.settings_x_1280),
        ]:
            folder = out / f'{theme}-{width}'
            move = {'kind': 'move', 'x': 30, 'y': 130}
            click_actual = {'kind': 'click', 'x': actual_x, 'y': 34}
            actions = [
                {'kind': 'configure-start'},
                move, wait(), mark('initial-fit'),
                {'kind': 'move', 'x': actual_x, 'y': 34}, wait(0.7), mark('actual-tooltip'),
                {'kind': 'down', 'x': actual_x, 'y': 34}, wait(0.05), mark('actual-pressed'),
                {'kind': 'up'}, click_actual, move, wait(), mark('actual-button'),
                key('F'), wait(), mark('fit-shortcut'),
                key('0'), wait(), mark('actual-shortcut'),
                {'kind': 'wheel', 'x': width//2, 'y': height//2, 'delta': 240},
                click_actual, move, wait(), mark('actual-after-zoom'),
                key('L'), key('D'), wait(), move, mark('locked-next'),
                key('F'), wait(), mark('fit-locked-next'),
                key('A'), key('L'), key('F'), wait(), move, mark('back-first-fit'),
                key('0'), move, wait(),
                {'kind': 'right-click', 'x': 200, 'y': 180}, wait(), mark('context-menu'),
                key(27), wait(),
                {'kind': 'click', 'x': settings_x, 'y': 34}, wait(), mark('settings'),
                key(27), wait(), move, wait(), mark('before-hide'),
                wait(1.6), mark('hidden'),
                {'kind': 'move', 'x': 35, 'y': 136}, wait(), mark('restored'),
            ]
            action_path = out / f'actions-{theme}-{width}.json'
            action_path.write_text(json.dumps(actions, indent=2), encoding='utf-8')
            def configure(action, hwnd, shot):
                if action['kind'] != 'configure-start':
                    raise ValueError(action['kind'])
                # Reset navigation and inspect real palette pixels before asserting a theme.
                # External keyboard input during startup must not produce a false dark/light pass.
                qa.key(hwnd, 0x24)  # Home: the first fixture
                qa.key(hwnd, ord('F'))
                qa.move(hwnd, 30, 130)
                time.sleep(0.25)
                probe = folder / 'palette-probe.png'
                qa.capture(hwnd, probe)
                with Image.open(probe).convert('RGB') as im:
                    is_dark = sum(im.getpixel((200, 20))) / 3 < 110
                if is_dark != (theme == 'dark'):
                    qa.key(hwnd, ord('T'))
                    qa.move(hwnd, 31, 131)
                    time.sleep(0.25)
                shot('setup')
                with Image.open(folder/'setup.png').convert('RGB') as im:
                    assert (sum(im.getpixel((200, 20))) / 3 < 110) == (theme == 'dark')

            qa.run(SimpleNamespace(binary=args.binary, output=folder, input=fixtures/'01-check.png',
                                   commit=args.commit, theme=theme, width=width, height=height,
                                   actions=action_path, isolated_profile=True, action_handler=configure))
            meta = json.loads((folder/'run.json').read_text(encoding='utf-8'))
            assert meta['preferences_restored'] and meta['isolated_profile']
            assert json.loads((folder/'actual-button.json').read_text(encoding='utf-8'))['dpi'] == 96
            expected = ((width-400)//2, (height-240)//2, (width+400)//2, (height+240)//2)
            for state in ('actual-button', 'actual-shortcut', 'actual-after-zoom'):
                bounds = red_bounds(folder / f'{state}.png')
                assert max(abs(a-b) for a, b in zip(bounds, expected)) <= 1, (state, bounds, expected)
            bounds = red_bounds(folder/'locked-next.png')
            assert abs(bounds[2]-bounds[0]-280) <= 1 and abs(bounds[3]-bounds[1]-180) <= 1, bounds
            for state in ('initial-fit', 'fit-shortcut', 'back-first-fit'):
                bounds = red_bounds(folder / f'{state}.png')
                assert bounds[2]-bounds[0] > 500, (state, bounds)
            roi = (170, 90, width-170, height-90)
            assert difference(folder/'initial-fit.png', folder/'fit-shortcut.png', roi) < 0.1
            assert difference(folder/'actual-button.png', folder/'actual-shortcut.png', roi) < 0.1
            assert difference(folder/'before-hide.png', folder/'hidden.png', roi) < 0.1
            top = (100, 10, width-100, 58)
            assert difference(folder/'before-hide.png', folder/'hidden.png', top) > 3
            assert difference(folder/'before-hide.png', folder/'restored.png', top) < 0.5
            settings_roi = (width//2-110, height//2-90, width//2+110, height//2+90)
            assert difference(folder/'settings.png', folder/'before-hide.png', settings_roi) > 4
            results.append({'theme': theme, 'width': width, 'captures': len(meta['captures']),
                            'actual_button_and_0_are_100_percent': True, 'F_and_initial_fit_preserved': True,
                            'view_lock_preserved': True, 'hide_restore_preserved': True,
                            'binary_sha256': meta['binary_sha256']})
    report = {'result': 'PASS', 'commit': args.commit, 'cases': results,
              'visual_review': 'Required separately for toolbar icons, spacing, tooltips and panel layout.'}
    (out/'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    for kind in ('actual', 'settings'):
        for width in (880, 1280):
            parser.add_argument(f'--{kind}-x-{width}', type=float, required=True)
    run(parser.parse_args())

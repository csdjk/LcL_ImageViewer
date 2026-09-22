"""Real viewer checks using newly created fixtures and an isolated preference profile.
Writes only to a NEW output directory; never changes user source images.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import time
from types import SimpleNamespace
from PIL import Image, ImageDraw
import psutil
import neumorphic_smoke as qa

COLORS = [(30, 90, 150), (40, 180, 100), (180, 120, 30), (120, 60, 190)]


def fixture(version):
    image = Image.new('RGBA', (480, 320), (*COLORS[version], 255))
    draw = ImageDraw.Draw(image)
    for x in range(0, 480, 24):
        draw.line((x, 0, x, 319), fill=(90, 95, 100, 255))
    draw.rectangle((24, 24, 47, 47), fill=(250, 20, 220, 255))
    draw.rectangle((200, 120, 280, 200), fill=(*COLORS[version], 255))
    return image


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    sources = out / 'fixtures'
    sources.mkdir()
    first = sources / '01_技术美术_长中文素材名_TextureInspection_AlphaMask_Version.png'
    second = sources / '02_对比素材_TextureComparison.png'
    fixture(0).save(first, compress_level=0)
    fixture(2).save(second, compress_level=0)
    initial_hash = qa.sha(first)
    captures = out / 'viewer'
    actions = []
    checks = []
    cx, cy = args.width / 2, args.height / 2

    def key(code):
        actions.append({'kind': 'key', 'code': code})

    def inspect(name, version=None, channel=False, baseline=False):
        actions.extend([{'kind': 'wait', 'seconds': 0.3},
                        {'kind': 'inspect', 'name': name, 'version': version,
                         'channel': channel, 'baseline': baseline}])

    def save(version, mode='direct'):
        actions.extend([{'kind': 'save-fixture', 'version': version, 'mode': mode},
                        {'kind': 'wait', 'seconds': 1.6}])

    key('0')
    key('N')
    actions.append({'kind': 'drag', 'button': 'left', 'x': cx, 'y': cy,
                    'dx': 36, 'dy': 24, 'expect_window_delta': [0, 0, 0, 0]})
    inspect('01-view-baseline', 0, baseline=True)
    if args.case == 'manual':
        save(1)
        inspect('02-auto-off-keeps-old', 0)
        key(116)
        inspect('03-f5-updated', 1)
    elif args.case == 'workflow':
        save(1, 'atomic')
        inspect('02-auto-refresh-keeps-view', 1)
        key('2')
        save(2)
        inspect('03-refresh-keeps-channel', 2, channel=True)
        key('5')
        inspect('03b-rgb-restored', 2)
        save(3, 'burst')
        inspect('04-burst-final-version', 3)
        save(0, 'corrupt')
        inspect('05-error-keeps-previous', 3)
        save(1)
        inspect('06-recovered-after-save', 1)
        save(0, 'missing')
        inspect('07-missing-keeps-previous', 1)
        key(116)
        inspect('07b-manual-missing-keeps-previous', 1)
        save(2, 'atomic')
        inspect('08-reappeared', 2)
        # Same size/time is intentionally invisible to metadata observation; F5 bypasses it.
        save(0, 'same-stamp')
        inspect('09-same-stamp-still-old', 2)
        key(116)
        inspect('10-f5-forces-fresh-pixels', 0)
        key('L')
        key('D')
        inspect('11-locked-next-view', 2)
        key('A')
        inspect('12-locked-return-view', 0)
    actions.extend([{'kind': 'move', 'x': 100, 'y': 100},
                    {'kind': 'wait', 'seconds': 1.5},
                    {'kind': 'shot', 'name': '13-auto-hidden'},
                    {'kind': 'move', 'x': 101, 'y': 100},
                    {'kind': 'shot', 'name': '14-restored'},
                    {'kind': 'right-click', 'x': 100, 'y': 100},
                    {'kind': 'shot', 'name': '15-context-menu'},
                    {'kind': 'click', 'x': 200, 'y': 405},
                    {'kind': 'wait', 'seconds': 0.3},
                    {'kind': 'shot', 'name': '16-settings-default'}])
    auto_x, auto_y = cx + 155, cy - 32
    lock_y = cy + 8
    actions.extend([{'kind': 'move', 'x': auto_x, 'y': auto_y},
                    {'kind': 'shot', 'name': '17-auto-hover'},
                    {'kind': 'down', 'x': auto_x, 'y': auto_y},
                    {'kind': 'shot', 'name': '18-auto-pressed'},
                    {'kind': 'up'},
                    {'kind': 'shot', 'name': '19-auto-toggled'},
                    {'kind': 'click', 'x': auto_x, 'y': auto_y},
                    {'kind': 'click', 'x': auto_x, 'y': lock_y},
                    {'kind': 'shot', 'name': '20-lock-toggled'},
                    {'kind': 'click', 'x': auto_x, 'y': lock_y},
                    {'kind': 'shot', 'name': '21-settings-restored'},
                    {'kind': 'click', 'x': cx + 165, 'y': cy - 153},
                    {'kind': 'move', 'x': 20, 'y': 190},
                    {'kind': 'idle-sample'}])

    reference_marker = None

    def handle(action, hwnd, shot):
        nonlocal reference_marker
        if action['kind'] == 'idle-sample':
            pid = qa.w.DWORD()
            qa.u.GetWindowThreadProcessId(hwnd, qa.c.byref(pid))
            process = psutil.Process(pid.value)
            time.sleep(1.5)
            samples = []
            for _ in range(3):
                cpu = process.cpu_times()
                samples.append({'wall_s': time.monotonic(), 'cpu_s': cpu.user + cpu.system,
                                'rss_bytes': process.memory_info().rss,
                                'thread_count': process.num_threads()})
                time.sleep(1.5)
            checks.append({'idle_samples': samples,
                           'cpu_s_over_3s': round(samples[-1]['cpu_s'] - samples[0]['cpu_s'], 3)})
            return
        if action['kind'] == 'save-fixture':
            mode, version = action['mode'], action['version']
            if mode == 'corrupt':
                first.write_bytes(b'not an image')
            elif mode == 'missing':
                first.unlink()
            elif mode == 'burst':
                for index in [0, 1, 2, version]:
                    fixture(index).save(first, compress_level=0)
                    time.sleep(0.06)
            elif mode == 'atomic':
                staging = sources / 'owned-replacement.tmp'
                fixture(version).save(staging, format='PNG', compress_level=0)
                os.replace(staging, first)
            else:
                before = first.stat()
                fixture(version).save(first, compress_level=0)
                if mode == 'same-stamp':
                    assert first.stat().st_size == before.st_size
                    os.utime(first, ns=(before.st_atime_ns, before.st_mtime_ns))
            checks.append({'operation': mode, 'version': version,
                           'source_sha256': qa.sha(first) if first.is_file() else None})
            return
        if action['kind'] != 'inspect':
            raise ValueError(action)
        shot(action['name'])
        image = Image.open(captures / (action['name'] + '.png')).convert('RGB')
        meta = json.loads((captures / (action['name'] + '.json')).read_text('utf-8'))
        scale = meta['dpi'] / 96
        center = (round((cx + 36) * scale), round((cy + 24) * scale))
        expected = COLORS[action['version']]
        if action['channel']:
            expected = (expected[1],) * 3
        actual = image.getpixel(center)
        assert max(abs(a - b) for a, b in zip(actual, expected)) <= 2, (action['name'], actual, expected)
        record = {'state': action['name'], 'center_rgb': actual, 'expected': expected}
        if not action['channel']:
            r, g, b = image.split()
            from PIL import ImageChops
            mask = ImageChops.multiply(r.point(lambda v: 255 if v > 240 else 0),
                                      g.point(lambda v: 255 if v < 35 else 0))
            mask = ImageChops.multiply(mask, b.point(lambda v: 255 if v > 205 else 0))
            bounds = mask.getbbox()
            assert bounds is not None, ('missing marker', action['name'])
            if action['baseline']:
                reference_marker = bounds
            assert bounds == reference_marker, ('view changed', action['name'], bounds, reference_marker)
            record['marker_bounds'] = bounds
        checks.append(record)

    action_file = out / 'actions.json'
    action_file.write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
    try:
        qa.run(SimpleNamespace(binary=args.binary, output=captures, input=first,
            commit=args.commit, theme=args.theme, width=args.width, height=args.height,
            actions=action_file, isolated_profile=True, action_handler=handle,
            seed_preferences={'iv-auto-refresh': 'off' if args.case == 'manual' else 'on'}))
        meta = json.loads((captures / 'run.json').read_text('utf-8'))
        assert meta['preferences_restored'] and meta['isolated_profile']
        assert meta['qa_saved_settings']['iv-auto-refresh'] == ('off' if args.case == 'manual' else 'on')
        if args.case == 'workflow':
            assert meta['qa_saved_settings']['iv-lock-view'] == 'on'
        checks.append({'preferences_restored': True, 'saved_settings': meta['qa_saved_settings']})
    finally:
        (out / 'assertions.json').write_text(json.dumps({
            'initial_source_sha256': initial_hash, 'checks': checks,
        }, ensure_ascii=False, indent=2), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=['light', 'dark'], default='light')
    parser.add_argument('--width', type=int, default=880)
    parser.add_argument('--height', type=int, default=560)
    parser.add_argument('--case', choices=['workflow', 'manual'], default='workflow')
    run(parser.parse_args())

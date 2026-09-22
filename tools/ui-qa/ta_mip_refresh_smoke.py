"""Verify native DDS mip selection survives save/reload, then clamps when mip count shrinks."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import struct
from types import SimpleNamespace

from PIL import Image

import neumorphic_smoke as qa


def dds(colors):
    header = ([124, 0x2100F, 128, 128, 512, 0, len(colors)] + [0] * 11
              + [32, 0x41, 0, 32, 0x000000FF, 0x0000FF00, 0x00FF0000, 0xFF000000]
              + [0x401008, 0, 0, 0, 0])
    assert len(header) == 31
    pixels = b''.join(bytes((*color, 255)) * ((128 >> index) ** 2)
                      for index, color in enumerate(colors))
    return b'DDS ' + struct.pack('<31I', *header) + pixels


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    source = out / 'Mipmap_贴图检查.dds'
    source.write_bytes(dds([(200, 30, 40), (30, 180, 70)]))
    captures = out / 'viewer'
    actions = [
        {'kind': 'key', 'code': '0'},
        {'kind': 'key', 'code': 38},
        {'kind': 'inspect', 'name': '01-selected-mip1', 'rgb': [30, 180, 70]},
        {'kind': 'save-dds', 'colors': [[20, 50, 200], [180, 50, 190]]},
        {'kind': 'wait', 'seconds': 1.6},
        {'kind': 'inspect', 'name': '02-refreshed-mip1', 'rgb': [180, 50, 190]},
        {'kind': 'save-dds', 'colors': [[100, 160, 30]]},
        {'kind': 'wait', 'seconds': 1.6},
        {'kind': 'inspect', 'name': '03-clamped-to-mip0', 'rgb': [100, 160, 30]},
    ]
    script = out / 'actions.json'
    script.write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
    checks = []

    def handle(action, hwnd, shot):
        if action['kind'] == 'save-dds':
            staging = out / 'replacement.tmp'
            staging.write_bytes(dds(action['colors']))
            os.replace(staging, source)
            return
        if action['kind'] != 'inspect':
            raise ValueError(action)
        shot(action['name'])
        image = Image.open(captures / (action['name'] + '.png')).convert('RGB')
        meta = json.loads((captures / (action['name'] + '.json')).read_text('utf-8'))
        scale = meta['dpi'] / 96
        # The viewer anchors the actual-size texture at the viewport centre;
        # probe inside the smallest mip rather than on its bottom-right edge.
        center = (round((args.width / 2 - 20) * scale),
                  round((args.height / 2 - 20) * scale))
        actual = image.getpixel(center)
        expected = action['rgb']
        assert max(abs(a - b) for a, b in zip(actual, expected)) <= 2, (action['name'], actual, expected)
        checks.append({'state': action['name'], 'center_rgb': actual, 'expected': expected})

    try:
        qa.run(SimpleNamespace(binary=args.binary, output=captures, input=source,
            commit=args.commit, theme=args.theme, width=args.width, height=args.height,
            actions=script, isolated_profile=True, action_handler=handle))
    finally:
        (out / 'assertions.json').write_text(json.dumps(checks, indent=2), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=['light', 'dark'], default='dark')
    parser.add_argument('--width', type=int, default=880)
    parser.add_argument('--height', type=int, default=560)
    run(parser.parse_args())

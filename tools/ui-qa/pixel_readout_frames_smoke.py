"""Native pixel-readout checks for animation frames and DDS Mips.

Uses generated, known RGBA data and the same isolated viewer profile harness.
Image colors are asserted from captured pixels; exact HUD text is reviewed
visually using the expected_text_for_visual_review metadata (no OCR).
"""
from __future__ import annotations
import argparse
import ctypes as c
from ctypes import wintypes as wt
import json
import os
from pathlib import Path
import struct
from PIL import Image
from pixel_readout_smoke import Viewer, assert_gray, expected_text
import neumorphic_smoke as qa


def make_mips(path: Path):
    # Uncompressed DDS RGBA8, standard mip ordering and explicit bit masks.
    head = [124, 0x2100F, 256, 512, 512 * 4, 0, 3] + [0] * 11
    head += [32, 0x41, 0, 32, 0xFF, 0xFF00, 0xFF0000, 0xFF000000]
    head += [0x401008, 0, 0, 0, 0]
    assert len(head) == 31
    body = b''.join(bytes(color) * (w * h) for w, h, color in [
        (512, 256, (244, 199, 123, 253)),
        (256, 128, (17, 88, 199, 128)),
        (128, 64, (0, 128, 255, 64)),
    ])
    path.write_bytes(b'DDS ' + struct.pack('<31I', *head) + body)


def run(args):
    args.binary = args.binary.resolve(strict=True)
    out = args.output.resolve(); out.mkdir(parents=True, exist_ok=False)
    qa.bind(qa.u, 'SetProcessDpiAwarenessContext', [qa.PTR], wt.BOOL)(qa.PTR(-4))
    normal = Path(os.environ['APPDATA']) / 'LcL ImageViewer/data/app.ron'
    original = normal.read_bytes() if normal.exists() else None
    foreground, cursor = qa.u.GetForegroundWindow(), wt.POINT()
    qa.u.GetCursorPos(c.byref(cursor))
    fixtures = out / 'fixtures'; fixtures.mkdir()
    frames = [Image.new('RGBA', (512, 256), color) for color in
              ((244, 199, 123, 253), (17, 88, 199, 128))]
    anim = fixtures / '01-animated.apng'
    frames[0].save(anim, format='PNG', save_all=True, append_images=frames[1:],
                   duration=[60000, 60000], loop=0, disposal=0, blend=0)
    dds = fixtures / '02-mips.dds'; make_mips(dds)
    hashes = {p.name: qa.sha(p) for p in (anim, dds)}
    reports = []
    try:
        for name, image in [('animation', anim), ('mips', dds)]:
            folder = out / name; folder.mkdir()
            v = Viewer(args, folder, 'dark', 880, 560, image)
            try:
                v.start()
                if name == 'animation':
                    v.key(32)  # Pause the long-duration first frame, before stepping.
                v.select_format('bytes')
                v.shot('base-rgba', expected_text('bytes', 'RGBA'))
                if name == 'animation':
                    v.key(0xBE); v.sample()
                    v.shot('second-frame-rgba', expected_text('bytes', 'RGBA', (17, 88, 199, 128)))
                    for channel, code, value in [('R', '1', 17), ('A', '4', 128)]:
                        v.key(code)
                        shot = v.shot('second-frame-' + channel,
                                      expected_text('bytes', channel, (17, 88, 199, 128)))
                        assert_gray(shot, (440, 280), value)
                    v.key(0xBC)
                    shot = v.shot('first-frame-A', expected_text('bytes', 'A'))
                    assert_gray(shot, (440, 280), 253)
                else:
                    for level, pos, rgba in [(1, (128, 64), (17, 88, 199, 128)),
                                             (2, (64, 32), (0, 128, 255, 64))]:
                        v.key(0x26); v.key('0'); v.key('5'); v.sample()  # Up advances Mip index in this viewer.
                        v.shot(f'mip-{level}-rgba', expected_text('bytes', 'RGBA', rgba, pos))
                        v.key('4')
                        shot = v.shot(f'mip-{level}-A', expected_text('bytes', 'A', rgba, pos))
                        assert_gray(shot, (440, 280), rgba[3])
                v.close('bytes')
                reports.append({'case': name, 'captures': len(v.captures), 'result': 'PASS',
                                'binary_sha256': qa.sha(args.binary)})
            finally:
                v.cleanup()
                (folder / 'run.json').write_text(json.dumps({'captures': v.captures,
                    'commit': args.commit}, indent=2), encoding='utf-8')
        assert hashes == {p.name: qa.sha(p) for p in (anim, dds)}
    finally:
        assert (normal.read_bytes() if normal.exists() else None) == original
        qa.u.SetCursorPos(cursor.x, cursor.y)
        if foreground: qa.u.SetForegroundWindow(foreground)
    report = {'result': 'PASS', 'commit': args.commit, 'cases': reports,
              'normal_preferences_untouched': True, 'input_hashes_unchanged': hashes}
    (out / 'report.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
    print(json.dumps(report, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--commit', required=True)
    run(parser.parse_args())

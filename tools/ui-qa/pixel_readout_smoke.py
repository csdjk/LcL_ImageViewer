"""Real Windows readout/format/persistence QA using isolated profiles only.

Reuses PID verification, bounded foreground acquisition and native capture.
Expected text is recorded for visual review (not claimed as OCR verification).
Native checks cover source pixels, same-profile restarts, stable capsule bounds,
channel rendering, and unchanged application data/normal user preferences.
"""
from __future__ import annotations
import argparse
import ctypes as c
from ctypes import wintypes as wt
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time
import uuid
from PIL import Image, ImageChops, ImageDraw
import neumorphic_smoke as qa


class Viewer:
    def __init__(self, args, folder, theme, width, height, image):
        self.args, self.folder, self.theme = args, folder, theme
        self.width, self.height, self.image = width, height, image
        self.profile = uuid.uuid4().hex
        self.app_id = 'LcL ImageViewer QA-' + self.profile
        self.storage = Path(os.environ['APPDATA']) / self.app_id / 'data/app.ron'
        if self.storage.parent.parent.exists():
            raise RuntimeError('Refusing an existing QA profile')
        self.storage.parent.mkdir(parents=True)
        self.storage.write_text(json.dumps({'iv-theme': theme, 'iv-backdrop': 'off',
            'iv-checkerboard': 'on', 'iv-reduce-motion': 'on'}), encoding='utf-8')
        self.proc = None
        self.captures, self.starts, self.actions = [], 0, []

    def start(self):
        self.starts += 1
        env = dict(os.environ, LCL_IV_QA_PROFILE=self.profile)
        log = self.folder / f'runtime-{self.starts}.log'
        with log.open('wb') as f:
            self.proc = subprocess.Popen([str(self.args.binary), str(self.image)],
                cwd=self.args.binary.parent, env=env, stdout=f, stderr=f)
        deadline = time.monotonic() + 25
        while time.monotonic() < deadline:
            if self.proc.poll() is not None:
                raise RuntimeError('Owned viewer exited at startup')
            handles = qa.windows_for_pid(self.proc.pid)
            if len(handles) == 1:
                self.hwnd = handles[0]
                break
            time.sleep(0.15)
        else:
            raise TimeoutError('Missing/ambiguous viewer window')
        assert qa.process_path(self.proc.pid) == self.args.binary
        time.sleep(0.8)
        assert ('QA isolated profile: ' + self.app_id) in log.read_text('utf-8', errors='replace')
        wr, cr, _, dpi = qa.geometry(self.hwnd)
        dw, dh = wr.right-wr.left-cr.right, wr.bottom-wr.top-cr.bottom
        qa.checked(qa.u.SetWindowPos(self.hwnd, None, 40, 40,
            round(self.width*dpi/96)+dw, round(self.height*dpi/96)+dh, 0x0040), 'Resize QA viewer')
        time.sleep(0.5)
        self.key('0')
        self.key('5')
        self.sample()

    def key(self, code):
        self.actions.append({'key': code})
        qa.key(self.hwnd, ord(code.upper()) if isinstance(code, str) else code)

    def click(self, x, y):
        self.actions.append({'click': [x, y]})
        qa.move(self.hwnd, x, y)
        qa.atomic_click(self.hwnd, x, y)
        time.sleep(0.2)

    def sample(self, x=256, y=128):
        ox, oy = (self.width-512)/2, (self.height-256)/2
        qa.move(self.hwnd, ox+x+1, oy+y)
        qa.move(self.hwnd, ox+x, oy+y)
        self.actions.append({'sample': [x, y]})
        time.sleep(0.08)

    def shot(self, name, expected=None):
        if qa.u.GetForegroundWindow() != self.hwnd:
            # One bounded reacquisition of our verified PID; keep the hard focus guard.
            assert qa.process_path(self.proc.pid) == self.args.binary
            qa.focus(self.hwnd)
        if qa.u.GetForegroundWindow() != self.hwnd:
            raise RuntimeError('Focus changed: refusing to capture another application')
        p = self.folder / f'{name}.png'
        meta = qa.capture(self.hwnd, p)
        assert meta['logical_client'] == [self.width, self.height]
        assert meta['dpi'] == 96, 'Pixel assertions are scoped to recorded DPI96 only'
        title = c.create_unicode_buffer(1024)
        qa.u.GetWindowTextW(self.hwnd, title, len(title))
        meta.update(commit=self.args.commit, binary=str(self.args.binary),
            binary_sha256=qa.sha(self.args.binary), input=str(self.image),
            input_sha256=qa.sha(self.image), state=name, theme=self.theme,
            window_title=title.value, pid=self.proc.pid, isolated_profile=self.profile,
            utc=datetime.now(timezone.utc).isoformat(), actions=list(self.actions),
            expected_text_for_visual_review=expected)
        p.with_suffix('.json').write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding='utf-8')
        self.captures.append(p.name)
        return p

    def open_settings(self, name):
        self.sample()
        self.click(628 if self.width == 880 else 955, 34)
        return self.shot(name)

    def select_format(self, fmt):
        self.open_settings(f'settings-before-{fmt}')
        # Calibrated against this build's native screenshots, see metadata.
        x, y = self.width/2+95, self.height/2-48
        self.click(x, y)
        self.shot(f'format-menu-{fmt}')
        index = ['normalized', 'bytes', 'hex'].index(fmt)
        self.click(x, y + 28 + index*24)  # native popup rows are 24px, not settings' 40px
        self.shot(f'settings-selected-{fmt}')
        self.key(27)
        self.sample()

    def close(self, expected_format=None):
        if self.proc is not None and self.proc.poll() is None:
            qa.u.PostMessageW(self.hwnd, 0x0010, 0, 0)
            try:
                self.proc.wait(timeout=12)
            except subprocess.TimeoutExpired:
                self.proc.terminate()  # only the process created by this tester
                self.proc.wait(timeout=5)
                raise RuntimeError('QA viewer did not exit normally')
        if expected_format is not None:
            text = self.storage.read_text('utf-8')
            stored = dict(re.findall(r'"(iv-[^"\\]+)":\s*"([^"\\]*)"', text))
            assert stored.get('iv-pixel-format') == expected_format, stored
            (self.folder / f'saved-{expected_format}.json').write_text(
                json.dumps(stored, indent=2), encoding='utf-8')
        self.proc = None

    def cleanup(self):
        self.close()
        shutil.rmtree(self.storage.parent.parent)  # generated unique QA profile only


def expected_text(fmt, channel, rgba=(244, 199, 123, 253), pos=(256, 128)):
    def value(v):
        if fmt == 'hex': return f'#{v:02X}'
        if fmt == 'bytes': return str(v)
        return f'{v/255:.3f}'.rstrip('0').rstrip('.')
    if channel in 'RGBA' and len(channel) == 1:
        col = channel + ' ' + value(rgba['RGBA'.index(channel)])
    else:
        count = 3 if channel == 'RGB' else 4
        col = ('#' + ''.join(f'{v:02X}' for v in rgba[:count])) if fmt == 'hex' else (
            channel + ' (' + ', '.join(value(v) for v in rgba[:count]) + ')')
    return f'({pos[0]}, {pos[1]})   {col}'


def assert_gray(p, xy, value):
    with Image.open(p).convert('RGB') as im:
        actual = im.getpixel(xy)
        assert max(abs(v-value) for v in actual) <= 3, (p.name, value, actual)


def capsule_extent(p, hidden, height):
    with Image.open(p).convert('RGB') as a, Image.open(hidden).convert('RGB') as b:
        diff = ImageChops.difference(a, b).crop((0, height-70, a.width, height))
        rr, gg, bb = diff.split()
        mask = ImageChops.lighter(ImageChops.lighter(rr, gg), bb).point(lambda v: 255 if v > 22 else 0)
        box = mask.getbbox()
        assert box and box[0] > 8 and box[2] < a.width-8, (p, box)
        return box[0], box[2]


def run(args):
    args.binary = args.binary.resolve(strict=True)
    out = args.output.resolve(); out.mkdir(parents=True, exist_ok=False)
    assert b'LcL ImageViewer QA-' in args.binary.read_bytes()
    qa.bind(qa.u, 'SetProcessDpiAwarenessContext', [qa.PTR], wt.BOOL)(qa.PTR(-4))
    normal = Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    original = normal.read_bytes() if normal.exists() else None
    foreground, cursor = qa.u.GetForegroundWindow(), wt.POINT()
    qa.u.GetCursorPos(c.byref(cursor))
    fixtures = out/'fixtures'; fixtures.mkdir()
    im = Image.new('RGBA', (512, 256), (244, 199, 123, 253))
    draw = ImageDraw.Draw(im)
    draw.rectangle((0, 0, 80, 80), fill=(17, 88, 199, 0))
    draw.rectangle((400, 0, 511, 80), fill=(0, 128, 255, 128))
    im.save(fixtures/'01-color.png')
    Image.new('RGBA', (512, 256), (11, 66, 133, 255)).save(fixtures/'02-color.png')
    source_hash = qa.sha(fixtures/'01-color.png')
    cases = []
    try:
        for theme in ('light', 'dark'):
            for width, height in ((880, 560), (1280, 860)):
                if args.case and args.case != f'{theme}-{width}':
                    continue
                folder = out/f'{theme}-{width}';folder.mkdir()
                v = Viewer(args, folder, theme, width, height, fixtures/'01-color.png')
                try:
                    v.start()
                    baseline = v.shot('legacy-hex-default', expected_text('hex', 'RGBA'))
                    for fmt in ('bytes', 'normalized', 'hex'):
                        v.select_format(fmt)
                        for channel, key in [('RGBA', '5'), ('R', '1'), ('G', '2'), ('B', '3'), ('A', '4'), ('RGB', 'O')]:
                            v.sample()  # show the auto-hiding toolbar before the next key event
                            v.key(key)  # no subsequent pointer movement: channel-only changes must reformat immediately
                            p = v.shot(f'{fmt}-{channel}', expected_text(fmt, channel))
                            if len(channel) == 1:
                                assert_gray(p, (width//2, height//2), (244,199,123,253)['RGBA'.index(channel)])
                        v.key('5'); v.sample()
                        before = v.shot(f'{fmt}-before-restart', expected_text(fmt, 'RGBA'))
                        v.close(fmt)
                        v.start()  # identical profile: settings read from app's actual saved file
                        after = v.shot(f'{fmt}-after-restart', expected_text(fmt, 'RGBA'))
                        # Pointer coordinates legitimately change when the desktop user moves
                        # the mouse. Check the color/value field and zoom after a real restart,
                        # not coordinate glyphs. DPI96/512px fixture geometry is fixed above.
                        color_left = width//2 + {'normalized': -80, 'bytes': -45, 'hex': 0}[fmt]
                        roi = (color_left, height-70, width, height)
                        with Image.open(before) as a, Image.open(after) as b:
                            diff = ImageChops.difference(a.crop(roi), b.crop(roi))
                            assert max(hi for _,hi in diff.getextrema()) <= 2, ('Restart changed color format/values', fmt)
                        if fmt == 'normalized':
                            stable = []
                            for xy in ((9,100), (99,100), (100,100)):
                                v.sample(*xy);stable.append(v.shot('coord-'+str(xy[0]), expected_text(fmt,'RGBA',pos=xy)))
                            time.sleep(1.4); hidden=v.shot('hidden')
                            extents=[capsule_extent(p,hidden,height) for p in stable]
                            assert len(set(extents))==1, ('Readout capsule jitter',extents)
                            v.sample();v.shot('restored')
                    for channel,key in [('R','1'),('G','2'),('B','3'),('A','4')]:
                        v.key(key); v.sample(40,40)
                        p=v.shot('transparent-'+channel, expected_text('hex',channel,(17,88,199,0),(40,40)))
                        assert_gray(p, ((width-512)//2+40,(height-256)//2+40), (17,88,199,0)['RGBA'.index(channel)])
                    v.key('5');v.sample();v.open_settings('final-settings');v.key(27)
                    v.sample();v.close('hex')
                    cases.append({'theme':theme,'width':width,'captures':len(v.captures),
                        'same_profile_restarts':3,'saved_formats':['bytes','normalized','hex'],
                        'stationary_pointer_channels':True,'transparent_source_channels':True,'stable_capsule':True,
                        'binary_sha256':qa.sha(args.binary)})
                finally:
                    v.cleanup()
                (folder/'run.json').write_text(json.dumps({'captures':v.captures,'commit':args.commit,
                    'theme':theme,'width':width,'normal_preferences_untouched':True},indent=2),encoding='utf-8')
                print('PASS',theme,width,flush=True)
        assert qa.sha(fixtures/'01-color.png')==source_hash
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==original, 'Normal profile changed'
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground: qa.u.SetForegroundWindow(foreground)
    report={'result':'PASS','commit':args.commit,'cases':cases,'input_hash_unchanged':True,
        'visual_review':'Review actual rendered text, color menu and capsule layout separately.'}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False,indent=2),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--case', choices=['light-880','light-1280','dark-880','dark-1280'])
    run(parser.parse_args())

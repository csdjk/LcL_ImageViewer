"""Real, PID-verified editor interactions and native PNG save dialogs.

All inputs/outputs are generated below a new QA directory. The normal profile
and the user's source images/installed application are never modified.
"""
from __future__ import annotations
import argparse
import ctypes as c
from ctypes import wintypes as wt
import json
import math
import os
from pathlib import Path
import time
from PIL import Image
import neumorphic_smoke as qa
from pixel_readout_smoke import Viewer


def text(hwnd, value):
    qa.focus(hwnd)
    encoded = str(value).encode('utf-16-le')
    for i in range(0, len(encoded), 2):
        assert qa.u.GetForegroundWindow() == hwnd, 'Input focus moved away from owned window'
        unit = int.from_bytes(encoded[i:i+2], 'little')
        inputs = (qa.NativeInput*2)(*[qa.NativeInput(1, qa.InputUnion(keyboard=qa.KeyInput(0, unit, flag, 0, 0))) for flag in (4,6)])
        assert qa.u.SendInput(2, inputs, c.sizeof(qa.NativeInput)) == 2
    time.sleep(0.15)


def combo(hwnd, modifier, key):
    qa.focus(hwnd)
    qa.u.keybd_event(modifier,0,0,0)
    try: qa.key(hwnd,key)
    finally: qa.u.keybd_event(modifier,0,2,0)


class EditViewer(Viewer):
    def field(self, x, y, value):
        self.click(x,y)
        combo(self.hwnd,0x11,ord('A'))
        text(self.hwnd,value)
        self.key(13)
        time.sleep(0.15)

    def save_dialog(self, x, y, path, cancel=False):
        assert path.parent.resolve().is_relative_to(self.args.output.resolve())
        assert not path.exists(), 'Never ask the native picker to overwrite a file'
        self.click(x,y)
        deadline=time.monotonic()+12
        dialog=None
        while time.monotonic()<deadline:
            others=[h for h in qa.windows_for_pid(self.proc.pid) if h!=self.hwnd]
            if len(others)==1: dialog=others[0];break
            time.sleep(0.1)
        if dialog is None: raise RuntimeError('Owned save dialog did not open')
        qa.focus(dialog)
        qa.capture(dialog,self.folder/('save-cancel-dialog.png' if cancel else 'save-dialog.png'))
        if cancel:
            qa.key(dialog,27);time.sleep(0.6)
            assert self.proc.poll() is None and not path.exists()
            return
        combo(dialog,0x12,ord('N'))  # Standard Windows common file dialog: File name (Alt+N).
        combo(dialog,0x11,ord('A'))
        text(dialog,str(path))
        qa.key(dialog,13)
        deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            if path.exists() and dialog not in qa.windows_for_pid(self.proc.pid):
                try:
                    with Image.open(path) as im: im.load()
                    break
                except OSError: pass
            time.sleep(0.15)
        else: raise RuntimeError('No complete PNG export from the native Save dialog')
        time.sleep(0.6)
        qa.focus(self.hwnd)

    def cleanup(self):
        # Only terminate a process created by this tester, never another viewer.
        if self.proc is not None and self.proc.poll() is None:
            self.proc.terminate();self.proc.wait(timeout=8)
        self.proc=None
        super().cleanup()


def run(args):
    args.binary=args.binary.resolve(strict=True)
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    before=normal.read_bytes() if normal.exists() else None
    foreground,cursor=qa.u.GetForegroundWindow(),wt.POINT();qa.u.GetCursorPos(c.byref(cursor))
    f=args.output/'fixtures';f.mkdir()
    im=Image.new('RGBA',(512,256))
    im.putdata([(x%256,y,32+(x//256)*100,0 if x<64 and y<64 else (128 if x>400 else 255)) for y in range(256) for x in range(512)])
    source=f/'coordinate-source.png';im.save(source)
    Image.new('RGBA',(512,256),(11,77,199,128)).save(f/'02-other.png')
    digest=qa.sha(source); reports=[]
    cases=[('light',880,560),('dark',1280,860),('dark',880,560),('light',1280,860)]
    if args.calibrate: cases=cases[:1]
    try:
        for theme,w,h in cases:
            folder=args.output/f'{theme}-{w}';folder.mkdir()
            v=EditViewer(args,folder,theme,w,h,source)
            # Exact client coordinates from current binary's two calibration screenshots.
            ox,oy=(0,0) if w==880 else (190,54)
            def click(x,y):v.click(x+ox,y+oy)
            try:
                v.start();v.key('1');v.key('E');time.sleep(0.8)
                v.shot('crop-initial', 'Editing raw RGBA, not the selected R display')
                v.key('D');v.key(46);state=v.shot('navigation-and-delete-blocked')
                assert json.loads(state.with_suffix('.json').read_text('utf-8'))['window_title'].startswith('coordinate-source.png')
                click(110,168);v.shot('crop-square','X=128, Y=0, width=height=256')
                click(120,114);time.sleep(0.5);v.shot('resize-page')
                if args.calibrate:
                    v.save_dialog(110,493,folder/'not-saved.png',cancel=True)
                    v.shot('native-save-cancel-keeps-editor')
                    reports.append({'case':f'{theme}-{w}','calibration':True})
                    continue
                # Use the half-size preset; ratio is kept exactly at 1:1.
                click(56,168);time.sleep(0.5);v.shot('resize-half','128 x 128')
                export=folder/'cropped-resized.png'
                v.save_dialog(110+ox,493+(246 if w==1280 else 0),export)
                with Image.open(export) as saved:
                    saved=saved.convert('RGBA');assert saved.size==(128,128),saved.size
                    assert saved.getpixel((0,0))==(129,1,32,255),saved.getpixel((0,0))
                    assert saved.getpixel((127,127))==(127,255,132,255),saved.getpixel((127,127))
                assert qa.sha(source)==digest
                v.shot('save-success')
                # Reset geometry; current exact source pixels including Alpha=0 must survive.
                click(810,114);click(56,114);time.sleep(0.3)
                reset=folder/'full-rgba.png'
                v.save_dialog(110+ox,493+(246 if w==1280 else 0),reset)
                with Image.open(reset) as saved:assert saved.convert('RGBA').tobytes()==im.tobytes()
                v.shot('full-source-exported')
                v.key(27);time.sleep(0.4);v.shot('back-to-viewer')
                v.close()
                reports.append({'case':f'{theme}-{w}','captures':len(v.captures),'crop_resize_dimensions':True,
                    'resize_pixels':True,'source_bytes_and_alpha_unchanged':True,'native_save':True})
            finally:v.cleanup()
            print('PASS',theme,w,flush=True)
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==before,'Normal viewer preferences changed'
        assert qa.sha(source)==digest
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    result={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'cases':reports,
        'normal_preferences_unchanged':True,'source_hash_unchanged':True}
    (args.output/'report.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),'utf-8')
    print(json.dumps(result,ensure_ascii=False,indent=2),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True)
    p.add_argument('--calibrate',action='store_true')
    run(p.parse_args())

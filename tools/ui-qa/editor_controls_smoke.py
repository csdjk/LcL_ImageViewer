"""Native UI verification for the editor's shared controls, using isolated QA profiles.
Only generated fixtures are exported; screenshots include build/hash/window metadata.
"""
from pathlib import Path
from ctypes import wintypes as wt
import argparse, json, os, time
from PIL import Image, ImageChops
from inline_editor_smoke import InlineViewer, drag
import neumorphic_smoke as qa


def run(args):
    args.binary = args.binary.resolve(strict=True)
    args.output = args.output.resolve(); args.output.mkdir(parents=True, exist_ok=False)
    qa.bind(qa.u, 'SetProcessDpiAwarenessContext', [qa.PTR], wt.BOOL)(qa.PTR(-4))
    normal = Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    original = normal.read_bytes() if normal.exists() else None
    cursor=wt.POINT(); foreground=qa.u.GetForegroundWindow(); qa.u.GetCursorPos(qa.c.byref(cursor))
    fixture=args.output/'fixture.png'
    im=Image.new('RGBA',(512,256));im.putdata([(x%256,y,32+100*(x//256),0 if x<64 and y<64 else (128 if x>400 else 255)) for y in range(256) for x in range(512)])
    im.save(fixture);digest=qa.sha(fixture);reports=[]
    cases=[('dark',880,560),('light',1280,860),('light',880,560),('dark',1280,860)]
    if args.calibrate:cases=cases[:1]
    try:
        for theme,w,h in cases:
            folder=args.output/f'{theme}-{w}';folder.mkdir()
            v=InlineViewer(args,folder,theme,w,h,fixture)
            try:
                v.start();v.key('E');time.sleep(.6)
                for key,name in [('V','move'),('C','crop'),('R','rotate'),('I','resize')]:
                    v.choose(key);v.shot(name+'-controls')
                if args.calibrate:
                    reports.append({'theme':theme,'width':w,'calibration':True,'captures':len(v.captures)});continue
                # Filled against the current native screenshots, never by calling editing internals.
                v.choose('C')
                v.click(236,64);v.shot('numeric-field-focus');v.key(9)
                v.click(424,64);v.shot('crop-ratio-menu');v.key(27)
                v.choose('C');v.key('0');time.sleep(.2)
                drag(v,(w/2-160,h/2-45),(w/2+90,h/2+60))
                active=v.shot('active-crop')
                v.choose('R');pending=v.shot('pending-switch')
                # The fixed-height confirmation row must not recenter or shrink the image.
                roi=(0,100,w,h-100)
                with Image.open(active) as a, Image.open(pending) as b:
                    diff=ImageChops.difference(a.crop(roi),b.crop(roi))
                    assert max(hi for _,hi in diff.getextrema())<=2,'Pending notice moved or changed canvas'
                # Esc returns from the pending switch to the unchanged crop draft.
                v.key(27);v.shot('back-to-crop-draft')
                v.key(27);v.choose('R')
                # Current toolbar: first rotation command is left 90 degrees.
                v.click(55,64);time.sleep(.5);v.shot('rotated-left')
                v.ctrl('Z');v.shot('undo-available-redo')
                v.ctrl('Y');v.shot('redo-applied')
                # Save through the native dialog using the actual styled primary button.
                target=folder/'rotated.png';qa.move(v.hwnd,w-60,h-70)
                v.save_dialog(w-64,h-72,target)
                with Image.open(target) as got:
                    ref=im.transpose(Image.Transpose.ROTATE_90)
                    assert got.size==ref.size and got.convert('RGBA').tobytes()==ref.tobytes()
                v.shot('saved-primary-action')
                v.choose('V');v.key('0');v.key('F');v.shot('viewport-actions')
                qa.move(v.hwnd,200,h-72);v.shot('button-hover')
                qa.focus(v.hwnd);qa.u.mouse_event(2,0,0,0,0)
                try:
                    time.sleep(.1);v.shot('button-pressed')
                finally:qa.u.mouse_event(4,0,0,0,0)
                assert len(qa.windows_for_pid(v.proc.pid))==1
                assert qa.sha(fixture)==digest
                reports.append({'theme':theme,'width':w,'captures':len(v.captures),'native_save_pixels_exact':True,'source_preserved':True})
            finally:v.cleanup()
            print('PASS',theme,w,flush=True)
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==original
        assert qa.sha(fixture)==digest
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    result={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'cases':reports,'normal_preferences_unchanged':True}
    (args.output/'report.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True);p.add_argument('--calibrate',action='store_true')
    run(p.parse_args())

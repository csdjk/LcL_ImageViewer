"""Native numeric/ratio/nearest, vertical flip, cancellation and source-safety checks.
Run serially, never alongside another desktop-input test.
"""
from pathlib import Path
from ctypes import wintypes as wt
import argparse,json,os,time
from PIL import Image
import neumorphic_smoke as qa
from inline_editor_smoke import InlineViewer

def run(args):
    args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron';before=normal.read_bytes() if normal.exists() else None
    foreground,cursor=qa.u.GetForegroundWindow(),wt.POINT();qa.u.GetCursorPos(qa.c.byref(cursor))
    f=args.output/'fixtures';f.mkdir();im=Image.new('RGBA',(512,256));im.putdata([(x%256,y,32+(x//256)*100,0 if x<64 and y<64 else (128 if x>400 else 255)) for y in range(256) for x in range(512)])
    source=f/'source.png';im.save(source);digest=qa.sha(source);folder=args.output/'dark-880';folder.mkdir();v=InlineViewer(args,folder,'dark',880,560,source)
    try:
        v.start();v.key('E');time.sleep(.8)
        v.choose('R');v.click(45,61);time.sleep(.8)
        ref=im.transpose(Image.Transpose.ROTATE_90);assert v.export('left90').tobytes()==ref.tobytes()
        v.click(247,61);time.sleep(.8);ref=ref.transpose(Image.Transpose.FLIP_TOP_BOTTOM)
        assert v.export('vertical').tobytes()==ref.tobytes();v.shot('vertical-flip')
        v.click(755,23);time.sleep(.7);v.choose('I')
        v.value(67,61,128);v.shot('numeric-locked-draft','128 x 64');v.apply();assert v.export('numeric-locked').size==(128,64)
        v.click(755,23);time.sleep(.7);v.choose('I');v.click(257,61)
        v.value(67,61,100);v.value(158,61,70)
        v.click(510,61);v.shot('sampling-menu');v.click(500,110);v.click(620,542)
        v.shot('nearest-numeric-draft');v.apply();out=v.export('nearest-numeric')
        assert out.size==(100,70),out.size
        # Exact rational pixel-center nearest: avoid Pillow's floating tie at x=64.
        expected=Image.new('RGBA',(100,70))
        expected.putdata([im.getpixel((min(511,((2*x+1)*512)//200),min(255,((2*y+1)*256)//140))) for y in range(70) for x in range(100)])
        assert out.tobytes()==expected.tobytes()
        # Draft changes must not silently follow the user to another tool.
        v.click(755,23);time.sleep(.7);v.choose('I');v.value(67,61,200);v.choose('R')
        v.shot('unapplied-tool-switch','Apply/switch, discard, or continue; no image mutation yet')
        v.key(27);v.shot('switch-cancelled');v.key(27);time.sleep(.25)
        assert v.export('cancel-draft').tobytes()==im.tobytes()
        # Precise transparent crop retains hidden RGB, not composited gray squares.
        v.choose('C');v.value(214,61,64);v.value(310,61,64);v.apply()
        assert v.export('transparent-crop').tobytes()==im.crop((0,0,64,64)).tobytes()
        v.ctrl('Z');v.choose('I');v.value(67,61,20000);v.shot('invalid-size','Error, Apply dimensions and save disabled')
        v.key(27);v.choose('R');v.click(110,61);time.sleep(.7);v.click(45,23);v.shot('unsaved-exit-confirm')
        v.key(27);v.shot('continue-after-exit-confirm')
        assert qa.sha(source)==digest
        result={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'captures':len(v.captures),
          'left90_vertical_exact':True,'numeric_ratio_and_unlocked_resize':True,'nearest_exact':True,
          'draft_switch_cancel_safety':True,'transparent_crop_exact':True,'oversize_guard_visible':True,'normal_preferences_unchanged':True,'source_hash_unchanged':True}
    finally:
        v.cleanup();assert (normal.read_bytes() if normal.exists() else None)==before
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    (args.output/'report.json').write_text(json.dumps(result,indent=2),'utf-8');print(json.dumps(result),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--commit',required=True);run(p.parse_args())

"""Native main-canvas editor QA. Uses only generated images and isolated profiles.
Pixel exports are checked independently with Pillow, never by mocking the GUI.
"""
from pathlib import Path
from ctypes import wintypes as wt
import argparse, json, os, time, math
from PIL import Image, ImageChops
from basic_editor_smoke import EditViewer, combo, text
import neumorphic_smoke as qa

class InlineViewer(EditViewer):
    def save_dialog(self,x,y,path,cancel=False):
        assert path.parent.resolve().is_relative_to(self.args.output.resolve()) and not path.exists()
        self.click(x,y)
        deadline=time.monotonic()+12;dialog=None
        while time.monotonic()<deadline:
            handles=[h for h in qa.windows_for_pid(self.proc.pid) if h!=self.hwnd]
            if len(handles)==1:dialog=handles[0];break
            time.sleep(.1)
        if dialog is None:raise RuntimeError('No owned native save dialog')
        qa.focus(dialog)
        if cancel:
            qa.key(dialog,27);time.sleep(.5);assert not path.exists();return
        combo(dialog,0x12,ord('N'));combo(dialog,0x11,ord('A'))
        # One atomic Unicode batch minimizes focus races while filling this owned dialog.
        qa.focus(dialog)
        assert dialog in qa.windows_for_pid(self.proc.pid) and qa.u.GetForegroundWindow()==dialog
        units=[int.from_bytes(str(path).encode('utf-16-le')[i:i+2],'little') for i in range(0,len(str(path).encode('utf-16-le')),2)]
        batch=(qa.NativeInput*(len(units)*2))(*[qa.NativeInput(1,qa.InputUnion(keyboard=qa.KeyInput(0,u,flag,0,0))) for u in units for flag in (4,6)])
        assert qa.u.SendInput(len(batch),batch,qa.c.sizeof(qa.NativeInput))==len(batch)
        assert qa.u.GetForegroundWindow()==dialog
        qa.key(dialog,13)
        deadline=time.monotonic()+20
        while time.monotonic()<deadline:
            if path.exists() and dialog not in qa.windows_for_pid(self.proc.pid):
                try:
                    with Image.open(path) as image:image.load()
                    break
                except OSError:pass
            time.sleep(.15)
        else:raise RuntimeError('No complete PNG written by native save workflow')
        time.sleep(.4);qa.focus(self.hwnd)
    def choose(self,key):
        self.click(self.width-260,self.height-18)
        self.key(key);time.sleep(0.25)
    def value(self,x,y,value):
        self.click(x,y);combo(self.hwnd,0x11,ord('A'));text(self.hwnd,value);self.key(9)
        self.click(self.width-260,self.height-18);time.sleep(0.2)
    def apply(self):
        self.click(self.width-260,self.height-18);self.key(13);time.sleep(0.75)
    def export(self,name):
        p=self.folder/(name+'.png')
        self.shot('before-export-'+name)
        # Locked winit suppresses same-position CursorMoved after a native modal;
        # returning the pointer with a real 2px motion models normal dialog usage.
        # It does not call a save function or bypass the actual button/dialog.
        qa.move(self.hwnd,self.width-78,self.height-47)
        try:
            self.save_dialog(self.width-80,self.height-47,p)
        except Exception:
            self.shot('failed-export-'+name)
            print('FAILED_DIALOG_WINDOWS',qa.windows_for_pid(self.proc.pid), 'INPUT',self.actions[-5:],flush=True)
            raise
        return Image.open(p).convert('RGBA')
    def ctrl(self,key):
        combo(self.hwnd,0x11,ord(key));time.sleep(0.6)

def drag(v,start,end,button=2,up=4):
    qa.move(v.hwnd,*start);qa.u.mouse_event(button,0,0,0,0)
    try:
        _,_,origin,dpi=qa.geometry(v.hwnd)
        for n in range(1,21):
            x=start[0]+(end[0]-start[0])*n/20;y=start[1]+(end[1]-start[1])*n/20
            qa.u.SetCursorPos(origin.x+round(x*dpi/96),origin.y+round(y*dpi/96));time.sleep(.025)
    finally:qa.u.mouse_event(up,0,0,0,0)
    time.sleep(.25)

def run(args):
    args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron';before=normal.read_bytes() if normal.exists() else None
    foreground,cursor=qa.u.GetForegroundWindow(),wt.POINT();qa.u.GetCursorPos(qa.c.byref(cursor))
    f=args.output/'fixtures';f.mkdir();im=Image.new('RGBA',(512,256))
    im.putdata([(x%256,y,32+(x//256)*100,0 if x<64 and y<64 else (128 if x>400 else 255)) for y in range(256) for x in range(512)])
    source=f/'coordinate-source.png';im.save(source);digest=qa.sha(source);reports=[]
    try:
        for theme,w,h in [('light',880,560),('dark',1280,860),('dark',880,560),('light',1280,860)]:
            folder=args.output/f'{theme}-{w}';folder.mkdir();v=InlineViewer(args,folder,theme,w,h,source)
            try:
                v.start();v.key('1');v.key('E');time.sleep(1.0)
                v.shot('main-canvas','Same native viewer, full central canvas, no editor window')
                assert len(qa.windows_for_pid(v.proc.pid))==1
                v.choose('C');v.shot('crop-tools')
                v.choose('R');v.shot('rotate-tools')
                v.choose('I');v.shot('resize-tools')
                if args.calibrate:
                    reports.append({'theme':theme,'width':w,'captures':len(v.captures),'calibration':True});continue
                # Filled after inspecting this build's screenshots. Exports are the actual pixel oracle.
                v.choose('R');v.click(128,61);time.sleep(.75)
                v.shot('right-90')
                exported=v.export('right-90-result');ref=im.transpose(Image.Transpose.ROTATE_270)
                assert exported.size==ref.size and exported.tobytes()==ref.tobytes(),('right-90',exported.size,ref.size)
                v.click(180,61);time.sleep(.75);exported=v.export('horizontal-result');ref=ref.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
                assert exported.tobytes()==ref.tobytes()
                v.shot('horizontal-flip');v.ctrl('Z');exported=v.export('undo-result');ref=im.transpose(Image.Transpose.ROTATE_270)
                assert exported.tobytes()==ref.tobytes();v.ctrl('Y');exported=v.export('redo-result');ref=ref.transpose(Image.Transpose.FLIP_LEFT_RIGHT)
                assert exported.tobytes()==ref.tobytes()
                # Zoom and pan are view-only, even when a document is already rotated.
                v.choose('V');v.key('0');time.sleep(.2);v.shot('view-100')
                qa.move(v.hwnd,w/2,h/2);qa.u.mouse_event(0x0800,0,0,240,0);time.sleep(.3)
                drag(v,(w/2,h/2),(w/2+60,h/2+35));v.shot('zoomed-and-panned')
                exported=v.export('zoom-does-not-resample');assert exported.tobytes()==ref.tobytes()
                v.key('F');v.choose('I');v.shot('resize-before')
                v.click(316,61);v.shot('resize-half-draft');v.apply();time.sleep(.4)
                v.shot('resize-applied');exported=v.export('resized-result');assert exported.size==(128,256),exported.size
                # Return to the original for an exact crop through mouse handles.
                v.click(w-125,23);time.sleep(.7);v.choose('C');v.key('0');time.sleep(.4)
                drag(v,(w/2-190,h/2-70),(w/2+100,h/2+60));v.shot('crop-drag')
                v.apply();exported=v.export('crop-result')
                px=exported.getpixel((0,0));x=px[0]+((px[2]-32)//100)*256;y=px[1]
                assert 1<exported.width<512 and 1<exported.height<256
                refcrop=im.crop((x,y,x+exported.width,y+exported.height));assert exported.tobytes()==refcrop.tobytes()
                v.shot('cropped-main-canvas');v.ctrl('Z');assert v.export('undo-crop-result').tobytes()==im.tobytes()
                # Arbitrary-angle preview and transparent expanded bounds.
                v.choose('R');v.value(365,61,30);time.sleep(.8);v.shot('angle-30-draft');v.apply()
                exported=v.export('angle-30-result');angle=math.radians(30)
                assert exported.size==(math.ceil(512*math.cos(angle)+256*math.sin(angle)),math.ceil(512*math.sin(angle)+256*math.cos(angle)))
                assert exported.getpixel((0,0))[3]==0;assert exported.getextrema()[3][1]>0
                v.shot('angle-30-applied')
                v.choose('V');v.save_dialog(w-80,h-47,folder/'cancelled.png',cancel=True);v.shot('cancel-keeps-editor')
                assert qa.sha(source)==digest
                reports.append({'theme':theme,'width':w,'captures':len(v.captures),'quarter_mirror_undo_redo_exact':True,
                    'view_zoom_preserves_export':True,'resize_and_mouse_crop':True,'arbitrary_angle_transparency':True,'source_unchanged':True})
            finally:v.cleanup()
            print('PASS',theme,w,flush=True)
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==before
        assert qa.sha(source)==digest
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    result={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'cases':reports,'normal_preferences_unchanged':True}
    (args.output/'report.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),'utf-8');print(json.dumps(result,ensure_ascii=False),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--commit',required=True);p.add_argument('--calibrate',action='store_true');run(p.parse_args())

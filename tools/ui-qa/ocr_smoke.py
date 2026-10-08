"""One native OCR request through the real viewer, followed by cached-result UI QA.
Generated text images only. Clipboard HGLOBAL formats are restored when still owned
by the test; uncopyable formats or externally changed clipboard cause a safe stop.
Use --commands JSON for replay, or feed JSON commands on stdin for initial calibration.
"""
from pathlib import Path
from ctypes import wintypes as wt
import argparse,ctypes as c,json,os,re,sys,time
from PIL import Image,ImageDraw,ImageFont
import neumorphic_smoke as qa
from pixel_readout_smoke import Viewer
from basic_editor_smoke import combo,text

u,k=qa.u,c.WinDLL('kernel32',use_last_error=True)
u.OpenClipboard.argtypes=[wt.HWND];u.CloseClipboard.argtypes=[]
u.GetClipboardData.argtypes=[wt.UINT];u.GetClipboardData.restype=wt.HANDLE
u.SetClipboardData.argtypes=[wt.UINT,wt.HANDLE];u.SetClipboardData.restype=wt.HANDLE
u.EnumClipboardFormats.argtypes=[wt.UINT];u.EnumClipboardFormats.restype=wt.UINT
k.GlobalSize.argtypes=[wt.HGLOBAL];k.GlobalSize.restype=c.c_size_t
k.GlobalLock.argtypes=[wt.HGLOBAL];k.GlobalLock.restype=c.c_void_p
k.GlobalUnlock.argtypes=[wt.HGLOBAL]
k.GlobalAlloc.argtypes=[wt.UINT,c.c_size_t];k.GlobalAlloc.restype=wt.HGLOBAL
k.GlobalFree.argtypes=[wt.HGLOBAL];k.GlobalFree.restype=wt.HGLOBAL

def open_clip(hwnd=None):
    for _ in range(30):
        if u.OpenClipboard(hwnd):return
        time.sleep(.03)
    raise RuntimeError('Clipboard busy; refusing to clear it')
def raw_clip(fmt):
    h=u.GetClipboardData(fmt)
    if not h:return None
    n=k.GlobalSize(h)
    if n<=0 or n>16*1024*1024:raise RuntimeError('Clipboard format cannot be safely snapshotted')
    ptr=k.GlobalLock(h)
    if not ptr:raise RuntimeError('Clipboard lock failed')
    try:return c.string_at(ptr,n)
    finally:k.GlobalUnlock(h)
def get_text():
    open_clip()
    try:
        raw=raw_clip(13)
        return raw.decode('utf-16-le').split('\0',1)[0] if raw else None
    finally:u.CloseClipboard()
class ClipboardBackup:
    def __init__(self):
        self.data={};self.restored=False;self.sequence=u.GetClipboardSequenceNumber();open_clip()
        try:
            fmt=0
            while True:
                fmt=u.EnumClipboardFormats(fmt)
                if not fmt:break
                if fmt in (2,3,9,14,128,130,131,142):raise RuntimeError('Non-memory clipboard format: stop instead of losing user clipboard data')
                self.data[fmt]=raw_clip(fmt)
            assert sum(len(d or b'') for d in self.data.values())<=16*1024*1024
        finally:u.CloseClipboard()
    def restore(self,hwnd,owned):
        # Do not overwrite a new clipboard value the user copied during this QA.
        current=get_text()
        if not owned:
            self.restored=u.GetClipboardSequenceNumber()==self.sequence
            return self.restored
        if current not in owned:return False
        open_clip(hwnd)
        try:
            assert u.EmptyClipboard()
            for fmt,data in self.data.items():
                if data is None:continue
                h=k.GlobalAlloc(2,len(data));assert h
                ptr=k.GlobalLock(h);assert ptr;c.memmove(ptr,data,len(data));k.GlobalUnlock(h)
                if not u.SetClipboardData(fmt,h):k.GlobalFree(h);raise RuntimeError('Clipboard restoration failed')
            self.restored=all(raw_clip(fmt)==data for fmt,data in self.data.items())
            return self.restored
        finally:u.CloseClipboard()

def chord(hwnd,*keys):
    qa.focus(hwnd)
    for key in keys[:-1]:u.keybd_event(key,0,0,0)
    try:qa.key(hwnd,keys[-1])
    finally:
        for key in reversed(keys[:-1]):u.keybd_event(key,0,2,0)
    time.sleep(.25)
def run(args):
    args.binary=args.binary.resolve(strict=True);out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    args.output=out
    qa.bind(u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    original=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron';prefs=original.read_bytes() if original.exists() else None
    fg,cursor=u.GetForegroundWindow(),wt.POINT();u.GetCursorPos(c.byref(cursor))
    im=Image.new('RGBA',(1120,430),'white');d=ImageDraw.Draw(im);font=ImageFont.truetype('C:/Windows/Fonts/msyh.ttc',48)
    for y,line in [(50,'图片文字识别测试'),(160,'Game UI 12345'),(270,'Alpha RGBA 255')]:d.text((65,y),line,font=font,fill=(25,40,65,255))
    source=out/'OCR_文字识别示例.png';im.save(source);digest=qa.sha(source)
    folder=out/'native';folder.mkdir();v=Viewer(args,folder,'dark',880,560,source)
    settings=json.loads(v.storage.read_text('utf-8'));settings['iv-ocr-language']='zh-Hans-CN';v.storage.write_text(json.dumps(settings),'utf-8')
    backup=None;owned=set();steps=[];result={};success=False
    try:
        v.start();v.key('F');v.key('1');time.sleep(.25);v.shot('viewer-before-ocr')
        backup=ClipboardBackup()
        chord(v.hwnd,0x11,0x10,ord('C')) # Actual recognize-and-copy shortcut, one OCR request.
        deadline=time.monotonic()+55;recognized=None
        while time.monotonic()<deadline:
            value=get_text()
            if value and '12345' in value:recognized=value;break
            assert v.proc.poll() is None
            time.sleep(.15)
        v.shot('ocr-result-first')
        assert recognized is not None,'No recognized text reached the real OS clipboard'
        owned.add(recognized);flat=re.sub(r'\s+','',recognized)
        assert '文字识别' in flat and 'game' in flat.lower() and '12345' in flat,recognized
        result={'native_request_count':1,'native_chinese_english_recognition':True,'recognized_fixture_text':recognized,'accuracy_note':'Native OCR may confuse similar Latin letters; results remain editable','clipboard_copy_verified':True}
        print('NATIVE_OCR_PASS; ready for UI commands',flush=True)
        commands=iter(json.loads(args.commands.read_text('utf-8'))) if args.commands else (json.loads(line) for line in sys.stdin if line.strip())
        for command in commands:
            if command.get('finish'):success=True;break
            if 'click' in command:v.click(*command['click'])
            if 'move' in command:qa.move(v.hwnd,*command['move'])
            if 'key' in command:v.key(command['key'])
            if 'chord' in command:chord(v.hwnd,*command['chord'])
            if 'type' in command:text(v.hwnd,command['type'])
            if 'theme' in command and command['theme']!=v.theme:
                v.click(v.width-250,99);v.key('T');v.theme=command['theme'];time.sleep(.4)
            if 'size' in command:
                v.width,v.height=command['size'];wr,cr,_,dpi=qa.geometry(v.hwnd);dw,dh=wr.right-wr.left-cr.right,wr.bottom-wr.top-cr.bottom
                assert u.SetWindowPos(v.hwnd,None,40,40,round(v.width*dpi/96)+dw,round(v.height*dpi/96)+dh,0x40);time.sleep(.5)
            if command.get('copy_cached'):chord(v.hwnd,0x11,0x10,ord('C'))
            time.sleep(command.get('wait',.25))
            if 'expect_clipboard' in command:
                value=get_text();expect=command['expect_clipboard']
                expected=recognized if expect=='original' else (' '.join(recognized.split()) if expect=='single_line' else expect)
                assert value and value.replace('\r\n','\n')==expected.replace('\r\n','\n'),'Clipboard differs from the controlled test result'
                owned.add(value)
            if command.get('recognition_started'):result['native_request_count']+=1
            if 'expect_contains' in command:
                value=get_text() or '';owned.add(value);flat_value=re.sub(r'\s+','',value).lower()
                assert all(re.sub(r'\s+','',part).lower() in flat_value for part in command['expect_contains']), 'Expected controlled English text not found'
                result['english_engine_checked']=True
            if 'shot' in command:v.shot(command['shot'])
            steps.append(command)
            (out/'replay.json').write_text(json.dumps(steps+[{'finish':True}],ensure_ascii=False,indent=2),'utf-8')
            print('UI_STEP',command.get('shot',len(steps)),flush=True)
    finally:
        # Cancellation completes before destroying our QA-only window; no queued copy after restore.
        if v.proc and v.proc.poll() is None:
            v.key(27);time.sleep(.25)
            if backup:
                restored=backup.restore(v.hwnd,owned)
                result['clipboard_original_formats_restored']=restored
        v.cleanup();u.SetCursorPos(cursor.x,cursor.y)
        if fg:u.SetForegroundWindow(fg)
        assert (original.read_bytes() if original.exists() else None)==prefs
        assert qa.sha(source)==digest
        result.update(result='PASS' if success else 'INCOMPLETE',commit=args.commit,binary_sha256=qa.sha(args.binary),captures=v.captures,
            normal_preferences_unchanged=True,source_hash_unchanged=True)
        (out/'report.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),'utf-8')
    print(json.dumps(result,ensure_ascii=False),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--commit',required=True);p.add_argument('--commands',type=Path);run(p.parse_args())

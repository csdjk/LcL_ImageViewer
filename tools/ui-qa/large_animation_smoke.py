"""Real Windows large-animation playback/stepping/edit-export and memory checks.
Uses generated GIF/APNG/WebP files, QA-only profiles/processes, no user file changes.
"""
from pathlib import Path
from ctypes import wintypes as wt
import argparse, ctypes as c, json, os, threading, time
from PIL import Image
from inline_editor_smoke import InlineViewer
import neumorphic_smoke as qa

class Memory(c.Structure):
    _fields_=[('cb',wt.DWORD),('PageFaultCount',wt.DWORD)]+[(n,c.c_size_t) for n in
        ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage',
         'QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage','PrivateUsage']]

def run(args):
    args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    os.environ['LCL_IV_PERF']='1'
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    before=normal.read_bytes() if normal.exists() else None
    foreground=qa.u.GetForegroundWindow();cursor=wt.POINT();qa.u.GetCursorPos(c.byref(cursor))
    kernel=c.WinDLL('kernel32',use_last_error=True);psapi=c.WinDLL('psapi',use_last_error=True)
    kernel.OpenProcess.argtypes=[wt.DWORD,wt.BOOL,wt.DWORD];kernel.OpenProcess.restype=wt.HANDLE
    kernel.CloseHandle.argtypes=[wt.HANDLE]
    psapi.GetProcessMemoryInfo.argtypes=[wt.HANDLE,c.POINTER(Memory),wt.DWORD];psapi.GetProcessMemoryInfo.restype=wt.BOOL
    cases=[]
    try:
        for ext,theme,w,h in [('gif','dark',880,560),('png','light',1280,860),('webp','dark',1280,860),('gif','light',880,560)]:
            folder=args.output/f'{ext}-{theme}-{w}';folder.mkdir()
            source=args.fixtures/ext/('large-animation.'+ext);source_hash=qa.sha(source)
            expected=json.loads((source.parent/'expected.json').read_text('utf-8'))
            v=InlineViewer(args,folder,theme,w,h,source)
            v.start();pid=v.proc.pid
            handle=kernel.OpenProcess(0x0410,False,pid);assert handle
            stopped=threading.Event();samples=[]
            def monitor():
                while not stopped.is_set():
                    m=Memory();m.cb=c.sizeof(m)
                    if psapi.GetProcessMemoryInfo(handle,c.byref(m),m.cb):
                        samples.append({'private':m.PrivateUsage,'working_set':m.WorkingSetSize,'peak_working_set':m.PeakWorkingSetSize,'peak_commit':m.PeakPagefileUsage})
                    stopped.wait(.05)
            thread=threading.Thread(target=monitor,daemon=True);thread.start()
            def frame_index(name):
                p=v.shot(name)
                with Image.open(p) as im: color=im.convert('RGB').getpixel((w//2,h//2))
                matches=[i for i,ex in enumerate(expected) if max(abs(a-b) for a,b in zip(color,ex['center'][:3]))<=2]
                assert len(matches)==1,(name,color,matches)
                return matches[0]
            try:
                log=folder/'runtime-1.log';deadline=time.monotonic()+40
                while '\tdecode_ready\t' not in log.read_text('utf-8',errors='replace'):
                    assert v.proc.poll() is None and time.monotonic()<deadline
                    time.sleep(.1)
                time.sleep(.8);v.key('F');v.key('5')
                a=frame_index('playing-a');time.sleep(.35);b=frame_index('playing-b');assert a!=b
                v.key(32);paused=frame_index('paused');time.sleep(.4);assert frame_index('paused-stable')==paused
                v.key(190);next_frame=frame_index('next-frame');assert next_frame==(paused+1)%72
                v.key(188);assert frame_index('previous-frame')==paused
                v.key('1');p=v.shot('red-channel')
                with Image.open(p) as im: gray=im.convert('RGB').getpixel((w//2,h//2))
                assert max(abs(x-expected[paused]['center'][0]) for x in gray)<=2
                # Editing the mapped current frame must export original RGBA, not gray.
                v.key('E');time.sleep(.5);v.shot('editor-current-frame')
                output=folder/'current-frame.png';qa.move(v.hwnd,w-62,h-72)
                v.save_dialog(w-64,h-72,output)
                with Image.open(output) as im:
                    assert im.size==(1024,1024)
                    rgba=im.convert('RGBA');exported=rgba.getpixel((512,512))
                    assert max(abs(a-b) for a,b in zip(exported,expected[paused]['center']))<=(2 if ext=='webp' else 0)
                    assert rgba.getpixel((950,60))[3]==0
                v.shot('saved-current-frame');v.key(27);time.sleep(.4)
                assert qa.sha(source)==source_hash
                assert samples
                cases.append({'format':ext,'theme':theme,'logical_client':[w,h],
                    'frames':72,'decoded_frame_mib':288,'autoplay':True,'pause_stable':True,
                    'previous_next_correct':True,'channel_and_export_correct':True,
                    'private_mib_max':round(max(s['private'] for s in samples)/1048576,2),
                    'working_set_mib_max':round(max(s['working_set'] for s in samples)/1048576,2),
                    'peak_commit_mib':round(max(s['peak_commit'] for s in samples)/1048576,2),
                    'captures':len(v.captures),'source_unchanged':True})
            finally:
                stopped.set();thread.join(2);kernel.CloseHandle(handle);v.cleanup()
            cache_files=list(Path(os.environ['TEMP']).glob(f'lcl-iv-animation-{pid}-*.tmp'))
            assert not cache_files, 'Animation cache leaked after process exit'
            cases[-1]['temporary_cache_cleaned']=True
            print('PASS',ext,theme,w,flush=True)
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==before
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    report={'result':'PASS','commit':args.commit,'binary':str(args.binary),'binary_sha256':qa.sha(args.binary),
        'cases':cases,'normal_preferences_unchanged':True,
        'memory_note':'Private/working-set measured separately; 64 MiB is the heap spill threshold, not total process or OS file-cache memory.'}
    (args.output/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),'utf-8')
    print(json.dumps(report,ensure_ascii=False),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',required=True,type=Path)
    p.add_argument('--fixtures',required=True,type=Path);p.add_argument('--output',required=True,type=Path)
    p.add_argument('--commit',required=True);run(p.parse_args())

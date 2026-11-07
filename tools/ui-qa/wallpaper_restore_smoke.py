import ctypes as c, uuid, json, os, subprocess
from pathlib import Path
from PIL import Image
import argparse, time
parser=argparse.ArgumentParser(description="Validate native wallpaper and restore the prior desktop configuration")
for name in ['binary','output','input','actions','commit']:parser.add_argument('--'+name,required=True)
args=parser.parse_args()
root=Path.cwd(); out=root/'Temp/ui-latest';out.mkdir(parents=True,exist_ok=True); ole=c.OleDLL('ole32')
ptr=c.c_void_p(); guid=lambda s:(c.c_ubyte*16).from_buffer_copy(uuid.UUID(s).bytes_le)
ole.CoInitializeEx(None,2)
cls=guid('c2cf3110-460e-4fc1-b9d0-8a1c0c9cc4bd'); iid=guid('b92b56a9-8b55-4e14-9a89-0199bbb6f93b')
ole.CoCreateInstance(c.byref(cls),None,23,c.byref(iid),c.byref(ptr))
vt=c.cast(ptr,c.POINTER(c.POINTER(c.c_void_p))).contents
ole.CoTaskMemFree.argtypes=[c.c_void_p]
def invoke(i,types,*args):
 hr=c.WINFUNCTYPE(c.c_long,c.c_void_p,*types)(vt[i])(ptr,*args)
 if hr<0:raise OSError(hex(hr&0xffffffff))
def number(i):
 v=c.c_uint();invoke(i,[c.POINTER(c.c_uint)],c.byref(v));return v.value
def string(i,types,*args):
 p=c.c_void_p();invoke(i,types+[c.POINTER(c.c_void_p)],*args,c.byref(p))
 try:return c.wstring_at(p) if p.value else ''
 finally:ole.CoTaskMemFree(p)
status=number(17);assert not status&4,'Remote session cannot verify wallpaper'
slides=c.c_void_p();options=c.c_uint();tick=c.c_uint()
if status&2:
 invoke(13,[c.POINTER(c.c_void_p)],c.byref(slides))
 invoke(15,[c.POINTER(c.c_uint),c.POINTER(c.c_uint)],c.byref(options),c.byref(tick))
monitors=[string(5,[c.c_uint],i) for i in range(number(6))]
before={m:string(4,[c.c_wchar_p],m) for m in monitors};position=number(11)
(out/'wallpaper-original.json').write_text(json.dumps({'status':status,'position':position,'paths':before,'slideshow_options':options.value,'slideshow_interval':tick.value},ensure_ascii=False,indent=2),encoding='utf-8')
env=os.environ.copy();env['LOCALAPPDATA']=str(out/'wallpaper-profile');env['PYTHONIOENCODING']='utf-8'
set_passed=False
try:
 subprocess.run(['python','-B','tools/ui-qa/neumorphic_smoke.py','--binary',args.binary,'--output',args.output,'--input',args.input,'--commit',args.commit,'--theme','light','--width','1280','--height','860','--actions',args.actions,'--isolated-profile'],env=env,check=True)
 paths={m:string(4,[c.c_wchar_p],m) for m in monitors}
 assert all(Path(p).is_relative_to(out/'wallpaper-profile') for p in paths.values()),'Wallpaper did not change to isolated generated snapshot'
 for p in set(paths.values()):
  with Image.open(p) as im:assert im.size==Image.open(args.input).size and im.convert('RGB').tobytes()==Image.open(args.input).convert('RGB').tobytes()
 assert number(11)==position
 set_passed=True
 print('PASS native wallpaper and original pixels',flush=True)
finally:
 for m,p in before.items():invoke(3,[c.c_wchar_p,c.c_wchar_p],m,p)
 if status&2:
  invoke(12,[c.c_void_p],slides)
  invoke(14,[c.c_uint,c.c_uint],options.value,tick.value)
 invoke(18,[c.c_int],int(bool(status&1)))
 restored={m:string(4,[c.c_wchar_p],m) for m in monitors}
 deadline=time.monotonic()+5
 while number(17)!=status and time.monotonic()<deadline:time.sleep(.2)
 assert number(11)==position and number(17)==status
 if not status&2:assert restored==before
 else:
  restored_options=c.c_uint();restored_tick=c.c_uint()
  invoke(15,[c.POINTER(c.c_uint),c.POINTER(c.c_uint)],c.byref(restored_options),c.byref(restored_tick))
  assert (restored_options.value,restored_tick.value)==(options.value,tick.value)
  sv=c.cast(slides,c.POINTER(c.POINTER(c.c_void_p))).contents
  c.WINFUNCTYPE(c.c_ulong,c.c_void_p)(sv[2])(slides)
 (out/'wallpaper-result.json').write_text(json.dumps({'native_set_passed':set_passed,'restored':True,'status':status,'position_unchanged':True,'monitors':len(monitors)}),encoding='utf-8')
 print('PASS original desktop restored',flush=True)
 c.WINFUNCTYPE(c.c_ulong,c.c_void_p)(vt[2])(ptr);ole.CoUninitialize()

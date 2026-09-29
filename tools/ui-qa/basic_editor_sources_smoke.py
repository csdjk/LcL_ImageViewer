"""Verify explicit current-animation-frame and highest-Mip editing semantics."""
from pathlib import Path
import argparse
from ctypes import wintypes as wt
import json
import os
import struct
import time
from PIL import Image
from basic_editor_smoke import EditViewer
import neumorphic_smoke as qa


def run(args):
    args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron';previous=normal.read_bytes() if normal.exists() else None
    cursor=wt.POINT();qa.u.GetCursorPos(qa.c.byref(cursor));foreground=qa.u.GetForegroundWindow()
    fixtures=args.output/'fixtures';fixtures.mkdir()
    a=Image.new('RGBA',(512,256),(244,199,123,253));b=Image.new('RGBA',(512,256),(17,88,199,128))
    a.save(fixtures/'two-frames.png',save_all=True,append_images=[b],duration=60000,loop=0,disposal=0,blend=0)
    header=[124,0x2100f,256,512,2048,0,2]+[0]*11+[32,0x41,0,32,0xff,0xff00,0xff0000,0xff000000]+[0x401008,0,0,0,0]
    assert len(header)==31
    (fixtures/'two-mips.dds').write_bytes(b'DDS '+struct.pack('<31I',*header)+a.tobytes()+bytes([11,77,233,64])*(256*128))
    hashes={p.name:qa.sha(p) for p in fixtures.iterdir()};cases=[]
    try:
        for name,expected in [('two-frames.png',b),('two-mips.dds',a)]:
            folder=args.output/name.replace('.','-');folder.mkdir();v=EditViewer(args,folder,'dark',880,560,fixtures/name)
            try:
                v.start()
                if name.endswith('.png'):v.key(32);v.key(190)
                else:v.key(0x26);v.key('0')
                v.shot('selected-frame-or-mip');v.key('E');time.sleep(0.6);v.shot('explicit-source-label')
                path=folder/'exported.png';v.save_dialog(110,493,path)
                with Image.open(path) as out:assert out.size==expected.size and out.convert('RGBA').tobytes()==expected.tobytes()
                v.shot('export-verified');v.key(27);v.close()
                cases.append({'source':name,'exact_export_bytes':True,'captures':len(v.captures)})
            finally:v.cleanup()
    finally:
        assert hashes=={p.name:qa.sha(p) for p in fixtures.iterdir()}
        assert (normal.read_bytes() if normal.exists() else None)==previous
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    report={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'cases':cases,'source_hashes_unchanged':True}
    (args.output/'report.json').write_text(json.dumps(report,indent=2),'utf-8');print(json.dumps(report,indent=2),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True);run(p.parse_args())

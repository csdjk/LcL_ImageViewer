"""Native animation-frame/Mip regressions for bottom pixel readouts.

Run after pixel_readout_smoke.py, never concurrently with another desktop test.
Generated PNG/DDS fixtures and copied AVIF input only; no source data modified.
"""
import argparse
import ctypes as c
from ctypes import wintypes as wt
import json
import os
from pathlib import Path
import shutil
import struct
import time
from PIL import Image, ImageChops
import neumorphic_smoke as qa
from pixel_readout_smoke import Viewer, capsule_extent


def run(args):
    args.binary = args.binary.resolve(strict=True)
    out = args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    original=normal.read_bytes() if normal.exists() else None
    foreground,cursor=qa.u.GetForegroundWindow(),wt.POINT();qa.u.GetCursorPos(c.byref(cursor))
    fixtures=out/'fixtures';fixtures.mkdir()
    frames=[Image.new('RGBA',(512,256),color) for color in ((220,70,110,255),(45,128,220,128),(17,88,199,0))]
    frames[0].save(fixtures/'animated.png',save_all=True,append_images=frames[1:],duration=3000,loop=0,disposal=0,blend=0)
    root=Path(__file__).resolve().parents[2]
    shutil.copy2(root/'crates/iv-core/tests/fixtures/avif/animated-alpha.avif',fixtures/'animated.avif')
    header=[124,0x2100f,256,512,512*4,0,3]+[0]*11+[32,0x41,0,32,0xff,0xff00,0xff0000,0xff000000]+[0x401008,0,0,0,0]
    assert len(header)==31
    payload=b''.join(bytes(color)*(w*h) for color,w,h in [((244,199,123,253),512,256),((17,88,199,128),256,128),((0,255,128,255),128,64)])
    (fixtures/'mips.dds').write_bytes(b'DDS '+struct.pack('<31I',*header)+payload)
    hashes={p.name:qa.sha(p) for p in fixtures.iterdir()}
    reports=[]
    try:
        for file in ('animated.png','animated.avif','mips.dds'):
            if args.source and file != args.source: continue
            folder=out/file.replace('.','-');folder.mkdir()
            v=Viewer(args,folder,'dark',880,560,fixtures/file)
            prefs=json.loads(v.storage.read_text('utf-8'));prefs['iv-pixel-format']='normalized'
            v.storage.write_text(json.dumps(prefs),encoding='utf-8')
            try:
                v.start()
                if file.startswith('animated'):
                    v.key(32) # Pause the auto-playing animation.
                    v.key('1');v.sample();first=v.shot('paused-R')
                    time.sleep(0.5);v.sample();held=v.shot('held-R')
                    v.key(190);v.sample();nxt=v.shot('next-R')
                    v.key(188);v.sample();prior=v.shot('previous-R')
                    roi=(0,490,880,560)
                    def diff(a,b):
                        with Image.open(a) as x,Image.open(b) as y:
                            d=ImageChops.difference(x.crop(roi),y.crop(roi))
                            return max(hi for _,hi in d.getextrema())
                    assert diff(first,held)<=2,('Paused readout changed',file)
                    assert diff(first,prior)<=2,('Previous frame failed to restore readout',file)
                    assert diff(first,nxt)>20,('Next frame did not update readout',file)
                    v.key('5');v.sample();shown=v.shot('full-normalized-with-frame-status')
                    time.sleep(1.4);hidden=v.shot('hidden')
                    capsule_extent(shown,hidden,560)
                    reports.append({'source':file,'frame_readout_updates_and_restores':True,'long_bottom_bar_fits':True})
                else:
                    for mip,red in enumerate((244,17,0)):
                        if mip: v.key(0x26)  # Current viewer maps Up to the next Mip.
                        v.key('F');v.key('1');v.sample();p=v.shot('mip-'+str(mip)+'-R')
                        with Image.open(p).convert('RGB') as im:
                            assert max(abs(x-red) for x in im.getpixel((440,280)))<=3,(mip,im.getpixel((440,280)))
                    v.key('5');v.sample();v.shot('mip-2-normalized')
                    reports.append({'source':file,'mip_samples_and_coordinates_reviewed_separately':True,'mip_render_values':True})
                v.close('normalized')
            finally:
                v.cleanup()
            print('PASS',file,flush=True)
        assert hashes=={p.name:qa.sha(p) for p in fixtures.iterdir()}
    finally:
        assert (normal.read_bytes() if normal.exists() else None)==original
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if foreground:qa.u.SetForegroundWindow(foreground)
    result={'result':'PASS','commit':args.commit,'cases':reports,'source_hashes_unchanged':hashes}
    (out/'report.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result,indent=2),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True)
    p.add_argument('--source',choices=['animated.png','animated.avif','mips.dds'])
    run(p.parse_args())

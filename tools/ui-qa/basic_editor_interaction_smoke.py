"""Native drag, numeric dimensions, undo/redo, nearest export and discard QA.

Only generated coordinate-encoded PNGs are saved. Actual exported bytes are
compared with independently cropped/nearest-resized Pillow reference images.
"""
from pathlib import Path
from types import SimpleNamespace
import argparse
import ctypes as c
from ctypes import wintypes as wt
import json
import os
import time
from PIL import Image
import neumorphic_smoke as qa
from basic_editor_smoke import EditViewer


def drag(v, start, end):
    qa.move(v.hwnd,*start)
    _,_,origin,dpi=qa.geometry(v.hwnd)
    qa.u.mouse_event(2,0,0,0,0)
    try:
        time.sleep(0.08)
        for step in range(1,21):
            assert qa.u.GetForegroundWindow()==v.hwnd
            x=start[0]+(end[0]-start[0])*step/20
            y=start[1]+(end[1]-start[1])*step/20
            qa.u.SetCursorPos(origin.x+round(x*dpi/96),origin.y+round(y*dpi/96))
            time.sleep(0.02)
    finally:qa.u.mouse_event(4,0,0,0,0)
    time.sleep(0.25)


def run(args):
    args.binary=args.binary.resolve(strict=True);args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    qa.bind(qa.u,'SetProcessDpiAwarenessContext',[qa.PTR],wt.BOOL)(qa.PTR(-4))
    normal=Path(os.environ['APPDATA'])/'LcL ImageViewer/data/app.ron'
    before=normal.read_bytes() if normal.exists() else None
    old_foreground=qa.u.GetForegroundWindow();cursor=wt.POINT();qa.u.GetCursorPos(c.byref(cursor))
    im=Image.new('RGBA',(512,256));im.putdata([(x%256,y,32+(x//256)*100,0 if x<64 and y<64 else (128 if x>400 else 255)) for y in range(256) for x in range(512)])
    source=args.output/'source.png';im.save(source);source_hash=qa.sha(source)
    folder=args.output/'interactions';folder.mkdir();v=EditViewer(args,folder,'dark',880,560,source)
    outputs=[]
    def save(name):
        p=folder/(name+'.png');v.save_dialog(110,493,p);outputs.append(p.name)
        with Image.open(p) as output:return output.convert('RGBA')
    def crop_data(output):
        r,g,b,_=output.getpixel((0,0));x=r+256*((b-32)//100);y=g;w,h=output.size
        assert 0<=x<512 and 0<=y<256 and x+w<=512 and y+h<=256
        assert output.tobytes()==im.crop((x,y,x+w,y+h)).tobytes(), 'GUI crop not byte-exact'
        return x,y,w,h
    try:
        v.start();v.key('E');time.sleep(0.6);v.shot('initial')
        drag(v,(288,227),(510,338));v.shot('drag-selection')
        cropped=save('selection');first=crop_data(cropped)
        assert abs(first[2]-256)<5 and abs(first[3]-128)<5,first
        drag(v,(400,280),(428,288));v.shot('moved-selection')
        moved=save('moved');second=crop_data(moved)
        assert second[2:]==first[2:] and second[0]>first[0] and second[1]>first[1],(first,second)
        scale=222/256
        corner=(218+(second[0]+second[2])*scale-2,193+(second[1]+second[3])*scale-2)
        drag(v,corner,(corner[0]+18,corner[1]+10));v.shot('corner-adjustment')
        resized_crop=save('corner');third=crop_data(resized_crop)
        assert third[:2]==second[:2] and third[2]>second[2] and third[3]>second[3],(second,third)
        v.click(735,64);undo=save('undo');assert undo.tobytes()==moved.tobytes() and undo.size==moved.size
        v.click(779,64);redo=save('redo');assert redo.tobytes()==resized_crop.tobytes() and redo.size==resized_crop.size
        v.click(120,114)
        v.field(80,140,160);time.sleep(0.5);v.shot('locked-dimensions')
        locked=save('locked');expected_height=max(1,(160*third[3]+third[2]//2)//third[2]);assert locked.size==(160,expected_height),locked.size
        v.click(227,140);v.field(165,140,96);time.sleep(0.5);v.shot('unlocked-dimensions')
        v.click(330,168);v.shot('sampling-menu');v.click(267,217);time.sleep(0.5);v.shot('nearest-selected')
        nearest=save('nearest');reference=resized_crop.resize((160,96),Image.Resampling.NEAREST)
        assert nearest.size==(160,96),nearest.size
        assert nearest.tobytes()==reference.tobytes(),'Nearest dropdown/export did not match the reference'
        v.field(80,140,20000);v.shot('dimension-limit')
        v.click(110,493);time.sleep(0.3)
        assert qa.windows_for_pid(v.proc.pid)==[v.hwnd], 'Invalid dimensions opened a save dialog'
        v.field(80,140,161);v.key(27);v.shot('discard-confirmation')
        v.click(285,493);v.shot('return-to-editor')
        v.key(27);v.click(210,493);time.sleep(0.3);v.shot('discarded-back-to-viewer')
        v.close()
        assert qa.sha(source)==source_hash
    finally:
        v.cleanup()
        assert (normal.read_bytes() if normal.exists() else None)==before
        qa.u.SetCursorPos(cursor.x,cursor.y)
        if old_foreground:qa.u.SetForegroundWindow(old_foreground)
    report={'result':'PASS','commit':args.commit,'binary_sha256':qa.sha(args.binary),'captures':len(v.captures),
        'crop_rects':[first,second,third],'drag_move_corner':True,'undo_redo':True,'numeric_dimensions':True,
        'aspect_locked_and_unlocked':True,'nearest_pixels_exact':True,'dimension_limit':True,
        'source_unchanged':True,'exports':outputs}
    (args.output/'report.json').write_text(json.dumps(report,indent=2),'utf-8')
    print(json.dumps(report,indent=2),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True);run(p.parse_args())

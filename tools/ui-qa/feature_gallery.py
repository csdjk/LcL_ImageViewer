"""Build README illustrations from actual native viewer captures, not UI mockups.
Only project-owned icon artwork and generated format fixtures are used. Captions sit
outside screenshots; no UI or image contents are painted over. Raw captures/metadata
remain in the new ignored output directory. Run GUI jobs serially on one desktop.
"""
from __future__ import annotations
import argparse, hashlib, json, math, shutil, struct, subprocess, time
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageDraw, ImageFont
import neumorphic_smoke as qa


def fixtures(root: Path, out: Path) -> dict[str, Path]:
    out.mkdir(parents=True, exist_ok=False)
    art = Image.open(root/'crates/iv-viewer/assets/icon.png').convert('RGBA')
    art = art.resize((384,384), Image.Resampling.LANCZOS)
    paths = {}
    for key in ['rgba','view','pin','palette','file','theme','probe']:
        folder=out/key;folder.mkdir()
        paths[key]=folder/'Material_RGBA.png';art.save(paths[key])
    browse=out/'browse';(browse/'Effects').mkdir(parents=True)
    art.save(browse/'01_Icon.png');art.transpose(Image.Transpose.FLIP_LEFT_RIGHT).save(browse/'Effects/02_Effect.png')
    paths['browse']=browse/'01_Icon.png'
    # A short transparent AVIF animation made only from this project's own icon.
    folder=out/'animation';folder.mkdir();frames=[]
    for i in range(24):
        phase=2*math.pi*i/24; im=Image.new('RGBA',(512,384),(0,0,0,0))
        size=round(236+20*math.sin(phase));sprite=art.resize((size,size),Image.Resampling.LANCZOS)
        im.alpha_composite(sprite,(round((512-size)/2+66*math.sin(phase)),round((384-size)/2-24*math.cos(phase))))
        frames.append(im)
    paths['animation']=folder/'Sprite_Loop.avif'
    frames[0].save(paths['animation'],save_all=True,append_images=frames[1:],duration=[100]*24,loop=0,quality=90,speed=8)
    # True RGBA8 DDS with five mip levels, not separate resized images pretending to be mips.
    folder=out/'mip';folder.mkdir();paths['mip']=folder/'Texture_Mipmaps.dds'
    levels=[art.resize((128>>i,128>>i),Image.Resampling.LANCZOS) for i in range(5)]
    header=[124,0x2100F,128,128,512,0,5]+[0]*11+[32,0x41,0,32,0x000000FF,0x0000FF00,0x00FF0000,0xFF000000]+[0x401008,0,0,0,0]
    assert len(header)==31
    paths['mip'].write_bytes(b'DDS '+struct.pack('<31I',*header)+b''.join(x.tobytes() for x in levels))
    folder=out/'hdr';folder.mkdir();paths['hdr']=folder/'Lighting_HDR.hdr';rgbe=bytearray()
    for y in range(64):
        for x in range(128):
            light=0.18+12*math.exp(-((x-77)**2+(y-28)**2)/220)
            rgb=[light*(0.3+x/150),light*(0.15+y/80),light*0.4];maximum=max(rgb)
            m,e=math.frexp(maximum);scale=m*256/maximum
            rgbe.extend([min(255,int(v*scale)) for v in rgb]+[e+128])
    paths['hdr'].write_bytes(b'#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y 64 +X 128\n'+rgbe)
    (out/'manifest.json').write_text(json.dumps({str(p.relative_to(out)):qa.sha(p) for p in out.rglob('*') if p.is_file()},indent=2),'utf-8')
    return paths


def actions_for(name: str, width: int=880) -> list[dict]:
    a=[]
    def key(code):a.append({'kind':'key','code':code})
    def click(x,y):a.append({'kind':'click','x':x,'y':y})
    def wait(n=.25):a.append({'kind':'wait','seconds':n})
    def move(x=32,y=135):a.extend([{'kind':'move','x':x,'y':y},{'kind':'move','x':x+4,'y':y+1}])
    def shot(n):move();wait(.2);a.append({'kind':'shot','name':n})
    key('0');key('5')
    if name=='rgba':
        for k,n in [('5','rgba'),('1','red'),('2','green'),('3','blue'),('4','alpha'),('O','rgb-opaque')]:key(k);shot(n)
    elif name=='view':
        key('B');shot('bounds');key('N')
        a.append({'kind':'wheel','x':440,'y':280,'delta':480});shot('zoom-nearest')
        a.append({'kind':'drag','x':430,'y':310,'dx':85,'dy':35,'expect_window_delta':[0,0,0,0]});shot('pan')
    elif name=='browse':
        key('S');wait(.7);key('D');wait(.5);key('0');shot('subfolder')
    elif name=='pin':
        move();click(693,34);wait();a.append({'kind':'assert-topmost','value':True});a.append({'kind':'assert-z-order','value':True})
        move(692,34);wait(.65);a.append({'kind':'shot','name':'pinned'})
    elif name=='palette':
        move();click(611,34);wait();shot('background-menu');click(714,275);wait()
        click(795,268);click(735,144);move(40,140);wait();a.append({'kind':'shot','name':'palette'})
    elif name=='file':
        move();a.append({'kind':'right-click','x':120,'y':140});wait();a.append({'kind':'shot','name':'file-menu'});key(27)
        key(46);wait();a.append({'kind':'shot','name':'delete-confirm'});key(27)
    elif name=='probe':
        move(440,320);a.append({'kind':'right-click','x':440,'y':320});wait();a.append({'kind':'shot','name':'probe-menu'})
        # Inspect the current pixel directly in the native status bar as well as menu.
        key(27);move(520,345);wait(.2);a.append({'kind':'shot','name':'pixel-value'})
    elif name=='theme':
        shot('theme');click(647,34);wait();a.append({'kind':'shot','name':'settings'});key(27)
    elif name=='mip':
        key('F');key('N');shot('mip-0');key(38);shot('mip-1')
    elif name=='hdr':
        key('F');shot('hdr')
    elif name=='animation':
        # Keep actual UI/position fixed; record real changing frames at a 10fps capture cadence.
        move();wait(.2);a.append({'kind':'burst','name':'play','count':28,'interval':.1,'keep_toolbar':True})
        key(32);shot('paused');a.append({'kind':'burst','name':'pause','count':12,'interval':.1,'keep_toolbar':True})
        for i in range(4):key(190);shot('step-'+str(i))
        key(32);a.append({'kind':'burst','name':'resume','count':16,'interval':.1,'keep_toolbar':True})
    return a


def run(args):
    root=Path(__file__).resolve().parents[2];out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    paths=fixtures(root,out/'fixtures');capture_commit=args.commit
    gallery={}
    for name,theme in [('rgba','dark'),('view','dark'),('browse','light'),('pin','light'),('palette','dark'),('file','light'),('probe','dark'),('theme','dark'),('theme','light'),('mip','dark'),('hdr','dark'),('animation','dark')]:
        key=name+'-'+theme;target=out/key;script=out/(key+'-actions.json');width,height=(1280,860) if name=='animation' else (880,560)
        script.write_text(json.dumps(actions_for(name,width),indent=2),'utf-8')
        qa.run(SimpleNamespace(binary=args.binary,output=target,input=paths[name],theme=theme,width=width,height=height,commit=capture_commit,actions=script,isolated_profile=True))
        metadata=json.loads((target/'run.json').read_text('utf-8'));assert metadata['preferences_restored']
        gallery[key]={'folder':str(target),'captures':len(metadata['captures']),'binary_sha256':metadata['binary_sha256']}
    (out/'gallery.json').write_text(json.dumps({'capture_commit':capture_commit,'runs':gallery},indent=2),'utf-8')
    print('FEATURE_CAPTURE_COMPLETE',sum(x['captures'] for x in gallery.values()),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--commit',required=True);run(p.parse_args())

"""Real-window checker/AABB regression using only new fixtures and isolated profiles."""
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageChops
import argparse, json, hashlib
import neumorphic_smoke as qa


def run(args):
    out=args.output.resolve(); out.mkdir(parents=True,exist_ok=False)
    w,h=args.width,args.height
    actions=[]
    def key(code): actions.append({'kind':'key','code':code})
    def shot(name):
        actions.extend([{'kind':'move','x':w-90,'y':h-90},{'kind':'wait','seconds':1.35},{'kind':'shot','name':name}])
    key('0'); key('N'); shot('bounds-off')
    key('B'); shot('bounds-on')
    actions.append({'kind':'drag','button':'left','x':w/2,'y':h/2,'dx':64,'dy':32,'expect_window_delta':[0,0,0,0]})
    shot('bounds-panned')
    # A full-resolution rectangle must stay larger than the visible coloured subject.
    key('0'); shot('bounds-reset')
    key('4'); shot('alpha-view')
    key('O'); shot('opaque-view')
    key('5'); shot('rgba-restored')
    if args.button_x is not None:
        actions.extend([{'kind':'move','x':args.button_x,'y':34},{'kind':'wait','seconds':0.8},{'kind':'shot','name':'button-hover'},
                        {'kind':'click','x':args.button_x,'y':34}])
        shot('button-disabled-bounds')
        key('B')
    actions.append({'kind':'wheel','x':w/2,'y':h/2,'delta':240})
    shot('zoom-bounds-on'); key('B'); shot('zoom-bounds-off'); key('B'); key('0')
    actions.extend([{'kind':'right-click','x':100,'y':100},{'kind':'click','x':200,'y':334}])
    shot('settings-above-bounds'); key('B')  # B is blocked while settings are open.
    actions.append({'kind':'click','x':w/2+153,'y':h/2-51})
    key(0x1b); shot('solid-canvas')
    actions.extend([{'kind':'right-click','x':100,'y':100},{'kind':'click','x':200,'y':334},
                    {'kind':'click','x':w/2+113,'y':h/2-51}])
    key(0x1b); shot('checker-restored')
    key('D'); key('0'); shot('static-webp')
    key('D'); key('0'); key(' '); shot('animated-webp')
    key('D'); key('0'); shot('raw-psd')
    key('D'); key('0'); shot('rle-extra-psd')
    key('D'); key('0'); shot('opaque-jpeg')
    key(0x24); key('0'); shot('back-first')
    path=out/'actions.json'; path.write_text(json.dumps(actions),encoding='utf-8')
    sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
    before={p.name:sha(p) for p in args.fixtures.iterdir() if p.is_file()}
    qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=args.fixtures/'01-transparent.png',
        commit=args.commit,theme=args.theme,width=w,height=h,actions=path,isolated_profile=True))
    image=lambda n:Image.open(out/'viewer'/f'{n}.png').convert('RGB')
    off=image('bounds-off'); on=image('bounds-on'); left,top=(w-320)//2,(h-192)//2
    # Grid at opposite sides of canvas: 8pt screen-anchored cells, no gradient strip.
    vals=[]
    for x,y in [(24,104),(32,104),(w-40,104),(w-32,104),(24,h-104),(32,h-104)]:
        vals.append(off.getpixel((x,y)))
    assert len(set(vals))==2, ('grid not covering full canvas',vals)
    assert off.getpixel((24,104)) != off.getpixel((32,104))
    # Transparent padding and outside pixels at matching checker parity are identical.
    for x,y in [(left+12,top+12),(left+24,top+12),(left+12,top+24)]:
        parity=((x//8)+(y//8))%2
        assert off.getpixel((x,y))==off.getpixel((24+8*(parity^((24//8+104//8)%2)),104))
    diff=ImageChops.difference(off,on); changed=diff.getbbox()
    assert changed and all(abs(a-b)<=4 for a,b in zip(changed,(left,top,left+320,top+192))),('incorrect full-image AABB',changed)
    inner=(left+4,top+4,left+316,top+188)
    assert ImageChops.difference(off.crop(inner),on.crop(inner)).getbbox() is None,'bounds changed image interior'
    # Border should surround the full image, not just x40..280/y40..152 artwork.
    pan=image('bounds-panned')
    old_patch=on.crop((left-3,top-3,left+3,top+50))
    new_patch=pan.crop((left+64-3,top+32-3,left+64+3,top+32+50))
    assert ImageChops.difference(old_patch,new_patch).getbbox() is None,'AABB did not follow image drag'
    for name in ['static-webp','raw-psd','rle-extra-psd','rgba-restored','back-first']:
        assert ImageChops.difference(on.crop(inner),image(name).crop(inner)).getbbox() is None,(name,'pixel output mismatch')
    if args.button_x is not None:
        assert ImageChops.difference(off,image('button-disabled-bounds')).getbbox() is None,'toolbar button did not disable bounds'
    zoom_box=ImageChops.difference(image('zoom-bounds-on'),image('zoom-bounds-off')).getbbox()
    assert zoom_box and zoom_box[2]-zoom_box[0]>326 and zoom_box[3]-zoom_box[1]>198
    zw,zh=zoom_box[2]-zoom_box[0]-4,zoom_box[3]-zoom_box[1]-4
    assert abs(zw/zh-320/192)<0.05,('zoom AABB aspect ratio',zoom_box)
    assert ImageChops.difference(image('checker-restored'),on).getbbox() is None,'grid or B state not restored after settings'
    solid=image('solid-canvas')
    assert solid.getpixel((24,104))==solid.getpixel((32,104)), 'solid background option failed'
    # Opaque JPEG must not force a checker background.
    opaque=image('opaque-jpeg')
    assert opaque.getpixel((24,104))==opaque.getpixel((32,104)),'opaque JPEG still has checker cells'
    saved=json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert saved['qa_saved_settings'].get('iv-image-bounds')=='on'
    assert before=={p.name:sha(p) for p in args.fixtures.iterdir() if p.is_file()}
    result={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':sha(args.binary),
            'captures':len(saved['captures']),'theme':args.theme,'logical_client':[w,h],
            'grid_colors':list(set(vals)),'bounds_delta_rect':changed,'full_rectangle':True,
            'image_pixels_unchanged':True,'pan_follows_image':True,'zoom_bounds':zoom_box,'solid_toggle_restores_grid':True,'bounds_persisted':True,
            'toolbar_click_tested':args.button_x is not None,'files_unchanged':True,'preferences_restored':saved['preferences_restored']}
    (out/'report.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['binary','output','fixtures']: p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--commit',required=True); p.add_argument('--theme',choices=['dark','light'],default='light')
    p.add_argument('--width',type=int,default=880); p.add_argument('--height',type=int,default=560)
    p.add_argument('--button-x',type=float)
    run(p.parse_args())

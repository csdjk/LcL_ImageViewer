"""Real Windows topmost/background regression. Uses new fixtures and isolated preferences only."""
from pathlib import Path
from types import SimpleNamespace
import argparse
import json
import shutil
from PIL import Image, ImageChops
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixture = out/'fixtures'
    fixture.mkdir()
    im = Image.new('RGBA', (160,96))
    for y in range(96):
        for x in range(160):
            im.putpixel((x,y),(220,80,45,[0,64,128,192,255][x//32]))
    for name in ['01-alpha.png','02-next.png']:
        im.save(fixture/name)
    before = {p.name:qa.sha(p) for p in fixture.iterdir()}
    actions=[]
    def key(code): actions.append({'kind':'key','code':code})
    def wait(seconds=0.25): actions.append({'kind':'wait','seconds':seconds})
    def shot(name): actions.append({'kind':'shot','name':name})
    def move(): actions.append({'kind':'move','x':30,'y':135})
    def click(x,y=34): actions.append({'kind':'click','x':x,'y':y})
    def pin(value): actions.append({'kind':'assert-topmost','value':value})
    def bg(): click(args.background_x); wait()
    menu_x = min(args.background_x-16, args.width-264) + 80
    def preset(row,name):
        bg(); click(menu_x,args.first_row_y + 34*row); wait(); move(); shot(name)
    key('0'); move(); wait(); shot('base-rgba'); pin(False)
    click(args.pin_x); wait(); pin(True)
    actions.append({'kind':'assert-z-order','value':True})
    actions.append({'kind':'minimize-restore'}); pin(True); move(); shot('pinned-restored')
    actions.append({'kind':'open-dialog-cancel'}); pin(True)
    key('D'); wait(); pin(True); key('A'); wait(); key('0')
    click(args.pin_x); wait(); pin(False)
    actions.append({'kind':'assert-z-order','value':False})
    bg(); move(); wait(1.6); shot('menu-held'); key(27); wait(); shot('menu-escaped')
    preset(2,'white'); key('4'); shot('alpha-white'); key('5')
    preset(3,'gray'); key('T'); wait(); shot('gray-other-theme'); key('T')
    preset(4,'black'); key('4'); shot('alpha-black'); key('5')
    preset(1,'theme-background')
    preset(0,'checker-restored')
    bg(); click(menu_x,args.first_row_y + 34*5); wait(); shot('custom-menu')
    # Drag the real R/G/B handles; verify the resulting canvas is no longer gray.
    for row,dx in enumerate([-32,28,-18]):
        actions.append({'kind':'drag','x':menu_x-22,'y':304+25*row,'dx':dx,'dy':0,'expect_window_delta':[0,0,0,0]})
    click(menu_x,407); wait(); move(); shot('custom-applied')
    preset(3,'gray-final')
    click(args.settings_x); wait(); shot('settings'); key(27)
    click(args.pin_x); wait(); pin(True); move(); shot('final-pinned')
    wait(1.6); shot('hidden'); move(); wait(); shot('restored')
    script=out/'actions.json'; script.write_text(json.dumps(actions,indent=2),encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=fixture/'01-alpha.png',
        theme=args.theme,width=args.width,height=args.height,commit=args.commit,actions=script,isolated_profile=True))
    meta=json.loads((out/'viewer/run.json').read_text('utf-8'))
    assert meta['preferences_restored']
    assert before=={p.name:qa.sha(p) for p in fixture.iterdir()}
    def image(name): return Image.open(out/'viewer'/f'{name}.png').convert('RGB')
    def peak(a,b): return max(hi for lo,hi in ImageChops.difference(a,b).getextrema())
    for name,v in [('white',255),('gray',128),('black',0),('gray-other-theme',128),('gray-final',128)]:
        for xy in [(24,145),(48,170),(args.width-26,180)]:
            assert all(abs(c-v)<=1 for c in image(name).getpixel(xy)),(name,xy,image(name).getpixel(xy))
    custom_rgb = image('custom-applied').getpixel((24,145))
    assert max(custom_rgb)-min(custom_rgb)>40, ('RGB sliders did not change the background',custom_rgb)
    opaque=(args.width//2+52,args.height//2-35,args.width//2+70,args.height//2+35)
    for name in ['white','gray','black','checker-restored']:
        assert peak(image(name).crop(opaque),image('base-rgba').crop(opaque))==0,(name,'opaque pixels changed')
    roi=(args.width//2-78,args.height//2-46,args.width//2+78,args.height//2+46)
    assert peak(image('alpha-white').crop(roi),image('alpha-black').crop(roi))==0,'Alpha altered by background'
    assert peak(image('base-rgba').crop(roi),image('checker-restored').crop(roi))==0,'Checker preference not restored'
    saved=meta['qa_saved_settings']
    assert saved['iv-always-on-top']=='on' and saved['iv-background-color']=='#808080' and saved['iv-checkerboard']=='off',saved
    # Round-trip the values actually serialized by the previous process into a fresh startup.
    restore_actions=[{'kind':'assert-topmost','value':True},{'kind':'assert-z-order','value':True},
        {'kind':'key','code':'0'},{'kind':'move','x':30,'y':135},{'kind':'wait','seconds':0.3},{'kind':'shot','name':'restored-preferences'}]
    restore_path=out/'restore-actions.json';restore_path.write_text(json.dumps(restore_actions),encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary,output=out/'restart',input=fixture/'01-alpha.png',
        theme=args.theme,width=args.width,height=args.height,commit=args.commit,actions=restore_path,isolated_profile=True,
        seed_preferences={k:saved[k] for k in ['iv-always-on-top','iv-background-color','iv-checkerboard']}))
    restored=Image.open(out/'restart/restored-preferences.png').convert('RGB')
    assert restored.getpixel((24,145))==(128,128,128)
    restart=json.loads((out/'restart/run.json').read_text('utf-8'))
    assert restart['preferences_restored']
    report={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':qa.sha(args.binary),
        'theme':args.theme,'client':[args.width,args.height],'captures':len(meta['captures'])+len(restart['captures']),
        'native_topmost_and_peer_z_order':True,'minimize_restore_and_navigation_keep_pin':True,
        'background_rgb_verified':True,'opaque_and_alpha_pixels_unchanged':True,'persistence_round_trip':True,
        'source_unchanged':True,'user_preferences_untouched':True,'saved_appearance':saved}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['binary','output']: p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--commit',required=True)
    p.add_argument('--theme',choices=['dark','light'],default='light')
    p.add_argument('--width',type=int,default=880)
    p.add_argument('--height',type=int,default=560)
    p.add_argument('--background-x',type=float,required=True)
    p.add_argument('--pin-x',type=float,required=True)
    p.add_argument('--settings-x',type=float,required=True)
    p.add_argument('--first-row-y',type=float,default=106)
    run(p.parse_args())

"""Real Windows regression: drag only the settings popup, not its host window.
Creates new PNG fixtures and an isolated viewer profile. Does not confirm deletion,
click system integration actions, or terminate existing viewers.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageDraw, ImageChops, ImageStat
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    fixtures.mkdir()
    for n in ('01_first.png', '02_middle.png', '03_last.png'):
        image = Image.new('RGB', (512, 512), (208, 219, 232))
        draw = ImageDraw.Draw(image)
        draw.rectangle((200, 210, 305, 300), outline=(85, 110, 145), width=3)
        draw.text((230, 250), n[:2], fill=(45, 65, 90))
        image.save(fixtures / n)
    hashes = {p.name: qa.sha(p) for p in fixtures.iterdir()}
    cx, cy = args.width / 2, args.height / 2
    actions = []
    def shot(name):
        # Move off the header so its hover tooltip cannot obscure pixel comparison.
        actions.extend([{'kind':'move','x':args.width-60,'y':args.height-30},
                        {'kind':'wait','seconds':0.22}, {'kind':'shot','name':name}])
    def click(x, y): actions.append({'kind':'click','x':x,'y':y})
    def key(k): actions.append({'kind':'key','code':k})
    def drag(button, x, y, dx, dy, window=(0,0,0,0)):
        actions.append({'kind':'drag','button':button,'x':x,'y':y,'dx':dx,'dy':dy,'expect_window_delta':list(window)})
    def open_settings():
        actions.append({'kind':'right-click','x':100,'y':100})
        click(200,334)
    open_settings(); shot('settings-default')
    drag('left',cx-160,cy-131,72,36); shot('popup-left')
    drag('right',cx+20+72,cy-131+36,-48,24); shot('popup-right')
    drag('left',cx+40+24,cy-131+60,-36,-24); shot('popup-blank')
    # Current popup translation is (-12,+36), not a native-window displacement.
    drag('left',cx-130-12,cy-52+36,40,10); shot('body-stays')
    drag('left',cx+154-12,cy-11+36,-42,0); shot('control-stays')
    click(cx+154-12,cy-11+36); shot('toggle-on')
    click(cx+154-12,cy-11+36); shot('toggle-off')
    # Keep the popup accessible even when dragged beyond the client bounds.
    drag('left',cx-160-12,cy-131+36,-700,-500); shot('clamped')
    click(368,44); shot('settings-closed')
    open_settings(); shot('reopened-position')
    click(368,44); shot('closed-again')
    key('D'); shot('d-next'); key('A'); shot('a-back')
    # The existing canvas right-drag must still move the native viewer.
    drag('right',cx,cy,40,24,(40,24,0,0)); shot('canvas-right-moves-host')
    action_file = out / 'actions.json'
    action_file.write_text(json.dumps(actions,indent=2),encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=fixtures/'02_middle.png',
                           commit=args.commit,theme=args.theme,width=args.width,height=args.height,
                           actions=action_file,isolated_profile=True))
    run_meta=json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert run_meta['isolated_profile'] and run_meta['preferences_restored']
    assert hashes=={p.name:qa.sha(p) for p in fixtures.iterdir()}
    metas={Path(n).stem:json.loads((out/'viewer'/Path(n).with_suffix('.json')).read_text(encoding='utf-8')) for n in run_meta['captures']}
    images={n:Image.open(out/'viewer'/f'{n}.png').convert('RGB') for n in metas}
    native=metas['settings-default']['physical_window']
    for state,m in metas.items():
        assert m['dpi']==96 and m['logical_client']==[args.width,args.height]
        if state!='canvas-right-moves-host': assert m['physical_window']==native,(state,'Host window moved')
        name='03_last.png' if state=='d-next' else '02_middle.png'
        assert m['window_title'].startswith(name),(state,'Unexpected image')
    # Compare the actual pixels of the settings contents against their translated baseline.
    base_roi=tuple(map(int,(cx-173,cy-108,cx+173,cy+143)))
    base=images['settings-default'].crop(base_roi)
    errors={}
    translations={'popup-left':(72,36),'popup-right':(24,60),'popup-blank':(-12,36),
                  'body-stays':(-12,36),'toggle-off':(-12,36),
                  'clamped':(8-(cx-196),8-(cy-167)),
                  'reopened-position':(8-(cx-196),8-(cy-167))}
    for state,(dx,dy) in translations.items():
        roi=(int(base_roi[0]+dx),int(base_roi[1]+dy),int(base_roi[2]+dx),int(base_roi[3]+dy))
        error=max(ImageStat.Stat(ImageChops.difference(base,images[state].crop(roi))).mean)
        errors[state]=error
        assert error<2.0,(state,'Popup did not follow expected displacement',error)
    # The central control pixels must actually change when the switch is clicked.
    toggle_roi=tuple(map(int,(cx+132-12,cy-27+36,cx+180-12,cy+6+36)))
    change=max(ImageStat.Stat(ImageChops.difference(images['toggle-on'].crop(toggle_roi),images['toggle-off'].crop(toggle_roi))).mean)
    assert change>2.0,'Toggle did not react'
    report={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':qa.sha(args.binary),
            'captures':len(metas),'theme':args.theme,'logical_client':[args.width,args.height], 'dpi':96,
            'native_window_fixed_for_popup':True,'popup_translation_pixel_errors':errors,
            'canvas_right_drag_preserved':True,'files_unchanged':True,'preferences_restored':True}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report),flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--theme',choices=('light','dark'),default='dark')
    parser.add_argument('--width',type=int,default=880)
    parser.add_argument('--height',type=int,default=560)
    run(parser.parse_args())

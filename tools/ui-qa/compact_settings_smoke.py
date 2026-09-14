"""Verify compact settings with real Windows input, geometry, and saved QA preferences.
Uses an isolated viewer profile and NEW test images. Never executes registration,
default-app, or confirmed-delete actions; existing viewers and user preferences are untouched.
"""
from __future__ import annotations
import argparse
import json
import time
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageDraw, ImageChops, ImageStat
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    fixtures = out / 'fixtures'
    fixtures.mkdir()
    for i, name in enumerate(('01_first.png', '02_middle.png', '03_last.png'), 1):
        image = Image.new('RGB', (512, 512), (209, 219, 232))
        draw = ImageDraw.Draw(image)
        for y in range(512):
            draw.line((0, y, 511, y), fill=(192 + y // 12, 203 + y // 16, 218 + y // 20))
        draw.rectangle((210, 210, 302, 302), outline=(110, 130, 158), width=2)
        draw.text((234, 250), f'FRAME {i}', fill=(55, 72, 95))
        image.save(fixtures / name)
    before = {p.name: qa.sha(p) for p in fixtures.iterdir()}
    cx, cy = args.width / 2, args.height / 2
    actions = []
    def shot(name):
        actions.extend([{'kind':'wait','seconds':0.25}, {'kind':'shot','name':name}])
    def click(x, y, theme=None):
        item={'kind':'click','x':x,'y':y}
        if theme: item['theme']=theme
        actions.append(item)
    def key(k): actions.append({'kind':'key','code':k})
    def drag(button,x,y,dx,dy,expect=None):
        item={'kind':'drag','button':button,'x':x,'y':y,'dx':dx,'dy':dy}
        if expect is not None: item['expect_window_delta']=expect
        actions.append(item)
    actions.append({'kind':'right-click','x':100,'y':100}); shot('flat-menu')
    click(200,334); shot('settings-default')
    actions.append({'kind':'move','x':cx-160,'y':cy-131}); shot('title-hover')
    drag('left',cx-160,cy-131,72,36); shot('title-left-drag')
    drag('right',cx+20,cy-131,-48,24,[-48,24,0,0]); shot('title-right-drag')
    # Blank title area also supports native left drag, without moving the inner panel.
    drag('left',cx+40,cy-131,-36,-24); shot('blank-title-left-drag')
    # Body/control drags must not route to the window drag path.
    drag('left',cx-130,cy-52,44,12,[0,0,0,0]); shot('body-no-window-drag')
    drag('left',cx+154,cy-11,-48,0,[0,0,0,0]); shot('switch-drag-no-window-move')
    click(cx+154,cy-11); shot('reduce-motion-on')
    click(cx+154,cy-11); shot('reduce-motion-off')
    other = 'dark' if args.theme=='light' else 'light'
    click(cx+(164 if other=='dark' else 116),cy-91,other); shot('theme-changed')
    click(cx+(164 if args.theme=='dark' else 116),cy-91,args.theme); shot('theme-restored')
    click(cx+162,cy-51); shot('alpha-solid')
    click(cx+110,cy-51); shot('alpha-checkerboard')
    click(cx+154,cy+29)
    actions.append({'kind':'wait','seconds':1.4}); shot('backdrop-expanded')
    # Enabling the three 32pt parameter rows raises the centered panel by48pt.
    drag('left',cx+8,cy+17,42,0,[0,0,0,0]); shot('opacity-changed-no-drag')
    drag('left',cx+8,cy+49,-25,0,[0,0,0,0]); shot('blur-changed-no-drag')
    click(cx+154,cy-19)
    actions.append({'kind':'wait','seconds':1.0}); shot('backdrop-collapsed')
    # Closing a settings panel must not close the viewer or move the native window.
    click(cx+164,cy-131); shot('settings-closed')
    key('D'); shot('d-next'); key('A'); shot('a-back')
    key(46); shot('delete-confirm-only'); key(27)
    actions.append({'kind':'right-click','x':cx,'y':100}); shot('flat-menu-with-pixel'); key(27)
    actions.append({'kind':'move','x':100,'y':100})
    actions.append({'kind':'wait','seconds':1.4}); shot('hidden')
    actions.append({'kind':'move','x':101,'y':100}); shot('restored')
    action_file=out/'actions.json'
    action_file.write_text(json.dumps(actions,indent=2),encoding='utf-8')
    capture = qa.capture
    def settled_capture(hwnd, destination):
        # Desktop-blur capture briefly excludes its own window. Wait for the
        # application's normal release of that flag; never change system affinity.
        for attempt in range(16):
            try:
                meta = capture(hwnd, destination)
                meta['capture_retry_count'] = attempt
                return meta
            except RuntimeError as error:
                if 'Blank or uniform capture' not in str(error) or attempt == 15:
                    raise
                time.sleep(0.12)
    qa.capture = settled_capture
    try:
        qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=fixtures/'02_middle.png',
                               commit=args.commit,theme=args.theme,width=args.width,height=args.height,
                               actions=action_file,isolated_profile=True))
    finally:
        qa.capture = capture
    assert before=={p.name:qa.sha(p) for p in fixtures.iterdir()},'Input files changed'
    result=json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert result['isolated_profile'] and result['preferences_restored']
    meta={Path(n).stem:json.loads((out/'viewer'/Path(n).with_suffix('.json')).read_text(encoding='utf-8')) for n in result['captures']}
    for state,m in meta.items():
        assert m['logical_client']==[args.width,args.height],(state,'wrong size')
        assert m['dpi']==96,'These coordinate checks currently require DPI96'
        name='03_last.png' if state=='d-next' else '02_middle.png'
        assert m['window_title'].startswith(name),(state,m['window_title'])
    traces=[a for a in meta['restored']['actions'] if a['kind']=='drag']
    assert len(traces)==7
    for index,delta in [(0,[72,36]),(2,[-36,-24])]:
        actual=traces[index]['window_delta_physical']
        # Settings title dragging preserves the complete physical pointer displacement.
        assert all(abs(a-b)<=3 for a,b in zip(actual[:2],delta)),(index,actual,delta)
        assert actual[2:]==[0,0]
    anchor=meta['blank-title-left-drag']['physical_window']
    for state in ['body-no-window-drag','switch-drag-no-window-move','reduce-motion-on','reduce-motion-off','theme-changed','theme-restored','backdrop-expanded','opacity-changed-no-drag','blur-changed-no-drag','backdrop-collapsed','settings-closed']:
        assert meta[state]['physical_window']==anchor,(state,'controls moved the window')
    prefs=result['qa_saved_settings']
    assert prefs['iv-theme']==args.theme and prefs['iv-checkerboard']=='on'
    assert prefs['iv-reduce-motion']=='off' and prefs['iv-backdrop']=='off',prefs
    assert abs(float(prefs['iv-bd-opacity'])-0.42)>0.02,'Opacity slider did not work'
    assert abs(float(prefs['iv-bd-blur'])-0.5)>0.02,'Blur slider did not work'
    image_roi=(int(cx-70),int(cy+185),int(cx+70),int(cy+215))
    def difference(a,b,roi):
        ia=Image.open(out/f'viewer/{a}.png').convert('RGB').crop(roi)
        ib=Image.open(out/f'viewer/{b}.png').convert('RGB').crop(roi)
        return max(ImageStat.Stat(ImageChops.difference(ia,ib)).mean)
    assert difference('settings-default','body-no-window-drag',image_roi)==0,'Body drag panned the image'
    panel_roi=(int(cx-185),int(cy-155),int(cx+185),int(cy+155))
    assert difference('settings-default','theme-changed',panel_roi)>30,'Theme selector did not change appearance'
    assert difference('settings-default','settings-closed',panel_roi)>1,'Close button did not close settings'
    report={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':qa.sha(args.binary),
            'captures':len(result['captures']),'theme':args.theme,'logical_client':[args.width,args.height],
            'dpi':96,'files_unchanged':True,'drag_deltas':[a['window_delta_physical'] for a in traces],
            'qa_saved_settings':prefs,'preferences_restored':True}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--theme',choices=('light','dark'),default='light')
    parser.add_argument('--width',type=int,default=1280)
    parser.add_argument('--height',type=int,default=860)
    run(parser.parse_args())

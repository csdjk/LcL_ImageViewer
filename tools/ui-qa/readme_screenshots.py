"""Capture README images from the actual built viewer using project-owned artwork.
No desktop capture, private paths, file deletion or system registration. PNG crops
are taken directly from screenshots; the UI is never redrawn or composited.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import shutil
from types import SimpleNamespace
from PIL import Image
import neumorphic_smoke as qa


def run(args):
    root=Path(__file__).resolve().parents[2]
    out=args.output.resolve(); out.mkdir(parents=True,exist_ok=False)
    fixtures=out/'fixtures'; fixtures.mkdir()
    artwork=root/'crates/iv-viewer/assets/icon.png'
    for name in ('01_Material.png','02_Texture_RGBA.png','03_Icon.png'):
        shutil.copy2(artwork,fixtures/name)
    docs=root/'docs'; (docs/'screenshots').mkdir(exist_ok=True)
    w,h=1280,860
    for theme in ('dark','light'):
        actions=[{'kind':'key','code':'0'},
                 {'kind':'wheel','x':640,'y':430,'delta':120},
                 {'kind':'move','x':710,'y':700},
                 {'kind':'wait','seconds':0.2},{'kind':'shot','name':'main'},
                 {'kind':'key','code':'4'},{'kind':'move','x':700,'y':700},
                 {'kind':'shot','name':'alpha-channel'},
                 {'kind':'key','code':'C'},
                 {'kind':'right-click','x':100,'y':100},
                 {'kind':'move','x':200,'y':159},{'kind':'shot','name':'context-menu'},
                 {'kind':'click','x':200,'y':334},
                 {'kind':'move','x':40,'y':800},{'kind':'wait','seconds':0.2},
                 {'kind':'shot','name':'settings'}]
        af=out/f'{theme}-actions.json';af.write_text(json.dumps(actions),encoding='utf-8')
        target=out/theme
        qa.run(SimpleNamespace(binary=args.binary.resolve(),output=target,input=fixtures/'02_Texture_RGBA.png',
                               commit=args.commit,theme=theme,width=w,height=h,actions=af,isolated_profile=True))
        meta=json.loads((target/'main.json').read_text(encoding='utf-8'))
        assert meta['dpi']==96 and meta['logical_client']==[w,h]
        Image.open(target/'main.png').convert('RGB').save(docs/f'screenshot-{theme}.jpg',quality=94,subsampling=0)
        if theme=='dark':
            Image.open(target/'context-menu.png').crop((84,84,364,376)).save(docs/'screenshots/context-menu.png')
            Image.open(target/'settings.png').crop((428,244,852,616)).save(docs/'screenshots/settings.png')
            Image.open(target/'alpha-channel.png').convert('RGB').save(docs/'screenshots/alpha-channel.jpg',quality=94,subsampling=0)
    print('README images captured. Visual review required.',flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True)
    run(p.parse_args())

"""Compare actual rendered image pixels from old/new viewers, with isolated QA profiles.
Compare only the central image area, not time-dependent toolbar shadows or labels.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
from types import SimpleNamespace
import sys
from PIL import Image, ImageChops
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools/ui-qa'))
import neumorphic_smoke as qa


def run(args):
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    actions=[]
    for key,name in [('C','rgb'),('1','red'),('2','green'),('3','blue'),('4','alpha')]:
        actions.extend([{'kind':'key','code':key},{'kind':'move','x':100,'y':100},
                        {'kind':'wait','seconds':.2},{'kind':'shot','name':name}])
    action_file=out/'actions.json';action_file.write_text(json.dumps(actions),encoding='utf-8')
    result={}
    for theme,width,height in [('dark',880,560),('light',1280,860)]:
        for name,binary,commit in [('before',args.before,args.before_commit),('after',args.after,args.after_commit)]:
            folder=out/f'{name}-{theme}-{width}'
            qa.run(SimpleNamespace(binary=binary,output=folder,input=args.input,
                                   commit=commit,theme=theme,width=width,height=height,
                                   actions=action_file,isolated_profile=True))
        roi=(width//2-160,height//2-120,width//2+160,height//2+120)
        for channel in ('rgb','red','green','blue','alpha'):
            before=Image.open(out/f'before-{theme}-{width}/{channel}.png').convert('RGB').crop(roi)
            after=Image.open(out/f'after-{theme}-{width}/{channel}.png').convert('RGB').crop(roi)
            difference=ImageChops.difference(before,after)
            assert difference.getbbox() is None,(theme,channel,'Rendered pixel mismatch')
            result[f'{theme}-{width}-{channel}']='pixel-exact'
    report={'result':'PASS','comparisons':result,'roi':'320x240 central image pixels',
            'before_sha256':qa.sha(args.before),'after_sha256':qa.sha(args.after)}
    (out/'report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')
    print(json.dumps(report,indent=2))

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('before','after','input','output'):p.add_argument('--'+key,type=Path,required=True)
    for key in ('before-commit','after-commit'):p.add_argument('--'+key,required=True)
    run(p.parse_args())

"""Verify animated toolbar width breakpoints and custom RGB/desktop-preference compatibility."""
from pathlib import Path
from types import SimpleNamespace
import argparse,json
from PIL import Image
import neumorphic_smoke as qa


def run(args):
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
    source=Path(__file__).resolve().parents[2]/'crates/iv-core/tests/fixtures/avif/animated-alpha.avif'
    original=qa.sha(source)
    actions=[{'kind':'key','code':'0'}]
    widths=[880,1000,1119,1120,1199,1200,1280]
    for width in widths:
        actions += [{'kind':'resize-client','width':width,'height':560},
            {'kind':'assert-topmost','value':True},{'kind':'move','x':30,'y':135},
            {'kind':'wait','seconds':0.3},{'kind':'shot','name':f'width-{width}'}]
    script=out/'actions.json';script.write_text(json.dumps(actions),encoding='utf-8')
    seed={'iv-background-color':'#177FD6','iv-checkerboard':'off','iv-always-on-top':'on','iv-backdrop':'on'}
    qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=source,theme=args.theme,
        width=880,height=560,commit=args.commit,actions=script,isolated_profile=True,seed_preferences=seed))
    assert qa.sha(source)==original
    for width in widths:
        im=Image.open(out/'viewer'/f'width-{width}.png').convert('RGB')
        for xy in [(24,145),(width-24,145),(4,34),(width-5,34)]:
            assert im.getpixel(xy)==(23,127,214),(width,xy,im.getpixel(xy),'background mismatch or toolbar clipped')
    meta=json.loads((out/'viewer/run.json').read_text('utf-8'))
    assert meta['preferences_restored'] and meta['qa_saved_settings']['iv-backdrop']=='on'
    script2=out/'empty-actions.json';script2.write_text(json.dumps([
        {'kind':'assert-topmost','value':True},{'kind':'move','x':30,'y':135},
        {'kind':'wait','seconds':0.25},{'kind':'shot','name':'empty-pinned-color'}]),encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary,output=out/'empty',input=None,theme=args.theme,
        width=880,height=560,commit=args.commit,actions=script2,isolated_profile=True,seed_preferences=seed))
    empty=Image.open(out/'empty/empty-pinned-color.png').convert('RGB')
    assert empty.getpixel((24,145))==(23,127,214)
    report={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':qa.sha(args.binary),
        'theme':args.theme,'captures':len(meta['captures'])+2,'widths':widths,
        'native_pin_retained_on_resize':True,'animated_controls_inside_window':True,
        'custom_rgb_exact':True,'desktop_preference_preserved_but_suspended':True,'empty_state_works':True}
    (out/'report.json').write_text(json.dumps(report,indent=2),encoding='utf-8');print(json.dumps(report),flush=True)


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['binary','output']:p.add_argument('--'+name,type=Path,required=True)
    p.add_argument('--commit',required=True);p.add_argument('--theme',choices=['dark','light'],default='dark')
    run(p.parse_args())

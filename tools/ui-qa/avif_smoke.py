"""Real Windows AVIF/PNG channel parity and mixed-format directory navigation. No registry writes."""
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
    root = out / 'fixtures' / 'AVIF 图库'
    (root/'子目录').mkdir(parents=True)
    source = Path(__file__).resolve().parents[2]/'crates/iv-core/tests/fixtures/avif'
    copies = {'01-透明.avif':'alpha.avif', '02-参考.png':'alpha-reference.png',
              '03-不透明.AVIF':'opaque.avif', '子目录/04-动态.avif':'animated.avif'}
    for target, original in copies.items():
        shutil.copyfile(source/original, root/target)
    hashes = {n:qa.sha(root/n) for n in copies}
    actions = []
    def key(code): actions.append({'kind':'key','code':code})
    def shot(name):
        actions.extend([{'kind':'move','x':args.width-100,'y':100},
                        {'kind':'move','x':args.width-90,'y':96},
                        {'kind':'wait','seconds':0.35},{'kind':'shot','name':name}])
    key('0'); key('5'); shot('rgba-avif')
    key('4'); shot('alpha-avif')
    key('1'); shot('red-avif')
    key('5'); shot('rgba-restored')
    key('B'); shot('image-bounds'); key('B')
    key('D'); actions.append({'kind':'wait','seconds':0.6}); key('0'); key('5'); shot('rgba-png')
    key('4'); shot('alpha-png'); key('5')
    key('D'); actions.append({'kind':'wait','seconds':0.6}); key('0'); shot('opaque-avif')
    key('S'); actions.append({'kind':'wait','seconds':0.7}); key('D')
    actions.append({'kind':'wait','seconds':0.6}); key('0'); shot('animated-first-frame')
    key('A'); actions.append({'kind':'wait','seconds':0.6}); shot('back-to-opaque')
    actions.extend([{'kind':'wait','seconds':1.5},{'kind':'shot','name':'hidden'}]); shot('restored')
    script=out/'actions.json';script.write_text(json.dumps(actions,ensure_ascii=False,indent=2),'utf-8')
    qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=root/'01-透明.avif',
                          theme=args.theme,width=args.width,height=args.height,commit=args.commit,
                          actions=script,isolated_profile=True))
    meta=json.loads((out/'viewer/run.json').read_text('utf-8'))
    assert meta['preferences_restored'] and meta['isolated_profile']
    assert hashes=={n:qa.sha(root/n) for n in copies}
    for state,name in [('rgba-avif','01-透明.avif'),('rgba-png','02-参考.png'),
                       ('opaque-avif','03-不透明.AVIF'),('animated-first-frame','04-动态.avif'),
                       ('back-to-opaque','03-不透明.AVIF')]:
        info=json.loads((out/'viewer'/f'{state}.json').read_text('utf-8'))
        assert info['window_title'].startswith(name), (state,info['window_title'])
    # Fixed actual-size image, cursor outside canvas ROI, no toolbar in the sampled region.
    box=(args.width//2-85,args.height//2-53,args.width//2+85,args.height//2+53)
    def crop(name): return Image.open(out/'viewer'/f'{name}.png').convert('RGB').crop(box)
    differences={}
    for channel in ['rgba','alpha']:
        diff=ImageChops.difference(crop(f'{channel}-avif'),crop(f'{channel}-png'))
        peak=max(high for low,high in diff.getextrema())
        assert peak <= 2, (channel,'AVIF and reference PNG differ',peak)
        differences[channel]=peak
    assert ImageChops.difference(crop('rgba-avif'),crop('alpha-avif')).getbbox()
    assert ImageChops.difference(crop('rgba-avif'),crop('red-avif')).getbbox()
    assert ImageChops.difference(crop('rgba-avif'),crop('rgba-restored')).getbbox() is None
    report={'result':'PASS; screenshots require visual review','commit':args.commit,
            'binary_sha256':qa.sha(args.binary),'captures':len(meta['captures']),
            'theme':args.theme,'logical_client':[args.width,args.height],
            'avif_reference_png_peak_channel_difference':differences,
            'mixed_format_and_recursive_navigation':True,'source_files_unchanged':True,
            'preferences_restored':True,'animation':'first frame only'}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),'utf-8')
    print(json.dumps(report,ensure_ascii=False),flush=True)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--theme',choices=['dark','light'],default='dark')
    parser.add_argument('--width',type=int,default=1280)
    parser.add_argument('--height',type=int,default=860)
    run(parser.parse_args())

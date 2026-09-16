"""Real Windows animated AVIF playback, pause, stepping, seeking and channel-reference QA.
Uses only copied synthetic fixtures, a new output directory and the existing isolated profile.
"""
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
    root = out / 'fixtures' / '动态 AVIF 图库'
    root.mkdir(parents=True)
    source = Path(__file__).resolve().parents[2] / 'crates/iv-core/tests/fixtures/avif'
    names = {'00-动态透明.avif': 'animated-alpha.avif'}
    names.update({f'0{i+1}-参考{i}.png': f'animated-alpha-{i}.png' for i in range(4)})
    for name, original in names.items():
        shutil.copyfile(source / original, root / name)
    before = {name: qa.sha(root / name) for name in names}
    actions = []
    def key(code): actions.append({'kind': 'key', 'code': code})
    def wait(seconds): actions.append({'kind': 'wait', 'seconds': seconds})
    def shot(name): actions.append({'kind': 'shot', 'name': name})
    def move(): actions.append({'kind': 'move', 'x': args.width-90, 'y': 160})
    def click(x, y): actions.append({'kind': 'click', 'x': x, 'y': y})
    key('0'); key('5'); move()
    for i in range(7): wait(0.13); shot(f'playing-{i}')
    key(32); move(); shot('paused'); wait(0.9); shot('paused-held')
    for i in range(4): key(190); shot(f'step-{i}')  # physical period key, not ASCII punctuation
    key(188); shot('step-back')
    key('4'); shot('alpha'); key('1'); shot('red'); key('5'); shot('rgba-restored')
    if args.width >= 1200:
        # Coordinates are local to the verified 1280pt top toolbar, never screen HWNDs.
        click(args.slider_first_x, 34); shot('seek-first')
        click(args.slider_last_x, 34); shot('seek-last'); wait(0.9); shot('seek-held')
        click(args.play_x, 34)
    else:
        key(32)
    move()
    for i in range(5): wait(0.13); shot(f'resumed-{i}')
    key(32)
    # Compare paused animation frames to independent PNGs rendered by the same pipeline.
    for i in range(4):
        key('D'); wait(0.2); key('0'); key('5'); move(); shot(f'reference-rgba-{i}')
        key('4'); shot(f'reference-alpha-{i}'); key('1'); shot(f'reference-red-{i}')
    key('5')
    actions.append({'kind': 'right-click', 'x': 100, 'y': 160}); shot('menu'); key(27)
    wait(1.5); shot('hidden'); move(); wait(0.3); shot('restored')
    script = out / 'actions.json'
    script.write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary, output=out/'viewer', input=root/'00-动态透明.avif',
                           theme=args.theme, width=args.width, height=args.height, commit=args.commit,
                           actions=script, isolated_profile=True))
    meta = json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert meta['preferences_restored'] and meta['isolated_profile']
    assert before == {name: qa.sha(root/name) for name in names}, 'input was changed'
    box = (args.width//2-85, args.height//2-53, args.width//2+85, args.height//2+53)
    def crop(name): return Image.open(out/'viewer'/f'{name}.png').convert('RGB').crop(box)
    def peak(a, b): return max(high for low, high in ImageChops.difference(a, b).getextrema())
    refs = {channel: [crop(f'reference-{channel}-{i}') for i in range(4)] for channel in ['rgba','alpha','red']}
    def identify(name):
        differences = [peak(crop(name), image) for image in refs['rgba']]
        i = min(range(4), key=lambda i: differences[i])
        assert differences[i] <= 2, (name, 'no reference match', differences)
        return i
    playing = [identify(f'playing-{i}') for i in range(7)]
    assert len(set(playing)) >= 2, ('animation did not advance', playing)
    paused = identify('paused')
    assert peak(crop('paused'), crop('paused-held')) == 0, 'pause did not hold'
    steps = [identify(f'step-{i}') for i in range(4)]
    assert steps == [(paused+i+1)%4 for i in range(4)], (paused, steps)
    selected = identify('step-back')
    assert selected == (paused-1)%4
    assert identify('rgba-restored') == selected
    for channel in ['alpha','red']:
        assert peak(crop(channel), refs[channel][selected]) <= 2, ('channel mismatch', channel)
    if args.width >= 1200:
        assert identify('seek-first') == 0, 'slider did not seek to first frame'
        assert identify('seek-last') == 3, 'slider did not seek to last frame'
        assert peak(crop('seek-last'), crop('seek-held')) == 0, 'seeking did not pause'
    resumed = [identify(f'resumed-{i}') for i in range(5)]
    assert len(set(resumed)) >= 2, ('resume did not advance', resumed)
    for i in range(4):
        title = json.loads((out/'viewer'/f'reference-rgba-{i}.json').read_text(encoding='utf-8'))['window_title']
        assert title.startswith(f'0{i+1}-参考{i}.png'), title
    report = {'result':'PASS; visual review required', 'commit':args.commit,
              'binary_sha256':qa.sha(args.binary), 'captures':len(meta['captures']),
              'theme':args.theme, 'logical_client':[args.width,args.height],
              'autoplay_observed_frames':playing, 'resumed_observed_frames':resumed,
              'paused_frame':paused, 'forward_steps':steps, 'backward_step':selected,
              'paused_pixels_stable':True, 'rgba_alpha_red_match_reference':True,
              'mouse_seek_and_play':args.width>=1200, 'source_files_unchanged':True,
              'preferences_restored':True}
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(report,ensure_ascii=False),flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--commit',required=True)
    parser.add_argument('--theme',choices=['dark','light'],default='dark')
    parser.add_argument('--width',type=int,choices=[880,1280],default=1280)
    parser.add_argument('--height',type=int,default=860)
    parser.add_argument('--play-x',type=int,default=874)
    parser.add_argument('--slider-first-x',type=int,default=903)
    parser.add_argument('--slider-last-x',type=int,default=985)
    run(parser.parse_args())

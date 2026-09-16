"""用新建同名图片与隔离偏好，验证顶部相对路径、悬停和范围切换。"""
from pathlib import Path
from types import SimpleNamespace
import argparse
import json
from PIL import Image, ImageDraw, ImageChops
import neumorphic_smoke as qa


def run(args):
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    root = out / 'fixtures' / '图库 根目录'
    names = ['billboard.gif', 'fx/billboard.gif', 'fx/粒子烟雾 很长的目录/素材.v2/烟雾循环/billboard.gif']
    for name in names:
        p = root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        image = Image.new('RGB', (160, 96), (52, 122, 164))
        draw = ImageDraw.Draw(image)
        draw.rectangle((20, 20, 140, 76), outline=(232, 241, 247), width=3)
        draw.line((20, 76, 140, 20), fill=(232, 241, 247), width=2)
        image.save(p, format='GIF')
    before = {name: qa.sha(root / name) for name in names}
    actions = []
    def key(code):
        actions.append({'kind': 'key', 'code': code})
    def shot(name):
        actions.extend([
            {'kind': 'move', 'x': args.width - 90, 'y': 90},
            {'kind': 'move', 'x': args.width - 80, 'y': 95},
            {'kind': 'wait', 'seconds': 0.3},
            {'kind': 'shot', 'name': name}])
    def hover(name):
        actions.extend([
            {'kind': 'move', 'x': 325 if args.width < 1000 else 430, 'y': 34},
            {'kind': 'wait', 'seconds': 0.55},
            {'kind': 'shot', 'name': name}])
    key('S'); actions.append({'kind': 'wait', 'seconds': 0.6}); shot('root-filename')
    key('D'); shot('child-relative'); hover('child-tooltip')
    key('D'); shot('deep-relative'); hover('deep-tooltip')
    key('S'); actions.append({'kind': 'wait', 'seconds': 0.6}); shot('scope-closed-filename')
    key('S'); actions.append({'kind': 'wait', 'seconds': 0.6}); shot('new-root-filename')
    # 只切換标签展示，原菜单和自动隐藏/恢复仍可正常工作。
    actions.extend([{'kind': 'right-click', 'x': 100, 'y': 100}, {'kind': 'shot', 'name': 'menu'}])
    key(27)
    actions.extend([{'kind': 'wait', 'seconds': 1.5}, {'kind': 'shot', 'name': 'hidden'}])
    shot('restored')
    script = out / 'actions.json'
    script.write_text(json.dumps(actions, ensure_ascii=False, indent=2), encoding='utf-8')
    qa.run(SimpleNamespace(binary=args.binary, output=out/'viewer', input=root/'billboard.gif',
                           theme=args.theme, width=args.width, height=args.height,
                           commit=args.commit, actions=script, isolated_profile=True))
    meta = json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert meta['preferences_restored'] and meta['isolated_profile']
    assert before == {name: qa.sha(root/name) for name in names}
    for file in meta['captures']:
        capture = json.loads((out/'viewer'/Path(file).with_suffix('.json')).read_text(encoding='utf-8'))
        assert capture['window_title'].startswith('billboard.gif'), capture['window_title']
        assert capture['logical_client'] == [args.width, args.height]
    # 名称槽宽固定；同名图片根目录/关闭子目录模式应相同，子目录应能观察到不同标签。
    box = (288, 20, 355, 48) if args.width < 1000 else (352, 20, 510, 48)
    images = {n: Image.open(out/'viewer'/f'{n}.png').convert('RGB') for n in
              ['root-filename', 'child-relative', 'deep-relative', 'scope-closed-filename']}
    assert ImageChops.difference(images['root-filename'].crop(box), images['child-relative'].crop(box)).getbbox(), '子目录标签与根目录完全相同'
    assert ImageChops.difference(images['root-filename'].crop(box), images['scope-closed-filename'].crop(box)).getbbox() is None, '关闭子目录浏览后标签没有恢复'
    report = {'result': 'PASS; visual review required', 'commit': args.commit,
              'binary_sha256': qa.sha(args.binary), 'captures': len(meta['captures']),
              'theme': args.theme, 'logical_client': [args.width,args.height],
              'same_basename_relative_label_changes': True, 'closing_scope_restores_filename': True,
              'native_window_title_unchanged': True, 'files_unchanged': True,
              'preferences_restored': True}
    (out/'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(report, ensure_ascii=False), flush=True)


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--commit', required=True)
    p.add_argument('--theme', choices=['dark','light'], default='dark')
    p.add_argument('--width', type=int, default=1280)
    p.add_argument('--height', type=int, default=860)
    run(p.parse_args())

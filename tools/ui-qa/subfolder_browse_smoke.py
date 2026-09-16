"""真实窗口验证子目录浏览。只创建测试图片；删除用例仅回收本次新建图片。
不触碰用户图片、安装目录、注册表，不关闭其他查看器；所有配置使用隔离 profile。
"""
from pathlib import Path
from types import SimpleNamespace
from PIL import Image, ImageDraw
import argparse
import json
import subprocess
import ctypes
import neumorphic_smoke as qa
from menu_controls_smoke import recycle_evidence


def run(args):
    out = args.output.resolve(); out.mkdir(parents=True, exist_ok=False)
    tree = out/'fixtures'; root = tree/'图库 根目录'
    relative = ['00-root.png', '01-root.png', 'a 子目录/02-child.png', 'a 子目录/deep/03-deep.png', 'b 子目录/04-branch.png']
    for i, name in enumerate(relative + ['../outside-parent.png', '../other/99-outside.png']):
        path = root/name; path.parent.mkdir(parents=True, exist_ok=True)
        img = Image.new('RGBA', (320, 192), (0, 0, 0, 0))
        d = ImageDraw.Draw(img); d.rectangle((24, 24, 296, 168), fill=(60+i*20, 120, 170, 180))
        d.text((60, 85), path.name, fill=(250,250,250,255)); img.save(path)
    files = sorted(tree.rglob('*.png')); hashes={str(p.relative_to(tree)):qa.sha(p) for p in files}
    # 实际 junction 指向根外/父目录；如果跟随链接，本用例会读到外部图或循环。
    junction=root/'escape-link'
    process=subprocess.run(['cmd','/C','mklink','/J',str(junction),str(tree)],capture_output=True)
    assert process.returncode==0,'测试目录链接创建失败'
    actions=[]; expected={}
    def key(code, modifiers=()): actions.append({'kind':'key','code':code,'modifiers':list(modifiers)})
    def shot(name, title):
        actions.extend([{'kind':'wait','seconds':0.35},{'kind':'move','x':args.width/2,'y':args.height-24},
                        {'kind':'wait','seconds':0.15},{'kind':'shot','name':name}]);expected[name]=title
    def click_side(right):
        x=args.width-42 if right else 42
        actions.extend([{'kind':'move','x':x-25,'y':args.height/2-50},
                        {'kind':'click','x':x,'y':args.height/2}])
    def toggle():
        if args.button_x is None: key('S')
        else:
            actions.extend([{'kind':'move','x':args.button_x-36,'y':74},
                            {'kind':'click','x':args.button_x,'y':34}])
        actions.append({'kind':'wait','seconds':0.6})
    shot('default-off','00-root.png')
    key('D');shot('same-folder-next','01-root.png')
    key('D');shot('flat-boundary','01-root.png')
    toggle();shot('scope-enabled','01-root.png')
    key(36);shot('recursive-first','00-root.png')
    click_side(True);shot('button-root-next','01-root.png')
    click_side(True);shot('button-child','02-child.png')
    if args.delete_case:
        key(46);shot('delete-confirm','02-child.png')
        key(27);shot('cancel-delete','02-child.png')
        key(46)
        actions.extend([{'kind':'click','x':args.width/2+85,'y':args.height/2+64},{'kind':'wait','seconds':1.2}])
        shot('delete-child-keeps-root','03-deep.png')
        click_side(True);shot('after-delete-cross-branch','04-branch.png')
        key(36);shot('after-delete-root-first','00-root.png')
    else:
        click_side(True);shot('button-grandchild','03-deep.png')
        click_side(True);shot('button-other-child','04-branch.png')
        click_side(True);shot('recursive-boundary','04-branch.png')
        key(36);shot('home-root','00-root.png')
        key(35);shot('end-descendant','04-branch.png')
        key('A');key('A');shot('back-to-child','02-child.png')
        key('S',['ctrl']);shot('modified-toggle-ignored','02-child.png')
        toggle();shot('disabled-in-child','02-child.png')
        key('D');shot('disabled-does-not-go-up','02-child.png')
        toggle();shot('reenabled-root-is-child','02-child.png')
        key('D');shot('narrowed-scope-descendant','03-deep.png')
        key('D');shot('narrowed-scope-no-sibling','03-deep.png')
        key(36);shot('narrowed-scope-first','02-child.png')
        key('B');shot('bounds-kept','02-child.png')
        key('4');shot('alpha-kept','02-child.png');key('5')
        actions.append({'kind':'right-click','x':100,'y':100})
        key('S');key(27);key('D');shot('menu-does-not-toggle-scope','03-deep.png')
    if args.button_x is not None:
        actions.extend([{'kind':'move','x':args.button_x-36,'y':74},{'kind':'move','x':args.button_x,'y':34},
                        {'kind':'wait','seconds':0.6},{'kind':'shot','name':'scope-tooltip'}])
    path=out/'actions.json';path.write_text(json.dumps(actions,ensure_ascii=False,indent=2),encoding='utf-8')
    original_capture=qa.capture
    def check_capture(hwnd,destination):
        title=ctypes.create_unicode_buffer(1024);qa.u.GetWindowTextW(hwnd,title,len(title))
        if destination.stem in expected:
            assert title.value.startswith(expected[destination.stem]),(destination.stem,title.value,expected[destination.stem])
        return original_capture(hwnd,destination)
    qa.capture=check_capture
    try:
        qa.run(SimpleNamespace(binary=args.binary,output=out/'viewer',input=root/'00-root.png',
            commit=args.commit,theme=args.theme,width=args.width,height=args.height,actions=path,isolated_profile=True))
    finally:
        qa.capture=original_capture
        if junction.is_dir(): junction.rmdir()  # 仅删除本用例建立的链接，不遍历目标。
    runmeta=json.loads((out/'viewer/run.json').read_text(encoding='utf-8'))
    assert runmeta['preferences_restored']
    deleted='图库 根目录/a 子目录/02-child.png'
    for p in files:
        rel=p.relative_to(tree).as_posix()
        if args.delete_case and rel==deleted: assert not p.exists()
        else: assert p.exists() and qa.sha(p)==hashes[str(p.relative_to(tree))],rel
    recycled=[]
    if args.delete_case:
        recycled=recycle_evidence(root/'a 子目录')
        assert len(recycled)==1 and recycled[0]['sha256']==hashes[str(Path(deleted))]
    result={'result':'PASS; visual review required','commit':args.commit,'binary_sha256':qa.sha(args.binary),
        'theme':args.theme,'logical_client':[args.width,args.height],'captures':len(runmeta['captures']),
        'top_toggle_clicked':args.button_x is not None,'side_buttons_clicked':True,
        'fixed_root_and_no_upward_search':True,'junction_not_followed':True,
        'deletion_preserves_scope':bool(args.delete_case),'recycled_test_files':recycled,
        'preferences_restored':True,'outside_files_unchanged':True}
    (out/'report.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False),flush=True)

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--commit',required=True);p.add_argument('--theme',choices=['light','dark'],default='light')
    p.add_argument('--width',type=int,default=880);p.add_argument('--height',type=int,default=560)
    p.add_argument('--button-x',type=float);p.add_argument('--delete-case',action='store_true')
    run(p.parse_args())

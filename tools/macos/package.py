"""Package an already-built native macOS app. Never installs or publishes a Release."""
from __future__ import annotations
import argparse, hashlib, json, os, plistlib, shutil, subprocess, sys, tempfile, tomllib
from pathlib import Path


def command(*args: object) -> str:
    return subprocess.check_output([str(a) for a in args], text=True).strip()


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(args: argparse.Namespace) -> None:
    if sys.platform != 'darwin': raise RuntimeError('Package on a native macOS runner')
    root = Path(__file__).resolve().parents[2]
    version = tomllib.loads((root/'Cargo.toml').read_text())['workspace']['package']['version']
    source = command('git', '-C', root, 'rev-parse', 'HEAD')
    binary = root/'target'/args.target/'release/imageview'
    arch = command('lipo', '-archs', binary)
    if arch != args.arch: raise RuntimeError(f'Wrong architecture: {arch}')
    imports = command('otool', '-L', binary).splitlines()[1:]
    for line in imports:
        library = line.strip().split(' (', 1)[0]
        if not library.startswith(('/System/Library/', '/usr/lib/')):
            raise RuntimeError(f'External runtime dependency must not escape the bundle: {library}')
    output = root/'dist/macos'
    if output.exists(): raise FileExistsError(output)
    output.mkdir(parents=True)
    app = output/'LcL ImageViewer.app'
    contents = app/'Contents'
    resources = contents/'Resources'; resources.mkdir(parents=True)
    executable = contents/'MacOS/imageview'; executable.parent.mkdir()
    shutil.copy2(binary, executable); executable.chmod(0o755)
    # Generate icon sizes with system tools; no downloaded assets or redistributed fonts.
    with tempfile.TemporaryDirectory(prefix='lcl-mac-icon-') as folder:
        iconset = Path(folder)/'LcLImageViewer.iconset';iconset.mkdir()
        for n in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = '@2x' if scale == 2 else ''
                command('sips','-z',n*scale,n*scale,root/'crates/iv-viewer/assets/icon.png','--out',iconset/f'icon_{n}x{n}{suffix}.png')
        command('iconutil','-c','icns',iconset,'-o',resources/'LcLImageViewer.icns')
    plist = {
        'CFBundleName':'LcL ImageViewer', 'CFBundleDisplayName':'LcL ImageViewer',
        'CFBundleIdentifier':'cn.lclgames.imageviewer', 'CFBundleExecutable':'imageview',
        'CFBundlePackageType':'APPL', 'CFBundleShortVersionString':version,
        'CFBundleVersion':version, 'CFBundleIconFile':'LcLImageViewer.icns',
        'CFBundleDevelopmentRegion':'zh_CN', 'CFBundleLocalizations':['zh_CN','en'],
        'LSMinimumSystemVersion':'12.0', 'NSHighResolutionCapable':True,
        'NSPrincipalClass':'NSApplication',
        'NSHumanReadableCopyright':'LcL Games. Third-party notices included in Resources/licenses.',
    }
    with (contents/'Info.plist').open('wb') as f: plistlib.dump(plist,f)
    command('plutil','-lint',contents/'Info.plist')
    notices = resources/'licenses';notices.mkdir()
    shutil.copy2(root/'LICENSE',notices/'LICENSE.txt')
    shutil.copy2(root/'docs/formats/AVIF第三方许可.txt',notices/'AVIF-third-party-notices.txt')
    notes = f'''LcL ImageViewer {version} · macOS preview · {args.arch}

安装：打开 DMG，将 LcL ImageViewer.app 拖入 Applications。
Apple Silicon（M 系列芯片） 使用 arm64；Intel Mac 使用 x86_64。
最低部署目标 macOS 12.0；云端实际验证系统见 build-manifest.json。

此测试包使用 ad-hoc 签名，未使用 Developer ID，也未经过 Apple 公证。
首次打开可能被 macOS 安全检查阻止。确认来源后，按系统“隐私与安全性”中的“仍要打开”提示操作。
不要关闭系统 Gatekeeper 或全局安全保护。生产分发需单独配置开发者签名/公证。

打开应用后使用顶部打开按钮、Command+O 或拖入图片。
RGBA：1/2/3/4，完整彩色：5/C，忽略透明度：O。
动画：Space 播放/暂停，逗号/句号逐帧。顶部支持置顶和背景调色板。
删除图片会先确认并移到系统废纸篓，不提供永久删除回退。
Windows 专属桌面磨砂/Explorer 缩略图不包含在 Mac 版；暂未提供 Finder Quick Look 扩展。
此预览请从应用内打开图片；Finder 文件“打开方式”关联尚未提供。

源码提交：{source}
此包由 GitHub Actions 构建，不替换已有 Windows 正式版。
'''
    (output/'安装说明.txt').write_text(notes,encoding='utf-8')
    (resources/'安装说明.txt').write_text(notes,encoding='utf-8')
    manifest = {'version':version,'preview':True,'source_commit':source,'target':args.target,
        'architecture':arch,'rustc':command('rustc','--version'),'minimum_macos':'12.0','build_macos':command('sw_vers','-productVersion'),
        'signing':'ad-hoc','notarized':False,'system_imports':imports,'unsigned_binary_sha256':sha(executable)}
    (resources/'build-manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    command('codesign','--force','--sign','-','--timestamp=none','--options','runtime',app)
    command('codesign','--verify','--deep','--strict','--verbose=2',app)
    manifest['signed_binary_sha256']=sha(executable)
    manifest['code_signature']=subprocess.run(['codesign','-dv','--verbose=2',str(app)],capture_output=True,text=True,check=True).stderr
    with tempfile.TemporaryDirectory(prefix='lcl-mac-dmg-') as folder:
        stage = Path(folder)
        shutil.copytree(app,stage/app.name,symlinks=True)
        os.symlink('/Applications',stage/'Applications')
        shutil.copy2(output/'安装说明.txt',stage/'安装说明.txt')
        dmg = output/f'LcL-ImageViewer-v{version}-macos-preview-{args.arch}.dmg'
        command('hdiutil','create','-volname','LcL ImageViewer','-srcfolder',stage,'-fs','HFS+','-format','UDZO',dmg)
    command('hdiutil','verify',dmg)
    manifest['dmg']={'name':dmg.name,'bytes':dmg.stat().st_size,'sha256':sha(dmg)}
    (output/'SHA256SUMS.txt').write_text(f'{sha(dmg)}  {dmg.name}\n',encoding='ascii')
    (output/'build-manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(manifest,ensure_ascii=False,indent=2),flush=True)


if __name__ == '__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--target',choices=['aarch64-apple-darwin','x86_64-apple-darwin'],required=True)
    p.add_argument('--arch',choices=['arm64','x86_64'],required=True)
    run(p.parse_args())

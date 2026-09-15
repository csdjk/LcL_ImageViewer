"""Package an already-built Windows release. Never installs or publishes anything.
Python 3.11+, Inno Setup 7; run from any working directory. Refuses existing outputs.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tomllib
import zipfile


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def assert_binary_version(path: Path, expected: str) -> None:
    import ctypes as c
    from ctypes import wintypes as w
    api = c.WinDLL('version', use_last_error=True)
    api.GetFileVersionInfoSizeW.argtypes = [w.LPCWSTR, c.POINTER(w.DWORD)]
    api.GetFileVersionInfoSizeW.restype = w.DWORD
    api.GetFileVersionInfoW.argtypes = [w.LPCWSTR, w.DWORD, w.DWORD, c.c_void_p]
    api.GetFileVersionInfoW.restype = w.BOOL
    api.VerQueryValueW.argtypes = [c.c_void_p, w.LPCWSTR, c.POINTER(c.c_void_p), c.POINTER(w.UINT)]
    api.VerQueryValueW.restype = w.BOOL
    size = api.GetFileVersionInfoSizeW(str(path.resolve()), None)
    if not size:
        raise RuntimeError(f'No version resource: {path}')
    data = c.create_string_buffer(size)
    assert api.GetFileVersionInfoW(str(path.resolve()), 0, size, data)
    pointer, length = c.c_void_p(), w.UINT()
    assert api.VerQueryValueW(data, '\\', c.byref(pointer), c.byref(length))
    fields = c.cast(pointer, c.POINTER(w.DWORD))
    actual = [fields[2] >> 16, fields[2] & 65535, fields[3] >> 16, fields[3] & 65535]
    wanted = [int(n) for n in expected.split('.')]
    wanted += [0] * (4 - len(wanted))
    assert actual == wanted, f'{path.name}: version {actual}, expected {wanted}'


def run(args: argparse.Namespace) -> None:
    root = Path(__file__).resolve().parent.parent
    version = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    binary_dir = (args.binary_dir or root / 'target/release').resolve()
    output = (args.output or root / 'dist' / f'v{version}').resolve()
    compiler = args.iscc.resolve()
    for p in (compiler, binary_dir / 'imageview.exe', binary_dir / 'iv_shell.dll'):
        if not p.is_file():
            raise FileNotFoundError(p)
    assert_binary_version(binary_dir / 'imageview.exe', version)
    if output.exists() and any(output.iterdir()):
        raise FileExistsError(f'Refusing nonempty output: {output}')
    output.mkdir(parents=True, exist_ok=True)
    commit = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    name = f'LcL-ImageViewer-v{version}-win64'
    stage = output / name
    stage.mkdir()
    for source in (binary_dir / 'imageview.exe', binary_dir / 'iv_shell.dll', root / 'LICENSE',
                   root / 'CHANGELOG.md', root / 'tools/register_thumbnail.ps1', root / 'tools/unregister_thumbnail.ps1'):
        shutil.copy2(source, stage / source.name)
    (stage / 'README.txt').write_text(f'''LcL ImageViewer v{version} — Windows x64

解压本目录后运行 imageview.exe。普通看图无需安装或注册 DLL。
升级前关闭旧版本，避免继续启动旧目录中的 EXE。

快捷操作
  Ctrl+O：打开图片；也可将图片拖进窗口。
  A/D、左右方向键或 PgUp/PgDn：上一张 / 下一张。
  左键/中键拖图；画布右键拖动主窗口；右键单击打开菜单。
  设置标题左键/右键拖动：只移动设置弹窗，主窗口不动。
  滚轮：光标中心缩放；F：适配；0：实际大小；N：切换采样。
  1/2/3/4：R/G/B/Alpha；5或C：完整显示；O：忽略Alpha。
  上下方向键：Mipmap；Space：动画播放暂停；逗号/句号：逐帧。
  T：深浅主题；Delete：确认后移入回收站；Esc：关弹层/退出。

可选的资源管理器缩略图扩展
  iv_shell.dll、register_thumbnail.ps1、unregister_thumbnail.ps1 请保持同目录。
  注册脚本只供需要 DDS/TGA/PSD 等资源管理器缩略图的用户手动运行。
  移动或删除本目录之前先运行注销脚本。普通看图无需这些操作。
  默认看图软件需要在 Windows 系统设置中自行确认。

偏好保存在 %APPDATA%\\LcL ImageViewer 中，不是EXE旁边。
回收站不可用时取消删除，不提供永久删除回退。

完整说明：https://github.com/csdjk/LcL_ImageViewer
更新下载：https://github.com/csdjk/LcL_ImageViewer/releases/latest
源代码提交：{commit}
许可：见 LICENSE；更新记录：见 CHANGELOG.md。
''', encoding='utf-8-sig')
    manifest = {'version': version, 'source_commit': commit,
                'files': {p.name: {'bytes': p.stat().st_size, 'sha256': sha(p)} for p in stage.iterdir()}}
    (stage / 'version.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding='utf-8')
    archive = output / f'{name}.zip'
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for p in sorted(stage.iterdir()):
            z.write(p, f'{name}/{p.name}')
    with zipfile.ZipFile(archive) as z:
        assert z.testzip() is None, 'ZIP CRC failure'
        for filename, record in manifest['files'].items():
            assert hashlib.sha256(z.read(f'{name}/{filename}')).hexdigest() == record['sha256']
    subprocess.run([str(compiler), f'/DMyAppVersion={version}', f'/DBuildDir={binary_dir}',
                    f'/O{output}', str(root / 'tools/setup.iss')], cwd=root, check=True)
    installer = output / f'LcL-ImageViewer-Setup-v{version}-win64.exe'
    if not installer.is_file():
        raise FileNotFoundError(installer)
    assert_binary_version(installer, version)
    assets = [installer, archive]
    (output / 'SHA256SUMS.txt').write_text(''.join(f'{sha(p)}  {p.name}\n' for p in assets), encoding='ascii')
    print(json.dumps({'version': version, 'commit': commit, 'assets': [
        {'file': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in assets]}, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--iscc', type=Path, required=True)
    parser.add_argument('--binary-dir', type=Path)
    parser.add_argument('--output', type=Path)
    run(parser.parse_args())

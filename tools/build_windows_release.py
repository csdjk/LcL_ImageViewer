"""Build a self-contained Windows x64 Release in an isolated Cargo target directory.
Requires Rust MSVC, Visual Studio C++ tools, CMake and NASM on the child PATH.
Does not change the global environment, install, register, stop viewers, or publish.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tomllib
from package_release import assert_binary_version, assert_portable_runtime, sha

TARGET = 'x86_64-pc-windows-msvc'


def run(args: argparse.Namespace) -> None:
    if os.name != 'nt':
        raise RuntimeError('Build this release on Windows with an MSVC toolchain')
    root = Path(__file__).resolve().parent.parent
    cargo = str(args.cargo) if args.cargo else shutil.which('cargo')
    if not cargo:
        raise FileNotFoundError('cargo is not on PATH; pass --cargo explicitly')
    for tool in ('cmake', 'nasm'):
        if not shutil.which(tool):
            raise FileNotFoundError(f'{tool} is required on the build-process PATH')
    target_dir = (args.target_dir or root/'target/windows-release').resolve()
    if target_dir == (root/'target').resolve():
        raise ValueError('Use a separate target directory, not the normal development target')
    version = tomllib.loads((root/'Cargo.toml').read_text(encoding='utf-8'))['workspace']['package']['version']
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    if subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=no'], cwd=root, text=True).strip():
        raise RuntimeError('Commit tracked source changes before making a release build')
    env = os.environ.copy()
    # Only affect this child process. Explicit target keeps host proc-macros separate.
    env.pop('RUSTFLAGS', None)
    env['CARGO_ENCODED_RUSTFLAGS'] = '-C\x1ftarget-feature=+crt-static'
    command = [cargo, 'build', '--release', '--workspace', '--locked', '--target', TARGET,
               '--target-dir', str(target_dir), '--jobs', str(args.jobs)]
    subprocess.run(command, cwd=root, env=env, check=True)
    binary_dir = target_dir/TARGET/'release'
    assert_binary_version(binary_dir/'imageview.exe', version)
    binaries = {}
    for name in ('imageview.exe', 'iv_shell.dll'):
        path = binary_dir/name
        binaries[name] = {'bytes': path.stat().st_size, 'sha256': sha(path),
                          'imports': assert_portable_runtime(path)}
    result = {'source_commit': commit, 'version': version, 'target': TARGET,
              'static_crt': True, 'binaries': binaries}
    (binary_dir/'release-build.json').write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps({'binary_dir': str(binary_dir), **result}, ensure_ascii=False, indent=2), flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cargo', type=Path)
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--jobs', type=int, choices=range(1, 33), default=8)
    run(parser.parse_args())

"""Capture the real Windows viewer, never a mockup. Requires Python 3.10+ and Pillow.

Temporarily seeds ONLY this application's app.ron, then restores its exact bytes.
Refuses to run while another imageview process exists. Never clicks integration actions.
Explicit --binary, --output, --input and --actions make the run reproducible.
"""
from __future__ import annotations

import argparse
import ctypes as c
from ctypes import wintypes as w
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time
from PIL import Image

u = c.WinDLL('user32', use_last_error=True)
g = c.WinDLL('gdi32', use_last_error=True)
k = c.WinDLL('kernel32', use_last_error=True)
PTR = c.c_void_p
ENUM = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)

def bind(dll, name, args, result):
    fn = getattr(dll, name)
    fn.argtypes, fn.restype = args, result
    return fn

bind(u, 'EnumWindows', [ENUM, w.LPARAM], w.BOOL)
bind(u, 'IsWindowVisible', [w.HWND], w.BOOL)
bind(u, 'GetWindowThreadProcessId', [w.HWND, c.POINTER(w.DWORD)], w.DWORD)
bind(u, 'GetWindowRect', [w.HWND, c.POINTER(w.RECT)], w.BOOL)
bind(u, 'GetClientRect', [w.HWND, c.POINTER(w.RECT)], w.BOOL)
bind(u, 'ClientToScreen', [w.HWND, c.POINTER(w.POINT)], w.BOOL)
bind(u, 'GetDpiForWindow', [w.HWND], w.UINT)
bind(u, 'SetWindowPos', [w.HWND, w.HWND, c.c_int, c.c_int, c.c_int, c.c_int, w.UINT], w.BOOL)
bind(u, 'SetForegroundWindow', [w.HWND], w.BOOL)
bind(u, 'GetForegroundWindow', [], w.HWND)
bind(u, 'GetCursorPos', [c.POINTER(w.POINT)], w.BOOL)
bind(u, 'SetCursorPos', [c.c_int, c.c_int], w.BOOL)
bind(u, 'mouse_event', [w.DWORD, w.DWORD, w.DWORD, w.DWORD, c.c_size_t], None)
bind(u, 'keybd_event', [w.BYTE, w.BYTE, w.DWORD, c.c_size_t], None)
bind(u, 'PostMessageW', [w.HWND, w.UINT, w.WPARAM, w.LPARAM], w.BOOL)
bind(u, 'GetDC', [w.HWND], w.HDC)
bind(u, 'ReleaseDC', [w.HWND, w.HDC], c.c_int)
bind(u, 'PrintWindow', [w.HWND, w.HDC, w.UINT], w.BOOL)
bind(g, 'CreateCompatibleDC', [w.HDC], w.HDC)
bind(g, 'CreateCompatibleBitmap', [w.HDC, c.c_int, c.c_int], w.HBITMAP)
bind(g, 'SelectObject', [w.HDC, w.HANDLE], w.HANDLE)
bind(g, 'DeleteObject', [w.HANDLE], w.BOOL)
bind(g, 'DeleteDC', [w.HDC], w.BOOL)
bind(k, 'OpenProcess', [w.DWORD, w.BOOL, w.DWORD], w.HANDLE)
bind(k, 'QueryFullProcessImageNameW', [w.HANDLE, w.DWORD, w.LPWSTR, c.POINTER(w.DWORD)], w.BOOL)
bind(k, 'CloseHandle', [w.HANDLE], w.BOOL)

class BMIHeader(c.Structure):
    _fields_ = [('size', w.DWORD), ('width', w.LONG), ('height', w.LONG),
                ('planes', w.WORD), ('bits', w.WORD), ('compression', w.DWORD),
                ('size_image', w.DWORD), ('xppm', w.LONG), ('yppm', w.LONG),
                ('used', w.DWORD), ('important', w.DWORD)]

class BMI(c.Structure):
    _fields_ = [('header', BMIHeader), ('colors', w.DWORD * 3)]

bind(g, 'GetDIBits', [w.HDC, w.HBITMAP, w.UINT, w.UINT, PTR, c.POINTER(BMI), w.UINT], c.c_int)


def checked(ok, label):
    if not ok:
        raise OSError(f'{label} failed: {c.get_last_error()}')


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def windows_for_pid(pid: int) -> list[int]:
    found = []
    @ENUM
    def callback(hwnd, _):
        owner = w.DWORD()
        u.GetWindowThreadProcessId(hwnd, c.byref(owner))
        if owner.value == pid and u.IsWindowVisible(hwnd):
            r = w.RECT()
            if u.GetClientRect(hwnd, c.byref(r)) and r.right > 100 and r.bottom > 100:
                found.append(hwnd)
        return True
    checked(u.EnumWindows(callback, 0), 'EnumWindows')
    return found


def process_path(pid: int) -> Path:
    handle = k.OpenProcess(0x1000, False, pid)
    checked(handle, 'OpenProcess')
    try:
        text, size = c.create_unicode_buffer(32768), w.DWORD(32768)
        checked(k.QueryFullProcessImageNameW(handle, 0, text, c.byref(size)), 'QueryFullProcessImageName')
        return Path(text.value).resolve()
    finally:
        k.CloseHandle(handle)


def geometry(hwnd):
    wr, cr, origin = w.RECT(), w.RECT(), w.POINT()
    checked(u.GetWindowRect(hwnd, c.byref(wr)), 'GetWindowRect')
    checked(u.GetClientRect(hwnd, c.byref(cr)), 'GetClientRect')
    checked(u.ClientToScreen(hwnd, c.byref(origin)), 'ClientToScreen')
    dpi = u.GetDpiForWindow(hwnd)
    if not dpi:
        raise RuntimeError('Invalid window DPI')
    return wr, cr, origin, dpi


def capture(hwnd, output: Path):
    wr, cr, origin, dpi = geometry(hwnd)
    width, height = wr.right - wr.left, wr.bottom - wr.top
    cw, ch = cr.right - cr.left, cr.bottom - cr.top
    if min(width, height, cw, ch) <= 0:
        raise RuntimeError('Empty or minimized client')
    dc = u.GetDC(None)
    memory = g.CreateCompatibleDC(dc)
    bitmap = g.CreateCompatibleBitmap(dc, width, height)
    checked(dc and memory and bitmap, 'Allocate capture resources')
    previous = g.SelectObject(memory, bitmap)
    try:
        checked(u.PrintWindow(hwnd, memory, 2), 'PrintWindow(PW_RENDERFULLCONTENT)')
        info = BMI()
        info.header = BMIHeader(c.sizeof(BMIHeader), width, -height, 1, 32, 0, 0, 0, 0, 0, 0)
        pixels = (c.c_ubyte * (width * height * 4))()
        rows = g.GetDIBits(memory, bitmap, 0, height, pixels, c.byref(info), 0)
        if rows != height:
            raise OSError(f'Incomplete capture: {rows}/{height} rows')
        img = Image.frombytes('RGB', (width, height), bytes(pixels), 'raw', 'BGRX')
        left, top = origin.x - wr.left, origin.y - wr.top
        img = img.crop((left, top, left + cw, top + ch))
        lo, hi = img.convert('L').getextrema()
        if hi - lo < 4 or hi <= 4:
            raise RuntimeError('Blank or uniform capture; no usable UI evidence')
        img.save(output)
        return {'physical_client': [cw, ch], 'logical_client': [cw * 96 / dpi, ch * 96 / dpi],
                'dpi': dpi, 'scale': dpi / 96, 'crop': [left, top, cw, ch],
                'capture_method': 'PrintWindow(PW_RENDERFULLCONTENT), client-only RGB crop'}
    finally:
        g.SelectObject(memory, previous)
        g.DeleteObject(bitmap)
        g.DeleteDC(memory)
        u.ReleaseDC(None, dc)


def focus(hwnd):
    u.SetForegroundWindow(hwnd)
    time.sleep(0.1)
    if u.GetForegroundWindow() != hwnd:
        bind(k, 'GetCurrentThreadId', [], w.DWORD)
        bind(u, 'AttachThreadInput', [w.DWORD, w.DWORD, w.BOOL], w.BOOL)
        bind(u, 'BringWindowToTop', [w.HWND], w.BOOL)
        current = k.GetCurrentThreadId()
        target = u.GetWindowThreadProcessId(hwnd, None)
        foreground = u.GetWindowThreadProcessId(u.GetForegroundWindow(), None)
        attached = []
        try:
            for tid in {target, foreground} - {0, current}:
                if u.AttachThreadInput(current, tid, True):
                    attached.append(tid)
            u.BringWindowToTop(hwnd)
            u.SetForegroundWindow(hwnd)
        finally:
            for tid in attached:
                u.AttachThreadInput(current, tid, False)
        time.sleep(0.15)
    if u.GetForegroundWindow() != hwnd:
        raise RuntimeError(f'Viewer could not acquire foreground (target={hwnd}, foreground={u.GetForegroundWindow()}); refusing input to another app')


def move(hwnd, x, y):
    _, _, origin, dpi = geometry(hwnd)
    focus(hwnd)
    checked(u.SetCursorPos(origin.x + round(x * dpi / 96), origin.y + round(y * dpi / 96)), 'SetCursorPos')
    time.sleep(0.18)


def key(hwnd, code):
    focus(hwnd)
    u.keybd_event(code, 0, 0, 0)
    time.sleep(0.06)
    u.keybd_event(code, 0, 2, 0)
    time.sleep(0.18)


def run(args):
    try:
        bind(u, 'SetProcessDpiAwarenessContext', [PTR], w.BOOL)(PTR(-4))
    except AttributeError:
        u.SetProcessDPIAware()
    binary = args.binary.resolve(strict=True)
    if binary.name.lower() != 'imageview.exe':
        raise ValueError('Expected the imageview.exe binary')
    existing = subprocess.run(['powershell', '-NoProfile', '-Command',
        "@(Get-Process -Name imageview -ErrorAction SilentlyContinue).Count"],
        capture_output=True, text=True, check=True)
    if int(existing.stdout.strip()) != 0:
        raise RuntimeError('Another imageview is running; close it before app-preference isolation')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    storage = Path(os.environ['APPDATA']) / 'LcL ImageViewer' / 'data' / 'app.ron'
    previous_bytes = storage.read_bytes() if storage.exists() else None
    if previous_bytes is not None:
        (out / 'app.ron.original').write_bytes(previous_bytes)
    storage.parent.mkdir(parents=True, exist_ok=True)
    foreground, cursor = u.GetForegroundWindow(), w.POINT()
    u.GetCursorPos(c.byref(cursor))
    proc = None
    history = []
    summary = {'theme': args.theme, 'binary': str(binary), 'binary_sha256': sha(binary),
               'commit': args.commit, 'input': str(args.input.resolve()) if args.input else None,
               'input_sha256': sha(args.input) if args.input else None, 'captures': []}
    try:
        # RON maps accept the same simple string key/value syntax as this JSON subset.
        prefs = {'iv-theme': args.theme, 'iv-backdrop': 'off', 'iv-checkerboard': 'on', 'iv-reduce-motion': 'off'}
        storage.write_text(json.dumps(prefs), encoding='utf-8')
        with (out / 'runtime.log').open('wb') as log:
            proc = subprocess.Popen([str(binary)] + ([str(args.input.resolve())] if args.input else []),
                                    stdout=log, stderr=log, cwd=binary.parent)
        deadline = time.monotonic() + 20
        while True:
            if proc.poll() is not None:
                raise RuntimeError(f'Viewer exited during startup: {proc.returncode}')
            handles = windows_for_pid(proc.pid)
            if len(handles) == 1:
                hwnd = handles[0]
                break
            if len(handles) > 1 or time.monotonic() > deadline:
                raise RuntimeError(f'Target window missing/ambiguous: {handles}')
            time.sleep(0.15)
        if process_path(proc.pid) != binary:
            raise RuntimeError('Launched executable identity mismatch')
        time.sleep(0.8)
        wr, cr, _, dpi = geometry(hwnd)
        dw = (wr.right - wr.left) - (cr.right - cr.left)
        dh = (wr.bottom - wr.top) - (cr.bottom - cr.top)
        checked(u.SetWindowPos(hwnd, None, 40, 40, round(args.width * dpi / 96) + dw,
                               round(args.height * dpi / 96) + dh, 0x0040), 'Resize viewer')
        time.sleep(0.6)
        move(hwnd, args.width / 2, args.height / 2)
        current_theme = args.theme
        def shot(name):
            filename = out / f'{name}.png'
            meta = capture(hwnd, filename)
            if meta['logical_client'] != [args.width, args.height]:
                raise RuntimeError(f'Client size mismatch: {meta}')
            meta.update({key: value for key, value in summary.items() if key != 'captures'})
            meta.update({'theme': current_theme, 'state': name, 'actions': list(history),
                         'utc': datetime.now(timezone.utc).isoformat(), 'pid': proc.pid,
                         'window_discovery': 'launched PID + verified process image path + unique visible client',
                         'background': 'theme canvas, desktop capture disabled for this run'})
            filename.with_suffix('.json').write_text(json.dumps(meta, ensure_ascii=False, indent=2), encoding='utf-8')
            summary['captures'].append(filename.name)
            print('captured', filename, meta['logical_client'], 'DPI', meta['dpi'], flush=True)
        shot('default')
        actions = json.loads(args.actions.read_text(encoding='utf-8')) if args.actions else []
        for action in actions:
            history.append(action)
            kind = action['kind']
            if kind == 'move':
                move(hwnd, action['x'], action['y'])
            elif kind in ('click', 'right-click', 'down', 'up'):
                if 'x' in action:
                    move(hwnd, action['x'], action['y'])
                focus(hwnd)
                down, up = (8, 16) if kind == 'right-click' else (2, 4)
                if kind != 'up': u.mouse_event(down, 0, 0, 0, 0)
                if kind not in ('down', 'up'): time.sleep(0.08)
                if kind != 'down': u.mouse_event(up, 0, 0, 0, 0)
                time.sleep(0.15)
            elif kind == 'key':
                code = action['code']
                code = ord(code.upper()) if isinstance(code, str) and len(code) == 1 else int(code)
                key(hwnd, code)
                if code == ord('T'): current_theme = 'dark' if current_theme == 'light' else 'light'
            elif kind == 'wait':
                time.sleep(float(action['seconds']))
            elif kind == 'shot':
                shot(action['name'])
            elif kind == 'wheel':
                move(hwnd, action['x'], action['y'])
                u.mouse_event(0x0800, 0, 0, action['delta'] & 0xffffffff, 0)
                time.sleep(0.3)
            else:
                raise ValueError(f'Unknown action: {kind}')
        summary['result'] = 'captured; visual review required'
    finally:
        if proc is not None and proc.poll() is None:
            for hwnd in windows_for_pid(proc.pid):
                u.PostMessageW(hwnd, 0x0010, 0, 0)
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.terminate()  # only the process created by this run
                proc.wait(timeout=5)
        if previous_bytes is not None:
            storage.write_bytes(previous_bytes)
            restored = storage.read_bytes() == previous_bytes
        else:
            storage.unlink(missing_ok=True)
            restored = not storage.exists()
        summary['preferences_restored'] = restored
        summary['preference_original_sha256'] = hashlib.sha256(previous_bytes).hexdigest() if previous_bytes else None
        (out / 'run.json').write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding='utf-8')
        u.SetCursorPos(cursor.x, cursor.y)
        if foreground: u.SetForegroundWindow(foreground)
        if not restored:
            raise RuntimeError('Application preference restoration failed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--input', type=Path)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--theme', choices=('light', 'dark'), default='light')
    parser.add_argument('--width', type=int, default=1280)
    parser.add_argument('--height', type=int, default=860)
    parser.add_argument('--actions', type=Path)
    run(parser.parse_args())

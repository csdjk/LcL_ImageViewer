"""Capture the real Windows viewer, never a mockup. Requires Python 3.10+ and Pillow.

Temporarily seeds ONLY this application's app.ron, then restores its exact bytes.
Use --isolated-profile with current binaries to leave running viewers and their settings untouched.
Without isolation, refuses to run while another viewer exists. Never clicks integration actions.
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
import re
from pathlib import Path
import subprocess
import time
import uuid
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

class MouseInput(c.Structure):
    _fields_ = [('dx', w.LONG), ('dy', w.LONG), ('data', w.DWORD),
                ('flags', w.DWORD), ('time', w.DWORD), ('extra', c.c_size_t)]

class KeyInput(c.Structure):
    _fields_ = [('key', w.WORD), ('scan', w.WORD), ('flags', w.DWORD), ('time', w.DWORD), ('extra', c.c_size_t)]

class InputUnion(c.Union):
    _fields_ = [('mouse', MouseInput), ('keyboard', KeyInput)]  # MouseInput is the largest variant.

class NativeInput(c.Structure):
    _fields_ = [('type', w.DWORD), ('value', InputUnion)]

bind(u, 'SendInput', [w.UINT, c.POINTER(NativeInput), c.c_int], w.UINT)
bind(u, 'GetSystemMetrics', [c.c_int], c.c_int)


def atomic_click(hwnd, x, y, right=False):
    """Queue absolute motion + down + up together so they cannot interleave."""
    focus(hwnd)
    _, _, origin, dpi = geometry(hwnd)
    vx, vy = u.GetSystemMetrics(76), u.GetSystemMetrics(77)
    vw, vh = u.GetSystemMetrics(78), u.GetSystemMetrics(79)
    if vw <= 1 or vh <= 1:
        raise RuntimeError('Invalid virtual desktop geometry')
    px, py = origin.x + round(x * dpi / 96), origin.y + round(y * dpi / 96)
    ax, ay = round((px-vx)*65535/(vw-1)), round((py-vy)*65535/(vh-1))
    down, up = (8, 16) if right else (2, 4)
    flags = [0xC001, down, up]  # MOVE | ABSOLUTE | VIRTUALDESK, then real buttons.
    batch = (NativeInput * 3)(*[NativeInput(0, InputUnion(mouse=MouseInput(
        ax if i==0 else 0, ay if i==0 else 0, 0, f, 0, 0))) for i,f in enumerate(flags)])
    if u.SendInput(3, batch, c.sizeof(NativeInput)) != 3:
        raise OSError(f'SendInput click failed: {c.get_last_error()}')


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
        # Different colors can have the same luminance (e.g. purple canvas and orange image).
        extrema = img.getextrema()
        if max(hi - lo for lo, hi in extrema) < 4 or max(hi for lo, hi in extrema) <= 4:
            raise RuntimeError('Blank or uniform capture; no usable UI evidence')
        img.save(output)
        return {'physical_window': [wr.left, wr.top, wr.right, wr.bottom],
                'physical_client': [cw, ch], 'logical_client': [cw * 96 / dpi, ch * 96 / dpi],
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
    bind(u, 'MapVirtualKeyW', [w.UINT, w.UINT], w.UINT)
    scan = u.MapVirtualKeyW(code, 0) & 0xff
    extended = 1 if code in (0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2d, 0x2e) else 0
    u.keybd_event(code, scan, extended, 0)
    try:
        time.sleep(0.06)
    finally:
        u.keybd_event(code, scan, extended | 2, 0)
    time.sleep(0.18)


def run(args):
    try:
        bind(u, 'SetProcessDpiAwarenessContext', [PTR], w.BOOL)(PTR(-4))
    except AttributeError:
        u.SetProcessDPIAware()
    binary = args.binary.resolve(strict=True)
    if binary.name.lower() != 'imageview.exe':
        raise ValueError('Expected the imageview.exe binary')
    out = args.output.resolve()
    out.mkdir(parents=True, exist_ok=False)
    child_env = os.environ.copy()
    child_env.pop('LCL_IV_QA_PROFILE', None)
    profile = uuid.uuid4().hex if args.isolated_profile else None
    if profile:
        # Only current binaries implement this marker and the opt-in isolated namespace.
        if b'LcL ImageViewer QA-' not in binary.read_bytes():
            raise RuntimeError('Binary does not advertise isolated QA profiles; refusing unsafe launch')
        child_env['LCL_IV_QA_PROFILE'] = profile
        app_id = 'LcL ImageViewer QA-' + profile
    else:
        existing = subprocess.run(['powershell', '-NoProfile', '-Command',
            "@(Get-Process -Name imageview -ErrorAction SilentlyContinue).Count"],
            capture_output=True, text=True, check=True)
        if int(existing.stdout.strip()) != 0:
            raise RuntimeError('Another imageview is running; use --isolated-profile with a supported binary')
        app_id = 'LcL ImageViewer'
    storage = Path(os.environ['APPDATA']) / app_id / 'data' / 'app.ron'
    if profile and storage.parent.parent.exists():
        raise RuntimeError('QA profile already exists; refusing to overwrite it')
    previous_bytes = storage.read_bytes() if storage.exists() else None
    if previous_bytes is not None:
        (out / 'app.ron.original').write_bytes(previous_bytes)
    storage.parent.mkdir(parents=True, exist_ok=True)
    foreground, cursor = u.GetForegroundWindow(), w.POINT()
    u.GetCursorPos(c.byref(cursor))
    proc = None
    history = []
    pressed_releases = set()
    summary = {'theme': args.theme, 'binary': str(binary), 'binary_sha256': sha(binary),
               'commit': args.commit, 'input': str(args.input.resolve()) if args.input else None,
               'input_sha256': sha(args.input) if args.input else None, 'captures': [],
               'isolated_profile': profile, 'preference_path': str(storage)}
    try:
        # RON maps accept the same simple string key/value syntax as this JSON subset.
        prefs = {'iv-theme': args.theme, 'iv-backdrop': 'off', 'iv-checkerboard': 'on', 'iv-reduce-motion': 'off'}
        seed = getattr(args, 'seed_preferences', {})
        allowed = {'iv-always-on-top', 'iv-background-color', 'iv-checkerboard', 'iv-backdrop', 'iv-theme',
                   'iv-auto-refresh', 'iv-lock-view'}
        if not set(seed) <= allowed or not all(isinstance(v, str) for v in seed.values()):
            raise ValueError('Only explicit appearance preference values can be seeded')
        prefs.update(seed)
        storage.write_text(json.dumps(prefs), encoding='utf-8')
        with (out / 'runtime.log').open('wb') as log:
            proc = subprocess.Popen([str(binary)] + ([str(args.input.resolve())] if args.input else []),
                                    stdout=log, stderr=log, cwd=binary.parent, env=child_env)
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
        if profile and ('QA isolated profile: ' + app_id) not in (out / 'runtime.log').read_text(encoding='utf-8', errors='replace'):
            raise RuntimeError('Viewer did not confirm isolated persistence profile')
        wr, cr, _, dpi = geometry(hwnd)
        dw = (wr.right - wr.left) - (cr.right - cr.left)
        dh = (wr.bottom - wr.top) - (cr.bottom - cr.top)
        checked(u.SetWindowPos(hwnd, None, 40, 40, round(args.width * dpi / 96) + dw,
                               round(args.height * dpi / 96) + dh, 0x0040), 'Resize viewer')
        time.sleep(0.6)
        move(hwnd, args.width / 2, args.height / 2)
        current_theme = args.theme
        def shot(name):
            title = c.create_unicode_buffer(1024)
            bind(u, 'GetWindowTextW', [w.HWND, w.LPWSTR, c.c_int], c.c_int)(hwnd, title, len(title))
            filename = out / f'{name}.png'
            meta = capture(hwnd, filename)
            get_style = bind(u, 'GetWindowLongPtrW', [w.HWND, c.c_int], c.c_ssize_t)
            meta['window_topmost'] = bool(get_style(hwnd, -20) & 8)
            if meta['logical_client'] != [args.width, args.height]:
                raise RuntimeError(f'Client size mismatch: {meta}')
            meta.update({key: value for key, value in summary.items() if key != 'captures'})
            if args.input:
                meta['input_sha256'] = sha(args.input) if args.input.is_file() else None
            meta.update({'theme': current_theme, 'state': name, 'actions': list(history),
                         'utc': datetime.now(timezone.utc).isoformat(), 'pid': proc.pid, 'window_title': title.value,
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
                if kind in ('click', 'right-click') and 'x' in action:
                    # Establish hover first; the absolute click batch reasserts its
                    # position immediately at button-down instead of trusting a stale cursor.
                    move(hwnd, action['x'], action['y'])
                    atomic_click(hwnd, action['x'], action['y'], kind == 'right-click')
                else:
                    if 'x' in action:
                        move(hwnd, action['x'], action['y'])
                    focus(hwnd)
                    down, up = (8, 16) if kind == 'right-click' else (2, 4)
                    if kind != 'up':
                        u.mouse_event(down, 0, 0, 0, 0)
                        pressed_releases.add(up)
                    if kind != 'down':
                        u.mouse_event(up, 0, 0, 0, 0)
                        pressed_releases.discard(up)
                time.sleep(0.15)
                if action.get('theme') in ('light', 'dark'):
                    current_theme = action['theme']
            elif kind == 'drag':
                # Client start, then absolute screen path: never chase the moving window.
                button = action.get('button', 'left')
                down, up = {'left': (2, 4), 'right': (8, 16), 'middle': (32, 64)}[button]
                move(hwnd, action['x'], action['y'])
                wr, cr, origin, dpi = geometry(hwnd)
                before = [wr.left, wr.top, cr.right, cr.bottom]
                start = w.POINT()
                checked(u.GetCursorPos(c.byref(start)), 'Read drag start')
                u.mouse_event(down, 0, 0, 0, 0)
                pressed_releases.add(up)
                path = action.get('path', [[action.get('dx', 0), action.get('dy', 0)]])
                prior = [0, 0]
                trace = []
                try:
                    time.sleep(0.1)
                    for end in path:
                        for step in range(1, 17):
                            x = prior[0] + (end[0] - prior[0]) * step / 16
                            y = prior[1] + (end[1] - prior[1]) * step / 16
                            checked(u.SetCursorPos(start.x + round(x * dpi / 96),
                                                  start.y + round(y * dpi / 96)), 'Drag cursor')
                            time.sleep(0.025)
                            r, _, _, _ = geometry(hwnd)
                            trace.append([r.left, r.top])
                        prior = end
                    time.sleep(0.08)
                finally:
                    u.mouse_event(up, 0, 0, 0, 0)
                    pressed_releases.discard(up)
                time.sleep(0.18)
                wr, cr, _, _ = geometry(hwnd)
                actual = [wr.left - before[0], wr.top - before[1], cr.right - before[2], cr.bottom - before[3]]
                action['window_delta_physical'] = actual
                action['window_trace'] = trace
                if 'expect_window_delta' in action:
                    expected = [round(v * dpi / 96) for v in action['expect_window_delta']]
                    if any(abs(a-b)>3 for a,b in zip(actual, expected)):
                        raise AssertionError(f'Drag {button}: window delta {actual}, expected {expected}')
            elif kind == 'text':
                # Unicode input bypasses the active IME without changing its language,
                # clipboard contents or global settings. Only the owned QA viewer is targeted.
                text = action['text']
                if not isinstance(text, str) or len(text) > 256:
                    raise ValueError('QA text must be a string of at most 256 characters')
                focus(hwnd)
                encoded = text.encode('utf-16-le')
                for i in range(0, len(encoded), 2):
                    if u.GetForegroundWindow() != hwnd:
                        raise RuntimeError('Foreground changed; refusing text input to another app')
                    unit = int.from_bytes(encoded[i:i+2], 'little')
                    batch = (NativeInput * 2)(*[NativeInput(1, InputUnion(keyboard=KeyInput(0, unit, flags, 0, 0))) for flags in (4, 6)])
                    if u.SendInput(2, batch, c.sizeof(NativeInput)) != 2:
                        raise OSError('Unicode input failed')
                    time.sleep(0.04)
                time.sleep(0.15)
            elif kind == 'key':
                code = action['code']
                code = ord(code.upper()) if isinstance(code, str) and len(code) == 1 else int(code)
                held_modifiers = []
                try:
                    for name in action.get('modifiers', []):
                        virtual = {'ctrl': 0x11, 'shift': 0x10, 'alt': 0x12}[name]
                        u.keybd_event(virtual, 0, 0, 0)
                        held_modifiers.append(virtual)
                    key(hwnd, code)
                finally:
                    for virtual in reversed(held_modifiers):
                        u.keybd_event(virtual, 0, 2, 0)
                if code == ord('T') and not action.get('modifiers'):
                    current_theme = 'dark' if current_theme == 'light' else 'light'
            elif kind == 'open-dialog-cancel':
                # Inspect only this new process's HWNDs; no dialog screenshot or file selection.
                old_pin = bool(bind(u, 'GetWindowLongPtrW', [w.HWND, c.c_int], c.c_ssize_t)(hwnd, -20) & 8)
                focus(hwnd)
                u.keybd_event(0x11, 0, 0, 0)
                try: key(hwnd, ord('O'))
                finally: u.keybd_event(0x11, 0, 2, 0)
                dialog = None
                try:
                    deadline = time.monotonic() + 12
                    while time.monotonic() < deadline:
                        found = [h for h in windows_for_pid(proc.pid) if h != hwnd]
                        if len(found) == 1: dialog = found[0]; break
                        time.sleep(0.1)
                    assert dialog is not None, 'File dialog was not created'
                    focus(dialog)
                    assert u.GetForegroundWindow() == dialog
                    get_owner = bind(u, 'GetWindow', [w.HWND,w.UINT], w.HWND)
                    assert get_owner(dialog,4) == hwnd, 'Dialog must be owned by this viewer'
                    assert bool(u.GetWindowLongPtrW(hwnd, -20) & 8) == old_pin
                    assert bool(u.GetWindowLongPtrW(dialog, -20) & 8) == old_pin, 'Owned dialog must inherit topmost level'
                    key(dialog, 27)
                    time.sleep(0.4)
                    assert dialog not in windows_for_pid(proc.pid)
                    is_window = bind(u, 'IsWindow', [w.HWND], w.BOOL)
                    is_enabled = bind(u, 'IsWindowEnabled', [w.HWND], w.BOOL)
                    print('DIALOG_CANCEL_STATE', {'exit':proc.poll(), 'main_exists':bool(is_window(hwnd)), 'main_enabled':bool(is_enabled(hwnd)), 'own_visible_windows':windows_for_pid(proc.pid)}, flush=True)
                    focus(hwnd)
                    assert bool(u.GetWindowLongPtrW(hwnd, -20) & 8) == old_pin
                    summary['file_dialog_cancel_keeps_pin'] = True
                finally:
                    if dialog and dialog in windows_for_pid(proc.pid): u.PostMessageW(dialog, 0x0010, 0, 0)
            elif kind == 'assert-topmost':
                get_style = bind(u, 'GetWindowLongPtrW', [w.HWND, c.c_int], c.c_ssize_t)
                actual = bool(get_style(hwnd, -20) & 8)
                assert actual == action['value'], ('WS_EX_TOPMOST', actual, action['value'])
            elif kind == 'minimize-restore':
                show = bind(u, 'ShowWindow', [w.HWND, c.c_int], w.BOOL)
                iconic = bind(u, 'IsIconic', [w.HWND], w.BOOL)
                show(hwnd, 6); time.sleep(0.25)
                assert iconic(hwnd), 'test window was not minimized'
                show(hwnd, 9); time.sleep(0.35); focus(hwnd)
                assert not iconic(hwnd), 'test window was not restored'
            elif kind == 'resize-client':
                wr, cr, _, dpi = geometry(hwnd)
                args.width, args.height = int(action['width']), int(action['height'])
                dw, dh = wr.right-wr.left-cr.right, wr.bottom-wr.top-cr.bottom
                checked(u.SetWindowPos(hwnd, None, wr.left, wr.top,
                    round(args.width*dpi/96)+dw, round(args.height*dpi/96)+dh,
                    0x0004 | 0x0010), 'Resize without changing Z order')
                time.sleep(0.4)
            elif kind == 'assert-z-order':
                # An inert peer owned by this QA process, not an existing user application.
                create = bind(u, 'CreateWindowExW', [w.DWORD,w.LPCWSTR,w.LPCWSTR,w.DWORD,
                    c.c_int,c.c_int,c.c_int,c.c_int,w.HWND,w.HMENU,w.HINSTANCE,PTR], w.HWND)
                destroy = bind(u, 'DestroyWindow', [w.HWND], w.BOOL)
                peer = create(0, 'STATIC', 'LcL isolated Z-order QA', 0x90000000,
                    80, 140, 180, 120, None, None, None, None)
                checked(peer, 'Create isolated peer')
                try:
                    focus(peer); time.sleep(0.2)
                    order = []
                    @ENUM
                    def collect(h, _):
                        if h in (hwnd, peer): order.append(h)
                        return True
                    checked(u.EnumWindows(collect, 0), 'Inspect test-window stacking')
                    assert len(order) == 2
                    assert (order.index(hwnd) < order.index(peer)) == action['value'], ('Z-order', order)
                finally:
                    destroy(peer); focus(hwnd)
            elif kind == 'burst':
                # Record the real native window at bounded intervals, never fabricate animation.
                count = int(action['count'])
                interval = float(action.get('interval', 0.1))
                if not 1 <= count <= 120 or not 0.05 <= interval <= 1.0:
                    raise ValueError('Capture burst exceeds its safe bounds')
                focus(hwnd)
                _, _, origin, dpi = geometry(hwnd)
                started = time.monotonic()
                for index in range(count):
                    time.sleep(max(0.0, started + index * interval - time.monotonic()))
                    if u.GetForegroundWindow() != hwnd:
                        raise RuntimeError('Foreground changed while recording this viewer')
                    if action.get('keep_toolbar'):
                        checked(u.SetCursorPos(origin.x + round((30 + index % 2 * 4) * dpi / 96),
                                               origin.y + round(135 * dpi / 96)), 'Keep toolbar visible')
                    shot(f"{action['name']}-{index:03}")
            elif kind == 'wait':
                time.sleep(float(action['seconds']))
            elif kind == 'shot':
                shot(action['name'])
            elif kind == 'wheel':
                move(hwnd, action['x'], action['y'])
                u.mouse_event(0x0800, 0, 0, action['delta'] & 0xffffffff, 0)
                time.sleep(0.3)
            else:
                handler = getattr(args, 'action_handler', None)
                if handler is None:
                    raise ValueError(f'Unknown action: {kind}')
                handler(action, hwnd, shot)
        summary['result'] = 'captured; visual review required'
    finally:
        for release in pressed_releases:
            u.mouse_event(release, 0, 0, 0, 0)
        if proc is not None and proc.poll() is None:
            for hwnd in windows_for_pid(proc.pid):
                u.PostMessageW(hwnd, 0x0010, 0, 0)
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.terminate()  # only the process created by this run
                proc.wait(timeout=5)
        # Export only known settings from an explicitly isolated test profile.
        if profile and storage.is_file():
            summary['qa_saved_settings'] = dict(re.findall(
                r'"(iv-[^"\\]+)":\s*"([^"\\]*)"', storage.read_text(encoding='utf-8')))
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
    parser.add_argument('--isolated-profile', action='store_true', help='Use a new QA-only preference namespace; leave running viewers untouched')
    run(parser.parse_args())

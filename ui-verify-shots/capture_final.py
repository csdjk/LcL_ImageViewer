import ctypes, time
from ctypes import wintypes as w
from PIL import Image
u = ctypes.windll.user32; g = ctypes.windll.gdi32
u.SetProcessDPIAware()
hwnd = 1119468
OUT = r"E:\LcL\ImageView\ui-verify-shots"

class BIH(ctypes.Structure):
    _fields_ = [("biSize", w.DWORD), ("biWidth", w.LONG), ("biHeight", w.LONG), ("biPlanes", w.WORD), ("biBitCount", w.WORD), ("biCompression", w.DWORD), ("biSizeImage", w.DWORD), ("x", w.LONG), ("y", w.LONG), ("c1", w.DWORD), ("c2", w.DWORD)]
class BI(ctypes.Structure):
    _fields_ = [("h", BIH), ("c", w.DWORD * 3)]

def rect():
    r = w.RECT(); u.GetWindowRect(hwnd, ctypes.byref(r)); return r

def grab(path):
    r = rect(); W, H = r.right - r.left, r.bottom - r.top
    hdcS = u.GetDC(0); hdcM = g.CreateCompatibleDC(hdcS)
    hbm = g.CreateCompatibleBitmap(hdcS, W, H); g.SelectObject(hdcM, hbm)
    u.PrintWindow(hwnd, hdcM, 2)
    bi = BI(); bi.h.biSize = 40; bi.h.biWidth = W; bi.h.biHeight = -H; bi.h.biPlanes = 1; bi.h.biBitCount = 32
    buf = (ctypes.c_ubyte * (W * H * 4))()
    g.GetDIBits(hdcM, hbm, 0, H, buf, ctypes.byref(bi), 0)
    g.DeleteObject(hbm); g.DeleteDC(hdcM); u.ReleaseDC(0, hdcS)
    Image.frombuffer("RGBA", (W, H), bytes(buf), "raw", "BGRA", 0, 1).crop((9, 0, 1933, 1337)).convert("RGB").save(path)

def mv(x, y):
    u.mouse_event(0x8001, int(x * 65535 / 3839), int(y * 65535 / 2159), 0, 0)

r = rect()
cx = r.left + (r.right - r.left) // 2
cy = r.top + ((r.bottom - r.top) + 45) // 2
print("canvas center:", cx, cy)

# step 2: move mouse to canvas center (no click)
mv(cx - 1, cy); time.sleep(0.02); mv(cx, cy)
t0 = time.time()

def at(t, name):
    d = t0 + t - time.time()
    if d > 0: time.sleep(d)
    grab(OUT + "\\" + name)
    print(name, "capture t=%.3f" % (t0 + t - t0), "actual=%.3f" % (time.time() - t0))

at(0.5, "stable-1.png")   # mouse stays absolutely still afterwards
at(1.0, "stable-2.png")   # +0.5s
at(1.5, "stable-3.png")   # +0.5s, still inside visible window (<1.6s)

# step 5: slow move to +100px right of canvas center
for i in range(1, 11):
    mv(cx + i * 10, cy); time.sleep(0.06)
time.sleep(0.45)          # one fade-in (~0.4s) so capsule is fully rendered
grab(OUT + "\\stable-4.png")
p = w.POINT(); u.GetCursorPos(ctypes.byref(p))
print("stable-4.png done; final cursor:", p.x, p.y, "expected:", cx + 100, cy)

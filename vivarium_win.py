#!/usr/bin/env python3
"""Claude Vivarium for Windows - a real Windows screensaver (.scr).

Windows calls a screensaver with:
  /s            show full screen on all monitors (main monitor: labels, banner, strip, panel;
                other monitors: another theme as decoration)
  /p <hwnd>     draw the small preview into the Screen Saver Settings dialog
  /c[:hwnd]     the "Settings..." button -> opens config.ini
Locking on wake is done by Windows itself ("On resume, display logon screen").

Extra commands:
  --windowed [--demo] [--themes x]   test in a normal window (N/Right/Space = next theme, Esc = close)
  --record                           CPU/memory recorder for the 30-minute graph (started at logon)
  --install / --uninstall            install for the current user (no admin rights needed)
"""
import ctypes
import os
import shutil
import subprocess
import sys
import threading
import time
from ctypes import wintypes

HERE = getattr(sys, "_MEIPASS", None) or os.path.dirname(os.path.realpath(__file__))
sys.path.insert(0, HERE)

import cairo  # noqa: E402

import config  # noqa: E402
import sessions as sessions_mod  # noqa: E402
import sysmon  # noqa: E402
import themes  # noqa: E402

APP_NAME = "ClaudeVivarium"
INSTALL_DIR = os.path.join(os.environ.get("LOCALAPPDATA", ""), "Programs", APP_NAME)
LOG_PATH = os.path.join(os.environ.get("LOCALAPPDATA", ""), APP_NAME, "vivarium.log")

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
kernel32 = ctypes.windll.kernel32

LRESULT = ctypes.c_ssize_t
HCURSOR = wintypes.HANDLE
WNDPROC = ctypes.WINFUNCTYPE(LRESULT, wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM)
MONITORENUMPROC = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HANDLE, wintypes.HDC, ctypes.POINTER(wintypes.RECT),
                                     wintypes.LPARAM)


class WNDCLASSEXW(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.UINT), ("style", wintypes.UINT), ("lpfnWndProc", WNDPROC),
                ("cbClsExtra", ctypes.c_int), ("cbWndExtra", ctypes.c_int), ("hInstance", wintypes.HINSTANCE),
                ("hIcon", wintypes.HICON), ("hCursor", HCURSOR), ("hbrBackground", wintypes.HBRUSH),
                ("lpszMenuName", wintypes.LPCWSTR), ("lpszClassName", wintypes.LPCWSTR), ("hIconSm", wintypes.HICON)]


class MONITORINFO(ctypes.Structure):
    _fields_ = [("cbSize", wintypes.DWORD), ("rcMonitor", wintypes.RECT), ("rcWork", wintypes.RECT),
                ("dwFlags", wintypes.DWORD)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [("biSize", wintypes.DWORD), ("biWidth", wintypes.LONG), ("biHeight", wintypes.LONG),
                ("biPlanes", wintypes.WORD), ("biBitCount", wintypes.WORD), ("biCompression", wintypes.DWORD),
                ("biSizeImage", wintypes.DWORD), ("biXPelsPerMeter", wintypes.LONG),
                ("biYPelsPerMeter", wintypes.LONG), ("biClrUsed", wintypes.DWORD), ("biClrImportant", wintypes.DWORD)]


user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.DefWindowProcW.restype = LRESULT
user32.CreateWindowExW.argtypes = [wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD, ctypes.c_int,
                                   ctypes.c_int, ctypes.c_int, ctypes.c_int, wintypes.HWND, wintypes.HMENU,
                                   wintypes.HINSTANCE, wintypes.LPVOID]
user32.CreateWindowExW.restype = wintypes.HWND
user32.RegisterClassExW.argtypes = [ctypes.POINTER(WNDCLASSEXW)]
user32.RegisterClassExW.restype = wintypes.ATOM
user32.GetDC.argtypes = [wintypes.HWND]
user32.GetDC.restype = wintypes.HDC
user32.ReleaseDC.argtypes = [wintypes.HWND, wintypes.HDC]
user32.PeekMessageW.argtypes = [ctypes.POINTER(wintypes.MSG), wintypes.HWND, wintypes.UINT, wintypes.UINT,
                                wintypes.UINT]
user32.TranslateMessage.argtypes = [ctypes.POINTER(wintypes.MSG)]
user32.DispatchMessageW.argtypes = [ctypes.POINTER(wintypes.MSG)]
user32.DispatchMessageW.restype = LRESULT
user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.IsWindow.argtypes = [wintypes.HWND]
user32.DestroyWindow.argtypes = [wintypes.HWND]
user32.SetCursor.argtypes = [HCURSOR]
user32.LoadCursorW.argtypes = [wintypes.HINSTANCE, wintypes.LPVOID]
user32.LoadCursorW.restype = HCURSOR
user32.EnumDisplayMonitors.argtypes = [wintypes.HDC, wintypes.LPVOID, MONITORENUMPROC, wintypes.LPARAM]
user32.GetMonitorInfoW.argtypes = [wintypes.HANDLE, ctypes.POINTER(MONITORINFO)]
user32.ValidateRect.argtypes = [wintypes.HWND, wintypes.LPVOID]
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
user32.SystemParametersInfoW.argtypes = [wintypes.UINT, wintypes.UINT, wintypes.LPVOID, wintypes.UINT]
gdi32.SetDIBitsToDevice.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.DWORD, wintypes.DWORD,
                                    ctypes.c_int, ctypes.c_int, wintypes.UINT, wintypes.UINT, ctypes.c_void_p,
                                    ctypes.c_void_p, wintypes.UINT]
gdi32.StretchDIBits.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p,
                                wintypes.UINT, wintypes.DWORD]
gdi32.SetStretchBltMode.argtypes = [wintypes.HDC, ctypes.c_int]
gdi32.SetBrushOrgEx.argtypes = [wintypes.HDC, ctypes.c_int, ctypes.c_int, wintypes.LPVOID]
gdi32.GetStockObject.restype = wintypes.HGDIOBJ
kernel32.GetModuleHandleW.argtypes = [wintypes.LPCWSTR]
kernel32.GetModuleHandleW.restype = wintypes.HMODULE
kernel32.CreateMutexW.argtypes = [wintypes.LPVOID, wintypes.BOOL, wintypes.LPCWSTR]
kernel32.CreateMutexW.restype = wintypes.HANDLE

WM_DESTROY, WM_CLOSE, WM_PAINT, WM_ERASEBKGND, WM_ACTIVATEAPP, WM_SETCURSOR = 0x02, 0x10, 0x0F, 0x14, 0x1C, 0x20
WM_KEYDOWN, WM_SYSKEYDOWN, WM_SYSCOMMAND, WM_MOUSEMOVE = 0x100, 0x104, 0x112, 0x200
WM_BUTTONS = (0x201, 0x204, 0x207, 0x20B)  # left, right, middle, x button down
WM_MOUSEWHEEL, WM_MOUSEHWHEEL = 0x20A, 0x20E
SC_SCREENSAVE, SC_MONITORPOWER = 0xF140, 0xF170
WS_POPUP, WS_VISIBLE, WS_CHILD, WS_OVERLAPPEDWINDOW = 0x80000000, 0x10000000, 0x40000000, 0x00CF0000
WS_EX_TOPMOST, WS_EX_TOOLWINDOW = 0x00000008, 0x00000080
PM_REMOVE = 1
VK_ESCAPE, VK_SPACE, VK_RIGHT, VK_N = 0x1B, 0x20, 0x27, 0x4E

GRACE = 1.0          # s after start during which input is ignored
MOTION_THRESHOLD = 25
MAX_RENDER_PIXELS = 1e9  # drawing at native size is cheaper than any smooth up-scaling (measured)
FPS = 30


def render_size_for(w, h):
    k = min(1.0, (MAX_RENDER_PIXELS / (w * h)) ** 0.5)
    return None if k > 0.97 else (int(w * k), int(h * k))


def log(msg):
    try:
        os.makedirs(os.path.dirname(LOG_PATH), exist_ok=True)
        with open(LOG_PATH, "a", encoding="utf-8") as f:
            f.write(time.strftime("%Y-%m-%d %H:%M:%S ") + msg + "\n")
    except OSError:
        pass


def say(msg):
    """Show a message: console if there is one, otherwise a message box (the built .exe has no console)."""
    log(msg.replace("\n", " | "))
    if sys.stdout is not None:
        print(msg)
    else:
        user32.MessageBoxW(None, msg, "Claude Vivarium", 0x40)


def dpi_aware():
    try:
        user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))  # per monitor v2: real pixels
    except (AttributeError, OSError):
        user32.SetProcessDPIAware()


def monitors():
    """[(x, y, w, h, primary)] in physical pixels."""
    found = []

    def cb(hmon, _hdc, _rect, _data):
        mi = MONITORINFO()
        mi.cbSize = ctypes.sizeof(MONITORINFO)
        user32.GetMonitorInfoW(hmon, ctypes.byref(mi))
        r = mi.rcMonitor
        found.append((r.left, r.top, r.right - r.left, r.bottom - r.top, bool(mi.dwFlags & 1)))
        return True
    user32.EnumDisplayMonitors(None, None, MONITORENUMPROC(cb), 0)
    return sorted(found, key=lambda m: not m[4])  # primary first


class Canvas:
    """One window and the Cairo image it shows. The scene can be rendered at a different size (preview)."""

    def __init__(self, hwnd, show, render_size=None, every=1):
        self.hwnd = hwnd
        self.show = show
        self.render_size = render_size
        self.every = every       # render only every n-th frame (extra monitors), dt is accumulated
        self.acc = 0.0
        self.frame = 0
        self.size = (0, 0)
        self.img = self.cr = self.big = self.big_cr = None
        self.bmi = BITMAPINFOHEADER()
        self.painted_black = False
        self.lock = threading.Lock()
        self.threaded = False
        self.frames = 0

    def _ensure(self):
        rc = wintypes.RECT()
        user32.GetClientRect(self.hwnd, ctypes.byref(rc))
        size = (max(1, rc.right - rc.left), max(1, rc.bottom - rc.top))
        if size != self.size:
            self.size = size
            # the scene is drawn at render_size (smaller on big monitors); GDI stretches it onto the window
            self.img_size = self.render_size or size
            self.img = cairo.ImageSurface(cairo.FORMAT_RGB24, *self.img_size)
            self.cr = cairo.Context(self.img)
            iw, ih = self.img_size
            self.bmi = BITMAPINFOHEADER(ctypes.sizeof(BITMAPINFOHEADER), iw, -ih, 1, 32, 0, 0, 0, 0, 0, 0)
            self.painted_black = False

    def render(self, dt):
        self._ensure()
        self.acc += dt
        self.frame += 1
        if self.frame % self.every:
            return
        dt, self.acc = self.acc, 0.0
        if self.show is None:  # black monitor: paint once
            if self.painted_black:
                return
            self.cr.set_source_rgb(0, 0, 0)
            self.cr.paint()
            self.painted_black = True
        else:
            self.show.update(dt)
            self.show.draw(self.cr, *self.img_size)
        self.blit()

    def blit(self):
        self.img.flush()
        buf = self.img.get_data()
        ptr = (ctypes.c_char * len(buf)).from_buffer(buf)
        (w, h), (iw, ih) = self.size, self.img_size
        hdc = user32.GetDC(self.hwnd)
        if (iw, ih) == (w, h):
            gdi32.SetDIBitsToDevice(hdc, 0, 0, w, h, 0, 0, 0, h, ptr, ctypes.byref(self.bmi), 0)
        else:
            gdi32.SetStretchBltMode(hdc, 4)  # HALFTONE: smooth scaling
            gdi32.SetBrushOrgEx(hdc, 0, 0, None)
            gdi32.StretchDIBits(hdc, 0, 0, w, h, 0, 0, iw, ih, ptr, ctypes.byref(self.bmi), 0, 0x00CC0020)
        user32.ReleaseDC(self.hwnd, hdc)


class App:
    def __init__(self, mode, opts, cfg, parent=None):
        self.mode = mode          # "full", "preview", "windowed"
        self.opts = opts
        self.cfg = cfg
        self.parent = parent
        self.running = True
        self.started = time.monotonic()
        self.canvases = []
        self.main_show = None
        self.mouse_origin = None
        self.monitor = sysmon.Monitor() if cfg["system_monitor"] else None
        self.demo = opts.get("demo", False)
        self._wndproc = WNDPROC(self.wndproc)  # keep a reference, or it is garbage-collected
        self.hinst = kernel32.GetModuleHandleW(None)
        wc = WNDCLASSEXW()
        wc.cbSize = ctypes.sizeof(WNDCLASSEXW)
        wc.lpfnWndProc = self._wndproc
        wc.hInstance = self.hinst
        wc.hbrBackground = gdi32.GetStockObject(4)  # black brush
        wc.hCursor = user32.LoadCursorW(None, ctypes.c_void_p(32512)) if mode == "windowed" else None
        wc.lpszClassName = APP_NAME
        user32.RegisterClassExW(ctypes.byref(wc))

    @property
    def fullscreen(self):
        return self.mode == "full"

    def in_grace(self):
        return time.monotonic() - self.started < GRACE

    def make_show(self, decor=False, avoid=None):
        names = themes.parse_list(self.opts.get("themes") or self.cfg["themes"])
        return themes.Show(names, self.cfg["theme_minutes"], show_text=not self.cfg["hide_text"],
                           decor=decor, avoid=avoid)

    def create_windows(self):
        if self.mode == "full":
            for i, (x, y, w, h, primary) in enumerate(monitors()):
                hwnd = user32.CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW, APP_NAME, "Claude Vivarium",
                                              WS_POPUP | WS_VISIBLE, x, y, w, h, None, None, self.hinst, None)
                if i == 0:
                    self.main_show = self.make_show()
                    self.canvases.append(Canvas(hwnd, self.main_show, render_size=render_size_for(w, h)))
                    user32.SetForegroundWindow(hwnd)
                elif self.cfg["other_monitors"] == "theme":
                    self.canvases.append(Canvas(hwnd, self.make_show(decor=True, avoid=self.main_show),
                                                render_size=render_size_for(w, h)))
                else:
                    self.canvases.append(Canvas(hwnd, None))
            user32.ShowCursor(False)
        elif self.mode == "preview":
            rc = wintypes.RECT()
            user32.GetClientRect(self.parent, ctypes.byref(rc))
            hwnd = user32.CreateWindowExW(0, APP_NAME, "preview", WS_CHILD | WS_VISIBLE, 0, 0,
                                          rc.right - rc.left, rc.bottom - rc.top, self.parent, None, self.hinst, None)
            self.main_show = self.make_show()
            self.canvases.append(Canvas(hwnd, self.main_show, render_size=(800, 500)))
        else:
            hwnd = user32.CreateWindowExW(0, APP_NAME, "Claude Vivarium (preview - N: next theme, Esc: close)",
                                          WS_OVERLAPPEDWINDOW | WS_VISIBLE, 80, 80, 1440, 900, None, None,
                                          self.hinst, None)
            self.main_show = self.make_show()
            self.canvases.append(Canvas(hwnd, self.main_show))

    def quit(self, reason):
        if self.running:
            log(f"exit: {reason}")
        self.running = False

    # ------------------------------------------------------------------ input
    def wndproc(self, hwnd, msg, wp, lp):
        try:
            r = self.on_message(hwnd, msg, wp, lp)
        except Exception as e:  # never let an exception escape into Windows
            log(f"wndproc error: {e!r}")
            r = None
        if r is not None:
            return r
        return user32.DefWindowProcW(hwnd, msg, wp, lp)

    def on_message(self, hwnd, msg, wp, lp):
        if msg == WM_SETCURSOR and self.fullscreen:
            user32.SetCursor(None)
            return 1
        if msg == WM_ERASEBKGND:
            return 1
        if msg == WM_PAINT:
            user32.ValidateRect(hwnd, None)
            return 0
        if msg == WM_SYSCOMMAND and (wp & 0xFFF0) in (SC_SCREENSAVE, SC_MONITORPOWER) and self.fullscreen:
            return 0  # do not start another screensaver on top of us
        if msg in (WM_CLOSE, WM_DESTROY):
            self.quit("window closed")
            return 0
        if self.mode == "preview":
            return None
        if msg in (WM_KEYDOWN, WM_SYSKEYDOWN):
            if self.mode == "windowed":
                if wp == VK_ESCAPE:
                    self.quit("escape")
                elif wp in (VK_N, VK_RIGHT, VK_SPACE):
                    for c in self.canvases:
                        if c.show:
                            c.show.next_theme()
                return 0
            if not self.in_grace():
                self.quit(f"key {wp}")
            return 0
        if not self.fullscreen:
            return None
        if msg in WM_BUTTONS or msg in (WM_MOUSEWHEEL, WM_MOUSEHWHEEL):
            if not self.in_grace():
                self.quit("mouse button")
            return 0
        if msg == WM_MOUSEMOVE:
            pt = wintypes.POINT()
            user32.GetCursorPos(ctypes.byref(pt))
            if self.in_grace() or self.mouse_origin is None:
                self.mouse_origin = (pt.x, pt.y)
            elif abs(pt.x - self.mouse_origin[0]) + abs(pt.y - self.mouse_origin[1]) > MOTION_THRESHOLD:
                if self.cfg["mouse_move"] == "hint":
                    self.mouse_origin = (pt.x, pt.y)
                    self.main_show.show_hint("Press a key or click to unlock")
                else:
                    self.quit("mouse moved")
            return 0
        if msg == WM_ACTIVATEAPP and not wp and not self.in_grace():
            self.quit("lost focus")
            return 0
        return None

    # ------------------------------------------------------------------ main loop
    def refresh(self):
        try:
            data = sessions_mod.demo_sessions() if self.demo else sessions_mod.read_sessions()
        except Exception as e:
            log(f"reading sessions failed: {e!r}")
            data = []
        system = None
        if self.monitor is not None:
            try:
                system = self.monitor.get()
            except Exception as e:
                log(f"reading system stats failed: {e!r}")
        for c in self.canvases:
            if c.show is not None:
                with c.lock:
                    c.show.set_sessions(data)
                    if c.show is self.main_show:
                        c.show.set_system(system)

    def render_loop(self, canvas):
        """Extra monitors draw in their own thread - Cairo releases the GIL, so this uses a second core."""
        last = time.perf_counter()
        while self.running:
            t0 = time.perf_counter()
            try:
                with canvas.lock:
                    canvas.render(t0 - last)
            except Exception as e:
                log(f"render thread error: {e!r}")
                return
            last = t0
            canvas.frames += 1
            spare = 1.0 / FPS - (time.perf_counter() - t0)
            if spare > 0:
                time.sleep(spare)

    def run(self):
        self.create_windows()
        log(f"start: mode={self.mode}, monitors={len(self.canvases)}, theme={self.main_show.name}")
        if self.fullscreen:
            # keep the display on while the screensaver is shown (like the GNOME version); max_minutes ends it
            kernel32.SetThreadExecutionState(0x80000000 | 0x00000002)  # ES_CONTINUOUS | ES_DISPLAY_REQUIRED
        ctypes.windll.winmm.timeBeginPeriod(1)
        fps = 15 if self.mode == "preview" else FPS
        frame_time = 1.0 / fps
        msg = wintypes.MSG()
        last = time.perf_counter()
        next_refresh = 0.0
        max_minutes = self.cfg["max_minutes"] if self.fullscreen else 0
        stats_t, stats_n = time.perf_counter(), 0
        test = os.environ.get("VIVARIUM_TEST")  # "<seconds>;<folder>": save each window as PNG, then exit
        test_secs, test_dir = (float(test.split(";")[0]), test.split(";")[1]) if test else (0, "")
        frames = 0
        threads = []
        for c in self.canvases[1:]:
            if c.show is not None:
                c.threaded = True
                th = threading.Thread(target=self.render_loop, args=(c,), daemon=True)
                th.start()
                threads.append(th)
        try:
            while self.running:
                while user32.PeekMessageW(ctypes.byref(msg), None, 0, 0, PM_REMOVE):
                    user32.TranslateMessage(ctypes.byref(msg))
                    user32.DispatchMessageW(ctypes.byref(msg))
                if self.mode == "preview" and not user32.IsWindow(self.parent):
                    break
                now = time.perf_counter()
                dt = now - last
                last = now
                if now >= next_refresh:
                    self.refresh()
                    next_refresh = now + 1.0
                if max_minutes and time.monotonic() - self.started > max_minutes * 60:
                    self.quit("max time reached")
                for c in self.canvases:
                    if not c.threaded:
                        with c.lock:
                            c.render(dt)
                stats_n += 1
                frames += 1
                if test and time.monotonic() - self.started > test_secs:
                    os.makedirs(test_dir, exist_ok=True)
                    secs = time.monotonic() - self.started
                    for i, c in enumerate(self.canvases):
                        with c.lock:
                            c.img.write_to_png(os.path.join(test_dir, f"{self.mode}_{i}.png"))
                    fps_list = [f"{(c.frames if c.threaded else frames) / secs:.1f}" for c in self.canvases]
                    log(f"test: fps per monitor {fps_list}, windows {[c.size for c in self.canvases]}, "
                        f"themes {[c.show.name if c.show else 'black' for c in self.canvases]}")
                    self.quit("test done")
                if now - stats_t > 30:
                    log(f"fps {stats_n / (now - stats_t):.1f}")
                    stats_t, stats_n = now, 0
                spare = frame_time - (time.perf_counter() - now)
                if spare > 0:
                    time.sleep(spare)
        finally:
            self.running = False
            for th in threads:
                th.join(timeout=2)
            ctypes.windll.winmm.timeEndPeriod(1)
            if self.fullscreen:
                kernel32.SetThreadExecutionState(0x80000000)  # back to normal power management
                user32.ShowCursor(True)
            for c in self.canvases:
                if user32.IsWindow(c.hwnd):
                    user32.DestroyWindow(c.hwnd)


# ---------------------------------------------------------------------- recorder / install
def record():
    """Background CPU/memory recorder (one instance per user)."""
    kernel32.CreateMutexW(None, False, "Local\\ClaudeVivariumRecorder")
    if kernel32.GetLastError() == 183:  # ERROR_ALREADY_EXISTS
        return
    s = sysmon.Sampler()
    old = sysmon.load(max_age=60)
    if old:
        s.cpu, s.mem = list(old.get("cpu", [])), list(old.get("mem", []))
    s.sample()
    while True:
        time.sleep(sysmon.INTERVAL)
        s.sample()
        s.save()


def _startup_shortcut(target, args, path):
    ps = ("$s=(New-Object -ComObject WScript.Shell).CreateShortcut($env:P);"
          "$s.TargetPath=$env:T;$s.Arguments=$env:A;$s.WorkingDirectory=(Split-Path $env:T);$s.Save()")
    env = dict(os.environ, P=path, T=target, A=args)
    subprocess.run(["powershell", "-NoProfile", "-Command", ps], env=env, check=True, capture_output=True)


def install(cfg):
    import winreg
    if not getattr(sys, "frozen", False):
        say("--install works from the built ClaudeVivarium.exe (see build-windows.ps1)")
        return
    src = os.path.dirname(sys.executable)
    subprocess.run(["taskkill", "/F", "/IM", APP_NAME + ".scr"], capture_output=True)  # stop the old recorder
    time.sleep(0.5)
    if os.path.normcase(src) != os.path.normcase(INSTALL_DIR):
        shutil.copytree(src, INSTALL_DIR, dirs_exist_ok=True)
    exe = os.path.join(INSTALL_DIR, APP_NAME + ".exe")
    scr = os.path.join(INSTALL_DIR, APP_NAME + ".scr")
    shutil.copy2(exe, scr)
    timeout = max(60, int(cfg["idle_seconds"]))
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Control Panel\Desktop", 0, winreg.KEY_SET_VALUE) as k:
        winreg.SetValueEx(k, "SCRNSAVE.EXE", 0, winreg.REG_SZ, scr)
        winreg.SetValueEx(k, "ScreenSaveActive", 0, winreg.REG_SZ, "1")
        winreg.SetValueEx(k, "ScreenSaveTimeOut", 0, winreg.REG_SZ, str(timeout))
        winreg.SetValueEx(k, "ScreenSaverIsSecure", 0, winreg.REG_SZ, "1")
    for spi, val in ((0x0011, 1), (0x000F, timeout), (0x0077, 1)):  # active, timeout, secure
        user32.SystemParametersInfoW(spi, val, None, 3)                # SPIF_UPDATEINIFILE | SPIF_SENDCHANGE
    startup = os.path.join(os.environ["APPDATA"], r"Microsoft\Windows\Start Menu\Programs\Startup")
    _startup_shortcut(scr, "--record", os.path.join(startup, "Claude Vivarium recorder.lnk"))
    subprocess.Popen([scr, "--record"], creationflags=0x00000008 | 0x00000200)  # detached, own process group
    say(f"Installed to {INSTALL_DIR}\nScreensaver: {scr}\nStarts after {timeout} s idle, locks on resume.\n"
          f"Settings: {config.PATH}")


def uninstall():
    import winreg
    subprocess.run(["taskkill", "/F", "/IM", APP_NAME + ".scr"], capture_output=True)
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Control Panel\Desktop", 0,
                        winreg.KEY_SET_VALUE | winreg.KEY_QUERY_VALUE) as k:
        try:
            if APP_NAME.lower() in winreg.QueryValueEx(k, "SCRNSAVE.EXE")[0].lower():
                winreg.DeleteValue(k, "SCRNSAVE.EXE")
        except OSError:
            pass
    lnk = os.path.join(os.environ["APPDATA"], r"Microsoft\Windows\Start Menu\Programs\Startup",
                       "Claude Vivarium recorder.lnk")
    if os.path.exists(lnk):
        os.remove(lnk)
    user32.SystemParametersInfoW(0x0011, 0, None, 3)
    say(f"Uninstalled. The files in {INSTALL_DIR} can be deleted.")


def main():
    args = sys.argv[1:]
    cfg = config.load()
    a0 = args[0].lower() if args else ""
    if a0.startswith("/s") or a0.startswith("-s"):
        dpi_aware()
        App("full", {}, cfg).run()
    elif a0.startswith("/p") or a0.startswith("-p"):
        hwnd = a0.split(":", 1)[1] if ":" in a0 else (args[1] if len(args) > 1 else "0")
        App("preview", {}, cfg, parent=wintypes.HWND(int(hwnd))).run()
    elif a0 == "--windowed":
        dpi_aware()
        opts = {"demo": "--demo" in args}
        if "--themes" in args:
            opts["themes"] = args[args.index("--themes") + 1]
        App("windowed", opts, cfg).run()
    elif a0 == "--record":
        record()
    elif a0 == "--install":
        install(cfg)
    elif a0 == "--uninstall":
        uninstall()
    else:  # "/c", no arguments or double click: settings
        os.startfile(config.PATH)


if __name__ == "__main__":
    try:
        main()
    except Exception as e:
        log(f"fatal: {e!r}")
        raise

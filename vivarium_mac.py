#!/usr/bin/env python3
"""Claude Vivarium for macOS: full-screen windows on all screens, drawn with Cairo via PyObjC.

Started by vivariumd_mac.py (LaunchAgent) when the Mac has been idle. Like the Linux version it
LOCKS THE SCREEN FIRST on any key, click or focus loss and only then closes (fails closed: if it
never becomes the active app it locks right away).

  --windowed [--demo] [--themes x]   test in a normal window (N/Right/Space = next theme, Esc = close)
  --selftest                         check fonts, sessions, CPU/memory, idle time, lock state; writes
                                     PNGs to ~/Desktop/vivarium-selftest (send me the output if
                                     something does not work)
Needs: pycairo, pyobjc-framework-Cocoa, pyobjc-framework-Quartz (install-mac.sh sets them up).
"""
import ctypes
import os
import signal
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import cairo  # noqa: E402
import objc  # noqa: E402
import Quartz  # noqa: E402
from AppKit import (NSApplication, NSBackingStoreBuffered, NSColor, NSCursor, NSEvent, NSGraphicsContext,  # noqa
                    NSScreen, NSView, NSWindow)
from Foundation import (NSData, NSNotificationCenter, NSObject, NSRunLoop, NSRunLoopCommonModes, NSTimer,  # noqa: E402
                        NSMakeRect)

import config  # noqa: E402
import governor  # noqa: E402
import sessions as sessions_mod  # noqa: E402
import sysmon  # noqa: E402
import themes  # noqa: E402

LOG = os.path.expanduser("~/Library/Logs/ClaudeVivarium.log")
FPS = 30
GRACE = 1.0
MOTION_THRESHOLD = 25
MAX_RENDER_PIXELS = 3.5e6   # drawn in points (not Retina pixels); macOS scales it up on the GPU

# AppKit constants as numbers (stable values; avoids name differences between PyObjC versions)
STYLE_BORDERLESS, STYLE_TITLED_WINDOW = 0, 1 | 2 | 4 | 8   # titled, closable, miniaturizable, resizable
SCREENSAVER_LEVEL = 1000                                    # NSScreenSaverWindowLevel
BEHAVIOR = (1 << 0) | (1 << 4) | (1 << 8)                   # all spaces, stationary, full-screen auxiliary
POLICY_ACCESSORY = 1                                        # no Dock icon
EV_LEFT_DOWN, EV_RIGHT_DOWN, EV_MOVED, EV_LEFT_DRAG, EV_KEY_DOWN, EV_SCROLL, EV_OTHER_DOWN = 1, 3, 5, 6, 10, 22, 25
EV_MASK = sum(1 << t for t in (EV_LEFT_DOWN, EV_RIGHT_DOWN, EV_MOVED, EV_LEFT_DRAG, EV_KEY_DOWN, EV_SCROLL,
                               EV_OTHER_DOWN))
KEY_ESCAPE, KEY_RIGHT = 53, 124


def log(msg):
    line = time.strftime("%Y-%m-%d %H:%M:%S ") + "[vivarium] " + msg
    print(line, file=sys.stderr, flush=True)


# ---------------------------------------------------------------------- lock helpers (shared with the agent)
def screen_locked():
    d = Quartz.CGSessionCopyCurrentDictionary()
    return bool(d and d.get("CGSSessionScreenIsLocked", 0))


def lock_screen():
    """Lock now. Uses the same call as the menu-bar 'Lock Screen'; falls back to sleeping the display,
    which locks when 'Require password immediately' is set (System Settings > Lock Screen)."""
    try:
        login = ctypes.CDLL("/System/Library/PrivateFrameworks/login.framework/Versions/Current/login")
        login.SACLockScreenImmediate()
        return True
    except (OSError, AttributeError) as e:
        log(f"SACLockScreenImmediate not available ({e}); using pmset displaysleepnow")
    return subprocess.run(["pmset", "displaysleepnow"], capture_output=True).returncode == 0


# ---------------------------------------------------------------------- drawing
class VivWindow(NSWindow):
    def canBecomeKeyWindow(self):
        return True

    def canBecomeMainWindow(self):
        return True


class VivView(NSView):
    canvas = None

    def isOpaque(self):
        return True

    def acceptsFirstResponder(self):
        return True

    def drawRect_(self, rect):
        img = self.canvas.cgimage if self.canvas is not None else None
        ctx = NSGraphicsContext.currentContext().CGContext()
        if img is None:
            Quartz.CGContextSetRGBFillColor(ctx, 0, 0, 0, 1)
            Quartz.CGContextFillRect(ctx, self.bounds())
            return
        Quartz.CGContextSetInterpolationQuality(ctx, Quartz.kCGInterpolationHigh)
        Quartz.CGContextDrawImage(ctx, self.bounds(), img)


class Canvas:
    """One window: renders its Show with Cairo and hands the pixels to the view as a CGImage."""
    colorspace = Quartz.CGColorSpaceCreateDeviceRGB()

    def __init__(self, view, show, every=1):
        self.view, self.show, self.every = view, show, every
        self.gov = governor.Governor(base_fps=FPS, levels=(30, 15))  # lower frame rate when the Mac is busy
        self.size = (0, 0)
        self.img = self.cr = self.cgimage = None
        self.acc, self.frame = 0.0, 0

    def render(self, dt):
        self.acc += dt
        self.frame += 1
        if self.frame % self.every or not self.gov.due(self.acc):
            return
        t0 = time.perf_counter()
        dt, self.acc = self.acc, 0.0
        b = self.view.bounds()
        w, h = max(1, int(b.size.width)), max(1, int(b.size.height))
        k = min(1.0, (MAX_RENDER_PIXELS / (w * h)) ** 0.5)
        w, h = max(1, int(w * k)), max(1, int(h * k))
        if (w, h) != self.size:
            self.size = (w, h)
            self.img = cairo.ImageSurface(cairo.FORMAT_RGB24, w, h)
            self.cr = cairo.Context(self.img)
        if self.show is None:
            self.cr.set_source_rgb(0, 0, 0)
            self.cr.paint()
        else:
            self.show.update(dt)
            self.show.draw(self.cr, w, h)
        self.img.flush()
        buf = self.img.get_data()
        data = NSData.dataWithBytes_length_(bytes(buf), len(buf))
        provider = Quartz.CGDataProviderCreateWithCFData(data)
        self.cgimage = Quartz.CGImageCreate(
            w, h, 8, 32, self.img.get_stride(), self.colorspace,
            Quartz.kCGBitmapByteOrder32Little | Quartz.kCGImageAlphaNoneSkipFirst,  # Cairo's BGRx memory layout
            provider, None, False, Quartz.kCGRenderingIntentDefault)
        self.view.setNeedsDisplay_(True)
        if self.show is not None:
            self.gov.record(time.perf_counter() - t0)


class Controller(NSObject):
    """Receives timer ticks and notifications and forwards them to the App."""
    app = None

    def tick_(self, _timer):
        self.app.tick()

    def resigned_(self, _note):
        self.app.on_resign()

    def activated_(self, _note):
        self.app.became_active = True


class App:
    def __init__(self, mode, opts, cfg):
        self.mode = mode  # "full" or "windowed"
        self.opts, self.cfg = opts, cfg
        governor.ENABLED = cfg["adaptive_fps"]
        self.started = time.monotonic()
        self.last = time.perf_counter()
        self.next_refresh = 0.0
        self.exiting = False
        self.exit_code = 0
        self.became_active = False
        self.mouse_origin = None
        self.canvases = []
        self.windows = []
        self.main_show = None
        self.caffeinate = None
        self.lock_deadline = 0.0
        self.monitor = sysmon.Monitor() if cfg["system_monitor"] else None
        self.nsapp = NSApplication.sharedApplication()
        self.nsapp.setActivationPolicy_(POLICY_ACCESSORY)
        self.ctrl = Controller.alloc().init()
        self.ctrl.app = self

    @property
    def fullscreen(self):
        return self.mode == "full"

    def in_grace(self):
        return time.monotonic() - self.started < GRACE

    def make_show(self, decor=False, avoid=None):
        names = themes.parse_list(self.opts.get("themes") or self.cfg["themes"])
        return themes.Show(names, self.cfg["theme_minutes"], show_text=not self.cfg["hide_text"],
                           decor=decor, avoid=avoid)

    def _window(self, frame, style, level):
        win = VivWindow.alloc().initWithContentRect_styleMask_backing_defer_(frame, style, NSBackingStoreBuffered,
                                                                             False)
        win.setBackgroundColor_(NSColor.blackColor())
        win.setReleasedWhenClosed_(False)
        if level:
            win.setLevel_(level)
            win.setCollectionBehavior_(BEHAVIOR)
            win.setOpaque_(True)
        win.setAcceptsMouseMovedEvents_(True)
        view = VivView.alloc().initWithFrame_(NSMakeRect(0, 0, frame.size.width, frame.size.height))
        win.setContentView_(view)
        self.windows.append(win)
        return win, view

    def create_windows(self):
        if self.fullscreen:
            for i, screen in enumerate(NSScreen.screens()):   # screens()[0] is the main screen (menu bar)
                win, view = self._window(screen.frame(), STYLE_BORDERLESS, SCREENSAVER_LEVEL)
                if i == 0:
                    self.main_show = self.make_show()
                    canvas = Canvas(view, self.main_show)
                elif self.cfg["other_monitors"] == "theme":
                    canvas = Canvas(view, self.make_show(decor=True, avoid=self.main_show), every=2)
                else:
                    canvas = Canvas(view, None, every=10 ** 9)
                view.canvas = canvas
                self.canvases.append(canvas)
                win.makeKeyAndOrderFront_(None)
            NSCursor.hide()
            # keep the display awake while we are visible (like the GNOME inhibitor)
            self.caffeinate = subprocess.Popen(["caffeinate", "-d", "-w", str(os.getpid())])
        else:
            win, view = self._window(NSMakeRect(100, 100, 1280, 800), STYLE_TITLED_WINDOW, 0)
            win.setTitle_("Claude Vivarium (preview - N: next theme, Esc: close)")
            self.main_show = self.make_show()
            view.canvas = Canvas(view, self.main_show)
            self.canvases.append(view.canvas)
            win.makeKeyAndOrderFront_(None)
        self.nsapp.activateIgnoringOtherApps_(True)

    # ------------------------------------------------------------------ input
    def on_event(self, event):
        t = event.type()
        if self.mode == "windowed":
            if t == EV_KEY_DOWN:
                chars = (event.charactersIgnoringModifiers() or "").lower()
                if event.keyCode() == KEY_ESCAPE:
                    self.finish(0)
                elif event.keyCode() == KEY_RIGHT or chars in ("n", " "):
                    self.main_show.next_theme()
                return None
            return event
        if t == EV_MOVED or t == EV_LEFT_DRAG:
            p = NSEvent.mouseLocation()
            if self.in_grace() or self.mouse_origin is None:
                self.mouse_origin = (p.x, p.y)
            elif abs(p.x - self.mouse_origin[0]) + abs(p.y - self.mouse_origin[1]) > MOTION_THRESHOLD:
                if self.cfg["mouse_move"] == "hint":
                    self.mouse_origin = (p.x, p.y)
                    self.main_show.show_hint("Press a key or click to unlock   ·   → next theme")
                else:
                    self.wake("mouse moved")
            return None
        if t == EV_KEY_DOWN and event.keyCode() == KEY_RIGHT:  # right arrow = next theme, other keys wake
            self.main_show.next_theme()
            return None
        if not self.in_grace():
            self.wake("key" if t == EV_KEY_DOWN else "mouse button")
        return None  # swallow the event

    def on_resign(self):
        if self.fullscreen and self.became_active:
            self.wake("lost focus")

    # ------------------------------------------------------------------ wake = lock, then close
    def wake(self, reason):
        if self.exiting:
            return
        self.exiting = True
        log(f"wake: {reason}")
        if not self.fullscreen or self.opts.get("no_lock"):
            self.finish(0)
            return
        if not screen_locked():
            lock_screen()
        self.lock_deadline = time.monotonic() + 5

    def finish(self, code):
        self.exit_code = code
        if self.caffeinate is not None:
            self.caffeinate.terminate()
        NSCursor.unhide()
        for w in self.windows:
            w.orderOut_(None)
        log(f"exit (code {code})")
        os._exit(code)  # leave immediately; AppKit cleanup is not needed

    # ------------------------------------------------------------------ main loop
    def refresh(self):
        try:
            data = sessions_mod.demo_sessions() if self.opts.get("demo") else sessions_mod.read_sessions()
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
                c.show.set_sessions(data)
                if c.show is self.main_show:
                    c.show.set_system(system)

    def tick(self):
        try:
            now = time.perf_counter()
            dt, self.last = now - self.last, now
            if self.exiting:  # waiting until macOS confirms the lock
                if screen_locked():
                    self.finish(0)
                elif time.monotonic() > self.lock_deadline:
                    log("lock NOT confirmed within 5 s")
                    self.finish(3)
                return
            if self.mode == "windowed" and not self.windows[0].isVisible():
                self.finish(0)  # preview window was closed with the red button
            if self.fullscreen and not self.became_active and time.monotonic() - self.started > 3:
                log("never became the active app - failing closed")
                self.wake("no focus")
                return
            if self.fullscreen and self.cfg["max_minutes"] and \
                    time.monotonic() - self.started > self.cfg["max_minutes"] * 60:
                self.wake("max time reached")
                return
            if now >= self.next_refresh:
                self.refresh()
                self.next_refresh = now + 1.0
            for c in self.canvases:
                c.render(dt)
        except Exception as e:  # an exception must never leave the screen open
            log(f"tick error: {e!r}")
            self.wake("error")

    def run(self):
        self.create_windows()
        log(f"start: mode={self.mode}, screens={len(self.canvases)}, theme={self.main_show.name}")
        NSEvent.addLocalMonitorForEventsMatchingMask_handler_(EV_MASK, self.on_event)
        nc = NSNotificationCenter.defaultCenter()
        nc.addObserver_selector_name_object_(self.ctrl, "resigned:", "NSApplicationDidResignActiveNotification", None)
        nc.addObserver_selector_name_object_(self.ctrl, "activated:", "NSApplicationDidBecomeActiveNotification", None)
        if self.nsapp.isActive():
            self.became_active = True
        timer = NSTimer.timerWithTimeInterval_target_selector_userInfo_repeats_(1.0 / FPS, self.ctrl, "tick:",
                                                                               None, True)
        # the real constant ('kCFRunLoopCommonModes'): the literal string "NSRunLoopCommonModes" is not a run-loop
        # mode, so the timer never fired (black window that ignored every key)
        NSRunLoop.currentRunLoop().addTimer_forMode_(timer, NSRunLoopCommonModes)
        for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):  # killed -> lock first (runs on the next tick)
            signal.signal(sig, lambda *_: self.wake("signal"))
        self.nsapp.run()


# ---------------------------------------------------------------------- self test
def selftest():
    out = os.path.expanduser("~/Desktop/vivarium-selftest")
    os.makedirs(out, exist_ok=True)
    import platform
    import scene
    print(f"macOS {platform.mac_ver()[0]}, Python {sys.version.split()[0]}, pycairo {cairo.version}, "
          f"cairo {cairo.cairo_version_string()}, PyObjC {objc.__version__}")
    print(f"text backend: {scene.Text.__name__}, font {scene.FONT}")
    surf = cairo.ImageSurface(cairo.FORMAT_RGB24, 900, 260)
    cr = cairo.Context(surf)
    cr.set_source_rgb(0.05, 0.08, 0.12)
    cr.paint()
    t = scene.Text()
    y = 10
    for name, wgt in (("LIGHT", scene.Weight.LIGHT), ("NORMAL", scene.Weight.NORMAL),
                      ("SEMIBOLD", scene.Weight.SEMIBOLD), ("BOLD", scene.Weight.BOLD)):
        lay = t.layout(cr, f"{name}: Netdocs Dev 1 · working · 3m 05s ● “quoted”", 26, wgt)
        w, h = lay.get_pixel_size()
        print(f"  font {name:8s} {t.FACES[wgt][0] if hasattr(t, 'FACES') else ''}: {w}x{h} px")
        scene.Text.show(cr, lay, 10, y, (1, 1, 1), 1)
        y += h + 8
    lay = t.layout(cr, "MONO: BASH EDIT READ", 26, scene.Weight.BOLD, family=scene.MONO)
    scene.Text.show(cr, lay, 10, y, (0.3, 1, 0.9), 1)
    surf.write_to_png(os.path.join(out, "fonts.png"))
    ss = sessions_mod.read_sessions()
    print(f"sessions: {len(ss)}")
    for s in ss:
        print(f"  {s['name']}: {s['status']} -> {s['state']}, {s['cwd']}, tool calls {len(s['tool_events'])}")
    smp = sysmon.Sampler()
    smp.sample()
    time.sleep(1)
    smp.sample()
    d = smp.data()
    print(f"cpu {d['cpu'][-1]:.1f} %, memory {d['mem'][-1] / 2**30:.1f} / {d['mem_total'] / 2**30:.1f} GB")
    idle = Quartz.CGEventSourceSecondsSinceLastEventType(Quartz.kCGEventSourceStateHIDSystemState,
                                                          Quartz.kCGAnyInputEventType)
    print(f"idle for {idle:.0f} s, screen locked: {screen_locked()}")
    try:
        ctypes.CDLL("/System/Library/PrivateFrameworks/login.framework/Versions/Current/login").SACLockScreenImmediate
        print("lock call: SACLockScreenImmediate available")
    except (OSError, AttributeError):
        print("lock call: NOT available - will use 'pmset displaysleepnow' (set 'Require password' to Immediately)")
    print(f"screens: {[(int(s.frame().size.width), int(s.frame().size.height)) for s in NSScreen.screens()]}")
    import render_preview
    sys.argv = ["render_preview", "--demo", "--sysmon", "--themes", "all", "--times", "8", "--size", "1600x1000",
                "--out", out]
    render_preview.main()
    print(f"PNGs written to {out}")


def main():
    args = sys.argv[1:]
    cfg = config.load()
    if "--selftest" in args:
        selftest()
        return
    opts = {"demo": "--demo" in args, "no_lock": "--no-lock" in args}
    if "--themes" in args:
        opts["themes"] = args[args.index("--themes") + 1]
    App("windowed" if "--windowed" in args else "full", opts, cfg).run()


if __name__ == "__main__":
    main()

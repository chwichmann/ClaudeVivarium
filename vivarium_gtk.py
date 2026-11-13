#!/usr/bin/env python3
"""Claude Vivarium for Linux (GNOME, Wayland): full-screen GTK4 screensaver showing Claude Code sessions.

Normal mode (started by vivariumd.py): fullscreen, hides the cursor, keeps GNOME from blanking,
and on any input LOCKS THE SCREEN FIRST and only then closes.

Defaults come from ~/.config/claude-vivarium/config.ini; command-line options override them.

  --windowed     preview in a normal window, Esc/any key closes, never locks
  --demo         fake sessions cycling through all states
  --no-lock      close on input without locking (not recommended)
  --max-minutes  after this long, lock and let GNOME blank the display (0 = never)
  --mouse-move   hint = mouse movement only shows a hint, wake = it locks like a key press
  --hide-text    do not show prompt/answer/tool text, only names and states
  --themes       all | one theme | comma list (aquarium, space, synthwave, pasture, seabed, forest)
  --theme-minutes  switch to the next theme after N minutes (0 = never)
In --windowed preview, N / Right / Space jumps to the next theme.
"""
import argparse
import os
import signal
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import gi  # noqa: E402

gi.require_version("Gtk", "4.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gdk, Gio, GLib, Gtk  # noqa: E402

import config  # noqa: E402
import gnome  # noqa: E402
import sessions as sessions_mod  # noqa: E402
import sysmon  # noqa: E402
import themes  # noqa: E402
from gnome import log  # noqa: E402

MOTION_THRESHOLD = 25  # px of mouse movement that counts as "wake up"
GRACE = 1.0            # s after start during which input is ignored (avoids instant wake)


class Screensaver(Gtk.Application):
    def __init__(self, opts):
        super().__init__(application_id="io.github.ClaudeVivarium", flags=Gio.ApplicationFlags.NON_UNIQUE)
        self.opts = opts
        self.scene = themes.Show(themes.parse_list(opts.themes), opts.theme_minutes, show_text=not opts.hide_text)
        self.monitor = sysmon.Monitor() if opts.system_monitor else None
        self.started = time.monotonic()
        self.exiting = False
        self.wake_by_user = False
        self.exit_code = 0
        self.was_active = False
        self.motion_origin = None
        self.last_frame = None
        self.bus = None
        self.lock_deadline = 0.0
        self.win = None

    @property
    def locking(self):
        return not (self.opts.windowed or self.opts.no_lock)

    def do_activate(self):
        css = Gtk.CssProvider()
        css.load_from_data(b"window, drawingarea { background: #020a14; }")
        Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css,
                                                  Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        win = Gtk.ApplicationWindow(application=self, title="Claude Vivarium")
        self.win = win
        area = Gtk.DrawingArea()
        area.set_draw_func(lambda _a, cr, w, h: self.scene.draw(cr, w, h))
        area.add_tick_callback(self.on_tick)
        win.set_child(area)

        key = Gtk.EventControllerKey()
        key.set_propagation_phase(Gtk.PropagationPhase.CAPTURE)
        key.connect("key-pressed", self.on_key)
        win.add_controller(key)
        click = Gtk.GestureClick()
        click.set_button(0)
        click.connect("pressed", lambda *_: self.wake("mouse click"))
        win.add_controller(click)
        motion = Gtk.EventControllerMotion()
        motion.connect("motion", self.on_motion)
        win.add_controller(motion)
        scroll = Gtk.EventControllerScroll.new(Gtk.EventControllerScrollFlags.BOTH_AXES)
        scroll.connect("scroll", lambda *_: self.wake("scroll") or True)
        win.add_controller(scroll)
        win.connect("notify::is-active", self.on_active_changed)
        win.connect("close-request", self.on_close_request)

        if self.opts.windowed:
            win.set_default_size(1280, 800)
        else:
            win.fullscreen()
            win.set_cursor(Gdk.Cursor.new_from_name("none", None))
        win.present()

        if not self.opts.windowed:
            self.bus = gnome.session_bus()
            # keep GNOME from blanking/locking underneath us while we are visible
            self.inhibit(win, Gtk.ApplicationInhibitFlags.IDLE, "Showing Claude Vivarium")
            # if GNOME locks by other means (suspend, Super+L, ...) we are done
            self.bus.signal_subscribe(gnome.SS_NAME, gnome.SS_IFACE, "ActiveChanged", gnome.SS_PATH, None,
                                      Gio.DBusSignalFlags.NONE, self.on_lock_changed)
            if self.opts.max_minutes > 0:
                GLib.timeout_add_seconds(int(self.opts.max_minutes * 60), self.on_max_time)
            if self.locking:
                GLib.timeout_add(3000, self.check_got_focus)

        # killed (service stop, logout, kill): lock first, then go - never leave the desktop open
        for sig in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            gnome.on_unix_signal(sig, self.on_signal, sig)

        self.refresh()
        GLib.timeout_add(1000, self.refresh)
        log(f"started ({'windowed' if self.opts.windowed else 'fullscreen'}, "
            f"{'demo' if self.opts.demo else 'live'} data, lock={'on' if self.locking else 'off'}, "
            f"max_minutes={self.opts.max_minutes or 'never'}, mouse_move={self.opts.mouse_move}, "
            f"theme={self.scene.name}, themes={','.join(self.scene.names)}, every {self.opts.theme_minutes} min)")

    # ---------------------------------------------------------------- data + animation
    def refresh(self):
        try:
            data = sessions_mod.demo_sessions() if self.opts.demo else sessions_mod.read_sessions()
            self.scene.set_sessions(data)
        except Exception as e:  # never let a parsing problem kill the screensaver
            log(f"reading sessions failed: {e!r}")
        if self.monitor is not None:
            try:
                self.scene.set_system(self.monitor.get())
            except Exception as e:
                log(f"reading system stats failed: {e!r}")
        return not self.exiting

    def on_tick(self, widget, clock):
        now = clock.get_frame_time() / 1e6
        if self.last_frame is not None:
            self.scene.update(now - self.last_frame)
        self.last_frame = now
        widget.queue_draw()
        return True

    # ---------------------------------------------------------------- input
    def in_grace(self):
        return time.monotonic() - self.started < GRACE

    def on_key(self, _ctrl, keyval, _code, _state):
        if self.opts.windowed and keyval in (Gdk.KEY_n, Gdk.KEY_N, Gdk.KEY_Right, Gdk.KEY_space):
            self.scene.next_theme()  # preview: flip through the themes
            return True
        if not self.in_grace():
            self.wake(f"key {Gdk.keyval_name(keyval)}")
        return True

    def on_motion(self, _ctrl, x, y):
        if self.in_grace() or self.motion_origin is None:
            self.motion_origin = (x, y)
            return
        ox, oy = self.motion_origin
        if abs(x - ox) + abs(y - oy) > MOTION_THRESHOLD:
            if self.opts.mouse_move == "hint" and not self.opts.windowed:
                self.motion_origin = (x, y)
                self.scene.show_hint("Press a key or click to unlock")
            else:
                self.wake("mouse moved")

    def on_active_changed(self, win, _pspec):
        if win.is_active():
            self.was_active = True
        elif self.was_active and self.locking:
            # something (e.g. a GNOME shortcut) took focus away -> lock right away
            self.wake("focus lost")

    def check_got_focus(self):
        if not self.was_active and not self.exiting:
            log("window never got keyboard focus - failing closed")
            self.wake("no focus")
        return False

    def on_close_request(self, _win):
        self.wake("close requested")
        return True  # we close ourselves once locked

    def on_max_time(self):
        self.wake(f"max time of {self.opts.max_minutes} min reached")
        return False

    def on_signal(self, sig):
        self.wake(f"got {signal.Signals(sig).name}")
        return True

    def on_lock_changed(self, _conn, _sender, _path, _iface, _signal, params):
        if params.unpack()[0] and not self.exiting:
            log("screen got locked by GNOME - closing")
            self.exiting = True
            self.quit()

    # ---------------------------------------------------------------- wake = lock, then close
    def wake(self, reason):
        if self.exiting:
            return
        self.exiting = True
        log(f"wake: {reason}")
        self.wake_by_user = reason.startswith(("key", "mouse", "scroll", "focus"))
        if not self.locking:
            self.quit()
            return
        gnome.request_lock(self.bus)
        self.lock_deadline = time.monotonic() + 5
        GLib.timeout_add(100, self.wait_locked)

    def wait_locked(self):
        if gnome.screen_locked(self.bus):
            log("screen is locked - waking the lock screen, then closing")
            GLib.timeout_add(500, self.wake_lock_screen)  # after GNOME's 0.3 s "fade to black" started
            return False
        if time.monotonic() > self.lock_deadline:
            log("lock NOT confirmed within 5 s - exiting with error so the daemon locks")
            self.exit_code = 3
            self.quit()
            return False
        return True

    def wake_lock_screen(self):
        """So one key is enough: without this, GNOME blanks right after our lock and needs a second key.
        Only when a person woke us - after max time, a kill signal etc. nobody is there and black is right."""
        if self.wake_by_user:
            gnome.nudge_pointer(self.bus)
        self.quit()
        return False


def main():
    cfg = config.load()
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--windowed", action="store_true")
    ap.add_argument("--demo", action="store_true")
    ap.add_argument("--no-lock", action="store_true")
    ap.add_argument("--hide-text", action="store_true", default=cfg["hide_text"])
    ap.add_argument("--max-minutes", type=float, default=cfg["max_minutes"])
    ap.add_argument("--mouse-move", choices=("hint", "wake"), default=cfg["mouse_move"])
    ap.add_argument("--themes", default=cfg["themes"], help="all, one theme or a comma list")
    ap.add_argument("--theme-minutes", type=float, default=cfg["theme_minutes"])
    ap.add_argument("--no-system-monitor", dest="system_monitor", action="store_false",
                    default=cfg["system_monitor"], help="hide the CPU/memory panel")
    opts = ap.parse_args()
    app = Screensaver(opts)
    app.run([sys.argv[0]])
    sys.exit(app.exit_code)


if __name__ == "__main__":
    main()

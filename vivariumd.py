#!/usr/bin/env python3
"""Claude Vivarium idle watcher: starts the screensaver when the desktop has been idle for a while.

Runs as a systemd user service inside the GNOME session. Asks Mutter to signal when the user has been
idle for --idle seconds (must be shorter than GNOME's own blank delay), then launches vivarium_gtk.py.
Whenever the screensaver exits and the screen is not locked, this daemon locks it (fail closed).

Any extra arguments after "--" are passed to vivarium_gtk.py (e.g. -- --hide-text --max-minutes 60).
"""
import argparse
import os
import signal
import sys

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import gi  # noqa: E402

from gi.repository import Gio, GLib  # noqa: E402

import config  # noqa: E402
import gnome  # noqa: E402
import sysmon  # noqa: E402
from gnome import log  # noqa: E402

SCREENSAVER = os.path.join(os.path.dirname(os.path.realpath(__file__)), "vivarium_gtk.py")


class Daemon:
    def __init__(self, opts, passthrough):
        self.opts = opts
        self.passthrough = passthrough
        self.no_lock = "--no-lock" in passthrough
        self.bus = gnome.session_bus()
        self.child = None
        self.watch_id = None
        self.idle = None
        self.cfg_mtime = None
        self.bus.signal_subscribe(gnome.IDLE_NAME, gnome.IDLE_IFACE, "WatchFired", gnome.IDLE_PATH, None,
                                  Gio.DBusSignalFlags.NONE, self.on_watch_fired)
        self.reload()
        GLib.timeout_add_seconds(30, self.reload)
        # CPU/memory history for the system monitor panel (always recorded, ~1 KB/s of work)
        self.sampler = sysmon.Sampler()
        old = sysmon.load(max_age=60)  # keep the history across a quick service restart
        if old:
            self.sampler.cpu, self.sampler.mem = list(old.get("cpu", [])), list(old.get("mem", []))
        self.sampler.sample()
        GLib.timeout_add_seconds(sysmon.INTERVAL, self.on_sample)

    def on_sample(self):
        self.sampler.sample()
        self.sampler.save()
        return True

    def reload(self):
        """(Re)register the idle watch when the configured idle time changes."""
        mt = config.mtime()
        if mt == self.cfg_mtime and self.watch_id is not None:
            return True
        self.cfg_mtime = mt
        wanted = self.opts.idle or config.load()["idle_seconds"]
        idle = wanted
        gnome_delay = gnome.gnome_idle_delay()
        if gnome_delay and idle > gnome_delay - 20:
            idle = max(20, gnome_delay - 30)
            log(f"idle {wanted}s is too close to GNOME's blank delay ({gnome_delay}s); using {idle}s")
        if idle == self.idle and self.watch_id is not None:
            return True
        if self.watch_id is not None:
            gnome.remove_idle_watch(self.bus, self.watch_id)
        self.watch_id = gnome.add_idle_watch(self.bus, idle * 1000)
        self.idle = idle
        log(f"watching for {idle}s of idle time (GNOME blanks {f'after {gnome_delay}s' if gnome_delay else 'never'})")
        return True

    def on_terminate(self, loop):
        """Service stopped/restarted while the screensaver is up: lock first, never leave it open."""
        if self.child is not None:
            log("stopping while the screensaver is shown - locking first")
            if not self.no_lock and not gnome.screen_locked(self.bus):
                gnome.request_lock(self.bus)
            self.child.send_signal(signal.SIGTERM)  # the screensaver also locks on SIGTERM itself
        loop.quit()
        return False

    def on_watch_fired(self, _conn, _sender, _path, _iface, _signal, params):
        if params.unpack()[0] == self.watch_id:
            self.start_screensaver()

    def start_screensaver(self):
        if self.child is not None:
            return
        if gnome.screen_locked(self.bus):
            return  # already on the lock screen
        if gnome.idle_inhibited(self.bus):
            log("idle is inhibited by another app (video, presentation?) - not starting")
            return
        argv = [sys.executable, SCREENSAVER] + self.passthrough
        log("idle - starting screensaver")
        self.child = Gio.Subprocess.new(argv, Gio.SubprocessFlags.NONE)
        self.child.wait_async(None, self.on_child_exit)

    def on_child_exit(self, proc, res):
        try:
            proc.wait_finish(res)
        except GLib.Error:
            pass
        code = proc.get_exit_status() if proc.get_if_exited() else -1
        self.child = None
        locked = gnome.screen_locked(self.bus)
        log(f"screensaver exited (code {code}, locked={locked})")
        if not locked and not self.no_lock:
            log("screen not locked after screensaver - locking now")
            gnome.request_lock(self.bus)


def main():
    argv = sys.argv[1:]
    passthrough = []
    if "--" in argv:
        i = argv.index("--")
        argv, passthrough = argv[:i], argv[i + 1:]
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--idle", type=int, default=0,
                    help="seconds of idle time before starting (default: idle_seconds from config.ini)")
    ap.add_argument("--now", action="store_true", help="start the screensaver immediately once (for testing)")
    opts = ap.parse_args(argv)
    daemon = Daemon(opts, passthrough)
    if opts.now:
        GLib.idle_add(lambda: daemon.start_screensaver() and False)
    loop = GLib.MainLoop()
    gnome.on_unix_signal(signal.SIGTERM, daemon.on_terminate, loop)
    gnome.on_unix_signal(signal.SIGINT, daemon.on_terminate, loop)
    loop.run()


if __name__ == "__main__":
    main()

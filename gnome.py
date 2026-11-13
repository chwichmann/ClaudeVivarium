"""Small helpers for talking to GNOME over D-Bus (lock screen, idle, inhibitors)."""
import os
import subprocess
import sys

from gi.repository import Gio, GLib

SS_NAME, SS_PATH, SS_IFACE = "org.gnome.ScreenSaver", "/org/gnome/ScreenSaver", "org.gnome.ScreenSaver"
IDLE_NAME, IDLE_PATH, IDLE_IFACE = ("org.gnome.Mutter.IdleMonitor", "/org/gnome/Mutter/IdleMonitor/Core",
                                    "org.gnome.Mutter.IdleMonitor")
INHIBIT_IDLE = 8


def on_unix_signal(sig, callback, *data):
    """Run callback(*data) in the main loop when sig arrives (works with old and new PyGObject)."""
    try:
        import gi
        gi.require_version("GLibUnix", "2.0")
        from gi.repository import GLibUnix
        add = GLibUnix.signal_add
    except (ValueError, ImportError, AttributeError):
        add = GLib.unix_signal_add
    return add(GLib.PRIORITY_HIGH, sig, callback, *data)


def log(msg):
    print(f"[claude-vivarium] {msg}", file=sys.stderr, flush=True)


def session_bus():
    return Gio.bus_get_sync(Gio.BusType.SESSION, None)


def _call(bus, name, path, iface, method, args=None, reply="()", timeout=2000):
    return bus.call_sync(name, path, iface, method, args, GLib.VariantType(reply),
                         Gio.DBusCallFlags.NONE, timeout, None).unpack()


def screen_locked(bus):
    try:
        return bool(_call(bus, SS_NAME, SS_PATH, SS_IFACE, "GetActive", reply="(b)")[0])
    except GLib.Error as e:
        log(f"GetActive failed: {e.message}")
        return False


def _graphical_session_id():
    sid = os.environ.get("XDG_SESSION_ID")
    if sid:
        return sid
    try:
        out = subprocess.run(["loginctl", "show-user", str(os.getuid()), "-p", "Display", "--value"],
                             capture_output=True, text=True, timeout=5)
        return out.stdout.strip() or None
    except Exception:
        return None


def request_lock(bus):
    """Ask GNOME to lock. Returns True if the request was accepted."""
    try:
        _call(bus, SS_NAME, SS_PATH, SS_IFACE, "Lock", timeout=5000)
        return True
    except GLib.Error as e:
        log(f"ScreenSaver.Lock failed: {e.message}; falling back to loginctl")
    sid = _graphical_session_id()
    try:
        return subprocess.run(["loginctl", "lock-session"] + ([sid] if sid else []), timeout=5).returncode == 0
    except Exception as e:
        log(f"loginctl lock-session failed: {e}")
        return False


def nudge_pointer(bus):
    """Wake the lock screen after we locked it ourselves.

    GNOME fades to black 0.3 s after any programmatic lock and only wakes up on input that arrives
    after that - so the key that woke the screensaver is not enough and a second key would be needed.
    A 1-pixel virtual pointer move (Mutter's remote desktop API, open to programs in the session)
    counts as new input, so the lock screen stays visible. On a locked screen input can only show
    the lock screen, never unlock it.
    """
    rd, path = "org.gnome.Mutter.RemoteDesktop", None
    iface = "org.gnome.Mutter.RemoteDesktop.Session"
    try:
        path = _call(bus, rd, "/org/gnome/Mutter/RemoteDesktop", rd, "CreateSession", reply="(o)")[0]
        _call(bus, rd, path, iface, "Start")
        for dx in (1.0, -1.0):
            _call(bus, rd, path, iface, "NotifyPointerMotionRelative", GLib.Variant("(dd)", (dx, 0.0)))
        return True
    except GLib.Error as e:
        log(f"could not wake the lock screen: {e.message}")
        return False
    finally:
        if path:
            try:
                _call(bus, rd, path, iface, "Stop")
            except GLib.Error:
                pass


def idle_inhibited(bus):
    """True if some app (video player, presentation, ...) asked GNOME not to go idle."""
    try:
        return bool(_call(bus, "org.gnome.SessionManager", "/org/gnome/SessionManager",
                          "org.gnome.SessionManager", "IsInhibited", GLib.Variant("(u)", (INHIBIT_IDLE,)),
                          reply="(b)")[0])
    except GLib.Error:
        return False


def gnome_idle_delay():
    """GNOME's own blank/lock delay in seconds (0 = never)."""
    try:
        return Gio.Settings.new("org.gnome.desktop.session").get_uint("idle-delay")
    except Exception:
        return 0


def remove_idle_watch(bus, watch_id):
    try:
        _call(bus, IDLE_NAME, IDLE_PATH, IDLE_IFACE, "RemoveWatch", GLib.Variant("(u)", (watch_id,)))
    except GLib.Error as e:
        log(f"RemoveWatch failed: {e.message}")


def add_idle_watch(bus, ms):
    return _call(bus, IDLE_NAME, IDLE_PATH, IDLE_IFACE, "AddIdleWatch", GLib.Variant("(t)", (ms,)), reply="(u)")[0]

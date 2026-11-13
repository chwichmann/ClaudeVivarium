"""CPU and memory history for the small system monitor panel.

The idle daemon runs all the time and records a sample every INTERVAL seconds into a small JSON file
in $XDG_RUNTIME_DIR (RAM). So when the screensaver appears, the graph already shows the last 30 minutes.
If no fresh file exists (daemon not running, preview), the screensaver samples by itself.
"""
import json
import os
import sys
import time

INTERVAL = 5                      # seconds between samples
HISTORY = 30 * 60 // INTERVAL     # 30 minutes
if sys.platform == "win32":
    _DIR = os.path.join(os.environ.get("LOCALAPPDATA") or os.path.expanduser("~"), "ClaudeVivarium")
elif sys.platform == "darwin":
    _DIR = os.path.expanduser("~/Library/Caches/ClaudeVivarium")
else:
    _DIR = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
PATH = os.path.join(_DIR, "claude-vivarium-stats.json")


if sys.platform == "win32":
    import ctypes
    from ctypes import wintypes

    class _MEMSTATUS(ctypes.Structure):
        _fields_ = [("dwLength", wintypes.DWORD), ("dwMemoryLoad", wintypes.DWORD),
                    ("ullTotalPhys", ctypes.c_ulonglong), ("ullAvailPhys", ctypes.c_ulonglong),
                    ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong),
                    ("ullTotalVirtual", ctypes.c_ulonglong), ("ullAvailVirtual", ctypes.c_ulonglong),
                    ("ullAvailExtendedVirtual", ctypes.c_ulonglong)]

    def _read_cpu():
        idle, kernel, user = wintypes.FILETIME(), wintypes.FILETIME(), wintypes.FILETIME()
        ctypes.windll.kernel32.GetSystemTimes(ctypes.byref(idle), ctypes.byref(kernel), ctypes.byref(user))

        def ft(f):
            return (f.dwHighDateTime << 32) | f.dwLowDateTime
        return ft(kernel) + ft(user), ft(idle)  # kernel time already includes idle time

    def _read_mem():
        st = _MEMSTATUS()
        st.dwLength = ctypes.sizeof(_MEMSTATUS)
        ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(st))
        return st.ullTotalPhys - st.ullAvailPhys, st.ullTotalPhys
elif sys.platform == "darwin":
    import ctypes
    import re
    import subprocess

    _libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
    _libc.mach_host_self.restype = ctypes.c_uint
    _libc.host_statistics.argtypes = [ctypes.c_uint, ctypes.c_int, ctypes.POINTER(ctypes.c_uint),
                                      ctypes.POINTER(ctypes.c_uint)]
    _HOST = _libc.mach_host_self()

    def _memsize():
        val, size = ctypes.c_uint64(0), ctypes.c_size_t(8)
        _libc.sysctlbyname(b"hw.memsize", ctypes.byref(val), ctypes.byref(size), None, ctypes.c_size_t(0))
        return val.value

    _TOTAL = _memsize()

    def _read_cpu():
        ticks = (ctypes.c_uint * 4)()       # HOST_CPU_LOAD_INFO: user, system, idle, nice
        count = ctypes.c_uint(4)
        _libc.host_statistics(_HOST, 3, ticks, ctypes.byref(count))
        return ticks[0] + ticks[1] + ticks[2] + ticks[3], ticks[2]

    def _read_mem():
        """'Memory used' like Activity Monitor: app (active) + wired + compressed pages."""
        out = subprocess.run(["vm_stat"], capture_output=True, text=True, timeout=5).stdout
        m = re.search(r"page size of (\d+) bytes", out)
        page = int(m.group(1)) if m else 4096

        def pages(name):
            p = re.search(re.escape(name) + r":\s+(\d+)", out)
            return int(p.group(1)) if p else 0
        used = (pages("Pages active") + pages("Pages wired down") + pages("Pages occupied by compressor")) * page
        return used, _TOTAL
else:
    def _read_cpu():
        with open("/proc/stat", encoding="ascii") as f:
            parts = f.readline().split()[1:]
        vals = [int(v) for v in parts[:8]]
        idle = vals[3] + vals[4]          # idle + iowait
        return sum(vals), idle

    def _read_mem():
        info = {}
        with open("/proc/meminfo", encoding="ascii") as f:
            for line in f:
                k, v = line.split(":", 1)
                if k in ("MemTotal", "MemAvailable"):
                    info[k] = int(v.split()[0]) * 1024
        total = info.get("MemTotal", 0)
        return total - info.get("MemAvailable", 0), total


class Sampler:
    def __init__(self):
        self.prev = None
        self.cpu = []
        self.mem = []
        self.mem_total = 0
        self.last = 0.0

    def sample(self):
        """Take one sample (CPU % since the previous call, memory used in bytes)."""
        try:
            total, idle = _read_cpu()
            used, mtotal = _read_mem()
        except (OSError, ValueError):
            return
        if self.prev is not None:
            dt, di = total - self.prev[0], idle - self.prev[1]
            self.cpu.append(round(100.0 * (1 - di / dt), 1) if dt > 0 else 0.0)
            self.mem.append(used)
            del self.cpu[:-HISTORY], self.mem[:-HISTORY]
        self.prev = (total, idle)
        self.mem_total = mtotal
        self.last = time.time()

    def data(self):
        return {"interval": INTERVAL, "time": self.last, "cpu": self.cpu, "mem": self.mem,
                "mem_total": self.mem_total, "cores": os.cpu_count() or 1}

    def save(self, path=PATH):
        tmp = path + ".tmp"
        try:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(tmp, "w", encoding="ascii") as f:
                json.dump(self.data(), f)
            os.replace(tmp, path)
        except OSError:
            pass


def load(path=PATH, max_age=3 * INTERVAL):
    """History written by the daemon, or None if missing/stale."""
    try:
        with open(path, encoding="ascii") as f:
            d = json.load(f)
    except (OSError, ValueError):
        return None
    if time.time() - d.get("time", 0) > max_age:
        return None
    return d


class Monitor:
    """Used by the screensaver: prefers the daemon's history, falls back to sampling itself."""

    def __init__(self):
        self.own = Sampler()
        self.own.sample()
        self.next_own = time.time() + 1.0

    def get(self):
        d = load()
        if d is not None and d.get("cpu"):
            return d
        now = time.time()
        if now >= self.next_own:
            self.own.sample()
            self.next_own = now + INTERVAL
        return self.own.data() if self.own.cpu else None

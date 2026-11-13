#!/usr/bin/env python3
"""Claude Vivarium for macOS - idle agent (runs as a LaunchAgent, see install-mac.sh).

Every 2 s it checks how long the Mac has been idle. After idle_seconds (and if the screen is not
locked and no app keeps the display awake, e.g. a video) it starts vivarium_mac.py. When the
screensaver exits and the screen is not locked, it locks (fail closed). It also records CPU/memory
every 5 s so the graph has a 30-minute history right away.
"""
import os
import signal
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.realpath(__file__)))

import Quartz  # noqa: E402

import config  # noqa: E402
import sysmon  # noqa: E402
from vivarium_mac import lock_screen, log, screen_locked  # noqa: E402

SCREENSAVER = os.path.join(os.path.dirname(os.path.realpath(__file__)), "vivarium_mac.py")


def idle_seconds():
    return Quartz.CGEventSourceSecondsSinceLastEventType(Quartz.kCGEventSourceStateHIDSystemState,
                                                         Quartz.kCGAnyInputEventType)


def display_kept_awake_by_others():
    """True if another app (video player, presentation) prevents display sleep."""
    try:
        out = subprocess.run(["pmset", "-g", "assertions"], capture_output=True, text=True, timeout=5).stdout
    except (OSError, subprocess.SubprocessError):
        return False
    for line in out.splitlines():
        s = line.strip()
        if s.startswith("PreventUserIdleDisplaySleep") and s.split()[-1] != "0":
            return True
    return False


def main():
    child = None
    started_this_idle = False
    running = True

    def stop(*_):
        nonlocal running
        running = False
    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    sampler = sysmon.Sampler()
    old = sysmon.load(max_age=60)
    if old:
        sampler.cpu, sampler.mem = list(old.get("cpu", [])), list(old.get("mem", []))
    sampler.sample()
    next_sample = time.monotonic() + sysmon.INTERVAL
    cfg = config.load()
    log(f"agent started, idle_seconds={cfg['idle_seconds']}")

    while running:
        now = time.monotonic()
        if now >= next_sample:
            sampler.sample()
            sampler.save()
            next_sample = now + sysmon.INTERVAL
            cfg = config.load()  # pick up changed settings
        idle = idle_seconds()
        if child is not None:
            code = child.poll()
            if code is not None:
                child = None
                locked = screen_locked()
                log(f"screensaver exited (code {code}, locked={locked})")
                if not locked:
                    log("screen not locked after screensaver - locking now")
                    lock_screen()
        else:
            if idle < 5:
                started_this_idle = False  # user is back: arm for the next idle period
            elif idle >= cfg["idle_seconds"] and not started_this_idle:
                started_this_idle = True
                if screen_locked():
                    pass
                elif display_kept_awake_by_others():
                    log("another app keeps the display awake (video?) - not starting")
                else:
                    log("idle - starting screensaver")
                    child = subprocess.Popen([sys.executable, SCREENSAVER])
        time.sleep(2)

    if child is not None and child.poll() is None:  # agent stopped while shown: lock first
        log("stopping while the screensaver is shown - locking first")
        if not screen_locked():
            lock_screen()
        child.send_signal(signal.SIGTERM)
    log("agent stopped")


if __name__ == "__main__":
    main()

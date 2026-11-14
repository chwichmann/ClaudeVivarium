"""Adaptive frame rate: keeps the animation smooth when the computer is busy.

The scenes are slow-moving, so 30 fps is plenty - and 15 fps that arrives evenly looks better than 30 fps
that stutters. The governor watches how long each frame takes to produce. When producing frames eats too
large a share of the time (a stressed computer makes every frame slower and more uneven), it steps the
frame rate down, which also leaves more CPU to everything else. When there is plenty of headroom again it
steps back up, slowly, and waits longer each time it has to step down again soon after.

Used by the render loops of all platforms:  if gov.due(acc): ...render...; gov.record(seconds)
"""
import math
import time

ENABLED = True            # set from the setting "adaptive_fps"
LEVELS = (30, 20, 15)     # frames per second, best first (pass only rates that fit the timer, e.g. 30/15)
DOWN_AT = 0.60            # step down when (mean + spread of frame time) * fps exceeds this share of a second
UP_BELOW = 0.35           # step up only if the same figure would stay below this at the higher rate
SMOOTH = 0.10             # weight of each new frame in the running averages (~10 frames)
MIN_FRAMES = 20           # frames needed after a change before judging again
DOWN_WAIT = 1.0           # s between steps down
UP_WAIT = 20.0            # s of calm before the first step up (doubles after a quick relapse, up to 5 min)


class Governor:
    def __init__(self, base_fps=30, levels=LEVELS, enabled=None):
        self.levels = tuple(f for f in levels if f <= base_fps) or (base_fps,)
        self.base_fps = base_fps
        self.enabled = ENABLED if enabled is None else enabled
        self.level = 0
        self.mean = self.sq = None
        self.frames = 0
        self.changed = time.monotonic()
        self.up_wait = UP_WAIT
        self.last_up = None

    @property
    def fps(self):
        return self.levels[self.level] if self.enabled else self.base_fps

    def due(self, waited):
        """True when a new frame should be produced after `waited` seconds since the last one."""
        return waited >= 1.0 / self.fps - 0.5 / self.base_fps

    def _stress(self, fps):
        spread = math.sqrt(max(0.0, self.sq - self.mean ** 2))
        return (self.mean + spread) * fps

    def record(self, seconds, now=None):
        """Report the wall time one frame took (update + draw + handing it to the screen)."""
        if not self.enabled:
            return
        now = time.monotonic() if now is None else now
        if self.mean is None:
            self.mean, self.sq = seconds, seconds * seconds
        else:
            self.mean += SMOOTH * (seconds - self.mean)
            self.sq += SMOOTH * (seconds * seconds - self.sq)
        self.frames += 1
        if self.frames < MIN_FRAMES:
            return
        if self.level < len(self.levels) - 1 and now - self.changed >= DOWN_WAIT \
                and self._stress(self.levels[self.level]) > DOWN_AT:
            if self.last_up is not None and now - self.last_up < 60:
                self.up_wait = min(300.0, self.up_wait * 2)  # that did not hold: be more careful next time
            self.level += 1
            self._changed(now)
        elif self.level > 0 and now - self.changed >= self.up_wait \
                and self._stress(self.levels[self.level - 1]) < UP_BELOW:
            self.level -= 1
            self.last_up = now
            self._changed(now)

    def _changed(self, now):
        self.changed = now
        self.frames = 0

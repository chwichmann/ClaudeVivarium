"""Theme registry and the rotating show (switches theme every N minutes with a fade through black)."""
import random

import scene  # noqa: F401  (picks the text backend before the themes load)
from theme_aquarium import Aquarium
from theme_archipelago import Archipelago
from theme_espresso import Espresso
from theme_forest import Forest
from theme_guppies import Guppies
from theme_meerkats import Meerkats
from theme_pasture import Pasture
from theme_seabed import Seabed
from theme_space import Space
from theme_synthwave import Synthwave
from theme_terrarium import Terrarium

THEMES = {cls.name: cls for cls in (Aquarium, Space, Synthwave, Pasture, Seabed, Forest, Guppies, Terrarium,
                                   Meerkats, Espresso, Archipelago)}


def parse_list(value):
    """Theme selection from the settings -> list of valid theme names (never empty).

      all                         every theme
      pasture, seabed, forest     only these
      all, -synthwave, -space     every theme except these
    """
    words = [n.strip().lower() for n in (value or "all").replace(";", ",").split(",") if n.strip()]
    include = [n for n in words if not n.startswith("-")]
    exclude = {n[1:].strip() for n in words if n.startswith("-")}
    if not include or any(n in ("all", "random", "*") for n in include):
        names = list(THEMES)
    else:
        names = [n for n in include if n in THEMES]
    names = [n for n in names if n not in exclude]
    return names or list(THEMES)


class Show:
    """Looks like a Scene to the screensaver; rotates through the configured themes in random order.

    The list is shuffled, played through, then reshuffled - every theme comes once per round and
    the same theme never plays twice in a row.
    """
    FADE = 1.2  # seconds to fade out (and again to fade in)

    def __init__(self, names, minutes=10, show_text=True, start=None, decor=False, avoid=None):
        self.names = list(names)
        self.minutes = minutes
        self.show_text = show_text
        self.decor = decor
        self.avoid = avoid  # another Show (main monitor) whose theme this one should not repeat
        self.queue = []
        self.current = None
        self.current = start if start in self.names else self._next_name()
        self.scene = self._make(self.current)
        self.sessions = []
        self.system = None
        self.elapsed = 0.0
        self.fade = 0.0
        self.fading = 0  # +1 fading out, -1 fading in

    def _next_name(self):
        if not self.queue:
            self.queue = random.sample(self.names, len(self.names))
            if len(self.queue) > 1 and self.queue[0] == self.current:
                self.queue.append(self.queue.pop(0))
        if self.avoid is not None and len(self.queue) > 1 and self.queue[0] == self.avoid.current:
            self.queue.append(self.queue.pop(0))  # show something different from the main monitor
        return self.queue.pop(0)

    def _make(self, name):
        return THEMES[name](show_text=self.show_text, decor=self.decor)

    @property
    def name(self):
        return self.scene.name

    def set_sessions(self, sessions):
        self.sessions = sessions
        self.scene.set_sessions(sessions)

    def set_system(self, data):
        self.system = data
        self.scene.set_system(data)

    def show_hint(self, text, seconds=4.0):
        self.scene.show_hint(text, seconds)

    def next_theme(self):
        if len(self.names) > 1 and self.fading == 0:
            self.fading = 1
            self.elapsed = 0.0  # the new theme gets its full time

    def update(self, dt):
        self.scene.update(dt)
        self.elapsed += dt
        if self.minutes > 0 and self.fading == 0 and self.elapsed >= self.minutes * 60:
            self.next_theme()
            self.elapsed = 0.0
        if self.fading:
            self.fade += self.fading * dt / self.FADE
            if self.fade >= 1.0:
                self.fade = 1.0
                hint, until = self.scene.hint, self.scene.hint_until - self.scene.t
                self.current = self._next_name()
                self.scene = self._make(self.current)
                self.scene.set_sessions(self.sessions)
                self.scene.set_system(self.system)
                if until > 0:
                    self.scene.show_hint(hint, until)
                self.fading = -1
            elif self.fade <= 0.0:
                self.fade = 0.0
                self.fading = 0

    def draw(self, cr, w, h):
        self.scene.draw(cr, w, h)
        if self.fade > 0:
            cr.set_source_rgba(0, 0, 0, self.fade)
            cr.paint()

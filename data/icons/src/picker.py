"""Picker glyph: hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg.

TRAY-02: while the primary browser is the Picker, the tray icon (with the "Primary
Browser" style) and the Picker's radio item show a bulleted list: three dots, three
lines. Filled shapes only, like the Wye symbolic icon, so GTK and Plasma recolour it.
"""

from common import fmt

# On the 16 px grid: dots of radius 1.25 at x 3, bars from x 6 to 14, rows at y 3.5, 8, 12.5.
ROWS = (3.5, 8, 12.5)
DOT = dict(cx=3, r=1.25)
BAR = dict(x0=6, x1=14, half=1)

TEMPLATE = """\
<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <!-- Wye picker glyph: a bulleted list, as fills only, so GTK and Plasma can recolour it. -->
  <style id="current-color-scheme" type="text/css">.ColorScheme-Text {{ color: #2e3436; }}</style>
  <path class="ColorScheme-Text" fill="currentColor" d="{d}"/>
</svg>
"""


def f(value):
    return fmt(value, 2)


def _dot(y):
    """A full circle as two arcs, clockwise."""
    cx, r = DOT["cx"], DOT["r"]
    return (
        f"M{f(cx - r)} {f(y)}"
        f"A{f(r)} {f(r)} 0 1 1 {f(cx + r)} {f(y)}"
        f"A{f(r)} {f(r)} 0 1 1 {f(cx - r)} {f(y)}Z"
    )


def _bar(y):
    """A bar with round ends, clockwise."""
    x0, x1, h = BAR["x0"], BAR["x1"], BAR["half"]
    return (
        f"M{f(x0)} {f(y - h)}H{f(x1)}"
        f"A{f(h)} {f(h)} 0 0 1 {f(x1)} {f(y + h)}"
        f"H{f(x0)}"
        f"A{f(h)} {f(h)} 0 0 1 {f(x0)} {f(y - h)}Z"
    )


def render():
    d = "".join(_dot(y) + _bar(y) for y in ROWS)
    return TEMPLATE.format(d=d)

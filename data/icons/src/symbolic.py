"""Symbolic icon: hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg.

The tray's fixed "Wye" icon style (GEN-02) takes the panel's colour. GTK recolours
symbolic icons by forcing `fill` on every path, so the glyph must be filled outlines, not
strokes. Plasma recolours through the ColorScheme-Text class. Each arm is a closed
"sausage" (offset curves and round caps); both wind the same way, so the nonzero fill
rule unions them where they overlap.
"""

import math

from common import fmt

# Centreline on the 16 px grid: 2 px stroke, stem edges on x 7 and 9.
GLYPH = dict(cx=8, bottom=14, vy=8.8, ey=3, dx=5, r=3.2, k=0.5, width=2)

TEMPLATE = """\
<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
  <!-- Wye symbolic icon: the app glyph as fills only, so GTK and Plasma can recolour it. -->
  <style id="current-color-scheme" type="text/css">.ColorScheme-Text {{ color: #2e3436; }}</style>
  <path class="ColorScheme-Text" fill="currentColor" d="{d}"/>
</svg>
"""


def f(value):
    return fmt(value, 2)


def _unit(x, y):
    length = math.hypot(x, y)
    return x / length, y / length


def _intersect(p, d, q, e):
    """Intersection of the lines p + s*d and q + t*e."""
    den = d[0] * e[1] - d[1] * e[0]
    s = ((q[0] - p[0]) * e[1] - (q[1] - p[1]) * e[0]) / den
    return p[0] + s * d[0], p[1] + s * d[1]


def _offset_cubic(p0, p1, p2, p3, h):
    """Tiller-Hanson offset of a cubic by h; positive h is to the right of travel
    (y points down, so (-dy, dx) is the right-hand normal)."""
    legs = []
    for a, b in ((p0, p1), (p1, p2), (p2, p3)):
        dx, dy = _unit(b[0] - a[0], b[1] - a[1])
        n = (-dy * h, dx * h)
        legs.append(((a[0] + n[0], a[1] + n[1]), (b[0] + n[0], b[1] + n[1]), (dx, dy)))
    q1 = _intersect(legs[0][0], legs[0][2], legs[1][0], legs[1][2])
    q2 = _intersect(legs[1][0], legs[1][2], legs[2][0], legs[2][2])
    return legs[0][0], q1, q2, legs[2][1]


def _sausage(g, side, with_stem):
    """Closed outline of one arm, clockwise; with_stem also takes in the stem."""
    cx, vy, r, h = g["cx"], g["vy"], g["r"], g["width"] / 2
    ex, ey = cx + side * g["dx"], g["ey"]
    ux, uy = _unit(ex - cx, ey - vy)
    t = r * (1 - g["k"])
    curve = ((cx, vy + r), (cx, vy + t), (cx + ux * t, vy + uy * t), (cx + ux * r, vy + uy * r))
    left, right = _offset_cubic(*curve, -h), _offset_cubic(*curve, h)
    nx, ny = -uy * h, ux * h
    start_y = g["bottom"] if with_stem else vy + r
    d = [f"M{f(cx - h)} {f(start_y)}"]
    if with_stem:
        d.append(f"V{f(left[0][1])}")
    d.append(f"C{f(left[1][0])} {f(left[1][1])} {f(left[2][0])} {f(left[2][1])} "
             f"{f(left[3][0])} {f(left[3][1])}")
    d.append(f"L{f(ex - nx)} {f(ey - ny)}")
    d.append(f"A{f(h)} {f(h)} 0 0 1 {f(ex + nx)} {f(ey + ny)}")
    d.append(f"L{f(right[3][0])} {f(right[3][1])}")
    d.append(f"C{f(right[2][0])} {f(right[2][1])} {f(right[1][0])} {f(right[1][1])} "
             f"{f(right[0][0])} {f(right[0][1])}")
    if with_stem:
        d.append(f"V{f(start_y)}")
    d.append(f"A{f(h)} {f(h)} 0 0 1 {f(cx - h)} {f(start_y)}Z")
    return "".join(d)


def render():
    """The symbolic SVG as text: the lit route with the stem, then the other arm."""
    return TEMPLATE.format(d=_sausage(GLYPH, 1, True) + _sausage(GLYPH, -1, False))

"""Geometry shared by the Wye icon generators: number formatting, the continuous-corner
plate and the centrelines of the Y. Standard library only."""

import math

# Plate gradient, top to bottom. Every full-colour size uses it.
PLATE_STOPS = """\
      <stop offset="0" stop-color="#3fd0ea"/>
      <stop offset=".55" stop-color="#1f9ae8"/>
      <stop offset="1" stop-color="#2458dc"/>"""


def fmt(value, precision=1):
    """Shortest decimal for value at the given precision: 20.0 -> "20", -0.0 -> "0"."""
    text = f"{value:.{precision}f}"
    if "." in text:
        text = text.rstrip("0").rstrip(".")
    return "0" if text in ("-0", "") else text


SYMBOLIC_SCALE = 0.8
"""The symbolic glyphs are drawn edge to edge on the 16 px grid, then shrunk to this
fraction about the canvas centre, strokes included: Breeze symbolic icons keep a margin
and hairline weight, and a full-bleed glyph looks bigger and heavier beside them in the
panel."""


def shrink(value, centre=8):
    """A coordinate of a symbolic glyph, scaled by SYMBOLIC_SCALE about centre. Pass
    centre=0 for a length (a radius, a stroke width, an offset)."""
    return centre + (value - centre) * SYMBOLIC_SCALE


def opacity(value):
    """Opacity without the leading zero: 0.016 -> ".016"."""
    text = fmt(value, 3)
    return text[1:] if text.startswith("0.") else text


def _corner(radius, smoothing, budget):
    """Figma-style corner-smoothing parameters (a, b, c, d, p, arc) for one corner."""
    p = (1 + smoothing) * radius
    smoothing = min(smoothing, budget / radius - 1)
    p = min(p, budget)
    arc_measure = 90 * (1 - smoothing)
    arc = math.sin(math.radians(arc_measure / 2)) * radius * math.sqrt(2)
    alpha = (90 - arc_measure) / 2
    p3_to_p4 = radius * math.tan(math.radians(alpha / 2))
    beta = 45 * smoothing
    c = p3_to_p4 * math.cos(math.radians(beta))
    d = c * math.tan(math.radians(beta))
    b = (p - arc - c - d) / 3
    return 2 * b, b, c, d, p, arc


def squircle(x, y, size, radius, smoothing=0.6, precision=1):
    """Path of a square with continuous-curvature corners (corner smoothing 0.6 is the
    app-icon shape). Relative segments keep it short; see Figma's corner smoothing."""
    a, b, c, d, p, arc = _corner(radius, smoothing, size / 2)

    def f(value):
        return fmt(value, precision)

    ab, abc, bc, r = f(a + b), f(a + b + c), f(b + c), f(radius)
    a, c, d, arc = f(a), f(c), f(d), f(arc)
    near, far = f(x + p), f(x + size - p)
    top, bottom = f(y + p), f(y + size - p)
    path = (
        f"M{far} {f(y)}"
        f"c{a} 0 {ab} 0 {abc} {d}a{r} {r} 0 0 1 {arc} {arc}c{d} {c} {d} {bc} {d} {abc}"
        f"V{bottom}"
        f"c0 {a} 0 {ab} -{d} {abc}a{r} {r} 0 0 1 -{arc} {arc}c-{c} {d} -{bc} {d} -{abc} {d}"
        f"H{near}"
        f"c-{a} 0 -{ab} 0 -{abc} -{d}a{r} {r} 0 0 1 -{arc} -{arc}c-{d} -{c} -{d} -{bc} -{d} -{abc}"
        f"V{top}"
        f"c0 -{a} 0 -{ab} {d} -{abc}a{r} {r} 0 0 1 {arc} -{arc}c{c} -{d} {bc} -{d} {abc} -{d}Z"
    )
    return path.replace(" -", "-")


def _arm(cx, vy, dx, ey, r, k, side):
    """One arm of the Y, from where it leaves the stem to its terminal.

    (cx, vy) is the virtual vertex where the stem axis meets the straight arm, (cx +
    side * dx, ey) the terminal, r the fillet length on each side of the vertex and k
    how far the control points sit from the vertex (0.55-0.6 reads as a round fillet).
    """
    ex = cx + side * dx
    length = math.hypot(ex - cx, ey - vy)
    ux, uy = (ex - cx) / length, (ey - vy) / length
    t = r * (1 - k)
    return (
        f"C{fmt(cx)} {fmt(vy + t)} {fmt(cx + ux * t)} {fmt(vy + uy * t)} "
        f"{fmt(cx + ux * r)} {fmt(vy + uy * r)}L{fmt(ex)} {fmt(ey)}"
    )


def route(cx, vy, dx, ey, r, k, bottom, side):
    """The lit route: the stem from bottom, then one arm, as one continuous stroke."""
    return f"M{fmt(cx)} {fmt(bottom)}V{fmt(vy + r)}" + _arm(cx, vy, dx, ey, r, k, side)


def branch(cx, vy, dx, ey, r, k, side):
    """The other arm alone, starting where it leaves the stem."""
    return f"M{fmt(cx)} {fmt(vy + r)}" + _arm(cx, vy, dx, ey, r, k, side)

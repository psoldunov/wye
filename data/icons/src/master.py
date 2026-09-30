"""The 1024 master: hicolor/scalable/apps/dev.soldunov.wye.svg."""

from common import PLATE_STOPS, fmt, opacity, branch, route, squircle

# App-icon grid: an 824 plate inset 100 on the 1024 canvas, corner radius 185.4.
PLATE = (100, 824, 185.4)
# The edge highlight is an 8-wide stroke on a plate inset by 4, so it lies inside the plate.
RIM_INSET = 4
# Centreline of the Y. A 112 stroke centred on x 512 is pixel-aligned at 128, 256 and 512.
GLYPH = dict(cx=512, vy=528, dx=178, ey=308, r=165, k=0.6, bottom=782)
WIDTH = 112
# Soft drop shadows without filters: stacked strokes, each wider and fainter.
SHADOW_DY = 14
SHADOW = ((56, 0.016), (46, 0.018), (36, 0.022), (27, 0.026), (18, 0.03), (9, 0.034), (0, 0.04))
BRANCH_SHADOW_SCALE = 0.5
PLATE_SHADOW = ((4, 0.08), (8, 0.06), (14, 0.04), (22, 0.025))
GLASS_EDGE = 10

TEMPLATE = """\
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="1024" height="1024" viewBox="0 0 1024 1024">
  <!--
    Wye app icon, 1024 master.
    Grid: 824 plate inset 100 (app-icon template); continuous-corner squircle, radius 185.4,
    corner smoothing 0.6. Glyph: a Y junction drawn with a 112 stroke (pixel-aligned at 128/256/512).
    The lit route (stem and right arm) is the link's chosen destination; the tinted left arm is
    the other way it could have gone. No filters: shadows are stacked translucent strokes.
  -->
  <defs>
    <path id="plate-shape" d="{plate}"/>
    <path id="route-line" d="{route}"/>
    <path id="branch-line" d="{branch}"/>
    <linearGradient id="plate-fill" x1="0" y1="{plate_top}" x2="0" y2="{plate_bottom}" gradientUnits="userSpaceOnUse">
{plate_stops}
    </linearGradient>
    <linearGradient id="plate-sheen" x1="0" y1="{plate_top}" x2="0" y2="560" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#fff" stop-opacity=".18"/>
      <stop offset="1" stop-color="#fff" stop-opacity="0"/>
    </linearGradient>
    <linearGradient id="plate-edge" x1="0" y1="{plate_top}" x2="0" y2="{plate_bottom}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#fff" stop-opacity=".35"/>
      <stop offset=".25" stop-color="#fff" stop-opacity="0"/>
      <stop offset=".75" stop-color="#0a1a66" stop-opacity="0"/>
      <stop offset="1" stop-color="#0a1a66" stop-opacity=".25"/>
    </linearGradient>
    <linearGradient id="route-fill" x1="0" y1="{glyph_top}" x2="0" y2="{glyph_bottom}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#fff"/>
      <stop offset="1" stop-color="#dfeaff"/>
    </linearGradient>
    <linearGradient id="branch-rim" x1="0" y1="{glyph_top}" x2="0" y2="{fork}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#dcf7ff"/>
      <stop offset="1" stop-color="#7ab8f6"/>
    </linearGradient>
    <linearGradient id="branch-fill" x1="0" y1="{glyph_top}" x2="0" y2="{fork}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#a2e1fb"/>
      <stop offset="1" stop-color="#78b6f6"/>
    </linearGradient>
  </defs>
  <g id="plate-shadow" fill="#0a1a66">
{plate_shadow}
  </g>
  <g id="plate">
    <use xlink:href="#plate-shape" fill="url(#plate-fill)"/>
    <use xlink:href="#plate-shape" fill="url(#plate-sheen)"/>
    <path id="plate-rim" d="{rim}" fill="none" stroke="url(#plate-edge)" stroke-width="{rim_width}"/>
  </g>
  <g id="glyph" fill="none" stroke-linecap="round" stroke-linejoin="round">
    <g id="branch-shadow" stroke="#0a1a66" transform="translate(0 {shadow_dy})">
{branch_shadow}
    </g>
    <use id="branch-glass" xlink:href="#branch-line" stroke="url(#branch-rim)" stroke-width="{width}"/>
    <use id="branch" xlink:href="#branch-line" stroke="url(#branch-fill)" stroke-width="{core_width}"/>
    <g id="route-shadow" stroke="#0a1a66" transform="translate(0 {shadow_dy})">
{route_shadow}
    </g>
    <use id="route" xlink:href="#route-line" stroke="url(#route-fill)" stroke-width="{width}"/>
  </g>
</svg>
"""


def _shadow(line, scale):
    return "\n".join(
        f'      <use xlink:href="#{line}" stroke-width="{WIDTH + grow}" '
        f'stroke-opacity="{opacity(alpha * scale)}"/>'
        for grow, alpha in SHADOW
    )


def _plate_shadow():
    return "\n".join(
        f'    <use xlink:href="#plate-shape" y="{dy}" opacity="{opacity(alpha)}"/>'
        for dy, alpha in PLATE_SHADOW
    )


def render():
    """The master SVG as text."""
    start, size, radius = PLATE
    g = GLYPH
    centreline = (g["cx"], g["vy"], g["dx"], g["ey"], g["r"], g["k"])
    return TEMPLATE.format(
        plate=squircle(start, start, size, radius),
        rim=squircle(start + RIM_INSET, start + RIM_INSET, size - 2 * RIM_INSET, radius - RIM_INSET),
        rim_width=2 * RIM_INSET,
        route=route(*centreline, g["bottom"], 1),
        branch=branch(*centreline, -1),
        plate_top=fmt(start),
        plate_bottom=fmt(start + size),
        plate_stops=PLATE_STOPS,
        glyph_top=fmt(g["ey"] - WIDTH / 2),
        glyph_bottom=fmt(g["bottom"] + WIDTH / 2),
        fork=fmt(g["vy"] + g["r"]),
        plate_shadow=_plate_shadow(),
        shadow_dy=SHADOW_DY,
        branch_shadow=_shadow("branch-line", BRANCH_SHADOW_SCALE),
        route_shadow=_shadow("route-line", 1),
        width=WIDTH,
        core_width=WIDTH - GLASS_EDGE,
    )

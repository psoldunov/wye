"""Hand-tuned small sizes: hicolor/{16x16,24x24,32x32}/apps/dev.soldunov.wye.svg.

The master blurs below 48 px, so these redraw the same icon on the pixel grid: a larger
plate on whole pixels, a heavier stroke, a stem whose edges fall on pixel boundaries and
a lighter left arm so it still reads at 16 px. No sheen, rim or glyph shadow.
"""

from common import PLATE_STOPS, fmt, branch, route, squircle

# plate: (inset, size, corner radius); the rest is the Y centreline as in the master.
# The 24 px stem sits at x 11.5 so a 3 px stroke covers whole pixels 10-13.
SIZES = {
    16: dict(plate=(1, 14, 3.2), width=2, cx=8, bottom=12, vy=8.4, ey=4.6, dx=3.4, r=2,
             k=0.55, branch_top="#c8eefd", branch_bottom="#a6d6fb"),
    24: dict(plate=(2, 20, 4.5), width=3, cx=11.5, bottom=17.5, vy=12.2, ey=6.9, dx=4.6, r=3,
             k=0.55, branch_top="#c2ebfd", branch_bottom="#98cdfa"),
    32: dict(plate=(3, 26, 5.8), width=4, cx=16, bottom=23, vy=16.2, ey=9.4, dx=6, r=4.2,
             k=0.55, branch_top="#bde9fd", branch_bottom="#8ec6f8"),
}
PLATE_SHADOW_OPACITY = ".14"

TEMPLATE = """\
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{size}" height="{size}" viewBox="0 0 {size} {size}">
  <!-- Wye app icon, hand-tuned for {size} px: {plate_size} px plate on whole pixels, {width} px pixel-aligned stem. -->
  <defs>
    <path id="plate-shape" d="{plate}"/>
    <linearGradient id="plate-fill" x1="0" y1="{plate_top}" x2="0" y2="{plate_bottom}" gradientUnits="userSpaceOnUse">
{plate_stops}
    </linearGradient>
    <linearGradient id="branch-fill" x1="0" y1="{glyph_top}" x2="0" y2="{fork}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="{branch_top}"/>
      <stop offset="1" stop-color="{branch_bottom}"/>
    </linearGradient>
  </defs>
  <use id="plate-shadow" xlink:href="#plate-shape" y="1" fill="#0a1a66" opacity="{shadow}"/>
  <use id="plate" xlink:href="#plate-shape" fill="url(#plate-fill)"/>
  <g id="glyph" fill="none" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round">
    <path id="branch" d="{branch}" stroke="url(#branch-fill)"/>
    <path id="route" d="{route}" stroke="#fff"/>
  </g>
</svg>
"""


def render(size):
    """The hand-tuned SVG for one of SIZES, as text."""
    v = SIZES[size]
    inset, plate_size, radius = v["plate"]
    centreline = (v["cx"], v["vy"], v["dx"], v["ey"], v["r"], v["k"])
    return TEMPLATE.format(
        size=size,
        plate_size=plate_size,
        width=v["width"],
        # Two decimals keep the relative segments from drifting off the pixel grid.
        plate=squircle(inset, inset, plate_size, radius, precision=2),
        plate_top=fmt(inset),
        plate_bottom=fmt(inset + plate_size),
        plate_stops=PLATE_STOPS,
        glyph_top=fmt(v["ey"] - v["width"] / 2),
        fork=fmt(v["vy"] + v["r"]),
        branch_top=v["branch_top"],
        branch_bottom=v["branch_bottom"],
        shadow=PLATE_SHADOW_OPACITY,
        branch=branch(*centreline, -1),
        route=route(*centreline, v["bottom"], 1),
    )

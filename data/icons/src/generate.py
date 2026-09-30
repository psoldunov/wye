#!/usr/bin/env python3
"""Regenerate every shipped Wye icon.

Usage: python3 data/icons/src/generate.py [HICOLOR_DIR]

HICOLOR_DIR defaults to data/icons/hicolor beside this directory. Standard library only.
"""

import sys

sys.dont_write_bytecode = True  # keep __pycache__ out of the source tree

from pathlib import Path  # noqa: E402

import master  # noqa: E402
import small  # noqa: E402
import symbolic  # noqa: E402

DEFAULT_OUT = Path(__file__).resolve().parent.parent / "hicolor"
ICON = "dev.soldunov.wye"


def outputs():
    """(path relative to the hicolor directory, SVG text) for every icon."""
    yield f"scalable/apps/{ICON}.svg", master.render()
    for size in small.SIZES:
        yield f"{size}x{size}/apps/{ICON}.svg", small.render(size)
    yield f"symbolic/apps/{ICON}-symbolic.svg", symbolic.render()


def main(argv):
    if len(argv) > 2:
        sys.exit(__doc__)
    out = Path(argv[1]) if len(argv) == 2 else DEFAULT_OUT
    for relative, svg in outputs():
        path = out / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(svg, encoding="utf-8")
        print(path)


if __name__ == "__main__":
    main(sys.argv)

"""Turns a stage recording (shot.py record) into the README's demo GIF.

  demo.py FRAMES-DIR POINTER-LOG OUT.gif --area X Y W H --link X0 Y0 X1 Y1
          --cursors ARROW.png HAND.png --width W --gifski PATH

The frames are of the whole screen. The GIF shows AREA of it, zoomed in on
the action, and zooms out to the whole screen over ZOOM seconds once something
appears above AREA after the last click: the browser window it opened. All
coordinates are logical pixels.

The stage's screenshots have no pointer, so each frame gets the Breeze cursor
where drive.py last put the pointer before the frame was taken: a pointing hand
over the link (X0 Y0 X1 Y1), else the arrow. A stretch where nothing moves is
cut to HOLD seconds, and a browser window that has not drawn its page yet shows
for two frames, so the GIF does not wait for a page to load. Once zoomed out,
AFTER seconds more are shown and the last frame is held for END seconds. gifski encodes the frames at W pixels wide.
"""

import argparse
import bisect
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageChops, ImageStat

SCALE = 2  # device pixels per logical pixel in the stage
SCREEN = (0, 0, 1280, 800)  # the stage's screen (stage.sh)
HOLD = 0.6
ZOOM = 0.8
# Once zoomed out: AFTER seconds of the page settling, then the last frame held
# for END seconds.
AFTER = 1.5
END = 2.0
# Hotspots of the Breeze cursors (cursors_scalable/*/metadata.json), in the
# 32-unit SVG, which renders at 32 logical pixels for the default size of 24.
HOTSPOTS = {"arrow": (4, 4), "hand": (16.5, 4)}


def parse():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("frames", type=Path)
    parser.add_argument("pointer_log", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--area", type=float, nargs=4, required=True)
    parser.add_argument("--link", type=float, nargs=4, required=True)
    parser.add_argument("--cursors", type=Path, nargs=2, required=True)
    parser.add_argument("--width", type=int, required=True)
    parser.add_argument("--fps", type=float, default=20)
    parser.add_argument("--gifski", required=True)
    return parser.parse_args()


def read_frames(directory):
    frames = []
    for line in (directory / "frames.txt").read_text(encoding="utf-8").splitlines():
        taken, name = line.split()
        frames.append((float(taken), directory / name))
    return frames


def read_pointer(log):
    """Pointer positions over time, and the time of the last click."""
    times, points, clicked = [], [], 0.0
    for line in log.read_text(encoding="utf-8").splitlines():
        taken, x, y, *event = line.split()
        if event == ["click"]:
            clicked = float(taken)
        times.append(float(taken))
        points.append((float(x), float(y)))
    return times, points, clicked


def pointer_at(times, points, taken):
    index = bisect.bisect_right(times, taken) - 1
    return points[max(index, 0)]


def draw_cursor(frame, point, link, cursors):
    x, y = point
    over_link = link[0] <= x <= link[2] and link[1] <= y <= link[3]
    kind = "hand" if over_link else "arrow"
    hot_x, hot_y = HOTSPOTS[kind]
    frame.paste(cursors[kind], (round((x - hot_x) * SCALE), round((y - hot_y) * SCALE)), cursors[kind])
    return frame


def device(box):
    """A logical (x, y, w, h) box as a device-pixel crop box."""
    x, y, w, h = box
    return tuple(round(value * SCALE) for value in (x, y, x + w, y + h))


def blank(frame, area):
    """Whether AREA below its top 60 pixels is one flat colour: a browser
    window that has not drawn its page yet."""
    x, y, w, h = area
    page = frame.crop(device((x, y + 60, w, h - 60))).convert("L").resize((64, 48))
    return ImageStat.Stat(page).stddev[0] < 3


def zoom_start(frames, clicked, area):
    """The time of the first frame after the last click with something new
    above AREA."""
    band = device((0, 0, SCREEN[2], area[1]))
    before = None
    for taken, path in frames:
        if taken < clicked:
            continue
        with Image.open(path) as frame:
            top = frame.convert("RGB").crop(band).resize((128, 8))
        if before is None:
            before = top
        elif max(high for _, high in ImageChops.difference(before, top).getextrema()) > 40:
            return taken
    return float("inf")


def view(area, progress):
    """The shown box, from AREA (0) to the whole screen (1), eased."""
    eased = progress * progress * (3 - 2 * progress)
    return tuple(a + (s - a) * eased for a, s in zip(area, SCREEN))


def still(previous, current):
    """Whether two frames look the same at a glance."""
    small = (previous.width // 4, previous.height // 4)
    difference = ImageChops.difference(previous.resize(small), current.resize(small))
    return max(high for _, high in difference.getextrema()) < 12


def main():
    args = parse()
    frames = read_frames(args.frames)
    times, points, clicked = read_pointer(args.pointer_log)
    cursors = {
        "arrow": Image.open(args.cursors[0]).convert("RGBA"),
        "hand": Image.open(args.cursors[1]).convert("RGBA"),
    }
    out_dir = args.frames.parent / "gif"
    out_dir.mkdir(parents=True, exist_ok=True)
    size = (args.width, round(args.width * args.area[3] / args.area[2]))
    hold = round(HOLD * args.fps)
    zoom_at = zoom_start(frames, clicked, args.area)
    # The zoom advances per frame shown, so frames cut below do not make it jump.
    zoom_step = 1 / (ZOOM * args.fps)
    names, run, previous, empty, progress, settled = [], 0, None, 0, 0.0, 0
    for taken, path in frames:
        if progress >= 1.0:
            settled += 1
            if settled > AFTER * args.fps:
                break
        with Image.open(path) as raw:
            frame = raw.convert("RGBA")
        # Judged before the cursor is drawn on it.
        empty_page = taken > clicked and blank(frame, args.area)
        frame = draw_cursor(frame, pointer_at(times, points, taken), args.link, cursors)
        if taken >= zoom_at:
            progress = min(1.0, progress + zoom_step)
        frame = frame.convert("RGB").crop(device(view(args.area, progress))).resize(size, Image.LANCZOS)
        run = run + 1 if previous is not None and still(previous, frame) else 0
        previous = frame
        if run >= hold:
            continue
        # After the last click the browser opens; skip its empty window.
        if empty_page:
            empty += 1
            if empty > 2:
                continue
        name = out_dir / f"{len(names):05d}.png"
        frame.save(name, compress_level=1)
        names.append(str(name))
    # Hold the last frame.
    names.extend([names[-1]] * round(END * args.fps))
    print(f"{len(frames)} frames recorded, {len(names)} in the GIF", file=sys.stderr)
    subprocess.run(
        [
            args.gifski,
            "--fps",
            str(args.fps),
            "--quality",
            "90",
            "--width",
            str(args.width),
            "--output",
            str(args.out),
            *names,
        ],
        check=True,
    )


if __name__ == "__main__":
    main()

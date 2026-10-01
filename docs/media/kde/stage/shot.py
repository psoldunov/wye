"""Screenshots and frames from the demo stage, through KWin's ScreenShot2.

Run it with `stage.sh run`:

  shot.py screen OUT.png                    the whole screen, device pixels
  shot.py window OUT.png                    the active window with its frame
                                            and shadow, on a transparent
                                            background
  shot.py record DIR SECONDS [FPS [X Y W H]]
                                            frames DIR/NNNNN.png of the screen
                                            or of an area (logical pixels), in
                                            device pixels, and DIR/frames.txt
                                            with the time each was taken
  shot.py changed BEFORE AFTER [TOP [BOTTOM]]
                                            print "x y width height", in device
                                            pixels, of where two screenshots
                                            differ (between rows TOP and BOTTOM)

The stage's KWin runs with KWIN_SCREENSHOT_NO_PERMISSION_CHECKS, so no dialog
asks first.
"""

import os
import queue
import sys
import threading
import time
from pathlib import Path

import dbus
from PIL import Image, ImageChops

# QImage::Format values KWin sends, as Pillow raw modes (little-endian memory).
RAW_MODES = {
    4: ("RGB", "BGRX"),  # Format_RGB32
    5: ("RGBA", "BGRA"),  # Format_ARGB32
    6: ("RGBA", "BGRa"),  # Format_ARGB32_Premultiplied
    16: ("RGB", "RGBX"),  # Format_RGBX8888
    17: ("RGBA", "RGBA"),  # Format_RGBA8888
    18: ("RGBA", "RGBa"),  # Format_RGBA8888_Premultiplied
}

bus = dbus.SessionBus()
screenshot = dbus.Interface(
    bus.get_object("org.kde.KWin", "/org/kde/KWin/ScreenShot2"),
    "org.kde.KWin.ScreenShot2",
)


def capture_raw(method, *args, **options):
    """Call a ScreenShot2 method; return its answer and the pixels it wrote."""
    read_end, write_end = os.pipe()
    chunks = []

    def drain():
        with os.fdopen(read_end, "rb") as pipe:
            chunks.append(pipe.read())

    reader = threading.Thread(target=drain)
    reader.start()
    try:
        result = getattr(screenshot, method)(
            *args,
            dbus.Dictionary(
                {name.replace("_", "-"): dbus.Boolean(value) for name, value in options.items()},
                signature="sv",
            ),
            dbus.types.UnixFd(write_end),
        )
    finally:
        os.close(write_end)
    reader.join()
    return result, chunks[0]


def to_image(result, data):
    mode, raw = RAW_MODES[int(result["format"])]
    size = (int(result["width"]), int(result["height"]))
    image = Image.frombuffer(mode, size, data, "raw", raw, int(result["stride"]), 1)
    return image.convert("RGBA") if mode == "RGB" else image


def capture(method, *args, **options):
    return to_image(*capture_raw(method, *args, **options))


def screen(out):
    capture("CaptureActiveScreen", native_resolution=True).save(out)


def window(out):
    capture(
        "CaptureActiveWindow",
        include_decoration=True,
        include_shadow=True,
        native_resolution=True,
    ).save(out)


def record(directory, seconds, fps, area=None):
    """Raw pixels go to disk on a writer thread, so encoding never costs a
    frame; they become PNGs when the recording ends."""
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True)
    pending = queue.Queue()
    results = {}

    def write():
        while (item := pending.get()) is not None:
            number, result, data = item
            (directory / f"{number:05d}.raw").write_bytes(data)
            results[number] = result

    writer = threading.Thread(target=write)
    writer.start()
    period = 1 / fps
    times = []
    start = time.monotonic()
    while time.monotonic() - start < seconds:
        times.append(time.time())
        if area:
            result, data = capture_raw("CaptureArea", *area, native_resolution=True)
        else:
            result, data = capture_raw("CaptureActiveScreen", native_resolution=True)
        pending.put((len(times) - 1, result, data))
        time.sleep(max(0.0, start + len(times) * period - time.monotonic()))
    pending.put(None)
    writer.join()
    with open(directory / "frames.txt", "w", encoding="utf-8") as log:
        for number, taken in enumerate(times):
            raw = directory / f"{number:05d}.raw"
            image = to_image(results[number], raw.read_bytes())
            image.save(raw.with_suffix(".png"), compress_level=1)
            raw.unlink()
            log.write(f"{taken:.4f} {number:05d}.png\n")
    print(f"{len(times)} frames in {seconds} s")


def changed_box(before, after, top=0, bottom=None):
    """The box, in device pixels, where two screenshots differ between rows
    TOP and BOTTOM: where a popup opened. Printed as "x y width height"."""
    with Image.open(before) as old, Image.open(after) as new:
        bottom = new.height if bottom is None else bottom
        area = (0, top, new.width, bottom)
        difference = ImageChops.difference(old.convert("RGB").crop(area), new.convert("RGB").crop(area))
        # Ignore faint changes (a blinking cursor, dithering).
        box = difference.convert("L").point(lambda value: 255 if value > 24 else 0).getbbox()
    if box is None:
        raise SystemExit("shot.py: the screenshots are the same")
    left, upper, right, lower = box
    print(left, upper + top, right - left, lower - upper)


def main(arguments):
    command, *rest = arguments or ["help"]
    if command == "screen" and len(rest) == 1:
        screen(rest[0])
    elif command == "changed" and len(rest) in (2, 3, 4):
        bounds = [int(value) for value in rest[2:]]
        changed_box(rest[0], rest[1], *bounds)
    elif command == "window" and len(rest) == 1:
        window(rest[0])
    elif command == "record" and len(rest) >= 2:
        fps = float(rest[2]) if len(rest) > 2 else 20.0
        area = tuple(int(value) for value in rest[3:7]) if len(rest) >= 7 else None
        record(rest[0], float(rest[1]), fps, area)
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])

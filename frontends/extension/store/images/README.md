# Store listing images

Images for the extension's listings on the Chrome Web Store and on
addons.mozilla.org. The screenshots are captures of the real extension and the real
Wye on the KDE demo stage ([`docs/media/kde/stage/`](../../../../docs/media/kde/stage/README.md)),
with its made-up home: demo browser profiles and rules, nobody's own browsers or history.
Every PNG is 24-bit RGB without alpha, at the exact size the stores ask for.

| File | Size | Store | Shows |
|------|------|-------|-------|
| `chrome-1-context-menu.png` | 1280×800 | Chrome Web Store | Chromium on an article, the context menu of a link with **Open Link with Wye** hovered. |
| `chrome-2-picker.png` | 1280×800 | Chrome Web Store | The Wye picker that entry opened, with "from Chromium" and the link; the Work profile's tile is hovered. |
| `chrome-3-toolbar.png` | 1280×800 | Chrome Web Store | The page's context menu with **Open Page with Wye** hovered, and Wye's button pinned to the toolbar. |
| `firefox-1-context-menu.png` | 1280×800 | addons.mozilla.org | Firefox, the context menu of a link with **Open Link with Wye** hovered. |
| `firefox-2-picker.png` | 1280×800 | addons.mozilla.org | The picker that entry opened, with "from Firefox". |
| `promo-small-440x280.png` | 440×280 | Chrome Web Store (small promo tile, required) | Wye's icon and name. |
| `promo-marquee-1400x560.png` | 1400×560 | Chrome Web Store (marquee, optional) | Wye's icon, name and tagline. |

## Screenshots

`docs/media/kde/stage/store.sh` takes all five. It needs KDE Plasma 6 on the host, as the
stage does, and no network:

```sh
docs/media/kde/stage/stage.sh up light
docs/media/kde/stage/store.sh             # or: store.sh chrome, store.sh firefox
docs/media/kde/stage/stage.sh down
```

`stage.sh up` builds the checkout's Wye and `store.sh` the browsers (`chromium`,
`firefox`) and tools from the repository's nixpkgs; build them through the compute queue
first if you have one, so the scripts only find them.

What it does:

- Starts a small proxy (`store/serve.py`) that serves `store/page.html` as
  `https://journal.example.org/rust-on-the-desktop` under a demo CA that it adds to the
  browsers' certificate stores, and refuses every other host. The address bar shows a
  secure page and nothing leaves the machine.
- Links Chromium's and Firefox's desktop entries into the stage home and restarts
  Wye's service, so the picker names the browser a link came from.
- Assembles the extension with `frontends/extension/build.sh` for each family. Chromium
  loads it with `--load-extension`, on a profile whose preferences pin Wye's button,
  after one warm-up launch (a new profile offers sign-in and translation once). Firefox
  loads it as a temporary add-on through `about:debugging` and its file chooser;
  Marionette or the DevTools server would stripe the address bar as remote-controlled.
- Opens each menu with the stage's pointer, clicks the Wye entry, and takes the screen.
  Wye's configuration on the stage leaves **Force show picker when opening from browser
  extension** (ADV-10) on, so the picker opens for every link the extension sends.
- Halves each 2560×1600 capture to 1280×800 with a Lanczos filter, flattens it to RGB
  and optimises it with `oxipng --nx`, which keeps that colour type.

## Promo tiles

The SVG sources sit next to the PNGs. They reference the app icon in
`data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg`, so they follow it, and set the
name in Noto Sans. Render them from this directory:

```sh
resvg=$(nix build --no-link --print-out-paths nixpkgs#resvg)/bin/resvg
fonts=$(nix build --no-link --print-out-paths nixpkgs#noto-fonts)/share/fonts/noto
for name in promo-small-440x280 promo-marquee-1400x560; do
    "$resvg" --skip-system-fonts --use-fonts-dir "$fonts" "$name.svg" "/tmp/$name.png"
    magick "/tmp/$name.png" -background white -alpha remove -alpha off \
        -define png:color-type=2 "$name.png"
    nix run nixpkgs#oxipng -- -q -o max --strip safe --nx "$name.png"
done
```

Check a result with `magick identify -format '%f %wx%h %[type] %[channels]\n' *.png`:
every file should say `TrueColor srgb 3.0`.

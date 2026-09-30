# Wye

Wye is a native Linux browser picker written in Rust. It registers as the desktop's
default web browser and sends every link to the right browser, browser profile, private
window or desktop app, or asks you with a small picker when no rule decides. It looks like
it shipped with your desktop, in the spirit of
[Token Station](https://github.com/psoldunov/token-station).

- **Rules** pick a target by link, by the app that opened it, or by both, and can run a
  small JavaScript transform on the link first.
- **The picker** asks when no rule decides. Hold a key to open the alternative browser or
  a private window instead.
- **Link cleaning** removes tracking parameters and unwraps redirects such as safe-link
  wrappers, and expands shortened links.
- **The tray** shows the current default, the clipboard link and your recent links.
- **The browser extension** sends a link or the page you are on to Wye.

KDE Plasma 6 comes first: the tray applet, the global shortcuts portal and the source-app
and focused-window detection all target it. The core and the CLI work on any Linux desktop.
GNOME, Sway and Hyprland get the same routing but not yet the desktop-specific extras (see
[Not yet implemented](docs/architecture.md#not-yet-implemented)).

The specification starts at [docs/spec/README.md](docs/spec/README.md); the design is in
[docs/architecture.md](docs/architecture.md) and the D-Bus contract in
[docs/dbus-api.md](docs/dbus-api.md).

## Install

Wye ships as a Nix flake with two channels:

| Channel | What it builds | Use it when |
|---------|----------------|-------------|
| `release` | The latest tagged release, built by that release's own flake. | You want a version that was tagged and built in CI. This is the default once a release exists. |
| `git` | The flake's own source, which is the latest master commit when the input tracks master. | You want the newest changes. |

`nix/release.json` records the latest release. Until the first one is tagged, only the
`git` channel exists and it is the default. Each channel is a package:
`packages.<system>.wye-release` and `packages.<system>.wye-git` (`default` is `wye-git`).

Try it without installing:

```sh
nix run github:psoldunov/wye -- browsers
```

### home-manager

```nix
{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    home-manager.url = "github:nix-community/home-manager";
    home-manager.inputs.nixpkgs.follows = "nixpkgs";
    wye.url = "github:psoldunov/wye";
    wye.inputs.nixpkgs.follows = "nixpkgs";
    wye.inputs.home-manager.follows = "home-manager";
  };

  outputs = { nixpkgs, home-manager, wye, ... }: {
    homeConfigurations.me = home-manager.lib.homeManagerConfiguration {
      pkgs = nixpkgs.legacyPackages.x86_64-linux;
      modules = [
        wye.homeManagerModules.default
        {
          programs.wye = {
            enable = true;
            channel = "release"; # or "git"
            defaultBrowser = true;
            settings = {
              browsers.primary.picker = true;
            };
          };
        }
      ];
    };
  };
}
```

The module installs the package and the Plasma applet, links the D-Bus activation files,
and runs `wye service` as the systemd user unit `wye.service`. The UI host `wye-ui` (picker,
windows, tray-menu popup) runs as `wye-ui.service`, started by D-Bus activation when the
service first needs it.

| Option | Meaning |
|--------|---------|
| `programs.wye.enable` | Install Wye. |
| `programs.wye.channel` | `"release"` or `"git"`. Default: `release` when `nix/release.json` names a release, else `git`. Choosing `release` before a release exists fails with a message saying so. |
| `programs.wye.package` | The package to install. Defaults to the channel's package; set it to override. |
| `programs.wye.settings` | The contents of `$XDG_CONFIG_HOME/wye/config.toml`. When set, the file is a read-only link into the Nix store and Wye's Settings window cannot save. Leave it empty to keep the file writable. |
| `programs.wye.defaultBrowser` | Make Wye the handler of `http` and `https` in `mimeapps.list` (through `xdg.mimeApps`), and of HTML files when `settings.general.open-local-html` is on. On Plasma it also sets `BrowserApplication` in `kdeglobals`. |
| `programs.wye.launchAtLogin` | Start the service with the graphical session (default on). Off, it still starts on the first link. The unit owns login start: Wye writes no XDG autostart entry, and its "Launch at login" setting shows as managed by Nix. The config file stays writable. |

The `release` channel builds with the packaging of the tagged release, so it brings its own
nixpkgs revision into the closure; the `nixpkgs.follows` line above applies to the `git`
channel. The module itself always comes from the flake revision you lock, so it installs a
release's package through the file layout every release keeps (listed in
`nix/channel.nix`). To move to a newer release run `nix flake update wye`.

The service's unit sets its own `PATH`, which desktop entries with a bare `Exec=firefox`
are resolved against: the Nix profile directories first, then `/usr/local/bin`,
`/usr/bin` and `/bin`.

### NixOS

```nix
{
  inputs.wye.url = "github:psoldunov/wye";

  outputs = { nixpkgs, wye, ... }: {
    nixosConfigurations.host = nixpkgs.lib.nixosSystem {
      modules = [
        wye.nixosModules.default
        {
          programs.wye = {
            enable = true;
            channel = "git"; # or "release"
          };
        }
      ];
    };
  };
}
```

`programs.wye.{enable, channel, package}` install the package system-wide, register its
D-Bus files (`services.dbus.packages`) and its systemd user units `wye.service` and
`wye-ui.service` (`systemd.packages`). `programs.wye.defaultBrowser` sets the system-wide
default handler, and `programs.wye.launchAtLogin` starts the service with every graphical
session (Wye then leaves the XDG autostart entry alone, as with home-manager). The NixOS
module has no `settings` and does not register local HTML files (DEF-07): the
configuration is per user, so set those in Wye's Settings window or with home-manager.

### Other distributions

There is no other package yet (an AppImage is planned). With Nix installed, use the
home-manager module above; it works on any distribution. `nix profile install
github:psoldunov/wye` installs the binaries too, but systemd never looks for user units in
a Nix profile, and the session bus finds the D-Bus files only when the profile's `share` is
on `XDG_DATA_DIRS`. The D-Bus files name `SystemdService=`, so link the units yourself
(`systemctl --user link ~/.nix-profile/share/systemd/user/wye{,-ui}.service`).

## Set up on KDE Plasma

1. Make Wye the default browser: run `wye default set`, use the banner on the Settings
   **General** page, or use `programs.wye.defaultBrowser` above. `wye default unset`
   gives the default back to the browser Wye replaced.
2. Add the tray applet: right-click the system tray, choose **Configure System Tray…**,
   **Entries**, and set **Wye** to **Shown**. Wye shows its own tray icon until the applet
   registers, and hides it once the applet does.
3. Optional: start the browser extension's helper (see [Browser extension](#browser-extension)).

After a home-manager switch, a running Plasma session picks the applet up immediately (the
module announces it over D-Bus). After a NixOS switch, or if the applet still does not
appear, log out and in.

## Command line

```sh
wye open [--pick | --alternative] <url>...
wye test <url> [--source <desktop-id-or-exe>] [--keys <Shift+Ctrl…>] [--entry handler|clipboard|extension|cli] [--locked]
wye browsers
wye default [status|set|unset]
wye config [path|check]
wye service [--activate]
wye settings [page]
wye menu
wye clipboard [--alternative]
wye debug probe [--delay <seconds>]
wye extension install|remove
```

- `wye open` routes each URL and launches the target. With the service running it hands
  the link over; without it, `wye open` routes the link itself.
- `wye test` is a dry run that prints each pipeline step.
- `wye browsers` lists discovered browsers, their private modes and profiles.
- `wye default` shows, sets or unsets Wye as the default web browser.
- `wye config` prints the config path or checks the file.
- `wye service` runs the session service. `--activate` only asks D-Bus to start it.
- `wye settings` opens the Settings window, on a page such as `browsers` or `rules`.
- `wye menu` opens or closes the tray menu; bind it to a shortcut if your desktop has no
  global shortcuts portal.
- `wye clipboard` opens the link on the clipboard, in the alternative browser with
  `--alternative`.
- `wye debug probe` prints the held modifier keys, the pointer position and the focused
  app, which is what a real session provides and a test cannot fake. It helps when a key
  binding does not fire.
- `wye extension install` writes the browser extension's host manifests.

## Configuration

`$XDG_CONFIG_HOME/wye/config.toml`. See the illustrative example in
[docs/spec/12-data-model.md](docs/spec/12-data-model.md#storage). Most settings are in the
Settings window (`wye settings`). Scripts live next to the file as `transform.js` and
`rules/<rule-id>.js`; edits take effect on the next link.

`wye open` keeps configuration warnings (unknown keys and the like) quiet, because links
clicked in apps have no terminal to show them in; errors that make Wye fall back to the
defaults are always reported. Set `WYE_DEBUG` to any value to see the warnings too, or run
`wye config check`.

## Browser profiles

Wye finds the profiles of Chromium-based browsers (Chrome, Chromium, Brave, Vivaldi,
Edge) and of Firefox-based browsers (Firefox, Zen, LibreWolf, Floorp) on its own:

- **Chromium family:** it reads the browser's `Local State` file and launches the profile
  with `--profile-directory`.
- **Firefox family:** it reads both the classic `profiles.ini` and the profile groups
  Firefox 138 and later keep in `Profile Groups/<id>.sqlite`, and launches with
  `--profile <path>`.

Every profile is a target: pick it in a rule, in the primary or alternative browser
setting, or add it to the picker, where it shows with its name and a badge in the
profile's colour. Open **Settings → Browsers** and click **Rescan** when a profile you
created is not listed yet. `wye browsers` prints what Wye found.

## Browser extension

The extension adds **Open Link with Wye** and **Open Page with Wye** to the context menu,
a toolbar button and the `Alt+Shift+W` shortcut. It talks to Wye through the
native-messaging host `wye-native-host`, which the package installs next to `wye`.

Build the extension and load it:

```sh
nix build github:psoldunov/wye#extension
ls result/share/wye/extension   # firefox/ chromium/ and one zip of each
wye extension install           # tell every detected browser where the host is
```

Details, permanent installs and the message format are in
[frontends/extension/README.md](frontends/extension/README.md).

## Troubleshooting

- **A link opens the wrong browser.** Run `wye test <url>`; it prints each step of the
  pipeline, including the rule that matched. Add `--source <app>` to test a link from an
  app, and `--keys Shift` for held keys.
- **Wye is not asked for links.** Run `wye default`. Some browsers reset the default at
  start; turn off their own default-browser check. Under Nix, `programs.wye.defaultBrowser`
  makes `mimeapps.list` read-only, so a browser cannot overwrite it.
- **The tray icon is missing.** On Plasma, show **Wye** in **Configure System Tray…**. On
  GNOME the icon needs the AppIndicator extension.
- **No picker, or the shortcut does nothing.** Check `systemctl --user status wye` and
  `journalctl --user -u wye`. Run `wye debug probe --delay 3` and hold the key to see what
  the session reports. Without the global shortcuts portal, bind `wye menu` and
  `wye clipboard` in your compositor.
- **The Settings window says the config is read-only.** It is managed by Nix
  (`programs.wye.settings`); change it there.
- **Configuration warnings.** `wye config check`, or set `WYE_DEBUG=1`.
- **Two services.** Only one `wye service` owns `dev.soldunov.wye`; a second exits with
  status 75 and is not restarted.

## Develop

Everything runs through the dev shell. See [AGENTS.md](AGENTS.md) for the layout, the
commands and the review gates.

```sh
nix develop -c cargo build
nix develop -c cargo test --workspace --locked
nix build -L
```

## Licence

MIT. See [LICENSE](LICENSE).

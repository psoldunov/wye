# Wye

Wye is a native Linux browser picker written in Rust. It registers as the desktop's
default web browser, lives in the system tray, and sends every link to the right
browser, browser profile, private window or desktop app, or asks you with a small
picker. It is meant to look like it shipped with your desktop, in the spirit of
[Token Station](https://github.com/psoldunov/token-station).

Status: early development. The routing core and the `wye` CLI work; there is no picker,
tray or settings UI yet. The specification starts at
[docs/spec/README.md](docs/spec/README.md); the design is in
[docs/architecture.md](docs/architecture.md).

## Build

```sh
nix build
nix develop -c cargo build
```

## Usage

```sh
wye open [--pick | --alternative] <url>...
wye test <url> [--source <desktop-id-or-exe>] [--keys <Shift+Ctrl…>] [--entry handler|clipboard|extension|cli] [--locked]
wye browsers
wye default [status|set|unset]
wye config [path|check]
```

- `wye open` routes each URL and launches the target.
- `wye test` is a dry run that prints each pipeline step.
- `wye browsers` lists discovered browsers, their private modes and profiles.
- `wye default` shows, sets or unsets Wye as the default web browser.
- `wye config` prints the config path or checks the file.

## Configuration

`$XDG_CONFIG_HOME/wye/config.toml`. See the illustrative example in
[docs/spec/12-data-model.md](docs/spec/12-data-model.md#storage).

## Licence

MIT. See [LICENSE](LICENSE).

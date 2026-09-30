# Wye

Wye is a native Linux browser picker written in Rust. It registers as the desktop's
default web browser and sends every link to the right browser, browser profile, private
window or desktop app. The plan is for it to live in the system tray and to ask you with
a small picker when no rule decides, looking like it shipped with your desktop, in the
spirit of [Token Station](https://github.com/psoldunov/token-station).

Status: early development. The routing core and the `wye` CLI work; the picker, tray and
settings UI are planned but do not exist yet. Until the picker exists, a link that would
show it opens in the browser Wye replaced as the default, else the first shown or
discovered browser. The specification starts at
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
- `wye default` shows, sets or unsets Wye as the default web browser. `unset` gives the
  default back to the browser Wye replaced, and changes nothing when Wye is no longer the
  default.
- `wye config` prints the config path or checks the file.

## Configuration

`$XDG_CONFIG_HOME/wye/config.toml`. See the illustrative example in
[docs/spec/12-data-model.md](docs/spec/12-data-model.md#storage).

`wye open` keeps configuration warnings (unknown keys and the like) quiet, because links
clicked in apps have no terminal to show them in; errors that make Wye fall back to the
defaults are always reported. Set `WYE_DEBUG` to any value to see the warnings too, or run
`wye config check`.

## Licence

MIT. See [LICENSE](LICENSE).

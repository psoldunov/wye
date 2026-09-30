# Architecture

## Components

| Path | What it is |
|------|-----------|
| `crates/wye-core` | Pure routing core: config, targets, rules, matchers, URL cleaning, redirect unwrapping, web app catalogue and the pipeline that turns a URL into a resolved target. No IO. |
| `crates/wye-desktop` | Linux integration: desktop entry parsing, browser discovery, browser profiles, Exec expansion and launching, `mimeapps.list` default browser, source-app detection. |
| `crates/wye` | The `wye` binary: `open`, `test`, `browsers`, `default`, `config`. Wires the core to the desktop layer. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`), the desktop entry `dev.soldunov.wye.desktop` and the hicolor icon. |
| `nix/`, `flake.nix` | Package (crane), checks (clippy, tests, fmt, deny, machete, desktop entry, nixfmt) and dev shell. |

## Design decisions

### Pure core

`wye-core` does no IO. It takes a URL, a source, held keys and the configuration, and
returns a decision. Everything that touches the system (files, processes, the desktop
environment) lives in `wye-desktop` or `wye`. The core is testable without a desktop.

### In-process pipeline, no daemon yet

This first slice runs the pipeline inside `wye open`. There is no long-lived service. A
D-Bus service (DEF-04, `DBusActivatable` in the desktop entry), the picker and the tray come
with the frontends. The frontend strategy is still open decision #1 in
[docs/spec/14-open-questions.md](spec/14-open-questions.md). Until then the desktop entry
omits `DBusActivatable`.

### Configuration and state

Configuration is `$XDG_CONFIG_HOME/wye/config.toml`. It is hand-editable, reloaded when it
changes and never written with internal state. Internal state (previous default browser,
onboarding flags) lives in `$XDG_STATE_HOME/wye/state.toml`, because home-manager may make
the config file read-only.

### Shipped data is embedded

`data/services.toml`, `data/expansion.toml` and `data/tracking-parameters.toml` are embedded
at build time. The binary needs no data directory at run time.

### Interim picker fallback

While no picker surface exists, a link that resolves to the Picker opens in the remembered
previous default browser. Otherwise it opens in the first shown browser, then the first
discovered browser. Wye prints a warning each time.

## Not yet implemented

- Picker
- Tray
- Settings UI
- D-Bus service
- Transform scripts (rquickjs)
- Network short-link expansion
- Held-modifier detection
- Screen-lock state
- Notifications
- Clipboard features
- History
- Firefox profile groups
- Systemd scopes for launched apps (LAUNCH-06)
- Browser extension

## Runtime files

| Path | Contents |
|------|----------|
| `$XDG_CONFIG_HOME/wye/config.toml` | User configuration. Hand-edited. |
| `$XDG_STATE_HOME/wye/state.toml` | Internal state: previous default browser, onboarding flags. |
| `$XDG_CONFIG_HOME/mimeapps.list` | Default browser association, written by `wye default set`. |

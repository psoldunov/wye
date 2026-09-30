# Architecture

## Components

| Path | What it is |
|------|-----------|
| `crates/wye-core` | Pure routing core: config, targets, rules, matchers, URL cleaning, redirect unwrapping, web app catalogue and the pipeline that turns a URL into a resolved target. No IO. |
| `crates/wye-desktop` | Linux integration: desktop entry parsing, browser discovery, browser profiles, Exec expansion and launching, `mimeapps.list` default browser, source-app detection. |
| `crates/wye` | The `wye` binary: `open`, `test`, `browsers`, `default`, `config`. Wires the core to the desktop layer. |
| `data/` | Shipped data (`services.toml`, `expansion.toml`, `tracking-parameters.toml`), the desktop entry `dev.soldunov.wye.desktop` and the hicolor icon. |
| `nix/`, `flake.nix` | Package (crane), checks (clippy, tests, fmt, deny, machete, source and installed desktop entry, nixfmt) and dev shell. The package rewrites the installed desktop entry's `Exec` to its own absolute `bin/wye` and adds `TryExec`; the source entry in `data/` stays generic. |

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

Configuration is `$XDG_CONFIG_HOME/wye/config.toml`. It is hand-editable and never written
with internal state. With no long-lived process yet, every `wye` invocation reads it afresh;
reloading when the file changes ([12-data-model.md](spec/12-data-model.md#storage)) comes
with the long-lived service. A broken or unreadable file never stops a link: Wye reports it and
uses the defaults. On the `wye open` path every diagnostic is written to stderr best effort,
and the picker stand-in is announced only after the launch, because apps often start Wye
with a closed or broken stderr.

Internal state lives in `$XDG_STATE_HOME/wye/state.toml`, because home-manager may make the
config file read-only. Today it holds only `previous-default-browser`; onboarding flags are
future work. State and `mimeapps.list` are replaced atomically through one helper
(`wye_desktop::atomic::write`: a temporary file in the same directory, synced, then renamed),
which never writes through a symlink.

### Loop guard (DEF-06)

Wye is the default browser, so anything that opens a link "in the default browser" opens it
in Wye again. Beyond Wye's own desktop ID, an entry whose `Exec` runs `wye` or a generic
opener (`xdg-open`, `gio`, `kde-open`, `handlr` and the others in
`wye_desktop::loop_guard::OPENERS`) is never a web handler or an available target, and
`build_command` refuses any command line that runs one, or that resolves to the running
executable. `wye default set` never remembers such an entry as the browser to restore, and
`wye default unset` never restores one. Shell wrappers and D-Bus calls to the `OpenURI`
portal are not recognised.

### Default browser (DEF-02, DEF-05)

The current default is the first *installed* desktop ID listed for
`x-scheme-handler/https` in the `mimeapps.list` lookup order, as the mime-apps specification
says; IDs whose entry is missing are skipped. `wye default unset` changes nothing (and
exits 0, saying so) when Wye is no longer the default, so a browser the user chose since is
never overwritten; the remembered browser is kept.

### Startup notification

The desktop entry keeps `StartupNotify=true`: that is how launchers hand Wye the activation
token (`XDG_ACTIVATION_TOKEN`, `DESKTOP_STARTUP_ID`) it passes on to the browser (LAUNCH-03).
The cost: when Wye starts nothing that consumes the token, as with a background launch
(LAUNCH-04, which removes the variables) or a rejected link, the launcher's startup sequence
is left to time out, which on X11 shows a busy cursor for a few seconds.

### Source-app detection

`wye open` walks its parent chain in `/proc`, skipping launch helpers and shells, and takes
the app's desktop ID from its systemd scope or `GIO_LAUNCHED_DESKTOP_FILE`. When the chain
names nothing, as when `gio open` has already exited and Wye was reparented to
`systemd --user`, the app unit in Wye's own `/proc/self/cgroup` is used, unless it is Wye's
own. Spec step 3 (matching the executable against desktop entries' `Exec`) and the portal
and focused-window fallbacks are not implemented yet.

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
- Reloading the configuration when it changes (it is read on every invocation)
- Watching `mimeapps.list` for default-browser changes (DEF-03) and `applications`
  directories for installed apps (DISC-02); both are read on every invocation
- Plasma's `kdeglobals` browser setting (DEF-02); only `mimeapps.list` is written
- Localised desktop-entry keys such as `Name[de]`
- Profile badges (DISC-08)
- Source-app detection: matching the executable against desktop entries' `Exec` (spec
  step 3), and the portal and focused-window fallbacks
- Onboarding state

## Runtime files

| Path | Contents |
|------|----------|
| `$XDG_CONFIG_HOME/wye/config.toml` | User configuration. Hand-edited. |
| `$XDG_STATE_HOME/wye/state.toml` | Internal state: the previous default browser (`previous-default-browser`). |
| `$XDG_CONFIG_HOME/mimeapps.list` | Default browser association, written by `wye default set`. |

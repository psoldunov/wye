# The AppImage cannot launch Flatpak browsers

**Status:** root cause confirmed and fixed
([LAUNCH-08](../spec/05-browsers.md#discovery-and-launching)). **Found:** 2026-10-04, against Wye 1.1.0
(`Wye-1.1.0-x86_64.AppImage`). **Severity:** every link dies silently on an AppImage
install whose browsers are Flatpaks.

The AppImage's runtime puts its own `bin/` first on `PATH`. That directory holds a
[sharun](https://github.com/VHSgunzo/sharun) shim named `bwrap`. Wye hands its whole
environment to the browser it launches, `flatpak run` resolves `bwrap` through `PATH`,
finds the shim instead of `/usr/bin/bwrap`, and exits. The browser never starts, and Wye
reports the launch as successful — so nothing in the log, the UI or the history says a
link was lost.

This breaks [LAUNCH-01](../spec/05-browsers.md#discovery-and-launching) ("so Flatpak and
Snap wrappers keep working") and slips past
[LAUNCH-07](../spec/05-browsers.md#discovery-and-launching) (the launch-failure
notification), because from Wye's side the launch did not fail.

## Symptom

Clicking any link routed by Wye does nothing at all. No window, no error, no
notification. The tray icon, the picker, the settings window and browser discovery all
work normally; only the last step — starting the browser — fails. The service log records
a successful open with a PID, and with history on
([ADV-09](../spec/10-advanced.md#history)) the lost link is written to the history as
opened.

## The system under test

| | |
|---|---|
| OS | SteamOS (`ID_LIKE=arch`), kernel `6.18.50-valve2-1-neptune-618-gc7289a96b14d` |
| Desktop | KDE Plasma 6.4.3, Wayland (`XDG_SESSION_TYPE=wayland`, `XDG_CURRENT_DESKTOP=KDE`) |
| Wye | 1.1.0, AppImage at `~/Downloads/Wye-1.1.0-x86_64.AppImage`, symlinked from `~/.local/bin/wye`; integrated by `packaging/appimage/AppRun` (desktop entry, three D-Bus service files, icons, links) |
| Default browser | Wye (`~/.config/mimeapps.list` and `BrowserApplication` in `~/.config/kdeglobals`); `wye default` agrees |
| Browsers | Firefox, Google Chrome and Zen Browser — **all three Flatpaks**, no natively installed browser |
| Flatpak | 1.16.6; bubblewrap 0.12.0 at `/usr/bin/bwrap` |

Browser discovery, profile discovery and routing are all correct. `wye browsers` finds
all three with their profiles and private-window actions, and `wye test` expands the right
`Exec`:

```console
$ wye test https://www.ensemblr.dev/
Link: https://www.ensemblr.dev/
  No match, primary browser: org.mozilla.firefox.desktop
Result: https://www.ensemblr.dev/
Opens in: Firefox (org.mozilla.firefox.desktop)
Command: /usr/bin/flatpak run --branch=stable --arch=x86_64 --command=firefox --file-forwarding org.mozilla.firefox @@u https://www.ensemblr.dev/ @@
```

## Root cause

The chain, in order:

1. The AppImage's runtime (sharun, through `quick-sharun`) prepends `$APPDIR/bin` to
   `PATH` before `packaging/appimage/AppRun` runs. `AppRun` knows this — its closing
   comment explains that Wye's own programs live in `libexec/` rather than `bin/` for
   exactly that reason — but the `PATH` itself stays.
2. `$APPDIR/bin` is not a directory of helper programs. Every executable in it is the
   same file: sharun, hardlinked under the name of each helper the bundled GTK, Qt and
   GStreamer libraries expect to find on `PATH`. One of those names is **`bwrap`**.
3. `wye_desktop::launch::spawn_with_env` (`crates/wye-desktop/src/launch.rs:151`) builds
   the child with `Command::new`, which inherits the service's environment. Only
   `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID` are ever touched
   ([LAUNCH-03](../spec/05-browsers.md#discovery-and-launching),
   [LAUNCH-04](../spec/05-browsers.md#discovery-and-launching)). So the browser command
   inherits the AppImage's `PATH`.
4. `flatpak run` looks up `bwrap` as a bare name, through `PATH`. It finds
   `$APPDIR/bin/bwrap` — the sharun shim — and execs it with `argv[0]` set to the bare
   string `"bwrap"`.
5. sharun locates its own installation relative to `argv[0]`. A bare `argv[0]` gives it
   nothing to resolve, so it prints `Failed to find ARG0 dir!` and exits 1. Firefox is
   never reached.

The failing step is inside `flatpak`'s child, after `fork`/`exec` of `/usr/bin/flatpak`
has already succeeded, which is why Wye sees a healthy launch.

## Evidence

Five experiments, narrowing from the symptom to the single exec. The AppImage's mount
point is per-start; `/tmp/.mount_Wye1remp1553915857831472810` is the one from this
session.

Experiments 2 to 5 run the command line `wye test` printed, under the environment of the
*live* service read out of `/proc`, so nothing about the environment is reconstructed by
hand. `replay-env.py` below is that helper; its three modes differ only in how they build
the child's `PATH`.

```python
import os, sys
mode, pid, cmd = sys.argv[1], sys.argv[2], sys.argv[3:]
env = {}
with open(f"/proc/{pid}/environ", "rb") as f:
    for item in f.read().split(b"\0"):
        if item:
            k, _, v = item.partition(b"=")
            env[k.decode()] = v.decode()
appdir = env.get("APPDIR", "")
clean = [p for p in env["PATH"].split(":") if not p.startswith(appdir)]
if mode == "--as-service":        # 2: inherit the service's PATH unchanged
    pass
elif mode == "--strip-appdir-path":   # 3: drop every $APPDIR entry
    env["PATH"] = ":".join(clean)
elif mode == "--probe":               # 4: clean PATH plus one shadowing directory
    env["PATH"] = os.environ["PROBE_DIR"] + ":" + ":".join(clean)
os.chdir(os.path.expanduser("~"))
os.execvpe(cmd[0], cmd, env)
```

### 1. End to end: Wye reports success, no browser exists

```console
$ wye open https://example.com            # exit 0, no window appears
$ journalctl --user --since @$mark
Oct 04 23:02:56 steamdeck Wye-1.1.0-x86_64.AppImage[10185]:  INFO wye_service::api::link::launch: opened target=org.mozilla.firefox.desktop url=https://example.com/ pid=15010
Oct 04 23:02:56 steamdeck systemd[1398]: Started Firefox launched by Wye.
Oct 04 23:02:56 steamdeck systemd[1398]: Started app-flatpak-org.mozilla.firefox-544582251.scope.
$ ps aux | grep '[f]irefox'               # nothing
```

Both scopes are created — Wye's (LAUNCH-06) and `flatpak`'s own — and then the process
tree is empty. `flatpak` got far enough to set up its scope before dying on the exec.

### 2. The same command in the service's own environment

Replaying the exact command line from `wye test`, with the environment read from the live
service's `/proc/<pid>/environ` so nothing is approximated:

```console
$ python3 replay-env.py --as-service 10185 /usr/bin/flatpak run --branch=stable \
      --arch=x86_64 --command=firefox --file-forwarding org.mozilla.firefox \
      @@u https://example.com @@
Failed to find ARG0 dir!
exit=1
```

### 3. The same command with `$APPDIR` removed from `PATH`

Identical environment, except that every `PATH` entry under `$APPDIR` is dropped:

```console
$ python3 replay-env.py --strip-appdir-path 10185 /usr/bin/flatpak run … @@
[2] Sandbox: CanCreateUserNamespace() clone() failure: EPERM
exit=124            # killed by `timeout` after 20s — it was running
$ ps aux | grep -c '[f]irefox'
4
```

Firefox starts and stays up. (The `CanCreateUserNamespace` line is Firefox's normal
nested-sandbox probe inside a Flatpak, not an error.) This is also the proof that the
proposed fix works: stripping `$APPDIR` from the child's `PATH` is sufficient.

### 4. `bwrap` alone reproduces it

A `PATH` whose only addition is a directory containing one symlink,
`bwrap -> $APPDIR/bin/bwrap`, with the rest of `PATH` clean:

```console
$ ln -s $APPDIR/bin/bwrap /tmp/wye-probe/bwrap
$ PROBE_DIR=/tmp/wye-probe python3 replay-env.py --probe 10185 /usr/bin/flatpak run … @@
Failed to find ARG0 dir!
exit=1
```

Nothing else in `$APPDIR/bin` is involved.

### 5. The exec itself

```console
$ strace -f -qq -e trace=execve -o log \
      python3 replay-env.py --as-service 10185 /usr/bin/flatpak run … @@
$ grep -i bwrap log
15216 execve("/tmp/.mount_Wye1remp1553915857831472810/bin/bwrap",
          ["bwrap", "--args", "74", "--", "firefox", "https://example.com"],
          0x5640e6b05050 /* 0 vars */) = 0
15224 execve("/tmp/.mount_Wye1remp1553915857831472810/bin/bwrap",
          ["bwrap", "--args", "76", "--", "xdg-dbus-proxy", "--args=75"], … <unfinished …>
```

`/usr/bin/bwrap` is never execed. Note `argv[0]` is the bare `"bwrap"`: that is what
sharun cannot resolve.

`FLATPAK_BWRAP=/usr/bin/bwrap` does **not** help — this Flatpak build takes `bwrap` from
`PATH` regardless, so there is no environment knob to work around it with.

### The shim, for the record

```console
$ stat -c '%i %h %n' $APPDIR/sharun $APPDIR/bin/bwrap $APPDIR/bin/gio-launch-desktop
7704 18 …/sharun
7704 18 …/bin/bwrap
7704 18 …/bin/gio-launch-desktop
$ ls -1 $APPDIR/bin/
05-gsettings-backend.hook
bwrap
gio-launch-desktop
glycin-image-rs
glycin-jxl
glycin-svg
gst-completion-helper
gst-hotdoc-plugins-scanner
gst-plugin-scanner
gst-ptp-helper
qt.conf
```

One inode, eighteen links. `gio-launch-desktop` is the second name in there that a
launched app may well resolve through `PATH` (GLib uses it for
`g_desktop_app_info_launch*`), so `bwrap` is unlikely to be the only casualty — it is
only the one every Flatpak hits.

## Why Wye reported success

Two places drop the information that would have named this in one line.

`crates/wye-desktop/src/launch.rs:176` — the reaper thread discards the child's exit
status on purpose:

```rust
std::thread::spawn(move || {
    // The exit status of a launched browser is of no interest; waiting
    // only releases the process-table entry.
    let _ = child.wait();
});
```

and `crates/wye-service/src/api/link/launch.rs:28` treats `Ok(pid)` as the end of the
story: it logs `opened`, and under
[PIPE-16](../spec/11-url-pipeline.md#processing-order) records the link in the history as
having opened, when history is on — the line above that call reads
`// PIPE-16: only a link that opened is recorded.` Standard output and standard error are
`/dev/null` (`crates/wye-desktop/src/launch.rs:158-160`), so `Failed to find ARG0 dir!`
goes nowhere.

LAUNCH-07's notification only fires on the `Err` arm, which means fork/exec failing. A
child that execs cleanly and then dies a millisecond later is, by construction, a
successful launch.

## Other environment that leaks into launched apps

`spawn_with_env` passes the service's whole environment on, and the AppImage's
environment is not the session's. Besides `PATH`:

| Variable | Value in the service | Effect on a launched app |
|---|---|---|
| `XDG_CACHE_HOME` | `~/.cache/AppImage-Cache` (set by sharun's `AppRun.lib` cache hook) | A Flatpak browser overrides it inside its sandbox, so no effect here. A **natively installed** browser would write its cache under the AppImage's cache directory. |
| `URUNTIME` | the AppImage's own path | The hazard `AppRun` already unsets `APPIMAGE` for, under a second name: an app whose updater takes it for its own file would overwrite Wye's AppImage. |
| `APPDIR`, `SHARUN_DIR`, `APPOFFSET`, `APPIMAGE_ARCH`, `APPIMAGE_UID`, `URUNTIME_DIR` | set | Markers that tell a launched app it is running inside an AppImage it has nothing to do with. |
| `HOSTPATH`, `HOST_HOME`, `HOST_XDG_{CACHE,CONFIG,DATA,STATE}_HOME` | set | sharun's saved originals. Harmless to inherit, and useful: they are how the launcher can restore the session's values. |

`AppRun` already unsets `APPIMAGE` and `OWD` for precisely this reason, with a comment
naming the Electron-updater case. The list was just incomplete, and `PATH` cannot be
fixed there anyway — Wye's own programs need `$APPDIR/bin`.

## What is affected

| Install | Affected |
|---|---|
| AppImage + Flatpak browsers | **Yes** — every launch of every browser. On an immutable distribution (SteamOS, Bazzite, Silverblue) that is usually every browser installed. |
| AppImage + natively installed browsers | Not by `bwrap`. Still gets the wrong `XDG_CACHE_HOME`, and anything that resolves a bundled helper name through `PATH` is at risk. |
| `.deb`, `.rpm`, Nix, home-manager, NixOS | No. These install real binaries with no sharun environment; the `Exec` and the `PATH` the service inherits are the session's. |

## The fix

Scrub the AppImage environment at the launch boundary — in `spawn_with_env`
(`crates/wye-desktop/src/launch.rs:151`), for the child only. It cannot be done in
`AppRun`, because Wye's own four programs need `$APPDIR/bin` on `PATH` to find the helpers
the bundled libraries look for.

1. Drop every `PATH` entry equal to or under `$APPDIR`, keeping all the others —
   including the `~/.local/bin` that `AppRun` deliberately prepends so the autostart entry
   and the native-messaging manifests name the stable links.
2. Generalise that: for each inherited variable, filter `$APPDIR` components out of
   `:`-separated path lists and unset the variable outright when its whole value points
   into `$APPDIR`. This build of the AppImage happens not to set `LD_LIBRARY_PATH`,
   `GI_TYPELIB_PATH`, `QT_PLUGIN_PATH` and friends, but a future sharun or a different
   bundling mode may, and the sweep costs nothing.
3. Restore `XDG_CACHE_HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME` and
   `HOME` from `HOST_XDG_*` and `HOST_HOME` where sharun has set them.
4. Unset the markers: `APPDIR`, `APPOFFSET`, `APPIMAGE_ARCH`, `APPIMAGE_UID`,
   `SHARUN_DIR`, `URUNTIME`, `URUNTIME_DIR`, `HOSTPATH`, `HOST_*`, next to the `APPIMAGE`
   and `OWD` that `AppRun` already drops.

Shaped as a pure function from an iterator of environment pairs to a list of changes, it
unit-tests without a process and without an AppImage, next to the existing tests in
`crates/wye-desktop/src/launch/tests.rs`.

Worth doing in the same change, independently of the cause: make the reaper log a warning
when the child exits non-zero, and consider extending LAUNCH-07 to a child that dies
within a moment of starting. Either one turns this class of bug from invisible into a
single log line, and the history entry (PIPE-16) would stop claiming a lost link opened.

## The fix as built

The scrub lives in the private module `crates/wye-desktop/src/launch/appimage.rs`.
`spawn_with_env` (`crates/wye-desktop/src/launch.rs`) applies it to every launched app's
environment before it handles the activation token.

- **Gate.** It acts only when `APPDIR` and `SHARUN_DIR` are both set to absolute paths
  other than `/` that name the same directory, and Wye's own executable is inside it.
  sharun sets both in every mode, an extracted AppDir included. A `.deb`, `.rpm` or Nix
  install changes nothing, and neither does a stray `APPDIR`, even one such as `/usr`
  above a packaged `/usr/bin/wye`.
- **Markers removed.** `APPDIR`, `APPIMAGE`, `APPOFFSET`, `APPIMAGE_ARCH`,
  `APPIMAGE_UID`, `ARGV0`, `OWD`, `SHARUN_DIR`, `URUNTIME`, `URUNTIME_DIR`, `HOSTPATH`,
  `HOST_HOME`, `HOST_XDG_{CONFIG,DATA,CACHE,STATE}_HOME` and `HOST_KERNEL_VERSION`.
- **Session values restored.** When `HOST_HOME` is a non-empty absolute path, `HOME` and
  `XDG_{CONFIG,DATA,CACHE,STATE}_HOME` come back from sharun's saved originals, so
  `XDG_CACHE_HOME` goes from `~/.cache/AppImage-Cache` back to the session's value.
- **Everything else swept.** In every other variable, the `:`-separated components inside
  the AppDir are dropped (component-wise match, against both `$APPDIR` as given and its
  canonical path, which sharun uses for the values it sets), and the variable is removed
  when nothing non-empty is left. `PATH` loses `$APPDIR/bin` and keeps `~/.local/bin`;
  `XDG_DATA_DIRS` loses `$APPDIR/share`; `GIO_LAUNCH_DESKTOP` and `GTK_EXE_PREFIX` go. A
  path embedded in a longer value (`--opt=$APPDIR/x`) is not a component and stays.
- **Portable mode.** quick-sharun's `05-gsettings-backend.hook` sets
  `GSETTINGS_BACKEND=keyfile` when `Wye.AppImage.home` or `Wye.AppImage.config` exists.
  When `HOME` or `XDG_CONFIG_HOME` is restored, a `GSETTINGS_BACKEND` of `keyfile` is
  removed too, so a GTK browser keeps the session's dconf.
- **Reaper warning.** The reaper logs a warning ("launched app exited with a failure",
  with the program, the pid, the status and how long it ran) when a launched app exits
  non-zero.

Tests: unit tests in `crates/wye-desktop/src/launch/appimage.rs`; an e2e test in
`crates/wye/tests/e2e.rs` that runs the real service on a private bus with a faked
AppImage environment and asserts that the launched fake browser's environment is clean;
and `check_appimage_launched_env` in `packaging/smoke-test.sh`, which does the same from
the real AppImage (see Test gap).

**Not done: extending LAUNCH-07 to an app that dies right after starting.** It changes
what LAUNCH-07 promises (a launch that failed to start becomes a launch that failed
shortly after), and it needs either a wait before the launch counts as successful or a
notification after the fact. That is a spec and UX decision, and it is the maintainer's
call. The reaper warning already puts this class of failure in the log.

## Test gap

The gap is covered. `packaging/appimage/test.sh` checks what the AppImage writes into the
session, that a second start rewrites nothing, that planted foreign files are left alone,
and `--version`. Before the fix nothing in it, or in `packaging/smoke-test.sh`, launched
anything *out of* the AppImage.

`check_appimage_launched_env` in `packaging/smoke-test.sh` now does. In AppImage mode it
registers a fake browser, makes it the primary browser, runs `wye open` against the
service started from the AppImage, and has the fake browser write out its own environment
and its parent's (the service's). It fails when the service did not run from the AppImage
(no `APPDIR`, so the check would prove nothing), when the browser's environment contains
the `$APPDIR` path or any runtime marker, or when `XDG_CACHE_HOME` is not the session's.
It needs no real browser and no Flatpak in the container. Both environments land in the
logs as `launched-app-environ.txt` and `service-environ.txt`.

The fake browser reads its own environment from `/proc/$$/environ`, not
`/proc/self/environ`. A redirection opens `/proc/self` in the child that then execs the
reader, and on Fedora, whose `sh` is bash, that read came back empty: the first run passed
the path and marker checks on an empty file. The check now also fails when the recorded
environment has no `PATH`.

## Reproducing it

On any machine with a Flatpak browser and the Wye AppImage:

```sh
wye open https://example.com            # exit 0, nothing opens
journalctl --user -n 20 | grep opened   # "opened target=… pid=…"
ps aux | grep -c '[f]irefox'            # 0
```

To see the error the launch throws away, run the command `wye test` prints, with the
service's environment:

```sh
pid=$(pgrep -f 'libexec/wye service')
tr '\0' '\n' < /proc/$pid/environ > /tmp/env   # then re-exec the command under it
# → Failed to find ARG0 dir!
```

## Notes

The machine this was found on has no Nix (`/nix` is an empty leftover directory, no
`nix-daemon`), so none of the review gates in [AGENTS.md](../../AGENTS.md#review-gates)
can run there; the fix has to be built and checked elsewhere. Nor is there a user-side
workaround on SteamOS: the `.deb` and `.rpm` do not apply, the AppImage is the only
install path, and the pollution happens inside it after start, where nothing external can
undo it.

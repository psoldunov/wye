# The `frontend` option both modules share (ADV-12): which frontend shows the
# picker, the tray-menu popup and the windows, stored as `advanced.frontend`
# in config.toml.
#
# config.toml is the user's own file, so the value reaches it two ways:
# home-manager writes it into a managed file (`settings` set); otherwise the
# `wye` unit sets it in the writable file each time the service starts
# (`execStartPre`), so a choice made in Settings lasts until the next start.
# `"auto"` leaves the file alone and the choice to Settings.
{ lib, pkgs }:
cfg:
let
  # home-manager's `settings.advanced`; the NixOS module has no `settings`.
  declared = (cfg.settings or { }).advanced or { };

  # Sets advanced.frontend in place, keeping the rest of the file, its
  # comments and its mode. A store link (home-manager's managed file) already
  # carries the value; a file made read-only on purpose stays as it is, as
  # Wye's Settings leave it; a file that is not TOML is left for Wye to
  # report.
  setFrontend =
    pkgs.writers.writePython3 "wye-set-frontend"
      {
        libraries = [ pkgs.python3Packages.tomlkit ];
        flakeIgnore = [ "E501" ];
      }
      ''
        import os
        import stat
        import sys
        import tempfile

        import tomlkit

        frontend = sys.argv[1]
        home = os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config")
        path = os.path.join(home, "wye", "config.toml")

        if os.path.islink(path):
            sys.exit(0)
        if os.path.exists(path) and not os.access(path, os.W_OK):
            print(f"{path} is read-only; advanced.frontend left as it is", file=sys.stderr)
            sys.exit(0)
        try:
            with open(path, encoding="utf-8") as file:
                text = file.read()
            mode = stat.S_IMODE(os.stat(path).st_mode)
        except FileNotFoundError:
            text = ""
            umask = os.umask(0)
            os.umask(umask)
            mode = 0o666 & ~umask
        try:
            document = tomlkit.parse(text)
            advanced = document.setdefault("advanced", tomlkit.table())
            if advanced.get("frontend") == frontend:
                sys.exit(0)
            advanced["frontend"] = frontend
        except Exception as error:
            print(f"cannot set advanced.frontend in {path}: {error}", file=sys.stderr)
            sys.exit(0)
        directory = os.path.dirname(path)
        os.makedirs(directory, exist_ok=True)
        handle, temporary = tempfile.mkstemp(dir=directory, prefix=".config.toml.")
        with os.fdopen(handle, "w", encoding="utf-8") as file:
            file.write(tomlkit.dumps(document))
        os.chmod(temporary, mode)
        os.replace(temporary, path)
      '';
in
{
  option = lib.mkOption {
    type = lib.types.enum [
      "auto"
      "kde"
      "gnome"
    ];
    default = "auto";
    description = ''
      Which frontend shows the picker, the tray-menu popup and the windows
      (`advanced.frontend` in {file}`config.toml`). `"auto"` uses the GNOME
      frontend in a GNOME session and the KDE one everywhere else, and leaves
      the choice to Wye's Settings window. `"kde"` uses `wye-ui` (Qt and
      Kirigami) on every desktop. `"gnome"` uses the GNOME Shell extension
      while it runs and the GTK host `wye-gtk` otherwise, which suits window
      managers. When the chosen frontend is not installed, Wye falls back to
      the other one. Other than `"auto"`, the value is written into the
      configuration file each time the service starts, so a choice made in
      Settings lasts only until then (with home-manager's `settings`, the
      file is read-only and Settings cannot change it). Going back to
      `"auto"` writes nothing: the value last written stays until it is
      changed in Settings.
    '';
  };

  # The `wye` unit's ExecStartPre; `-`: a failure never stops the service.
  execStartPre = lib.optional (cfg.frontend != "auto") "-${setFrontend} ${cfg.frontend}";

  # Merged into home-manager's managed file, where the script cannot write.
  settings = lib.optionalAttrs (cfg.frontend != "auto") { advanced.frontend = cfg.frontend; };

  assertions = [
    {
      assertion =
        cfg.frontend == "auto"
        || !(builtins.hasAttr "frontend" declared)
        || declared.frontend == cfg.frontend;
      message = "programs.wye.frontend and programs.wye.settings.advanced.frontend disagree; set one of them.";
    }
  ];
}

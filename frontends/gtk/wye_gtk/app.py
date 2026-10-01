"""GTK 4/libadwaita windows backed by Wye's public D-Bus API."""

from __future__ import annotations

import argparse
import json
import os
import sys
import traceback
from collections.abc import Callable
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from importlib import import_module
from pathlib import Path
from typing import Any, cast
from urllib.parse import urlsplit

from .bus import GtkHost
from .fixtures import FixtureError, load as load_fixture, load_trace
from .models import expansion_disabled, filter_history, nested, patch_for, rule_patch, target_label
from .service import ServiceClient, ServiceError, parse_revision

ROOT = Path(__file__).resolve().parents[3]
# Installed assets live in share/; a source checkout keeps the icon under data/.
LOGO = ROOT / "share/icons/hicolor/scalable/apps/dev.soldunov.wye.svg"
if not LOGO.is_file():
    LOGO = ROOT / "data/icons/hicolor/scalable/apps/dev.soldunov.wye.svg"

PAGES = (
    ("general", "General", "preferences-system-symbolic"),
    ("browsers", "Browsers", "web-browser-symbolic"),
    ("apps", "Apps", "application-x-executable-symbolic"),
    ("picker", "Picker", "view-list-symbolic"),
    ("rules", "Rules", "insert-object-symbolic"),
    ("extras", "Extras", "starred-symbolic"),
    ("advanced", "Advanced", "preferences-system-symbolic"),
)


def _history_labels(entry: dict[str, object]) -> tuple[str, str]:
    """DLG-HIS-02: keep the destination path and routing context readable."""
    url = str(entry.get("finalUrl") or "")
    try:
        parts = urlsplit(url)
        title = (parts.hostname or parts.netloc) + parts.path if parts.netloc else url
        if parts.netloc:
            title += f"?{parts.query}" if parts.query else ""
            title += f"#{parts.fragment}" if parts.fragment else ""
    except ValueError:
        title = url
    source = entry.get("sourceName")
    target = entry.get("targetName")
    route = f"from {source} → {target}" if source and target else str(target or source or "")
    timestamp = entry.get("time")
    if isinstance(timestamp, (int, float)) and not isinstance(timestamp, bool):
        try:
            when = datetime.fromtimestamp(timestamp).strftime("%H:%M")
            route = f"{route} · {when}" if route else when
        except (OverflowError, OSError, ValueError):
            pass
    details = [str(entry["reason"])] if entry.get("reason") else []
    details += [label for field, label in (("cleaned", "Cleaned"), ("expanded", "Expanded")) if entry.get(field)]
    return title, "\n".join(part for part in (route, " · ".join(details)) if part)


class WyeGtk:
    def __init__(self, self_test: bool = False) -> None:
        gi = import_module("gi")
        gi.require_version("Gtk", "4.0")
        gi.require_version("Adw", "1")
        self.Gtk: Any = import_module("gi.repository.Gtk")
        self.Adw: Any = import_module("gi.repository.Adw")
        self.Gdk: Any = import_module("gi.repository.Gdk")
        self.GdkPixbuf: Any = import_module("gi.repository.GdkPixbuf")
        self.Gio: Any = import_module("gi.repository.Gio")
        self.GLib: Any = import_module("gi.repository.GLib")
        self.self_test = self_test
        self.client: ServiceClient | None = None
        self.state: dict[str, object] = {}
        self.windows: dict[str, Any] = {}
        self.settings_stack: Any = None
        self._executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="wye-service")
        self._snapshot_error: str | None = None
        self.app = self.Adw.Application(application_id="dev.soldunov.wye.Gtk")
        self.app.connect("startup", self._style)
        self.app.connect("activate", self._activate)
        self.host = GtkHost(self.show, self.app.quit)

    def _style(self, _: object) -> None:
        scheme = os.environ.get("WYE_GTK_SCHEME", "")
        if scheme in {"light", "dark"}:
            manager = self.Adw.StyleManager.get_default()
            manager.set_color_scheme(self.Adw.ColorScheme.FORCE_DARK if scheme == "dark" else self.Adw.ColorScheme.FORCE_LIGHT)
        provider = self.Gtk.CssProvider()
        provider.load_from_data("""
            .wye-hero { padding: 28px 24px 16px; }
            .wye-brand { color: @accent_color; }
            .wye-editor { padding: 14px; border-radius: 12px; }
            .wye-result { padding: 18px; border-radius: 12px; }
        """)
        self.Gtk.StyleContext.add_provider_for_display(
            self.Gdk.Display.get_default(), provider, self.Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def _logo(self, size: int = 104) -> Any:
        # Render the SVG at the screenshot/display scale so its edges stay sharp on HiDPI.
        monitor = self.Gdk.Display.get_default().get_monitors().get_item(0)
        scale = monitor.get_scale_factor() if monitor is not None else 1
        pixbuf = self.GdkPixbuf.Pixbuf.new_from_file_at_scale(str(LOGO), size * scale, size * scale, True)
        picture = self.Gtk.Picture.new_for_paintable(self.Gdk.Texture.new_for_pixbuf(pixbuf))
        picture.set_size_request(size, size)
        picture.set_alternative_text("Wye")
        return picture

    def _heading(self, text: str) -> Any:
        label = self.Gtk.Label(label=text, css_classes=["title-1"], wrap=True)
        label.set_justify(self.Gtk.Justification.CENTER)
        return label

    def run(self) -> int:
        result = self.app.run(sys.argv[:1])
        self._executor.shutdown(wait=False, cancel_futures=True)
        if self._snapshot_error:
            print(self._snapshot_error, file=sys.stderr)
            return 1
        return result

    def _activate(self, _: object) -> None:
        self.show("settings", {})

    def _load_state(self, window: str, argument: dict[str, object]) -> None:
        fixture = argument.get("fixture")
        if self.self_test:
            base = load_fixture(window)
            embedded = base.get("fixture")
            if isinstance(embedded, dict):
                base.update(embedded)
            if isinstance(fixture, dict):
                base.update(fixture)
            if window == "settings" and argument.get("page") == "rules":
                rules = nested(load_fixture("rule-editor"), "fixture", "config", "rules", default=[])
                config = base.get("config", {})
                if isinstance(config, dict):
                    base["config"] = {**config, "rules": rules}
            self.state = base
            self.client = None

    def _submit(self, operation: Callable[[], object], success: Callable[[object], None] | None = None) -> None:
        def finished(future: Any) -> bool:
            try:
                result = future.result()
                if success is not None:
                    success(result)
            except (ServiceError, FixtureError, KeyError, ValueError) as error:
                self._error_window(str(error))
            except Exception as error:
                traceback.print_exception(error)
                self._error_window("Unexpected service error; see the application log.")
            return False

        future = self._executor.submit(operation)
        future.add_done_callback(lambda done: self.GLib.idle_add(finished, done))

    def show(self, window: str, argument: dict[str, object]) -> None:
        if window == "settings" and window in self.windows and self.settings_stack is not None:
            page = str(argument.get("page", argument.get("value", "")))
            if page in {item[0] for item in PAGES}:
                self.settings_stack.set_visible_child_name(page)
            self.windows["settings"].present()
            return
        if not self.self_test:
            def load() -> object:
                if self.client is None:
                    self.client = ServiceClient()
                return self.client.state()

            self._submit(load, lambda result: self._show_loaded(window, argument, result))
            return
        try:
            self._load_state(window, argument)
            self._show_loaded(window, argument, self.state)
        except (FixtureError, ServiceError, KeyError) as error:
            self._error_window(str(error))

    def _show_loaded(self, window: str, argument: dict[str, object], state: object) -> None:
        if isinstance(state, dict) and parse_revision(state.get("revision", 0)) >= parse_revision(self.state.get("revision", 0)):
            self.state = state
        try:
            builders = {
                "settings": self._settings,
                "first-run": self._onboarding,
                "history": self._history,
                "about": self._about,
                "script-editor": self._script_editor,
                "rule-editor": self._rule_editor,
                "test-rules": self._tester,
            }
            builders[window](argument)
        except (FixtureError, ServiceError, KeyError) as error:
            self._error_window(str(error))

    def _window(self, key: str, title: str, width: int = 700, height: int = 620) -> Any:
        existing = self.windows.get(key)
        if existing is not None:
            existing.close()
        window = self.Adw.ApplicationWindow(application=self.app, title=title, default_width=width, default_height=height)
        window.connect("close-request", lambda *_: self._forget_window(key, window))
        self.windows[key] = window
        return window

    def _forget_window(self, key: str, window: object) -> None:
        if self.windows.get(key) is window:
            self.windows.pop(key, None)
            if key == "settings":
                self.settings_stack = None

    def _present(self, window: Any, content: Any) -> None:
        toolbar = self.Adw.ToolbarView()
        toolbar.add_top_bar(self.Adw.HeaderBar())
        toolbar.set_content(content)
        window.set_content(toolbar)
        window.present()

    def _error_window(self, message: str) -> None:
        window = self._window("error", "Wye GTK")
        page = self.Adw.StatusPage(title="Wye is unavailable", description=message, icon_name="dialog-error-symbolic")
        self._present(window, page)

    def _config(self) -> dict[str, object]:
        value = self.state.get("config", {})
        return value if isinstance(value, dict) else {}

    def _writable(self) -> bool:
        return bool(nested(self.state, "status", "config", "writable", default=True))

    def _config_saved(self, result: object) -> None:
        config, revision = cast(tuple[dict[str, object], int], result)
        if revision >= parse_revision(self.state.get("revision", 0)):
            self.state = {**self.state, "config": config, "revision": revision}

    def _update(self, path: tuple[str, ...], value: object) -> None:
        client = self.client
        if client is None:
            return

        def save() -> object:
            client.update(patch_for(path, value))
            return client.config()

        self._submit(save, self._config_saved)

    def _update_array(self, path: tuple[str, ...], change: Callable[[list[object]], list[object]], success: Callable[[], None] | None = None) -> None:
        client = self.client
        if client is None:
            return

        def save() -> object:
            # Arrays replace in JSON merge patches (SET-06): read on the serial worker,
            # immediately before applying the user's change, not when the GTK event fires.
            config, _ = client.config()
            current = nested(config, *path, default=[])
            values = change(list(current) if isinstance(current, list) else [])
            client.update(patch_for(path, values))
            return client.config()

        def saved(result: object) -> None:
            self._config_saved(result)
            if success is not None:
                success()

        self._submit(save, saved)

    def _row(self, kind: str, **properties: object) -> Any:
        # Adw.PreferencesRow uses Pango markup by default, including for titles from D-Bus.
        row = getattr(self.Adw, kind)(use_markup=False)
        for name, value in properties.items():
            row.set_property(name.replace("_", "-"), value)
        return row

    def _group(self, page: Any, title: str, description: str = "") -> Any:
        group = self.Adw.PreferencesGroup(title=title, description=description)
        page.add(group)
        return group

    def _switch(self, group: Any, title: str, path: tuple[str, ...], subtitle: str = "") -> Any:
        row = self._row("SwitchRow", title=title, subtitle=subtitle)
        row.set_active(bool(nested(self._config(), *path, default=False)))
        row.set_sensitive(self._writable())
        row.connect("notify::active", lambda control, *_: self._update(path, control.get_active()))
        group.add(row)
        return row

    def _button_row(self, group: Any, title: str, subtitle: str, label: str, callback: Any) -> Any:
        row = self._row("ActionRow", title=title, subtitle=subtitle)
        button = self.Gtk.Button(label=label)
        button.connect("clicked", callback)
        row.add_suffix(button)
        row.set_activatable_widget(button)
        group.add(row)
        return row

    def _choice(self, group: Any, title: str, path: tuple[str, ...], values: tuple[str, ...], subtitle: str = "") -> Any:
        row = self._row("ComboRow", title=title, subtitle=subtitle)
        row.set_model(self.Gtk.StringList.new(values))
        current = nested(self._config(), *path, default=values[0])
        current = current[0] if isinstance(current, list) and current else current
        row.set_selected(values.index(current) if current in values else 0)
        row.set_sensitive(self._writable())
        row.connect(
            "notify::selected",
            lambda control, *_: self._update(path, [values[control.get_selected()]] if path[-1].endswith("-key") else values[control.get_selected()]),
        )
        group.add(row)
        return row

    def _target_choice(self, group: Any, title: str, path: tuple[str, ...], current: object, subtitle: str = "") -> Any:
        candidates = [{"default": True}, {"picker": True}] + [item["target"] for item in nested(self.state, "targets", "targets", default=[]) if isinstance(item, dict) and isinstance(item.get("target"), dict) and not item.get("missing")]
        labels = tuple(target_label(item, self.state.get("targets", {})) for item in candidates)
        row = self._row("ComboRow", title=title, subtitle=subtitle)
        row.set_model(self.Gtk.StringList.new(labels))
        row.set_selected(next((index for index, candidate in enumerate(candidates) if candidate == current), 0))
        row.set_sensitive(self._writable())
        row.connect("notify::selected", lambda control, *_: self._update(path, candidates[control.get_selected()]))
        group.add(row)
        return row

    def _action(self, method: str) -> None:
        client = self.client
        if client is not None:
            self._submit(lambda: client.action(method))

    def _page(self, title: str, icon: str) -> Any:
        return self.Adw.PreferencesPage(title=title, icon_name=icon)

    def _settings(self, argument: dict[str, object]) -> None:
        window = self._window("settings", "General", 780, 700)
        stack = self.Adw.ViewStack()
        self.settings_stack = stack
        for name, title, icon in PAGES:
            page = self._page(title, icon)
            getattr(self, f"_build_{name}")(page)
            stack.add_titled(page, name, title)
            stack.get_page(page).set_icon_name(icon)
        switcher = self.Adw.ViewSwitcher(stack=stack)
        header = self.Adw.HeaderBar()
        brand = self.Gtk.Box(spacing=6)
        brand.append(self._logo(22))
        brand.append(self.Gtk.Label(label="Wye", css_classes=["heading"]))
        header.pack_start(brand)
        header.set_title_widget(switcher)
        toolbar = self.Adw.ToolbarView()
        toolbar.add_top_bar(header)
        toolbar.set_content(stack)
        window.set_content(toolbar)
        requested = argument.get("page", argument.get("value", nested(self.state, "status", "uiState", "lastPage", default="general")))
        page_name = str(requested)
        stack.set_visible_child_name(page_name if page_name in {item[0] for item in PAGES} else "general")
        window.set_title(stack.get_visible_child().get_title())
        stack.connect("notify::visible-child-name", lambda control, *_: window.set_title(control.get_visible_child().get_title()))
        window.present()

    def _build_general(self, page: object) -> None:
        status = nested(self.state, "status", "defaultBrowser", default={})
        is_default = bool(nested(status, "isDefault", default=False))
        group = self._group(page, "Default Browser", "Every link starts with Wye, then opens where you choose.")
        self._button_row(group, "Wye is your default browser" if is_default else "Wye is not your default browser", "", "Stop Being Default" if is_default else "Make Default", lambda *_: self._action("StopBeingDefault" if is_default else "MakeDefault"))
        self._switch(group, "Also open local HTML files", ("general", "open-local-html"))
        startup = self._group(page, "Startup")
        self._switch(startup, "Launch at login", ("general", "launch-at-login"))
        tray = self._group(page, "Tray")
        self._choice(tray, "Tray icon", ("general", "tray-icon"), ("primary-browser", "wye"))
        self._switch(tray, "Show tray icon", ("general", "show-tray-icon"))
        info = self._group(page, "Browser links")
        info.add(self._row("ActionRow", title="Links clicked inside a browser", subtitle="Install the browser extension or open a copied URL from the Wye menu."))

    def _build_browsers(self, page: object) -> None:
        targets = self.state.get("targets", {})
        group = self._group(page, "Fallback browsers")
        self._target_choice(group, "Browser", ("browsers", "primary"), nested(self._config(), "browsers", "primary", default={"picker": True}))
        alternative = target_label(nested(self._config(), "browsers", "alternative", default={}), targets)
        self._target_choice(group, "Alternative browser", ("browsers", "alternative"), nested(self._config(), "browsers", "alternative", default={"picker": True}), f"{alternative}. Hold the alternative key while opening a link.")
        self._choice(group, "Alternative browser key", ("browsers", "alternative-key"), ("Shift", "Ctrl", "Alt", "Super"))
        shown = self._group(page, "Picker and tray")
        self._button_row(shown, "Shown browsers", "Browsers shown in the picker and tray menu.", "Choose…", lambda *_: self._shown_browsers())
        profiles = [item for item in nested(targets, "targets", default=[]) if isinstance(item, dict) and item.get("kind") == "profile"]
        self._button_row(shown, "Browser profiles", f"{len(profiles)} profiles found.", "Rescan", lambda *_: self._action("Rescan"))

    def _build_apps(self, page: object) -> None:
        group = self._group(page, "Open links to web apps in their desktop app or a specific browser")
        services = nested(self.state, "services", "services", default=[])
        for service in services if isinstance(services, list) else []:
            if isinstance(service, dict):
                service_id = str(service.get("id", ""))
                current = nested(self._config(), "apps", service_id, default=service.get("target", {"default": True}))
                self._target_choice(group, str(service.get("name", service_id)), ("apps", service_id), current)

    def _build_picker(self, page: object) -> None:
        appearance = self._group(page, "Appearance")
        self._choice(appearance, "Icon size", ("picker", "icon-size"), ("small", "medium", "large"))
        self._switch(appearance, "Show browser names", ("picker", "show-names"))
        self._switch(appearance, "Show URL", ("picker", "show-url"))
        self._switch(appearance, "Show profile badge", ("picker", "show-profile-badge"))
        behavior = self._group(page, "Behaviour")
        self._switch(behavior, "Skip picker when screen is locked", ("picker", "skip-when-locked"), "Links will then open in the alternative browser.")
        keys = self._group(page, "Keys")
        self._choice(keys, "Target hotkeys", ("picker", "hotkeys"), ("per-target", "numbers", "letters", "off"))
        self._button_row(keys, "Picker keys", "View current keyboard actions", "View…", lambda *_: self._picker_keys())
        preview = self._group(page, "Preview")
        self._button_row(preview, "Preview Picker", "See the picker as it looks now. Choosing a target opens nothing.", "Preview Picker", lambda *_: self._action("PreviewPicker"))

    def _build_rules(self, page: object) -> None:
        group = self._group(page, "Rules", "Rules are evaluated from top to bottom; the first match wins.")
        rules = nested(self._config(), "rules", default=[])
        if not isinstance(rules, list) or not rules:
            group.add(self._row("ActionRow", title="No Rules", subtitle="A rule opens a specific app based on a URL or source app."))
        else:
            for rule in rules:
                if isinstance(rule, dict):
                    matchers = rule.get("url-matchers", [])
                    pattern = ", ".join(str(item.get("pattern", "")) for item in matchers if isinstance(item, dict)) if isinstance(matchers, list) else ""
                    row = self._row("ActionRow", title=str(rule.get("name", "Unnamed rule")), subtitle=pattern or "Source app rule")
                    row.add_suffix(self.Gtk.Image.new_from_icon_name("go-next-symbolic"))
                    row.set_activatable(True)
                    row.connect("activated", lambda *_args, current=rule: self._rule_editor({"rule": current}))
                    group.add(row)
        tools = self._group(page, "Manage rules")
        self._button_row(tools, "Add a routing rule", "", "Add Rule…", lambda *_: self._rule_editor({}))
        self._button_row(tools, "Try a link without opening it", "", "Test Rules…", lambda *_: self._tester({}))

    def _build_extras(self, page: object) -> None:
        group = self._group(page, "Link cleaning")
        self._switch(group, "Remove tracking parameters when opening links", ("extras", "strip-tracking-on-open"))
        self._switch(group, "Remove tracking parameters when copying links", ("extras", "strip-tracking-on-copy"))
        self._switch(group, "Remove leading “mailto:” when copying email addresses", ("extras", "strip-mailto-on-copy"))
        secure = self._group(page, "Secure links")
        self._switch(secure, "Force opened links to be HTTPS", ("extras", "force-https"))
        music = self._group(page, "Music")
        self._switch(music, "Convert copied music links to Songlink", ("extras", "songlink-on-copy"), "For easy sharing across music services.")

    def _build_advanced(self, page: object) -> None:
        expansion = self._group(page, "URL Expansion")
        self._switch(expansion, "Expand redirect and short URLs", ("advanced", "expand-urls"))
        self._button_row(expansion, "Expansion services", "", "Configure…", lambda *_: self._expansion())
        transform = self._group(page, "URL Transformation")
        self._switch(transform, "Transform all URLs before matching rules", ("advanced", "transform"), "Runs after URL expansion and tracking removal.")
        self._button_row(transform, "Transform script", "", "Edit Script…", lambda *_: self._script_editor({"scope": "global"}))
        shortcuts = self._group(page, "Keyboard Shortcuts")
        bindings = nested(self.state, "shortcuts", "bindings", default=[])
        for binding in bindings if isinstance(bindings, list) else []:
            if isinstance(binding, dict):
                self._button_row(shortcuts, str(binding.get("description", "Shortcut")), "", str(binding.get("trigger", "Change…")), lambda *_: self._action("ConfigureShortcuts"))
        history = self._group(page, "History")
        self._switch(history, "Store history of the last 100 opened links", ("advanced", "history"))
        self._button_row(history, "History", "", "Show…", lambda *_: self._history({}))
        misc = self._group(page, "Miscellaneous")
        self._switch(misc, "Force show picker when opening from browser extension", ("advanced", "force-picker-from-extension"), "Hold Alt while opening to bypass the picker.")
        self._choice(misc, "Bypass key", ("advanced", "bypass-key"), ("Shift", "Ctrl", "Alt", "Super"))

    def _shown_browsers(self) -> None:
        window = self._window("shown", "Shown Browsers", 480, 620)
        page = self.Adw.PreferencesPage()
        group = self._group(page, "Browsers shown in the picker and tray menu")
        for item in nested(self.state, "targets", "targets", default=[]):
            if isinstance(item, dict) and not item.get("missing") and item.get("kind") != "picker":
                row = self._row("ActionRow", title=str(item.get("name", "Target")))
                check = self.Gtk.CheckButton(active=any(entry.get("target") == item.get("target") for entry in nested(self._config(), "browsers", "shown", default=[]) if isinstance(entry, dict)))
                check.set_sensitive(self._writable())
                check.connect("toggled", lambda control, target=item["target"]: self._toggle_shown(target, control.get_active()))
                row.add_suffix(check)
                group.add(row)
        self._present(window, page)

    def _toggle_shown(self, target: object, enabled: bool) -> None:
        def change(shown: list[object]) -> list[object]:
            values = [entry for entry in shown if not isinstance(entry, dict) or entry.get("target") != target]
            if enabled:
                values.append({"target": target})
            return values

        self._update_array(("browsers", "shown"), change)

    def _picker_keys(self) -> None:
        window = self._window("picker-keys", "Picker Keys", 500, 620)
        page = self.Adw.PreferencesPage()
        group = self._group(page, "Keyboard actions")
        keys = nested(self._config(), "picker", "keys", default={})
        for action, binding in keys.items() if isinstance(keys, dict) else []:
            group.add(self._row("ActionRow", title=str(action).replace("-", " ").title(), subtitle=", ".join(binding) if isinstance(binding, list) else ""))
        self._present(window, page)

    def _onboarding(self, argument: dict[str, object]) -> None:
        try:
            step = min(max(int(str(argument.get("step", 0))), 0), 4)
        except (TypeError, ValueError):
            step = 0
        window = self._window("first-run", "Welcome to Wye", 600, 570)
        titles = ("Welcome to Wye", "Make Wye your default browser", "Choose your browser", "Start with your session", "Ready for every link")
        descriptions = (
            "Every link finds its place. Wye opens it in the browser, profile, or app you choose.",
            "Let Wye receive links first, then decide where each one belongs.",
            "Keep the picker for each link, or choose a browser to use by default.",
            "Have Wye ready whenever you sign in.",
            "Your links have a home. You can change any choice in Settings.",
        )
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=14, valign=self.Gtk.Align.CENTER, halign=self.Gtk.Align.CENTER)
        box.set_size_request(440, -1)
        box.add_css_class("wye-hero")
        box.append(self._logo(112))
        box.append(self._heading(titles[step]))
        description = self.Gtk.Label(label=descriptions[step], wrap=True, justify=self.Gtk.Justification.CENTER, css_classes=["dim-label"])
        description.set_max_width_chars(48)
        box.append(description)
        if step == 1:
            is_default = bool(nested(self.state, "status", "defaultBrowser", "isDefault", default=False))
            button = self.Gtk.Button(label="Already the default browser" if is_default else "Make Wye Default", halign=self.Gtk.Align.CENTER)
            button.set_sensitive(not is_default and self.client is not None)
            if button.get_sensitive():
                button.add_css_class("suggested-action")
            button.connect("clicked", lambda *_: self._action("MakeDefault"))
            box.append(button)
        elif step == 2:
            group = self.Adw.PreferencesGroup(title="Primary browser")
            self._target_choice(group, "Open links in", ("browsers", "primary"), nested(self._config(), "browsers", "primary", default={"picker": True}))
            box.append(group)
        elif step == 3:
            group = self.Adw.PreferencesGroup(title="Startup")
            self._switch(group, "Launch at login", ("general", "launch-at-login"))
            box.append(group)
        navigation = self.Gtk.Box(spacing=12, halign=self.Gtk.Align.CENTER, margin_top=20)
        if step:
            back = self.Gtk.Button(label="Back")
            back.connect("clicked", lambda *_: self._onboarding({"step": step - 1}))
            navigation.append(back)
        next_button = self.Gtk.Button(label="Open Settings" if step == 4 else "Get Started" if step == 0 else "Continue", css_classes=["suggested-action"])
        next_button.connect("clicked", lambda *_: self.show("settings", {}) if step == 4 else self._onboarding({"step": step + 1}))
        navigation.append(next_button)
        box.append(navigation)
        indicator = self.Gtk.Label(label=f"{step + 1} of 5", css_classes=["dim-label"], margin_top=12)
        box.append(indicator)
        self._present(window, box)

    def _history(self, _: dict[str, object]) -> None:
        window = self._window("history", "History", 760, 600)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=16, margin_top=24, margin_bottom=24, margin_start=30, margin_end=30)
        box.append(self.Gtk.Label(label="Recently opened", xalign=0, css_classes=["title-2"]))
        entries = nested(self.state, "history", "entries", default=[])
        count = len(entries) if isinstance(entries, list) else 0
        box.append(self.Gtk.Label(label=f"{count} links routed by Wye", xalign=0, css_classes=["dim-label"]))
        search = self.Gtk.SearchEntry(placeholder_text="Search links or apps")
        box.append(search)
        list_box = self.Gtk.ListBox(css_classes=["boxed-list"])
        list_box.set_selection_mode(self.Gtk.SelectionMode.NONE)
        content = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, valign=self.Gtk.Align.START)
        content.append(list_box)
        scroll = self.Gtk.ScrolledWindow(vexpand=True, hscrollbar_policy=self.Gtk.PolicyType.NEVER, child=content)
        targets = nested(self.state, "targets", "targets", default=[])

        def populate(query: str) -> None:
            while child := list_box.get_first_child():
                list_box.remove(child)
            matches = filter_history(entries if isinstance(entries, list) else [], query)
            for entry in matches:
                title, subtitle = _history_labels(entry)
                row = self._row("ActionRow", title=title, subtitle=subtitle, title_lines=1, subtitle_lines=2)
                row.set_tooltip_text(str(entry.get("originalUrl") or entry.get("finalUrl") or ""))
                target = entry.get("target")
                match = next((item for item in targets if isinstance(item, dict) and item.get("target") == target), None) if isinstance(targets, list) else None
                icon_name = match.get("icon") if match else None
                theme = self.Gtk.IconTheme.get_for_display(self.Gdk.Display.get_default())
                if not isinstance(icon_name, str) or not theme.has_icon(icon_name):
                    icon_name = "view-list-symbolic" if isinstance(target, dict) and target.get("picker") else "web-browser-symbolic"
                icon = self.Gtk.Image.new_from_icon_name(icon_name)
                icon.set_pixel_size(28)
                row.add_prefix(icon)
                delete = self.Gtk.Button(icon_name="user-trash-symbolic", tooltip_text=f"Delete {title} from history", css_classes=["flat"])
                delete.set_sensitive(self.client is not None)
                delete.connect("clicked", lambda *_args, item=entry: self._delete_history(item))
                row.add_suffix(delete)
                list_box.append(row)
            if not matches:
                list_box.append(self._row("ActionRow", title="No matching links" if query else "No history yet", subtitle="Try another search." if query else "Links you open will appear here."))

        search.connect("search-changed", lambda control: populate(control.get_text()))
        populate("")
        box.append(scroll)
        clear = self.Gtk.Button(label="Clear History", halign=self.Gtk.Align.END)
        clear.set_sensitive(self.client is not None and count > 0)
        clear.connect("clicked", lambda *_: self._confirm_clear_history(window))
        box.append(clear)
        self._present(window, box)

    def _delete_history(self, entry: dict[str, object]) -> None:
        if self.client is None:
            return
        try:
            entry_id = int(str(entry.get("id", 0)))
        except (TypeError, ValueError):
            self._error_window("History entry has an invalid identifier")
            return
        client = self.client
        self._submit(lambda: client._call("DeleteHistoryEntry", "(t)", (entry_id,)), lambda _: self.show("history", {}))

    def _confirm_clear_history(self, window: Any) -> None:
        if self.client is None:
            return
        dialog = self.Adw.AlertDialog(heading="Clear history?", body="All saved links will be removed. This cannot be undone.")
        dialog.add_response("cancel", "Cancel")
        dialog.add_response("clear", "Clear History")
        dialog.set_response_appearance("clear", self.Adw.ResponseAppearance.DESTRUCTIVE)
        dialog.set_default_response("cancel")
        dialog.set_close_response("cancel")
        dialog.connect("response", lambda _, response: self._clear_history() if response == "clear" else None)
        dialog.present(window)

    def _clear_history(self) -> None:
        client = self.client
        if client is not None:
            self._submit(lambda: client.action("ClearHistory"), lambda _: self.show("history", {}))

    def _about(self, _: dict[str, object]) -> None:
        window = self._window("about", "About Wye", 620, 620)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=24, margin_top=28, margin_bottom=28, margin_start=42, margin_end=42)
        hero = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=12, halign=self.Gtk.Align.CENTER)
        hero.append(self._logo(96))
        hero.append(self._heading("Wye"))
        version = self.state.get("version", nested(self.state, "fixture", "version", default=""))
        hero.append(self.Gtk.Label(label=f"Version {version}" if version else "A native Linux browser picker", css_classes=["dim-label"]))
        hero.append(self.Gtk.Label(label="Every link, right where it belongs.", css_classes=["wye-brand"]))
        box.append(hero)
        group = self.Adw.PreferencesGroup(title="About", description="Made by Philipp Soldunov")
        group.add(self._row("ActionRow", title="License", subtitle="MIT License"))
        link = self._row("ActionRow", title="Source code", subtitle="github.com/psoldunov/wye")
        open_source = self.Gtk.LinkButton(uri="https://github.com/psoldunov/wye", label="Open")
        link.add_suffix(open_source)
        group.add(link)
        box.append(group)
        diagnostics = self.state.get("troubleshooting", nested(self.state, "fixture", "troubleshooting", default=""))
        if diagnostics:
            details = self.Gtk.Expander(label="Troubleshooting details")
            text = self.Gtk.Label(label=str(diagnostics), selectable=True, wrap=True, xalign=0, css_classes=["caption", "dim-label"])
            text.set_margin_top(12)
            details.set_child(text)
            box.append(details)
        scroll = self.Gtk.ScrolledWindow(child=box, hscrollbar_policy=self.Gtk.PolicyType.NEVER)
        self._present(window, scroll)

    def _script_editor(self, argument: dict[str, object]) -> None:
        scope = str(argument.get("scope", argument.get("value", "global")))
        window = self._window("script-editor", "Transform Script", 820, 650)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=12, margin_top=20, margin_bottom=20, margin_start=24, margin_end=24)
        box.append(self.Gtk.Label(label="Transform links before routing", xalign=0, css_classes=["title-2"]))
        box.append(self.Gtk.Label(label="Return a new URL, or return nothing to keep the original.", xalign=0, css_classes=["dim-label"]))
        source = str(nested(self.state, "fixture", "source", default=self.state.get("source", "")))
        if self.client is not None:
            source = "Loading script…"
        buffer = self.Gtk.TextBuffer(text=source)
        editor = self.Gtk.TextView(buffer=buffer, monospace=True, wrap_mode=self.Gtk.WrapMode.NONE, css_classes=["wye-editor"])
        scroll = self.Gtk.ScrolledWindow(vexpand=True, child=editor, css_classes=["card"])
        box.append(scroll)
        box.append(self.Gtk.Label(label="Test with a link", xalign=0, css_classes=["heading"]))
        test_url = self.Gtk.Entry(text=str(self.state.get("testUrl", "https://example.com")), placeholder_text="Test URL")
        box.append(test_url)
        result = self.Gtk.Label(xalign=0, wrap=True, selectable=True, css_classes=["dim-label"])
        sample = nested(self.state, "run", "url") if self.self_test else None
        result.set_text(f"Fixture test result: {sample}" if sample else "Enter a link and test the script before saving.")
        buttons = self.Gtk.Box(spacing=8, halign=self.Gtk.Align.END)
        run = self.Gtk.Button(label="Test")
        run.connect("clicked", lambda *_: self._run_script(buffer, test_url.get_text(), result))
        save = self.Gtk.Button(label="Save", css_classes=["suggested-action"])
        save.connect("clicked", lambda *_: self._save_script(scope, buffer))
        client = self.client
        if client is not None:
            run.set_sensitive(False)
            save.set_sensitive(False)
            def loaded(text: object) -> None:
                buffer.set_text(str(text))
                run.set_sensitive(True)
                save.set_sensitive(self._writable())
            self._submit(lambda: client.script(scope), loaded)
        buttons.append(run)
        buttons.append(save)
        box.append(result)
        box.append(buttons)
        self._present(window, box)

    def _script_text(self, buffer: Any) -> str:
        return buffer.get_text(buffer.get_start_iter(), buffer.get_end_iter(), False)

    def _run_script(self, buffer: Any, url: str, output: Any) -> None:
        client = self.client
        if client is None:
            run = nested(self.state, "fixture", "run", default=self.state.get("run", {}))
            output.set_text(json.dumps(run, indent=2))
            return
        source = self._script_text(buffer)
        self._submit(lambda: client.run_script(source, url), lambda result: output.set_text(json.dumps(result, indent=2)))

    def _save_script(self, scope: str, buffer: Any) -> None:
        client = self.client
        if client is None:
            return
        source = self._script_text(buffer)
        self._submit(lambda: client.set_script(scope, source))

    def _rule_editor(self, argument: dict[str, object]) -> None:
        rule = argument.get("rule", {})
        existing = rule if isinstance(rule, dict) else {}
        if self.self_test and argument.get("preview"):
            rules = nested(self._config(), "rules", default=[])
            existing = rules[1] if isinstance(rules, list) and len(rules) > 1 and isinstance(rules[1], dict) else {}
        window = self._window("rule-editor", "Edit Rule" if existing else "New Rule", 650, 740)
        page = self.Adw.PreferencesPage()
        group = self._group(page, "Rule")
        name = self._row("EntryRow", title="Name", text=str(existing.get("name", "")))
        group.add(name)
        candidates = [{"default": True}, {"picker": True}] + [item["target"] for item in nested(self.state, "targets", "targets", default=[]) if isinstance(item, dict) and isinstance(item.get("target"), dict) and not item.get("missing")]
        targets = self._row("ComboRow", title="Open in")
        targets.set_model(self.Gtk.StringList.new([target_label(candidate, self.state.get("targets", {})) for candidate in candidates]))
        targets.set_selected(candidates.index(existing["target"]) if existing.get("target") in candidates else 0)
        group.add(targets)
        original_matchers = existing.get("url-matchers", [])
        original_sources = existing.get("source-apps", [])
        matcher = original_matchers[0] if isinstance(original_matchers, list) and original_matchers else {}
        matcher = matcher if isinstance(matcher, dict) else {}
        kinds = ("domain", "prefix")
        editable = (isinstance(original_matchers, list) and len(original_matchers) <= 1
                    and isinstance(original_sources, list) and len(original_sources) <= 1
                    and matcher.get("kind", "domain") in kinds)
        matchers = self._group(page, "URL Matcher", "Match a domain or a URL prefix.")
        kind = self._row("ComboRow", title="Pattern type")
        kind.set_model(self.Gtk.StringList.new(kinds))
        kind.set_selected(kinds.index(matcher.get("kind")) if matcher.get("kind") in kinds else 0)
        matchers.add(kind)
        pattern = self._row("EntryRow", title="Pattern", text=str(matcher.get("pattern", argument.get("domain", ""))))
        matchers.add(pattern)
        source_group = self._group(page, "Source App", "Optional desktop entry ID.")
        source_value = original_sources[0] if editable and original_sources else argument.get("sourceApp", "")
        source = self._row("EntryRow", title="Desktop entry ID", text=str(source_value or ""))
        source_group.add(source)
        for control in (name, targets, kind, pattern, source):
            control.set_sensitive(editable and self._writable())
        save_group = self._group(page, "")
        save_row = self._button_row(save_group, "Save this rule", "A name and at least one condition are required." if editable else "This rule has unsupported conditions; edit it in Wye's other frontend.", "Save", lambda *_: self._save_rule(existing, name.get_text(), candidates[targets.get_selected()], kinds[kind.get_selected()], pattern.get_text(), source.get_text(), window))
        save_row.set_sensitive(editable and self._writable())
        self._present(window, page)

    def _save_rule(self, existing: dict[str, object], name: str, target: dict[str, object], kind: str, pattern: str, source: str, window: Any) -> None:
        client = self.client
        if client is None:
            return
        try:
            rule = rule_patch(existing, name, target, kind, pattern, source)
        except ValueError as error:
            self._error_window(str(error))
            return
        def change(rules: list[object]) -> list[object]:
            if not existing:
                return [*rules, rule]
            values = list(rules)
            index = next((i for i, item in enumerate(values) if isinstance(item, dict) and (item.get("id") == existing["id"] if existing.get("id") else item == existing)), None)
            if index is None or values[index] != existing:
                raise ServiceError("Rule changed or was removed; reopen it before saving.")
            values[index] = rule
            return values

        self._update_array(("rules",), change, window.close)

    def _tester(self, _: dict[str, object]) -> None:
        window = self._window("test-rules", "Test Rules", 650, 460)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=16, margin_top=28, margin_bottom=28, margin_start=32, margin_end=32)
        box.append(self.Gtk.Label(label="Where will this link open?", xalign=0, css_classes=["title-2"]))
        box.append(self.Gtk.Label(label="Run a link through your rules without opening it.", xalign=0, css_classes=["dim-label"]))
        trace = load_trace() if self.self_test else {}
        url = self.Gtk.Entry(text=str(trace.get("finalUrl", "https://github.com/example/repo")), placeholder_text="Paste a link to test")
        output = self.Gtk.Label(xalign=0, wrap=True, selectable=True, css_classes=["wye-result", "dim-label"])
        steps = trace.get("steps", [])
        summary = "\n".join(str(step.get("text", "")) for step in steps if isinstance(step, dict)) if isinstance(steps, list) else ""
        output.set_text(f"Destination: {trace.get('targetName', '')}\n\n{summary}" if trace else "The matching rule and destination appear here.")
        test = self.Gtk.Button(label="Test Link", css_classes=["suggested-action"], halign=self.Gtk.Align.START)
        test.connect("clicked", lambda *_: self._test_link(url.get_text(), output))
        box.append(url)
        box.append(test)
        box.append(self.Gtk.Separator(orientation=self.Gtk.Orientation.HORIZONTAL))
        box.append(output)
        self._present(window, box)

    def _test_link(self, url: str, output: Any) -> None:
        client = self.client
        if client is None:
            output.set_text("Fixture mode: service test is unavailable.")
            return
        self._submit(lambda: client.test_link(url), lambda result: output.set_text(json.dumps(result, indent=2)))

    def _expansion(self) -> None:
        window = self._window("expansion", "URL Expansion", 560, 600)
        page = self.Adw.PreferencesPage()
        group = self._group(page, "Redirect wrappers")
        for item in nested(self.state, "expansion", "wrappers", default=[]):
            if isinstance(item, dict) and isinstance(item.get("id"), str):
                self._expansion_row(group, str(item.get("name", item["id"])), item["id"], bool(item.get("enabled")))
        links = self._group(page, "Short link services")
        for item in nested(self.state, "expansion", "shortLinks", default=[]):
            if isinstance(item, dict) and isinstance(item.get("domain"), str):
                self._expansion_row(links, item["domain"], item["domain"], bool(item.get("enabled")))
        self._present(window, page)

    def _expansion_row(self, group: Any, title: str, service_id: str, enabled: bool) -> None:
        row = self._row("SwitchRow", title=title, active=enabled)
        row.set_sensitive(self._writable() and self.client is not None)
        row.connect("notify::active", lambda control, *_: self._set_expansion(service_id, control.get_active()))
        group.add(row)

    def _set_expansion(self, service_id: str, enabled: bool) -> None:
        def change(values: list[object]) -> list[object]:
            if not all(isinstance(value, str) for value in values):
                raise ServiceError("Expansion settings contain an invalid service identifier.")
            return list(expansion_disabled(cast(list[str], values), service_id, enabled))

        self._update_array(("advanced", "expansion", "disabled"), change)

    def snapshot_all(self, directory: Path) -> None:
        from .snapshots import snapshot_all

        snapshot_all(self, directory, PAGES)


def main() -> int:
    parser = argparse.ArgumentParser(description="Wye's native GTK settings host")
    parser.add_argument("--self-test", action="store_true", help="render existing JSON fixtures without a Wye service")
    parser.add_argument("--snapshots", type=Path, help="directory for self-test PNG snapshots")
    parser.add_argument("--scheme", choices=("light", "dark"), help="force the snapshot color scheme")
    args = parser.parse_args()
    if args.scheme:
        os.environ["WYE_GTK_SCHEME"] = args.scheme
    if args.snapshots and not args.self_test:
        parser.error("--snapshots requires --self-test")
    app = WyeGtk(self_test=args.self_test)
    if args.self_test and args.snapshots:
        app.app.connect("activate", lambda *_: app.snapshot_all(args.snapshots))
    return app.run()


if __name__ == "__main__":
    raise SystemExit(main())

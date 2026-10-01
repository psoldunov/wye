"""GTK 4/libadwaita windows backed by Wye's public D-Bus API."""

from __future__ import annotations

import argparse
import json
import sys
import traceback
from collections.abc import Callable, Sequence
from concurrent.futures import ThreadPoolExecutor
from importlib import import_module
from pathlib import Path
from typing import Any
from uuid import uuid4

from .bus import GtkHost
from .fixtures import FixtureError, load as load_fixture
from .service import ServiceClient, ServiceError

PAGES = (
    ("general", "General", "preferences-system-symbolic"),
    ("browsers", "Browsers", "web-browser-symbolic"),
    ("apps", "Apps", "application-x-executable-symbolic"),
    ("picker", "Picker", "view-list-symbolic"),
    ("rules", "Rules", "share-symbolic"),
    ("extras", "Extras", "starred-symbolic"),
    ("advanced", "Advanced", "preferences-system-symbolic"),
)


def nested(data: object, *keys: str, default: Any = None) -> Any:
    value = data
    for key in keys:
        if not isinstance(value, dict):
            return default
        value = value.get(key, default)
    return value


def patch_for(path: tuple[str, ...], value: object) -> dict[str, object]:
    result: dict[str, object] = {path[-1]: value}
    for key in reversed(path[:-1]):
        result = {key: result}
    return result


def target_label(target: object, targets: object) -> str:
    if isinstance(target, dict):
        for item in nested(targets, "targets", default=[]):
            if isinstance(item, dict) and item.get("target") == target:
                return str(item.get("name", "Unknown target"))
        if target.get("picker"):
            return "Picker"
        if target.get("default"):
            return "Default"
    return "Default"


def filter_history(entries: Sequence[object], query: str) -> list[dict[str, object]]:
    needle = query.casefold().strip()
    return [item for item in entries if isinstance(item, dict) and any(needle in str(item.get(field, "")).casefold() for field in ("finalUrl", "originalUrl", "sourceName", "targetName"))]


def rule_patch(existing: dict[str, object], name: str, target: dict[str, object], kind: str, pattern: str, source: str) -> dict[str, object]:
    if not name.strip() or not (pattern.strip() or source.strip()):
        raise ValueError("Give the rule a name and a URL pattern or source app.")
    rule = dict(existing)
    rule.update({"id": str(existing.get("id") or uuid4()), "name": name.strip(), "target": target,
                 "url-matchers": [{"kind": kind, "pattern": pattern.strip()}] if pattern.strip() else [],
                 "source-apps": [source.strip()] if source.strip() else []})
    return rule


def expansion_disabled(disabled: list[str], service_id: str, enabled: bool) -> list[str]:
    values = [item for item in disabled if item.casefold() != service_id.casefold()]
    return values if enabled else [*values, service_id]


class WyeGtk:
    def __init__(self, self_test: bool = False) -> None:
        gi = import_module("gi")
        gi.require_version("Gtk", "4.0")
        gi.require_version("Adw", "1")
        self.Gtk: Any = import_module("gi.repository.Gtk")
        self.Adw: Any = import_module("gi.repository.Adw")
        self.Gio: Any = import_module("gi.repository.Gio")
        self.GLib: Any = import_module("gi.repository.GLib")
        self.self_test = self_test
        self.client: ServiceClient | None = None
        self.state: dict[str, object] = {}
        self.windows: dict[str, Any] = {}
        self.settings_stack: Any = None
        self._expansion_disabled: list[str] = []
        self._executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix="wye-service")
        self._snapshot_error: str | None = None
        self.app = self.Adw.Application(application_id="dev.soldunov.wye.Gtk")
        self.app.connect("activate", self._activate)
        self.host = GtkHost(self.show, self.app.quit)

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
        if isinstance(state, dict):
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

    def _update(self, path: tuple[str, ...], value: object) -> None:
        client = self.client
        if client is None:
            return
        self._submit(lambda: client.update(patch_for(path, value)))

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
        window = self._window("settings", "General", 680, 920)
        stack = self.Adw.ViewStack()
        self.settings_stack = stack
        for name, title, icon in PAGES:
            page = self._page(title, icon)
            getattr(self, f"_build_{name}")(page)
            stack.add_titled(page, name, title)
            stack.get_page(page).set_icon_name(icon)
        switcher = self.Adw.ViewSwitcher(stack=stack)
        header = self.Adw.HeaderBar()
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
        group = self._group(page, "Default Browser")
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
                    row = self._row("ActionRow", title=str(rule.get("name", "Unnamed rule")), subtitle="Click to edit")
                    row.set_activatable(True)
                    row.connect("activated", lambda *_args, current=rule: self._rule_editor({"rule": current}))
                    group.add(row)
        tools = self._group(page, "")
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
            if isinstance(item, dict) and not item.get("missing"):
                row = self._row("ActionRow", title=str(item.get("name", "Target")))
                check = self.Gtk.CheckButton(active=any(entry.get("target") == item.get("target") for entry in nested(self._config(), "browsers", "shown", default=[]) if isinstance(entry, dict)))
                check.set_sensitive(self._writable())
                check.connect("toggled", lambda control, target=item["target"]: self._toggle_shown(target, control.get_active()))
                row.add_suffix(check)
                group.add(row)
        self._present(window, page)

    def _toggle_shown(self, target: object, enabled: bool) -> None:
        shown = nested(self._config(), "browsers", "shown", default=[])
        values = [entry for entry in shown if isinstance(entry, dict) and entry.get("target") != target] if isinstance(shown, list) else []
        if enabled:
            values.append({"target": target})
        self._update(("browsers", "shown"), values)

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
            step = int(str(argument.get("step", 0)))
        except (TypeError, ValueError):
            step = 0
        window = self._window("first-run", "Welcome to Wye", 600, 500)
        page = self.Adw.StatusPage(icon_name="web-browser-symbolic")
        titles = ("Welcome to Wye", "Make Wye your default browser", "Choose your primary browser", "All set")
        descriptions = ("Wye sends each link to the right browser or app.", "Set Wye as your default browser to route links.", "You can use the picker or select a browser.", "Settings stay available from the Wye menu.")
        page.set_title(titles[min(max(step, 0), 3)])
        page.set_description(descriptions[min(max(step, 0), 3)])
        button = self.Gtk.Button(label="Make Default" if step == 1 else "Open Settings")
        button.connect("clicked", lambda *_: self._action("MakeDefault") if step == 1 else self.show("settings", {}))
        page.set_child(button)
        self._present(window, page)

    def _history(self, _: dict[str, object]) -> None:
        window = self._window("history", "History", 760, 640)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=12, margin_top=18, margin_bottom=18, margin_start=18, margin_end=18)
        search = self.Gtk.SearchEntry(placeholder_text="Search history")
        box.append(search)
        list_box = self.Gtk.ListBox(css_classes=["boxed-list"])
        entries = nested(self.state, "history", "entries", default=[])
        def populate(query: str) -> None:
            while child := list_box.get_first_child():
                list_box.remove(child)
            for entry in filter_history(entries if isinstance(entries, list) else [], query):
                row = self._row("ActionRow", title=str(entry.get("finalUrl", "")), subtitle=f"{entry.get('sourceName', 'Unknown source')} → {entry.get('targetName', 'Unknown target')}")
                delete = self.Gtk.Button(icon_name="user-trash-symbolic", tooltip_text="Delete history entry")
                delete.connect("clicked", lambda *_args, item=entry: self._delete_history(item))
                row.add_suffix(delete)
                list_box.append(row)
        search.connect("search-changed", lambda control: populate(control.get_text()))
        populate("")
        box.append(list_box)
        clear = self.Gtk.Button(label="Clear History")
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
        window = self._window("about", "About Wye", 620, 540)
        page = self.Adw.StatusPage(title="Wye", description="A native Linux browser picker.", icon_name="web-browser-symbolic")
        diagnostics = self.state.get("troubleshooting", nested(self.state, "fixture", "troubleshooting", default=""))
        if diagnostics:
            page.set_child(self.Gtk.Label(label=str(diagnostics), selectable=True, wrap=True, xalign=0))
        self._present(window, page)

    def _script_editor(self, argument: dict[str, object]) -> None:
        scope = str(argument.get("scope", argument.get("value", "global")))
        window = self._window("script-editor", "Transform Script", 820, 680)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=12, margin_top=18, margin_bottom=18, margin_start=18, margin_end=18)
        source = str(nested(self.state, "fixture", "source", default=self.state.get("source", "")))
        if self.client is not None:
            source = "Loading script…"
        buffer = self.Gtk.TextBuffer(text=source)
        editor = self.Gtk.TextView(buffer=buffer, monospace=True, wrap_mode=self.Gtk.WrapMode.NONE)
        scroll = self.Gtk.ScrolledWindow(vexpand=True, child=editor)
        box.append(scroll)
        test_url = self.Gtk.Entry(text=str(self.state.get("testUrl", "https://example.com")), placeholder_text="Test URL")
        box.append(test_url)
        result = self.Gtk.Label(xalign=0, wrap=True)
        buttons = self.Gtk.Box(spacing=6, halign=self.Gtk.Align.END)
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
        window = self._window("rule-editor", "Edit Rule" if existing else "New Rule", 700, 650)
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
        rules = nested(self._config(), "rules", default=[])
        values = list(rules) if isinstance(rules, list) else []
        if existing in values:
            values[values.index(existing)] = rule
        else:
            values.append(rule)
        self._submit(lambda: client.update({"rules": values}), lambda _: window.close())

    def _tester(self, _: dict[str, object]) -> None:
        window = self._window("test-rules", "Test Rules", 700, 480)
        box = self.Gtk.Box(orientation=self.Gtk.Orientation.VERTICAL, spacing=12, margin_top=18, margin_bottom=18, margin_start=18, margin_end=18)
        url = self.Gtk.Entry(text="https://example.com", placeholder_text="Link to test")
        output = self.Gtk.Label(xalign=0, wrap=True, selectable=True)
        test = self.Gtk.Button(label="Test Rules", css_classes=["suggested-action"])
        test.connect("clicked", lambda *_: self._test_link(url.get_text(), output))
        box.append(url)
        box.append(test)
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
        disabled = nested(self._config(), "advanced", "expansion", "disabled", default=[])
        self._expansion_disabled = list(disabled) if isinstance(disabled, list) else []
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
        self._expansion_disabled = expansion_disabled(self._expansion_disabled, service_id, enabled)
        self._update(("advanced", "expansion", "disabled"), self._expansion_disabled)

    def snapshot_all(self, directory: Path) -> None:
        self.app.hold()  # Keep the application alive between closing each captured window.
        try:
            directory.mkdir(parents=True, exist_ok=True)
        except OSError as error:
            self._snapshot_error = f"Cannot create screenshot directory: {error}"
            self.app.quit()
            return
        work: list[tuple[str, str, dict[str, object]]] = [(f"settings-{name}", "settings", {"page": name}) for name, _, _ in PAGES]
        work += [("onboarding", "first-run", {}), ("history", "history", {}), ("about", "about", {}), ("script-editor", "script-editor", {}), ("rule-editor", "rule-editor", {"preview": True}), ("test-rules", "test-rules", {})]
        index = 0

        def save_current(name: str, window_name: str) -> bool:
            nonlocal index
            try:
                window = self.windows.get(window_name)
                if window is None:
                    raise RuntimeError(f"Could not create {window_name}")
                paintable = self.Gtk.WidgetPaintable.new(window)
                snapshot = self.Gtk.Snapshot.new()
                paintable.snapshot(snapshot, float(window.get_width()), float(window.get_height()))
                node = snapshot.to_node()
                if node is None:
                    raise RuntimeError(f"Could not render {name}")
                if not window.get_renderer().render_texture(node, None).save_to_png(str(directory / f"{name}.png")):
                    raise RuntimeError(f"Could not save {name}")
                window.close()
                index += 1
                self.GLib.timeout_add(100, capture_next)
            except (OSError, RuntimeError, FixtureError, KeyError) as error:
                self._snapshot_error = str(error)
                self.app.quit()
            return False

        def capture_next() -> bool:
            if index >= len(work):
                self.app.quit()
                return False
            name, window_name, argument = work[index]
            try:
                self.show(window_name, argument)
                self.GLib.timeout_add(250, save_current, name, window_name)
            except (OSError, RuntimeError, FixtureError, KeyError) as error:
                self._snapshot_error = str(error)
                self.app.quit()
            return False

        self.GLib.timeout_add(100, capture_next)


def main() -> int:
    parser = argparse.ArgumentParser(description="Wye's native GTK settings host")
    parser.add_argument("--self-test", action="store_true", help="render existing JSON fixtures without a Wye service")
    parser.add_argument("--snapshots", type=Path, help="directory for self-test PNG snapshots")
    args = parser.parse_args()
    if args.snapshots and not args.self_test:
        parser.error("--snapshots requires --self-test")
    app = WyeGtk(self_test=args.self_test)
    if args.self_test and args.snapshots:
        app.app.connect("activate", lambda *_: app.snapshot_all(args.snapshots))
    return app.run()


if __name__ == "__main__":
    raise SystemExit(main())

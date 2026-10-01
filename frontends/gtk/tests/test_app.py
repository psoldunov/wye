import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import Mock

from wye_gtk.app import WyeGtk, _history_labels
from wye_gtk.fixtures import load as load_fixture, load_trace
from wye_gtk.models import expansion_disabled, filter_history, nested, rule_patch
from wye_gtk.service import ServiceClient, ServiceError


class BehaviorTest(unittest.TestCase):
    # DLG-HIS-01: search filters displayed history without changing saved entries.
    def test_history_search_matches_url_and_source_case_insensitively(self):
        entries = [{"finalUrl": "https://example.org/?a=1&b=2", "sourceName": "Slack"}, {"finalUrl": "https://other.test", "sourceName": "Mail"}]
        self.assertEqual(filter_history(entries, "EXAMPLE"), entries[:1])
        self.assertEqual(filter_history(entries, "slack"), entries[:1])

    # RUL-14: a saved matcher needs a pattern or another condition.
    def test_rule_patch_requires_a_condition_and_preserves_other_fields(self):
        existing = {"id": "old", "run": "before", "url-matchers": [{"kind": "prefix", "pattern": "https://old/"}]}
        self.assertEqual(rule_patch(existing, "New", {"app": "firefox.desktop"}, "domain", "example.org", ""), {"id": "old", "run": "before", "name": "New", "target": {"app": "firefox.desktop"}, "url-matchers": [{"kind": "domain", "pattern": "example.org"}], "source-apps": []})
        with self.assertRaises(ValueError):
            rule_patch({}, "Name", {"default": True}, "domain", "", "")

    # DLG-EXP-01/02: one service toggle must preserve the other disabled IDs.
    def test_expansion_toggle_keeps_other_disabled_services(self):
        self.assertEqual(expansion_disabled(["Google", "bit.ly"], "google", True), ["bit.ly"])
        self.assertEqual(expansion_disabled(["Google"], "t.co", False), ["Google", "t.co"])

    # DLG-HIS-01/SET-01: opening an existing settings window raises it.
    def test_show_reuses_window_without_reloading_state(self):
        app = object.__new__(WyeGtk)
        window = Mock()
        stack = Mock()
        app.windows = {"settings": window}
        app.settings_stack = stack
        app._load_state = Mock()
        app._settings = Mock()
        app.show("settings", {"page": "rules"})
        stack.set_visible_child_name.assert_called_once_with("rules")
        window.present.assert_called_once()
        app._load_state.assert_not_called()
        app._settings.assert_not_called()

    def test_snapshot_directory_failure_sets_nonzero_exit(self):
        app = object.__new__(WyeGtk)
        app.app = Mock()
        app.app.run.return_value = 0
        app._executor = Mock()
        app._snapshot_error = None
        app.snapshot_all(Path("/dev/null/shots"))
        self.assertEqual(app.run(), 1)
        app.app.quit.assert_called_once()

    def test_rows_disable_markup_before_setting_untrusted_text(self):
        app = object.__new__(WyeGtk)
        app.Adw = Mock()
        row = app._row("ActionRow", title="https://example.test/?a=1&b=2")
        app.Adw.ActionRow.assert_called_once_with(use_markup=False)
        row.set_property.assert_called_once_with("title", "https://example.test/?a=1&b=2")

    # DLG-HIS-02: link paths and actual routing context are visible, without markup.
    def test_history_labels_include_path_route_reason_and_time(self):
        title, detail = _history_labels({
            "finalUrl": "https://example.org/reports/2026?filter=all&sort=new",
            "sourceName": "Mail <team>", "targetName": "Firefox",
            "reason": "rule <Work>", "time": 1789050720, "cleaned": True,
        })
        self.assertEqual(title, "example.org/reports/2026?filter=all&sort=new")
        self.assertIn("Mail <team> → Firefox", detail)
        self.assertIn("rule <Work>", detail)
        self.assertIn("Cleaned", detail)
        self.assertRegex(detail, r"\b\d\d:\d\d\b")
        self.assertEqual(_history_labels({"finalUrl": "https://example.net/a"}), ("example.net/a", ""))

    def test_history_preview_adds_local_demo_entries_only(self):
        entries = load_fixture("history")["fixture"]["history"]["entries"]
        self.assertGreaterEqual(len(entries), 7)
        self.assertEqual(entries[-1]["id"], 2)
        self.assertTrue(all("finalUrl" in item and "reason" in item for item in entries))

    def test_preview_trace_uses_the_existing_tester_fixture(self):
        trace = load_trace()
        self.assertEqual(trace["targetName"], "Work (Google Chrome)")
        self.assertTrue(trace["steps"])

    def test_config_read_refreshes_revision_before_array_update(self):
        client = object.__new__(ServiceClient)
        client.revision = 1
        client._call = Mock(side_effect=[('{"rules": []}', 5), (6,)])
        self.assertEqual(client.config(), ({"rules": []}, 5))
        self.assertEqual(client.update({"rules": [{"id": "new"}]}), 6)
        self.assertEqual(client._call.call_args.args[2][1], 5)

    def test_service_read_failure_is_not_hidden(self):
        client = object.__new__(ServiceClient)
        client._call = Mock(return_value=('{}', 1))
        client._json = Mock(side_effect=ServiceError("GetTargets unavailable"))
        with self.assertRaisesRegex(ServiceError, "GetTargets unavailable"):
            client.state()

    def test_consecutive_shown_toggles_keep_both_targets(self):
        app, client, flush = self._array_app({"browsers": {"shown": []}})
        first, second = {"app": "first.desktop"}, {"app": "second.desktop"}
        app._toggle_shown(first, True)
        app._toggle_shown(second, True)
        flush()
        self.assertEqual(client.saved["browsers"]["shown"], [{"target": first}, {"target": second}])

    def test_consecutive_new_rules_keep_both_rules(self):
        app, client, flush = self._array_app({"rules": []})
        first, second = Mock(), Mock()
        app._save_rule({}, "First", {"default": True}, "domain", "first.test", "", first)
        app._save_rule({}, "Second", {"default": True}, "domain", "second.test", "", second)
        flush(reverse_callbacks=True)
        self.assertEqual([rule["name"] for rule in client.saved["rules"]], ["First", "Second"])
        self.assertEqual([rule["name"] for rule in nested(app.state, "config", "rules")], ["First", "Second"])
        first.close.assert_called_once()
        second.close.assert_called_once()

    def test_external_array_change_before_queued_toggle_is_preserved(self):
        app, client, flush = self._array_app({"browsers": {"shown": []}})
        app._toggle_shown({"app": "mine.desktop"}, True)
        client.saved["browsers"]["shown"].append({"target": {"app": "external.desktop"}})
        client.revision += 1
        flush()
        self.assertEqual([item["target"]["app"] for item in client.saved["browsers"]["shown"]], ["external.desktop", "mine.desktop"])

    def test_stale_rule_edit_does_not_replace_external_changes(self):
        original = {"id": "first", "name": "Old", "target": {"default": True}, "url-matchers": [{"kind": "domain", "pattern": "old.test"}]}
        app, client, flush = self._array_app({"rules": [original]})
        app._save_rule(original, "Edited", {"default": True}, "domain", "new.test", "", Mock())
        client.saved["rules"][0]["name"] = "External edit"
        client.revision += 1
        with self.assertRaisesRegex(ServiceError, "Rule changed"):
            flush()
        self.assertEqual(client.saved["rules"][0]["name"], "External edit")

    def test_consecutive_expansion_toggles_keep_both_disabled_services(self):
        app, client, flush = self._array_app({"advanced": {"expansion": {"disabled": []}}})
        app._set_expansion("one", False)
        app._set_expansion("two", False)
        flush()
        self.assertEqual(client.saved["advanced"]["expansion"]["disabled"], ["one", "two"])

    @staticmethod
    def _array_app(config):
        class Client:
            def __init__(self):
                self.saved = deepcopy(config)
                self.revision = 1

            def config(self):
                return deepcopy(self.saved), self.revision

            def update(self, patch):
                for key, value in patch.items():
                    if isinstance(value, dict) and isinstance(self.saved.get(key), dict):
                        self.saved[key].update(deepcopy(value))
                    else:
                        self.saved[key] = deepcopy(value)
                self.revision += 1
                return self.revision

        client = Client()
        app = object.__new__(WyeGtk)
        app.state = {"config": deepcopy(config), "revision": 1}
        app.client = Mock(spec=ServiceClient, config=client.config, update=client.update)
        pending = []
        app._submit = lambda operation, success=None: pending.append((operation, success))

        def flush(reverse_callbacks=False):
            completed = [(operation(), success) for operation, success in pending]
            for result, success in reversed(completed) if reverse_callbacks else completed:
                if success is not None:
                    success(result)

        return app, client, flush


if __name__ == "__main__":
    unittest.main()

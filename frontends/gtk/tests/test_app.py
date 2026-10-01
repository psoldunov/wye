import unittest
from pathlib import Path
from unittest.mock import Mock

from wye_gtk.app import WyeGtk, expansion_disabled, filter_history, rule_patch
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

    def test_service_read_failure_is_not_hidden(self):
        client = object.__new__(ServiceClient)
        client._call = Mock(return_value=('{}', 1))
        client._json = Mock(side_effect=ServiceError("GetTargets unavailable"))
        with self.assertRaisesRegex(ServiceError, "GetTargets unavailable"):
            client.state()


if __name__ == "__main__":
    unittest.main()

import unittest

from wye_gtk.dispatch import dispatch_window


class DispatchWindowTest(unittest.TestCase):
    def test_settings_argument_is_normalized_and_dispatched(self):
        seen = []

        dispatch_window("settings", '{"page":"advanced"}', lambda key, value: seen.append((key, value)))

        self.assertEqual(seen, [("settings", {"page": "advanced"})])


if __name__ == "__main__":
    unittest.main()

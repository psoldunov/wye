# Privacy

Wye and its browser extension collect no data. Nothing is sent to the author or to anyone
else, and there is no telemetry, analytics or crash reporting. This page covers the Wye
browser extension (for Firefox and for Chromium-family browsers) and the Wye app it talks
to.

## The browser extension

The extension does one thing: when you choose **Open Link with Wye** or **Open Page with
Wye** (context menu, toolbar button or keyboard shortcut), it passes that one address to
Wye on the same computer.

- **What it handles.** The address of the link or page you chose, the modifier keys you
  held while choosing it (Shift, Ctrl, Alt) when the browser reports them, and whether it
  was a link or a page. It reads nothing else: no page content, no browsing history, no
  cookies, no form data, no other tabs.
- **Where it goes.** Only to Wye's helper program `wye-native-host`, which the browser
  starts on your computer through native messaging. The helper hands the address to the
  Wye app over the local session bus (D-Bus). The extension makes no network requests.
- **What it stores.** One setting, "Close the tab after sending the page to Wye", in the
  browser's local extension storage. Removing the extension removes it.
- **Permissions.** `activeTab` (read the address of the current tab when you send the
  page), `contextMenus` (the two menu entries), `nativeMessaging` (reach the helper) and
  `storage` (the one setting).

## The Wye app

Wye decides which browser opens an address and starts that browser. It runs on your
computer and keeps its data there:

- Its settings are in `~/.config/wye/`, its state in `~/.local/state/wye/`.
- **History** of the last 100 opened links is off by default. When you turn it on, it is
  kept on your computer only, and you can delete entries or all of it at any time.
- **Link expansion** (on by default) shows rules the real address behind a link.
  Redirect wrappers, such as a search engine's result links, are unwrapped on your
  computer. For a short link from one of the services listed in its settings, such as
  `bit.ly`, Wye asks that service's own server where the link leads, as your browser would
  when you open it. Turn it off on the Advanced page of Wye's settings.
- **Tracking-parameter removal** happens on your computer.

## Contact

Questions about privacy: open an issue at <https://github.com/psoldunov/wye/issues>.

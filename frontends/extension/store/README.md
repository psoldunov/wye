# Store listings

What the Wye extension's listings on addons.mozilla.org (AMO) and the Chrome Web Store
(CWS) say, and how a version is submitted. The images are in [`images/`](images/), with
notes on how they were made.

| | Firefox (AMO) | Chromium family (CWS) |
|---|---|---|
| Listing | <https://addons.mozilla.org/en-US/firefox/addon/wye/> | <https://chromewebstore.google.com/detail/wye/jdcifhpoallkdjnbflfienpboodjfjei> |
| ID | `wye@soldunov.dev` (`browser_specific_settings.gecko.id`) | `jdcifhpoallkdjnbflfienpboodjfjei` (fixed by `key` in `manifest.chromium.json`) |
| Upload | `wye-extension-firefox-<version>.zip` | `wye-extension-chromium-webstore-<version>.zip` (the manifest without `key`, which the store refuses) |
| Images | `firefox-*.png` | `chrome-*.png`, `promo-small-440x280.png`, `promo-marquee-1400x560.png` |

Both zips are attached to every GitHub release (built by Nix from the tag, listed in
`SHA256SUMS`), and `nix build .#extension` puts them in `result/share/wye/extension/`.
Wye's native-messaging host allows both IDs (`crates/wye-desktop/src/native_messaging.rs`,
BEXT-04); the store ID and the `key` must not change.

## Shared text

**Name:** Wye

**Summary** (AMO, 250 characters at most; CWS takes the manifest's `description`):

> Send links and pages to Wye, the Linux browser picker. Wye opens each one in the
> browser, profile or app your rules choose, or asks with a small picker.

**Description:**

```text
Wye is a browser picker for Linux. It becomes your default browser and sends every link to the browser, profile, private window or app you want, or asks you with a small picker when no rule decides.

This extension brings Wye into the browser, for the links you meet there:

• Right-click a link and choose "Open Link with Wye".
• Right-click a page, click the toolbar button or press Alt+Shift+W to send the page you are on.

Wye then does what your rules say: work links to your work profile, a meeting link to the desktop app, everything else to the picker. You can also close the tab after sending its page (extension options).

Requires Wye on Linux. The extension talks to Wye's helper on your computer and does nothing on its own. Get Wye from https://github.com/psoldunov/wye: packages for Debian testing and Fedora, an AppImage for any other distribution, and a Nix flake. Wye installs the helper for every browser it finds; restart the browser once after installing Wye. Without the helper, the toolbar button explains what is missing.

Privacy: the extension reads only the address you choose to send and passes it to Wye on the same computer. Nothing leaves your computer and nothing is collected.

Free and open source (MIT): https://github.com/psoldunov/wye
```

**Homepage:** <https://github.com/psoldunov/wye>
**Support:** <https://github.com/psoldunov/wye/issues>
**Privacy policy:** <https://github.com/psoldunov/wye/blob/master/docs/privacy.md>
**Licence:** MIT

## Chrome Web Store

**Category:** Tools. **Language:** English.

**Single purpose:**

> Send a link, or the page you are on, from the browser to Wye, the browser picker
> installed on the same Linux computer, which opens it in the browser, profile or app the
> user's rules choose.

**Permission justifications:**

| Permission | Justification |
|---|---|
| `activeTab` | Reads the address of the current tab, only when the user clicks the toolbar button, chooses "Open Page with Wye" or presses the shortcut, so that page can be sent to Wye. |
| `contextMenus` | Adds "Open Link with Wye" to the context menu of links and "Open Page with Wye" to the context menu of pages. |
| `nativeMessaging` | Passes the chosen address to Wye's helper program (`wye-native-host`) on the user's computer, which hands it to the Wye app. This is the extension's only output; it makes no network requests. |
| `storage` | Keeps one option: "Close the tab after sending the page to Wye". |

**Remote code:** No, I am not using remote code.

**Data usage:** tick no data type: the extension sends nothing off the computer and
nothing to the developer. Tick the three certifications (no sale to third parties, no use
unrelated to the single purpose, no use for creditworthiness or lending).

**Test instructions** (Test instructions tab):

```text
The extension is a companion to Wye, a browser picker for Linux. On other systems, or without Wye, the toolbar button only opens a popup explaining that Wye's helper is missing: that is the expected behaviour.

To test on Linux x86_64 (no account or login needed):
1. Download Wye-<version>-x86_64.AppImage from https://github.com/psoldunov/wye/releases/latest
2. chmod +x Wye-<version>-x86_64.AppImage, then run: ./Wye-<version>-x86_64.AppImage settings
   This starts Wye, opens its settings window and installs the native-messaging helper for every browser it finds.
3. Restart Chrome, then install the extension.
4. Right-click any link and choose "Open Link with Wye": Wye's picker opens and lists the installed browsers; choose one to open the link. The toolbar button and Alt+Shift+W send the current page the same way.
```

## addons.mozilla.org

**Distribution:** On this site (listed). **Platform:** Firefox for desktop only (the
extension relies on native messaging, which Firefox for Android lacks).
**Categories:** Tabs. **Source code:** not needed: the files are not minified, bundled or
generated. **Data collection:** none, declared in the manifest
(`data_collection_permissions`).

**Notes to reviewer:** the test instructions above, with "Firefox" for "Chrome". Add:

```text
The add-on ID wye@soldunov.dev is the one Wye's native-messaging manifest allows (allowed_extensions). The helper speaks the documented native-messaging protocol; its source is crates/wye-native-host in the repository.
```

## Submitting a version

The release workflow submits every new version. Once the GitHub release is published,
[`stores.yml`](../../../.github/workflows/stores.yml) downloads the release's two zips,
checks them against `SHA256SUMS` and runs [`submit.sh`](submit.sh) for each store:

- AMO gets `wye-extension-firefox-<version>.zip` as a new listed version, with release
  notes that link the GitHub release and the test instructions as notes to the reviewer.
- CWS gets `wye-extension-chromium-webstore-<version>.zip`, followed by a publish request.

Both stores review the version and publish it when the review passes. A store build never
goes out before the Wye release, because that release's native-messaging host is what allows
the store ID. A version a store already has is skipped, so re-running is safe. To submit a
released version by hand, run `gh workflow run stores.yml -f version=X.Y.Z`, or run
`submit.sh amo|chrome ZIP VERSION` locally with the variables below. A store whose secrets
are not set is skipped with a notice in the run.

### One-time setup

AMO, signed in as the add-on's developer:

1. Open <https://addons.mozilla.org/developers/addon/api/key/> and generate new
   credentials.
2. Run `gh secret set AMO_JWT_ISSUER` and paste the JWT issuer (`user:…`).
3. Run `gh secret set AMO_JWT_SECRET` and paste the JWT secret.

CWS, with a service account:

1. In the [Google Cloud console](https://console.cloud.google.com/), pick or create a
   project and enable the Chrome Web Store API.
2. Create a service account in that project. It needs no roles. Add a JSON key to it and
   download the key file.
3. In the [Developer Dashboard](https://chrome.google.com/webstore/devconsole), add the
   service account's email under Account. A publisher can have one service account.
4. Run `gh secret set CWS_SERVICE_ACCOUNT_KEY < key.json`, then delete the key file.
5. Run `gh secret set CWS_PUBLISHER_ID` and paste the publisher ID from Publisher › Settings
   in the dashboard.

The first CWS upload was special: the zip also carried the private key, as `key.pem` at its
root, so that the store keeps the ID the `key` fixes. That key lives outside the
repository and is never committed. Later uploads do not carry it.

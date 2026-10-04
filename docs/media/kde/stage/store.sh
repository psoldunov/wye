#!/usr/bin/env bash
# The browser extension's store screenshots (frontends/extension/store/images/)
# from a running light stage (stage.sh up light): Chromium and Firefox with the
# extension loaded, on a made-up article, sending links and the page to Wye.
#
#   store.sh [chrome|firefox]...
#
# WYE_STORE_OUT moves the images elsewhere. The browsers come from the
# repository's nixpkgs. They reach the web through store/serve.py, which
# answers https://journal.example.org/ alone with a certificate from a demo CA
# they trust, and refuses every other host. They start without the session's
# LD_LIBRARY_PATH and GTK_PATH, whose libraries may need a newer glibc, and
# Chromium takes its own CHROME_DESKTOP (its Wayland app ID), not one an
# Electron app may have left in the environment.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib.sh
. "$(dirname "${BASH_SOURCE[0]}")/lib.sh"
out=${WYE_STORE_OUT:-$repo/frontends/extension/store/images}
work=$stage/store
home=$stage/home
host=journal.example.org
url=https://$host/rust-on-the-desktop
proxy_port=18443
extension_id=lphepmclmllmbbkjkdhjbdgbjfpmmdnn

# tool ATTR: the store path of a nixpkgs package from the repository's flake,
# kept from garbage collection by a link in $stage/tools.
tool() {
    nix build --impure --print-out-paths --out-link "$stage/tools/${1//./-}" \
        --expr "(builtins.getFlake \"git+file://$repo\").inputs.nixpkgs.legacyPackages.\${builtins.currentSystem}.$1"
}

setup() {
    [[ -d $work ]] && return 0
    mkdir -p "$work"
    chromium=$(tool chromium)
    firefox=$(tool firefox)
    certutil=$(tool nss.tools)/bin/certutil
    openssl=$(tool openssl.bin)/bin/openssl
    printf '%s\n' "$chromium" "$firefox" >"$work/browsers"

    # Desktop entries and icons in the stage home, so Wye names the browser
    # that sent the link (and the panel shows its icon).
    mkdir -p "$home/.local/share/applications" "$home/.local/share/icons"
    ln -sf "$chromium/share/applications/chromium-browser.desktop" "$firefox/share/applications/firefox.desktop" \
        "$home/.local/share/applications/"
    cp -rs --no-preserve=mode "$chromium/share/icons/hicolor" "$firefox/share/icons/hicolor" "$home/.local/share/icons/" 2>/dev/null || true
    # The service did not watch the applications directory, which did not
    # exist when it started: start it again, so it reads them.
    local pid
    for pid in $(pgrep -f "wye service"); do
        tr '\0' '\n' 2>/dev/null <"/proc/$pid/environ" | grep -qxF "WYE_STAGE_MARK=$stage" && kill "$pid"
    done
    sleep 1
    run wye service --activate
    sleep 3

    # A demo CA and the article's certificate.
    "$openssl" req -x509 -newkey rsa:2048 -nodes -days 30 -subj "/CN=Wye demo stage CA" \
        -keyout "$work/ca.key" -out "$work/ca.pem" 2>/dev/null
    "$openssl" req -newkey rsa:2048 -nodes -subj "/CN=$host" \
        -keyout "$work/site.key" -out "$work/site.csr" 2>/dev/null
    printf 'subjectAltName=DNS:%s\nbasicConstraints=CA:FALSE\n' "$host" >"$work/site.ext"
    "$openssl" x509 -req -in "$work/site.csr" -CA "$work/ca.pem" -CAkey "$work/ca.key" -CAcreateserial \
        -days 30 -extfile "$work/site.ext" -out "$work/site.pem" 2>/dev/null
    # trust NSSDB: create it and trust the demo CA for websites.
    trust() {
        mkdir -p "$1"
        "$certutil" -N -d "sql:$1" --empty-password
        "$certutil" -A -d "sql:$1" -t C,, -n "Wye demo stage CA" -i "$work/ca.pem"
    }
    trust "$home/.pki/nssdb"
    trust "$work/firefox"

    run bash -c 'setsid -f "$1" "$2" "$3" "$4" "$5" "$6" "$7" >"$8" 2>&1 </dev/null' _ \
        "$py" "$here/store/serve.py" "$proxy_port" "$host" "$work/site.pem" "$work/site.key" \
        "$here/store/page.html" "$stage/log/serve.log"

    # The extension, as each family loads it unpacked.
    "$repo/frontends/extension/build.sh" chromium "$work/extension-chromium"
    "$repo/frontends/extension/build.sh" firefox "$work/extension-firefox"

    # Chromium: a fresh profile with Wye's button pinned to the toolbar, and no
    # sign-in or translation offers. The service writes the host manifest once
    # the browser's directory exists.
    mkdir -p "$home/.config/chromium/Default"
    printf '{"extensions":{"pinned_extensions":["%s"]},"browser":{"has_seen_welcome_page":true},"signin":{"allowed":false},"translate":{"enabled":false}}\n' \
        "$extension_id" >"$home/.config/chromium/Default/Preferences"
    # Firefox: no first-run pages, the proxy, and GTK's file chooser for Load
    # Temporary Add-on (the stage has no portal to show one).
    cat >"$work/firefox/user.js" <<EOF
user_pref("browser.shell.checkDefaultBrowser", false);
user_pref("browser.aboutwelcome.enabled", false);
user_pref("browser.preonboarding.enabled", false);
user_pref("termsofuse.bypassNotification", true);
user_pref("browser.startup.homepage_override.mstone", "ignore");
user_pref("startup.homepage_welcome_url", "");
user_pref("datareporting.policy.dataSubmissionPolicyBypassNotification", true);
user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);
user_pref("browser.translations.automaticallyPopup", false);
user_pref("browser.tabs.warnOnClose", false);
user_pref("sidebar.revamp", false);
user_pref("network.proxy.type", 1);
user_pref("network.proxy.http", "127.0.0.1");
user_pref("network.proxy.http_port", $proxy_port);
user_pref("network.proxy.ssl", "127.0.0.1");
user_pref("network.proxy.ssl_port", $proxy_port);
user_pref("network.proxy.allow_hijacking_localhost", true);
user_pref("widget.use-xdg-desktop-portal.file-picker", 0);
EOF
    for manifest in "$home/.config/chromium/NativeMessagingHosts" "$home/.mozilla/native-messaging-hosts"; do
        for _ in $(seq 50); do
            [[ -s $manifest/dev.soldunov.wye.json ]] && break
            sleep 0.2
        done
        [[ -s $manifest/dev.soldunov.wye.json ]] || {
            echo "store.sh: Wye wrote no host manifest in $manifest" >&2
            exit 1
        }
    done
}

# save NAME: the screen, halved to 1280×800, flattened to 8-bit RGB (store
# listings take no alpha), optimised without changing that colour type.
save() {
    shot screen "$work/$1-full.png"
    magick "$work/$1-full.png" -filter Lanczos -resize 1280x800 -background white -alpha remove \
        -alpha off -define png:color-type=2 "$out/$1.png"
    "$stage/tools/oxipng/bin/oxipng" -q -o max --strip safe --nx "$out/$1.png"
    echo "$out/$1.png"
}

maximize() {
    each_window "$1" 'w.setMaximize(true, true);'
    activate "$1"
}

# running PATTERN: whether a stage process's command line matches PATTERN.
running() {
    local pid
    for pid in $(pgrep -f "$1" || true); do
        tr '\0' '\n' 2>/dev/null <"/proc/$pid/environ" | grep -qxF "WYE_STAGE_MARK=$stage" && return 0
    done
    return 1
}

start_chromium() {
    # The scope's name escapes the dash in the desktop ID, as systemd does: Wye
    # would read "app-chromium-browser-N" as the launcher "chromium".
    "$here/stage.sh" app 'chromium\x2dbrowser' env -u LD_LIBRARY_PATH -u GTK_PATH \
        CHROME_DESKTOP=chromium-browser.desktop "$(sed -n 1p "$work/browsers")/bin/chromium" \
        --user-data-dir="$home/.config/chromium" --ozone-platform=wayland \
        --no-first-run --no-default-browser-check --password-store=basic \
        --proxy-server="127.0.0.1:$proxy_port" --load-extension="$work/extension-chromium" "$url"
    sleep 8
}

chrome() {
    # Open the page once beforehand, as in a browser that has been used
    # before: a new profile offers sign-in and translation once, whatever
    # its preferences say.
    start_chromium
    close_window " - Chromium"
    for _ in $(seq 50); do
        running "chromium/chromium" || break
        sleep 0.2
    done
    start_chromium
    maximize " - Chromium"
    drive move:640,400 sleep:500 move:1100,300 sleep:1500

    # 1: the link's context menu, its Wye entry under the pointer.
    drive glide:"${chrome_link[0]},${chrome_link[1]}",600 sleep:300 rclick sleep:900 \
        glide:"${chrome_entry[0]},${chrome_entry[1]}",400 sleep:700
    save chrome-1-context-menu
    # 2: the picker, opened by that entry.
    drive move:"$((chrome_entry[0] + 2)),${chrome_entry[1]}" sleep:200 click sleep:2500 \
        glide:"${chrome_work_tile[0]},${chrome_work_tile[1]}",400 sleep:2000
    save chrome-2-picker
    drive key:esc sleep:800
    # 3: the page's context menu with Open Page with Wye, and the pinned button.
    drive glide:"${chrome_blank[0]},${chrome_blank[1]}",600 sleep:300 rclick sleep:900 \
        glide:"${chrome_page_entry[0]},${chrome_page_entry[1]}",400 sleep:700
    save chrome-3-toolbar
    drive key:esc sleep:500
    close_window " - Chromium"
}

firefox() {
    local firefox
    firefox=$(sed -n 2p "$work/browsers")
    # Without the last run's session, Firefox does not offer to restore it.
    rm -f "$work/firefox/sessionstore.jsonlz4"
    [[ ! -d $work/firefox/sessionstore-backups ]] || rm -r "$work/firefox/sessionstore-backups"
    "$here/stage.sh" app firefox env -u LD_LIBRARY_PATH -u GTK_PATH -u CHROME_DESKTOP "$firefox/bin/firefox" \
        --profile "$work/firefox" --no-remote "about:debugging#/runtime/this-firefox"
    sleep 8
    maximize "— Mozilla Firefox"
    # Load Temporary Add-on, as the extension's README says: a remote-control
    # protocol would stripe the address bar.
    drive glide:"${firefox_load[0]},${firefox_load[1]}",400 sleep:200 click sleep:2500 \
        down:ctrl key:l up:ctrl sleep:500 "type:$work/extension-firefox/manifest.json" sleep:500 key:enter sleep:2500 \
        down:ctrl key:l up:ctrl sleep:300 "type:$url" key:enter sleep:3000 move:1100,300 sleep:1000

    # 1: a link's context menu, its Wye entry under the pointer. The link is
    # in the page's header: Firefox's menu is tall, and from further down the
    # picker would open over the panel.
    drive glide:"${firefox_link[0]},${firefox_link[1]}",600 sleep:300 rclick sleep:900 \
        glide:"${firefox_entry[0]},${firefox_entry[1]}",400 sleep:700
    save firefox-1-context-menu
    # 2: the picker, opened by that entry.
    drive move:"$((firefox_entry[0] + 2)),${firefox_entry[1]}" sleep:200 click sleep:2500 \
        glide:"${firefox_work_tile[0]},${firefox_work_tile[1]}",400 sleep:2000
    save firefox-2-picker
    drive key:esc sleep:800
    close_window "— Mozilla Firefox"
}

# Where to point, in logical pixels on the maximised browser: the link, the
# extension's entry in its context menu, and the picker's Work tile, hovered so
# its tooltip gives the profile's whole name (the picker opens at the pointer,
# moved in from the screen's edge); for Chromium a blank spot in the page and
# the entry in the page's menu; for Firefox the Load Temporary Add-on button.
chrome_link=(420 365)
chrome_entry=(530 620)
chrome_work_tile=(395 610)
chrome_blank=(1080 170)
chrome_page_entry=(990 587)
firefox_load=(870 299)
firefox_link=(975 115)
firefox_entry=(1074 551)
firefox_work_tile=(801 540)

setup
mkdir -p "$out"
for step in "${@:-chrome firefox}"; do
    for name in $step; do
        "$name"
    done
done

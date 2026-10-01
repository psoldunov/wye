#!/usr/bin/env bash
# Reproducible GNOME Shell 48 screenshots. Requires OrbStack Docker image wye-gnome:42.
set -euo pipefail
cd "$(dirname "$0")/../.."
name="wye-gnome-capture-$$"
out="${1:-.context/gnome-shots}"
mkdir -p "$out"
cleanup() {
    docker logs --tail 12 "$name" 2>/dev/null || true
    docker rm -f "$name" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --rm --privileged --cgroupns=host --tmpfs /run --tmpfs /tmp \
    --name "$name" wye-gnome:42 /usr/lib/systemd/systemd --system >/dev/null
for _ in {1..30}; do
    if docker exec "$name" systemctl is-active --quiet systemd-logind; then break; fi
    sleep 1
done
docker exec "$name" systemctl is-active --quiet systemd-logind
# GDM cannot own a real video seat in Docker; nested Mutter renders into Xvfb.
docker exec "$name" systemctl stop gdm
docker exec "$name" mkdir -p /run/user/0 /workspace/tests/gnome /workspace/crates/wye-ui/fixtures \
    /workspace/shots /root/.local/share/gnome-shell/extensions/wye@dev.soldunov \
    /usr/share/icons/hicolor/symbolic/apps
docker exec "$name" chmod 700 /run/user/0
docker cp frontends/gnome-shell/. "$name":/root/.local/share/gnome-shell/extensions/wye@dev.soldunov/
docker cp tests/gnome/. "$name":/workspace/tests/gnome/
docker cp crates/wye-ui/fixtures/picker.json "$name":/workspace/crates/wye-ui/fixtures/
docker cp crates/wye-ui/fixtures/tray-menu.json "$name":/workspace/crates/wye-ui/fixtures/
docker cp data/icons/hicolor/symbolic/apps/dev.soldunov.wye-symbolic.svg \
    "$name":/usr/share/icons/hicolor/symbolic/apps/
docker cp data/icons/hicolor/symbolic/apps/dev.soldunov.wye-picker-symbolic.svg \
    "$name":/usr/share/icons/hicolor/symbolic/apps/
docker exec "$name" gtk-update-icon-cache -q -f /usr/share/icons/hicolor
docker exec -d "$name" sh -lc 'exec Xvfb :99 -screen 0 1440x900x24 -nolisten tcp >/workspace/xvfb.log 2>&1'
sleep 1
docker exec -d "$name" sh -lc '
    export DISPLAY=:99 XDG_RUNTIME_DIR=/run/user/0 XDG_SESSION_TYPE=wayland
    export LIBGL_ALWAYS_SOFTWARE=1 MUTTER_DEBUG_DUMMY_MODE_SPECS=1440x900
    dbus-run-session -- sh -c '\''
        echo "$DBUS_SESSION_BUS_ADDRESS" >/workspace/bus-address
        gsettings set org.gnome.shell enabled-extensions "[\"wye@dev.soldunov\"]"
        python3 /workspace/tests/gnome/fixture_service.py >/workspace/fixture.log 2>&1 &
        exec gnome-shell --nested --wayland --unsafe-mode >/workspace/shell.log 2>&1
    '\''
'
for _ in {1..30}; do
    if docker exec "$name" sh -lc 'test -f /workspace/bus-address && DBUS_SESSION_BUS_ADDRESS=$(cat /workspace/bus-address) gnome-extensions info wye@dev.soldunov 2>/dev/null | grep -q "State: ACTIVE"'; then break; fi
    sleep 1
done
docker exec "$name" sh -lc 'DBUS_SESSION_BUS_ADDRESS=$(cat /workspace/bus-address) gnome-extensions info wye@dev.soldunov | grep "State: ACTIVE"'
docker exec "$name" sh -lc 'DBUS_SESSION_BUS_ADDRESS=$(cat /workspace/bus-address) python3 /workspace/tests/gnome/capture.py /workspace/shots'
for image in shell-picker shell-picker-overflow shell-tray; do
    docker cp "$name:/workspace/shots/$image.png" "$out/$image.png"
done
printf 'Real GNOME Shell screenshots: %s/{shell-picker,shell-picker-overflow,shell-tray}.png\n' "$out"
docker exec "$name" sh -lc 'grep -E "RegisterTray|UnregisterTray|PickerChose|ClipboardHasUrl" /workspace/fixture.log | tail -8; grep "GNOME Shell started" /workspace/shell.log | tail -1'

#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Light and dark screenshots of the popup, for UI PRs. Runs a headless GNOME
# Shell, the real daemon and the app's screenshot example, isolated like
# tools/ext-smoke/run.sh: private D-Bus session, throwaway XDG directories,
# no keyring, no window on your screen. The History holds synthetic Items.
#
#   just app-screenshot [OUT_DIR]     (default: _build/screenshots)
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
out=$(mkdir -p "${1:-$root/_build/screenshots}" && cd "${1:-$root/_build/screenshots}" && pwd)
work=$(mktemp -d)
runtime=$(mktemp -d "${TMPDIR:-/tmp}/cv.XXXXXX")
chmod 700 "$runtime"
trap 'rm -rf "$work" "$runtime"' EXIT
mkdir -p "$work/home" "$work/config" "$work/data" "$work/cache" "$work/state"
cp "$root/tools/isolated-bus.conf" "$work/bus.conf"

cat > "$work/steps.sh" <<STEPS
#!/bin/sh
set -u
gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x900 \
    --wayland-display copyveil-shot > "$work/shell.log" 2>&1 &
shell=\$!
"$root/target/debug/copyveil-daemon" > "$work/daemon.log" 2>&1 &
daemon=\$!
sleep 8
O=io.github.joaoferrete.Copyveil.Daemon
P=/io/github/joaoferrete/Copyveil/Daemon
capture() { busctl --user call \$O \$P io.github.joaoferrete.Copyveil.Capture1 Capture sayayb text/plain "\$(printf %s "\$1" | wc -c)" \$(printf %s "\$1" | od -An -tu1) 0 "\$2" > /dev/null; }
capture 'docker compose up -d' false
capture 'https://github.com/joaoferrete/copyveil/pull/19' false
capture 'ghp_SyntheticTokenForScreenshots0123456' false
capture 'cargo test --workspace --locked' false
capture 'correct-horse-battery-staple' true
busctl --user call \$O \$P io.github.joaoferrete.Copyveil.History1 Pin t 4 > /dev/null
for view in popup settings onboarding; do
    for theme in light dark; do
        WAYLAND_DISPLAY=copyveil-shot "$root/target/debug/examples/screenshot" "$out/\$view-\$theme.png" \$theme \$view
    done
done
kill \$daemon \$shell 2>/dev/null; wait 2>/dev/null
STEPS
chmod +x "$work/steps.sh"

env -u WAYLAND_DISPLAY -u DISPLAY -u GNOME_KEYRING_CONTROL -u DBUS_SESSION_BUS_ADDRESS \
    HOME="$work/home" XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" \
    XDG_CACHE_HOME="$work/cache" XDG_STATE_HOME="$work/state" XDG_RUNTIME_DIR="$runtime" \
    GSETTINGS_BACKEND=keyfile XDG_DATA_DIRS="$root/data:/usr/local/share:/usr/share" \
    dbus-run-session --config-file="$work/bus.conf" -- "$work/steps.sh"
ls "$out"/*.png

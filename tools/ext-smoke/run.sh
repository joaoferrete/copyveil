#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Smoke test of the GNOME extension against the real daemon, in a headless
# GNOME Shell. Everything is isolated: a private D-Bus session with no
# activatable services (so no keyring is touched), throwaway HOME and XDG
# directories, and a GSettings keyfile backend. Nothing in your session or
# your keyring is read or changed, and no window opens.
#
#   just ext-smoke      (builds the daemon and the extension zip first)
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
# Wayland socket paths are limited to 108 bytes, so the runtime dir is short.
runtime=$(mktemp -d "${TMPDIR:-/tmp}/cv.XXXXXX")
chmod 700 "$runtime"
trap 'rm -rf "$work" "$runtime"' EXIT

uuid=copyveil@joaoferrete.github.io
ext="$work/data/gnome-shell/extensions/$uuid"
mkdir -p "$ext" "$work/home" "$work/config/glib-2.0/settings" "$work/cache" "$work/state"
(cd "$ext" && unzip -q "$root/_build/$uuid.shell-extension.zip" && glib-compile-schemas schemas)
printf "[org/gnome/shell]\nenabled-extensions=['%s']\ndisable-user-extensions=false\n" "$uuid" \
    > "$work/config/glib-2.0/settings/keyfile"
cat > "$work/bus.conf" <<'CONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF

cat > "$work/steps.sh" <<STEPS
#!/bin/sh
set -u
gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x800 \
    --wayland-display copyveil-smoke > "$work/shell.log" 2>&1 &
shell=\$!
"$root/target/debug/copyveil-daemon" > "$work/daemon.log" 2>&1 &
daemon=\$!
sleep 10

O=io.github.joaoferrete.Copyveil.Daemon
P=/io/github/joaoferrete/Copyveil/Daemon
H=io.github.joaoferrete.Copyveil.History1
S="gsettings --schemadir $ext/schemas"
clip() { WAYLAND_DISPLAY=copyveil-smoke python3 "$root/tools/ext-smoke/clip.py" "\$@" 2>/dev/null | grep CLIP || true; }
list() { busctl --user call \$O \$P \$H List; }
failed=0
check() { if echo "\$2" | grep -q -- "\$3"; then echo "ok   \$1"; else echo "FAIL \$1: \$2"; failed=1; fi; }

clip copy COPYVEIL-MARKER-plain
clip secret COPYVEIL-MARKER-secret
check "a copy with the password-manager hint is a Secret" "\$(list)" '"secret" "From password manager"'
check "a Secret is listed without a preview" "\$(list)" '22 "" 1'
check "a plain copy is Plain with a preview" "\$(list)" '"plain" "" 0 [0-9]* 0 21 "COPYVEIL-MARKER-plain"'

sleep 31
check "the Secret is gone after its Expiry" "\$(list)" '^a(tssssuttts) 1 '
check "the clipboard is cleared after the Secret's Expiry" "\$(clip read)" 'CLIP:\[\]'

busctl --user call \$O \$P \$H Paste t 1
sleep 1
check "Paste puts the Item back on the clipboard" "\$(clip read)" 'CLIP:\[COPYVEIL-MARKER-plain\]'
check "pasting doesn't capture the Item again" "\$(list)" '^a(tssssuttts) 1 '

\$S set org.gnome.shell.extensions.copyveil take-super-v true; sleep 1
check "agreeing takes only Super+V from notifications" "\$(gsettings get org.gnome.shell.keybindings toggle-message-tray)" "\['<Super>m'\]"
gnome-extensions disable $uuid; sleep 1
check "disabling gives Super+V back" "\$(gsettings get org.gnome.shell.keybindings toggle-message-tray)" "\['<Super>v', '<Super>m'\]"
gnome-extensions enable $uuid; sleep 1
\$S set org.gnome.shell.extensions.copyveil take-super-v false; sleep 1
check "declining gives Super+V back" "\$(gsettings get org.gnome.shell.keybindings toggle-message-tray)" "\['<Super>v', '<Super>m'\]"

if grep -q "JS ERROR\|Extension $uuid:" "$work/shell.log"; then
    echo "FAIL the shell logged errors from the extension:"; grep -A3 "JS ERROR\|Extension $uuid:" "$work/shell.log"; failed=1
else
    echo "ok   no extension errors in the shell log"
fi
kill \$daemon \$shell 2>/dev/null; wait 2>/dev/null
exit \$failed
STEPS
chmod +x "$work/steps.sh"

env -u WAYLAND_DISPLAY -u DISPLAY -u GNOME_KEYRING_CONTROL -u DBUS_SESSION_BUS_ADDRESS \
    HOME="$work/home" XDG_CONFIG_HOME="$work/config" XDG_DATA_HOME="$work/data" \
    XDG_CACHE_HOME="$work/cache" XDG_STATE_HOME="$work/state" XDG_RUNTIME_DIR="$runtime" \
    GSETTINGS_BACKEND=keyfile \
    dbus-run-session --config-file="$work/bus.conf" -- "$work/steps.sh"

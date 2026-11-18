#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the GNOME Shell extension zip into _build/, as extensions.gnome.org
# takes it and as `gnome-extensions install` wants it. Used by `just ext-pack`
# and ./install.sh. Staged from copies, so the zip holds real files and no
# dev tooling, plus the compiled settings schema GNOME Shell loads.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
stage="$root/_build/extension"
zip="$root/_build/copyveil@joaoferrete.github.io.shell-extension.zip"

rm -rf "$stage"
mkdir -p "$stage/icons"
(cd "$root/gnome-extension" && cp -r metadata.json extension.js clipboard.js daemon.js \
    indicator.js keyboard.js lib schemas "$stage/")
# The panel icon in its three states (normal, capture paused, Secret in memory).
for state in "" -paused -secret; do
    cp "$root/data/icons/hicolor/symbolic/apps/io.github.joaoferrete.Copyveil$state-symbolic.svg" \
        "$stage/icons/copyveil$state-symbolic.svg"
done
glib-compile-schemas --strict "$stage/schemas"
(cd "$stage" && gnome-extensions pack --force --out-dir="$root/_build" \
    --extra-source=clipboard.js --extra-source=daemon.js \
    --extra-source=indicator.js --extra-source=keyboard.js \
    --extra-source=lib --extra-source=icons \
    --schema=schemas/org.gnome.shell.extensions.copyveil.gschema.xml)
# `gnome-extensions pack` leaves the compiled schema out; add it.
python3 -c 'import sys, zipfile; z = zipfile.ZipFile(sys.argv[1], "a"); z.write(sys.argv[2], "schemas/gschemas.compiled"); z.close()' \
    "$zip" "$stage/schemas/gschemas.compiled"
echo "$zip"

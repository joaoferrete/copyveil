#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Extracts the GNOME Shell extension's strings into
# gnome-extension/po/copyveil-extension.pot. With --update, also merges them
# into every catalog listed in gnome-extension/po/LINGUAS.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
po="$root/gnome-extension/po"
pot="$po/copyveil-extension.pot"

cd "$root/gnome-extension"
xgettext --from-code=UTF-8 --language=JavaScript --keyword=_ --keyword=N_ \
    --package-name=copyveil-extension --add-comments=Translators \
    --output="$pot" ./*.js lib/*.js
echo "$pot"

if [ "${1:-}" = "--update" ]; then
    while read -r lang; do
        case "$lang" in '' | '#'*) continue ;; esac
        msgmerge --quiet --update --backup=none "$po/$lang.po" "$pot"
        echo "$po/$lang.po"
    done < "$po/LINGUAS"
fi

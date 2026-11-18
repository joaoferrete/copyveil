#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Fails when a catalog is broken or falls behind the English: extracts the
# current strings, merges them into every catalog, and requires every string
# translated and none fuzzy. Run by CI and `just i18n-check`.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failed=0

check() { # domain pot po...
    domain=$1 pot=$2
    shift 2
    for po in "$@"; do
        msgmerge --quiet --no-fuzzy-matching -o "$work/merged.po" "$po" "$pot"
        stats=$(msgfmt --check --statistics -o /dev/null "$work/merged.po" 2>&1)
        case "$stats" in
            *untranslated* | *fuzzy* | *"não traduzida"* | *aproximad*)
                echo "FAIL $domain $(basename "$po"): $stats"; failed=1 ;;
            *) echo "ok   $domain $(basename "$po"): $stats" ;;
        esac
    done
}

cd "$root"
xgettext --from-code=UTF-8 --package-name=copyveil --keyword=gettext --keyword=N_! \
    --keyword=ngettext:1,2 -f po/POTFILES.in -o "$work/copyveil.pot" 2> /dev/null
check copyveil "$work/copyveil.pot" po/*.po

(cd gnome-extension && xgettext --from-code=UTF-8 --language=JavaScript --keyword=_ \
    --keyword=N_ -o "$work/extension.pot" ./*.js lib/*.js)
check copyveil-extension "$work/extension.pot" gnome-extension/po/*.po

exit $failed

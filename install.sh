#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Installs Copyveil from this source tree.
#
#   ./install.sh                 check, build and install for you (~/.local, no sudo)
#   ./install.sh --check         only check the dependencies
#   ./install.sh --install-deps  install missing system packages first (asks for sudo)
#   ./install.sh --system        install for everyone in /usr/local (asks for sudo)
#   ./install.sh --uninstall     remove Copyveil (keeps your History and settings)
#   ./install.sh --uninstall --purge   also erase your History and settings
#
# Other options: --yes (don't ask), --no-session (skip systemd and the GNOME
# Shell extension: for packaging and tests), --help.
#
# It never pipes anything from the network into a shell: if Rust is missing,
# it tells you how to install it.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")" && pwd)
UUID=copyveil@joaoferrete.github.io
GTK_MIN=4.14
ADW_MIN=1.5
MESON_MIN=1.1

mode=install
system=false
install_deps=false
assume_yes=false
session=true
purge=false

for arg in "$@"; do
    case "$arg" in
        --check) mode=check ;;
        --uninstall) mode=uninstall ;;
        --purge) purge=true ;;
        --system) system=true ;;
        --install-deps) install_deps=true ;;
        --yes | -y) assume_yes=true ;;
        --no-session) session=false ;;
        --help | -h) sed -n '3,17p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "Unknown option: $arg (see --help)" >&2; exit 2 ;;
    esac
done

if [ -t 1 ]; then
    bold=$'\e[1m' green=$'\e[32m' red=$'\e[31m' yellow=$'\e[33m' reset=$'\e[0m'
else
    bold='' green='' red='' yellow='' reset=''
fi
ok() { printf '  %s✔%s %s\n' "$green" "$reset" "$*"; }
bad() { printf '  %s✘%s %s\n' "$red" "$reset" "$*"; }
warn() { printf '  %s!%s %s\n' "$yellow" "$reset" "$*"; }
step() { printf '\n%s%s%s\n' "$bold" "$*" "$reset"; }
die() { printf '%sError:%s %s\n' "$red" "$reset" "$*" >&2; exit 1; }
confirm() {
    $assume_yes && return 0
    read -r -p "$1 [y/N] " answer
    [[ "$answer" =~ ^[Yy] ]]
}

if $system; then
    prefix=/usr/local
    unit_dir=""
    sudo=sudo
else
    prefix="$HOME/.local"
    unit_dir="${XDG_DATA_HOME:-$HOME/.local/share}/systemd/user"
    sudo=""
fi

# --- Distribution -----------------------------------------------------------

distro=unknown
if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    case " ${ID:-} ${ID_LIKE:-} " in
        *" debian "* | *" ubuntu "*) distro=debian ;;
        *" fedora "* | *" rhel "*) distro=fedora ;;
        *" arch "*) distro=arch ;;
    esac
fi

# Package names for each missing piece, per distribution.
package_for() {
    case "$distro:$1" in
        debian:meson) echo meson ;;
        debian:ninja) echo ninja-build ;;
        debian:pkg-config) echo pkg-config ;;
        debian:compiler) echo build-essential ;;
        debian:gtk) echo libgtk-4-dev ;;
        debian:adw) echo libadwaita-1-dev ;;
        debian:python) echo python3 ;;
        debian:glib) echo libglib2.0-bin ;;
        fedora:meson) echo meson ;;
        fedora:ninja) echo ninja-build ;;
        fedora:pkg-config) echo pkgconf-pkg-config ;;
        fedora:compiler) echo gcc ;;
        fedora:gtk) echo gtk4-devel ;;
        fedora:adw) echo libadwaita-devel ;;
        fedora:python) echo python3 ;;
        fedora:glib) echo glib2 ;;
        arch:meson) echo meson ;;
        arch:ninja) echo ninja ;;
        arch:pkg-config) echo pkgconf ;;
        arch:compiler) echo base-devel ;;
        arch:gtk) echo gtk4 ;;
        arch:adw) echo libadwaita ;;
        arch:python) echo python ;;
        arch:glib) echo glib2 ;;
    esac
}

install_command() {
    case "$distro" in
        debian) echo "sudo apt install $*" ;;
        fedora) echo "sudo dnf install $*" ;;
        arch) echo "sudo pacman -S --needed $*" ;;
        *) echo "install with your package manager: $*" ;;
    esac
}

# --- Dependencies -----------------------------------------------------------

missing=()
rust_missing=false

version_at_least() { [ "$(printf '%s\n%s\n' "$2" "$1" | sort -V | head -n1)" = "$2" ]; }

check_dependencies() {
    step "Checking dependencies"
    if command -v cargo > /dev/null && command -v rustup > /dev/null; then
        ok "Rust ($(cargo --version | cut -d' ' -f2); rustup installs the pinned toolchain)"
    else
        bad "Rust with rustup"
        rust_missing=true
    fi
    if command -v cc > /dev/null; then ok "C compiler"; else bad "C compiler"; missing+=(compiler); fi
    if command -v meson > /dev/null && version_at_least "$(meson --version)" "$MESON_MIN"; then
        ok "Meson $(meson --version)"
    else
        bad "Meson ≥ $MESON_MIN"; missing+=(meson)
    fi
    if command -v ninja > /dev/null; then ok "Ninja $(ninja --version)"; else bad "Ninja"; missing+=(ninja); fi
    if command -v pkg-config > /dev/null; then
        ok "pkg-config"
        if pkg-config --atleast-version="$GTK_MIN" gtk4 2> /dev/null; then
            ok "GTK $(pkg-config --modversion gtk4) (development files)"
        else
            bad "GTK ≥ $GTK_MIN development files"; missing+=(gtk)
        fi
        if pkg-config --atleast-version="$ADW_MIN" libadwaita-1 2> /dev/null; then
            ok "libadwaita $(pkg-config --modversion libadwaita-1) (development files)"
        else
            bad "libadwaita ≥ $ADW_MIN development files"; missing+=(adw)
        fi
    else
        bad "pkg-config"; missing+=(pkg-config gtk adw)
    fi
    if command -v python3 > /dev/null; then ok "Python 3"; else bad "Python 3"; missing+=(python); fi
    if command -v glib-compile-schemas > /dev/null; then
        ok "GLib tools"
    else
        bad "GLib tools (glib-compile-schemas)"; missing+=(glib)
    fi

    if command -v gnome-shell > /dev/null; then
        ok "GNOME Shell $(gnome-shell --version | awk '{print $3}')"
        gnome=true
    else
        warn "GNOME Shell not found: Copyveil captures on GNOME for now (KDE, COSMIC and wlroots arrive in 0.2)"
        gnome=false
    fi
    if busctl --user list 2> /dev/null | grep -q org.freedesktop.secrets \
        || ls /usr/share/dbus-1/services/*secret* > /dev/null 2>&1; then
        ok "Secret Service keyring"
    else
        warn "No Secret Service keyring found: the History will stay in memory only (install GNOME Keyring or KWallet to keep it)"
    fi
}

report_missing() {
    if $rust_missing; then
        printf '\n  Install Rust with rustup (see https://rustup.rs), then run this again.\n'
    fi
    if [ ${#missing[@]} -gt 0 ]; then
        local packages=()
        for piece in "${missing[@]}"; do
            local package
            package=$(package_for "$piece")
            packages+=("${package:-$piece}")
        done
        printf '\n  Missing packages: %s\n' "$(install_command "${packages[@]}")"
        if $install_deps && [ "$distro" != unknown ]; then
            if confirm "  Run that now?"; then
                case "$distro" in
                    debian) sudo apt install -y "${packages[@]}" ;;
                    fedora) sudo dnf install -y "${packages[@]}" ;;
                    arch) sudo pacman -S --needed --noconfirm "${packages[@]}" ;;
                esac
                missing=()
            fi
        fi
    fi
    if $rust_missing || [ ${#missing[@]} -gt 0 ]; then
        return 1
    fi
}

# --- Install ----------------------------------------------------------------

build_and_install() {
    step "Building (release)"
    local options=(--prefix="$prefix" -Dprofile=release)
    [ -n "$unit_dir" ] && options+=(-Dsystemd_user_unit_dir="$unit_dir")
    if [ -d "$ROOT/_build/meson-info" ]; then
        meson setup --reconfigure "$ROOT/_build" "$ROOT" "${options[@]}" > /dev/null
    else
        meson setup "$ROOT/_build" "$ROOT" "${options[@]}" > /dev/null
    fi
    meson compile -C "$ROOT/_build"

    step "Installing into $prefix"
    $sudo meson install -C "$ROOT/_build" --no-rebuild --quiet
    ok "copyveil and copyveil-daemon in $prefix/bin"
    case ":$PATH:" in
        *":$prefix/bin:"*) ;;
        *) warn "$prefix/bin isn't on your PATH; the app still starts from your apps and through D-Bus" ;;
    esac
}

start_session() {
    $session || return 0
    step "Starting the daemon"
    systemctl --user daemon-reload
    systemctl --user enable --now copyveil-daemon.service
    ok "copyveil-daemon runs now and starts with your session"

    if ! $gnome; then
        return 0
    fi
    step "Installing the GNOME Shell extension"
    local zip
    zip=$("$ROOT/tools/pack-extension.sh" | tail -n1)
    gnome-extensions install --force "$zip"
    if gnome-extensions enable "$UUID" 2> /dev/null; then
        ok "Extension installed and turned on"
    else
        ok "Extension installed"
        warn "GNOME Shell loads new extensions at login: log out and back in, then it turns on"
    fi
}

uninstall() {
    step "Uninstalling Copyveil"
    if $session; then
        systemctl --user disable --now copyveil-daemon.service 2> /dev/null || true
        gnome-extensions uninstall "$UUID" 2> /dev/null && ok "Extension removed" || true
    fi
    if [ -f "$ROOT/_build/meson-logs/install-log.txt" ]; then
        $sudo ninja -C "$ROOT/_build" uninstall > /dev/null
        ok "Files removed"
    else
        warn "No install log in _build/: nothing was installed from this folder"
    fi
    $session && systemctl --user daemon-reload 2> /dev/null || true
    if $purge; then
        rm -rf "${XDG_DATA_HOME:-$HOME/.local/share}/copyveil" \
            "${XDG_CONFIG_HOME:-$HOME/.config}/copyveil" \
            "${XDG_STATE_HOME:-$HOME/.local/state}/copyveil"
        ok "History, settings and onboarding state erased"
        warn "The History key may remain in your keyring as “Copyveil History key”; delete it in Passwords and Keys"
    else
        ok "Your History and settings are kept (use --purge to erase them)"
    fi
}

# --- Main -------------------------------------------------------------------

printf '%sCopyveil%s — remembers what you copy, forgets your secrets.\n' "$bold" "$reset"
printf 'Distribution: %s · Install into: %s\n' "${PRETTY_NAME:-unknown}" "$prefix"

case "$mode" in
    uninstall)
        uninstall
        exit 0
        ;;
    check)
        check_dependencies
        report_missing || exit 1
        printf '\n%sEverything Copyveil needs is here.%s\n' "$green" "$reset"
        exit 0
        ;;
esac

check_dependencies
report_missing || die "install the missing dependencies, then run ./install.sh again (or use --install-deps)"
build_and_install
start_session

step "Done"
if $session && ${gnome:-false}; then
    printf '  Open %sCopyveil%s from your apps: a short onboarding checks that everything works.\n' "$bold" "$reset"
    printf '  Then press Super+Shift+V to open the History.\n'
fi

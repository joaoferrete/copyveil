#!/bin/sh
# Called by meson.build. Builds the workspace with Cargo and copies the
# binaries into Meson's output directory so `meson install` can install them.
set -eu

cargo="$1"
source_root="$2"
target_dir="$3"
profile="$4"
out_dir="$(cd "$5" && pwd)"
# Where the app finds its translations (compiled in, see copyveil-app's i18n).
export COPYVEIL_LOCALEDIR="${6:-/usr/local/share/locale}"

# rustup picks the toolchain from rust-toolchain.toml in the working
# directory, and Meson runs this from the build directory.
cd "$source_root"

if [ "$profile" = "release" ]; then
  "$cargo" build --locked --release --manifest-path "$source_root/Cargo.toml" --target-dir "$target_dir"
else
  "$cargo" build --locked --manifest-path "$source_root/Cargo.toml" --target-dir "$target_dir"
fi

cp "$target_dir/$profile/copyveil" "$out_dir/copyveil"
cp "$target_dir/$profile/copyveil-daemon" "$out_dir/copyveil-daemon"

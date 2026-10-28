#!/bin/sh
set -eu
relay_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
relay_output=${1:?Pass an absolute output .deb path}
relay_version=$(sed -n 's/^version = "\([^"]*\)"$/\1/p' "$relay_root/Cargo.toml" | head -n 1)
test -n "$relay_version"
case "$relay_output" in /*) ;; *) printf '%s\n' 'Use an absolute output path.' >&2; exit 1;; esac
relay_stage=$(mktemp -d)
trap 'rm -rf "$relay_stage"' EXIT HUP INT TERM
mkdir -p "$relay_stage/DEBIAN" "$relay_stage/usr/bin" \
 "$relay_stage/usr/share/applications" "$relay_stage/usr/share/doc/relay-native"
find "$relay_stage" -type d -exec chmod 755 {} +
install -m 755 "$relay_root/bin/relay" "$relay_stage/usr/bin/relay-native"
install -m 644 "$relay_root/packaging/messages.desktop" "$relay_stage/usr/share/applications/relay-native.desktop"
install -m 644 "$relay_root/LICENSE" "$relay_stage/usr/share/doc/relay-native/copyright"
cat > "$relay_stage/DEBIAN/control" <<CONTROL
Package: relay-native
Version: $relay_version
Architecture: amd64
Maintainer: Relay contributors
Depends: libc6 (>= 2.34), libgcc-s1, libgtk-3-0t64 (>= 3.24) | libgtk-3-0 (>= 3.24), libwebkit2gtk-4.1-0 (>= 2.42)
Section: net
Priority: optional
Description: Native GTK messaging workspace
 Rust desktop shell with an embedded WebKitGTK engine and an icon-only sidebar.
CONTROL
dpkg-deb --root-owner-group --build "$relay_stage" "$relay_output"

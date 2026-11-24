#!/usr/bin/env bash
# Build a .deb from an existing release build.
#   cargo build --release && packaging/linux/build-deb.sh
# Writes dist/webby_<version>_<arch>.deb.
set -euo pipefail
cd "$(dirname "$0")/../.."

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
arch=$(dpkg --print-architecture)
binary=target/release/webby
[ -x "$binary" ] || { echo "error: $binary not found; run cargo build --release first" >&2; exit 1; }

id=co.coderstudio.webby
root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT

install -Dm755 "$binary" "$root/usr/bin/webby"
install -Dm644 packaging/linux/$id.desktop "$root/usr/share/applications/$id.desktop"
install -Dm644 packaging/icons/webby.svg "$root/usr/share/icons/hicolor/scalable/apps/$id.svg"
install -Dm644 packaging/icons/webby-256.png "$root/usr/share/icons/hicolor/256x256/apps/$id.png"

mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<CONTROL
Package: webby
Version: $version
Architecture: $arch
Maintainer: rcdelacruz <rcdelacruz@users.noreply.github.com>
Section: web
Priority: optional
Depends: libgtk-4-1, libwebkitgtk-6.0-4
Installed-Size: $(du -sk "$root/usr" | cut -f1)
Homepage: https://github.com/rcdelacruz/webby
Description: Small web browser built on WebKit
 webby is a tabbed web browser written in Rust that renders with
 WebKitGTK.
CONTROL

mkdir -p dist
out=dist/webby_${version}_${arch}.deb
dpkg-deb --build --root-owner-group "$root" "$out" >/dev/null
echo "$out"

#!/usr/bin/env bash
# Assemble webby.app and wrap it in a disk image. Run on macOS.
#   packaging/macos/build-dmg.sh [path/to/webby-binary]
# Without an argument, builds a universal (Apple silicon + Intel) binary.
# Writes dist/webby-<version>-macos.dmg.
set -euo pipefail
cd "$(dirname "$0")/../.."

version=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
binary=${1:-}
if [ -z "$binary" ]; then
    rustup target add aarch64-apple-darwin x86_64-apple-darwin
    cargo build --release --target aarch64-apple-darwin
    cargo build --release --target x86_64-apple-darwin
    binary=target/webby-universal
    lipo -create -output "$binary" target/aarch64-apple-darwin/release/webby target/x86_64-apple-darwin/release/webby
fi

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/webby.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
install -m755 "$binary" "$app/Contents/MacOS/webby"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"

# Build the .icns from the 1024px master.
iconset="$stage/webby.iconset"
mkdir "$iconset"
for size in 16 32 128 256 512; do
    sips -z $size $size packaging/icons/webby-1024.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z $((size * 2)) $((size * 2)) packaging/icons/webby-1024.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$iconset" -o "$app/Contents/Resources/webby.icns"
rm -rf "$iconset"

# Ad-hoc signature: enough to run locally, not a Developer ID signature.
codesign --force --deep --sign - "$app"

ln -s /Applications "$stage/Applications"
mkdir -p dist
out=dist/webby-${version}-macos.dmg
rm -f "$out"
hdiutil create -volname webby -srcfolder "$stage" -ov -format UDZO "$out" >/dev/null
echo "$out"

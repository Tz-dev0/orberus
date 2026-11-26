#!/bin/sh
# Builds the release archive of the menu bar app: one universal
# "Claude Profiles.app" that runs on Apple Silicon and Intel Macs.
#
#   scripts/package-app.sh 0.1.0
#
# Writes dist/Claude-Profiles-macos-universal.tar.gz and its .sha256, next to
# what scripts/package.sh left there. Needs Node and both Rust targets:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
#
# The app is not signed with an Apple developer certificate. That is why it
# ships as a tar.gz to fetch with curl: macOS only blocks unsigned apps that a
# browser downloaded.
set -eu

version="${1:?usage: scripts/package-app.sh <version>}"
cd "$(dirname "$0")/.."

declared="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' app/src-tauri/tauri.conf.json | head -n 1)"
if [ "$version" != "$declared" ]; then
    echo "error: asked for version $version but tauri.conf.json says $declared" >&2
    exit 1
fi

# npm ci installs exactly what package-lock.json lists.
(cd app && npm ci && npm run tauri build -- --target universal-apple-darwin)

app="app/src-tauri/target/universal-apple-darwin/release/bundle/macos/Claude Profiles.app"
lipo -archs "$app/Contents/MacOS/"*

name="Claude-Profiles-macos-universal"
mkdir -p dist
# The app sits at the top of the archive, so that
# `tar -xz -C /Applications` puts it straight in place.
tar -czf "dist/$name.tar.gz" -C "$(dirname "$app")" "Claude Profiles.app"
(cd dist && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
cat "dist/$name.tar.gz.sha256"

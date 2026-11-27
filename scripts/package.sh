#!/bin/sh
# Builds the release archive of the CLI: one universal macOS binary that runs
# on Apple Silicon and Intel Macs.
#
#   scripts/package.sh 0.1.0
#
# Writes dist/orberus-cli-macos-universal.tar.gz and its .sha256.
# The release workflow runs this; it also works on a Mac with both Rust
# targets installed:
#   rustup target add aarch64-apple-darwin x86_64-apple-darwin
set -eu

version="${1:?usage: scripts/package.sh <version>}"
cd "$(dirname "$0")/.."

# The archive name and `orberus --version` must say the same thing.
declared="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)"
if [ "$version" != "$declared" ]; then
    echo "error: asked for version $version but Cargo.toml says $declared" >&2
    exit 1
fi

targets="aarch64-apple-darwin x86_64-apple-darwin"
for target in $targets; do
    cargo build --release --locked --target "$target"
done

# No version in the name: releases/latest/download/<name> then always works.
name="orberus-cli-macos-universal"
stage="dist/$name"
rm -rf dist
mkdir -p "$stage"

# lipo glues the two builds into a single file; macOS picks the right half.
lipo -create -output "$stage/orberus" \
    target/aarch64-apple-darwin/release/orberus \
    target/x86_64-apple-darwin/release/orberus
cp LICENSE README.md "$stage/"

# Fail here rather than publish a binary that does not start.
"$stage/orberus" --version
lipo -archs "$stage/orberus"

tar -czf "dist/$name.tar.gz" -C dist "$name"
rm -rf "$stage"
(cd dist && shasum -a 256 "$name.tar.gz" > "$name.tar.gz.sha256")
cat "dist/$name.tar.gz.sha256"

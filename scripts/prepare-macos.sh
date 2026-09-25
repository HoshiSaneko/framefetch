#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
[[ "$(uname -s)" == Darwin ]] || { echo 'This script requires macOS.' >&2; exit 1; }
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT
mkdir -p src-tauri/bin
version=2026.08.19
base="https://github.com/yt-dlp/yt-dlp/releases/download/$version"
curl --fail --silent --show-error --location --retry 5 --retry-all-errors "$base/yt-dlp_macos" -o "$staging/yt-dlp_macos"
curl --fail --silent --show-error --location --retry 5 --retry-all-errors "$base/SHA2-256SUMS" -o "$staging/SHA2-256SUMS"
(cd "$staging" && awk '$2 == "yt-dlp_macos" {print; found=1} END {if (!found) exit 1}' SHA2-256SUMS > checksum && shasum -a 256 -c checksum)
install -m 755 "$staging/yt-dlp_macos" src-tauri/bin/yt-dlp
case "$(uname -m)" in
  arm64)
    suffix=9arm
    ffmpeg_sha=591260c945d0eef150e3bf82b0ef988bd36a9cecc18ff05d6679617159f0a95e
    ffprobe_sha=e11c17e8200b3ee4c4c186d245e2b4053f01d56957336c1817fca0b997469106
    ;;
  x86_64)
    suffix=80intel
    ffmpeg_sha=df3f1e3facdc1ae0ad0bd898cdfb072fbc9641bf47b11f172844525a05db8d11
    ffprobe_sha=5228e651e2bd67bb55819b27f6138351587b16d2b87446007bf35b7cf930d891
    ;;
  *) echo 'Unsupported architecture' >&2; exit 1 ;;
esac
for component in ffmpeg ffprobe; do
  curl --fail --silent --show-error --location --retry 5 --retry-all-errors "https://www.osxexperts.net/${component}${suffix}.zip" -o "$staging/$component.zip"
  unzip -q "$staging/$component.zip" -d "$staging/$component-extracted"
  binary=$(find "$staging/$component-extracted" -type f -name "$component" -print -quit)
  [[ -n "$binary" ]] || { echo "Missing $component in archive" >&2; exit 1; }
  if [[ "$component" == ffmpeg ]]; then expected=$ffmpeg_sha; else expected=$ffprobe_sha; fi
  printf '%s  %s\n' "$expected" "$binary" | shasum -a 256 -c -
  install -m 755 "$binary" "src-tauri/bin/$component"
done
src-tauri/bin/yt-dlp --version
src-tauri/bin/ffmpeg -version | head -n 1
src-tauri/bin/ffprobe -version | head -n 1
echo 'macOS download components are ready.'

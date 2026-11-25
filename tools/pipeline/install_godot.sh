#!/usr/bin/env bash
# Finds the pinned Godot, or downloads it (checksum verified). Prints the path on stdout.
#   install_godot.sh [4.7.2-stable]     Needs curl and unzip. Installs under ~/.cache/gdp only.
set -euo pipefail
VERSION="${1:-4.7.2-stable}"
WANT="${VERSION/-/.}"
CACHE="${GDP_CACHE:-$HOME/.cache/gdp}/godot-$VERSION"
case "$(uname -s)-$(uname -m)" in
  Darwin-*)      ASSET="macos.universal";  BIN="Godot.app/Contents/MacOS/Godot" ;;
  Linux-x86_64)  ASSET="linux.x86_64";     BIN="Godot_v${VERSION}_linux.x86_64" ;;
  Linux-aarch64|Linux-arm64) ASSET="linux.arm64"; BIN="Godot_v${VERSION}_linux.arm64" ;;
  *) echo "install_godot: unsupported system $(uname -sm)" >&2; exit 1 ;;
esac
for g in "${GODOT:-}" "$CACHE/$BIN" /Applications/Godot.app/Contents/MacOS/Godot "$(command -v godot || true)"; do
  if [[ -n "$g" && -x "$g" ]]; then
    have="$("$g" --version 2>/dev/null | head -1 || true)"
    if [[ "$have" == "$WANT"* ]]; then echo "$g"; exit 0; fi
  fi
done
zip="Godot_v${VERSION}_${ASSET}.zip"
base="https://github.com/godotengine/godot/releases/download/$VERSION"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
echo "install_godot: downloading $zip" >&2
curl -fsSL -o "$tmp/$zip" "$base/$zip"
curl -fsSL -o "$tmp/SUMS" "$base/SHA512-SUMS.txt"
want_sum="$(awk -v f="$zip" '$2==f || $2=="*"f {print $1}' "$tmp/SUMS")"
have_sum="$( (sha512sum "$tmp/$zip" 2>/dev/null || shasum -a 512 "$tmp/$zip") | cut -d' ' -f1)"
if [[ -z "$want_sum" || "$want_sum" != "$have_sum" ]]; then echo "install_godot: checksum mismatch, nothing installed" >&2; exit 1; fi
mkdir -p "$CACHE"
unzip -q -o "$tmp/$zip" -d "$CACHE"
"$CACHE/$BIN" --version >/dev/null 2>&1 || { echo "install_godot: $CACHE/$BIN does not run" >&2; exit 1; }
echo "$CACHE/$BIN"

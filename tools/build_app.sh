#!/bin/zsh
# Build ElfStory.app (macOS, release): Rust bridge in release, then the
# Godot "macOS" export preset (godot/export_presets.cfg). Needs the Godot
# 4.7.2 export templates in ~/Library/Application Support/Godot/export_templates/4.7.2.stable/.
# Usage: tools/build_app.sh [--smoke]   (--smoke launches the app, saves build/smoke.png, quits)
set -euo pipefail
ROOT="${0:A:h:h}"
GODOT=/Applications/Godot.app/Contents/MacOS/Godot
cd "$ROOT/rust" && cargo build --release -p elfstory-gdext
mkdir -p "$ROOT/build" && rm -rf "$ROOT/build/ElfStory.app"
"$GODOT" --headless --path "$ROOT/godot" --export-release "macOS" ../build/ElfStory.app
echo "Built $ROOT/build/ElfStory.app"
if [[ "${1:-}" == "--smoke" ]]; then
  "$ROOT/build/ElfStory.app/Contents/MacOS/ElfStory Reborn" -- --elfstory-capture="$ROOT/build/smoke.png"
fi

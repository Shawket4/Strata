#!/bin/sh
# Regenerates every app icon and native splash from design/brand (docs/RUNBOOK.md §13).
# Needs rsvg-convert (librsvg2-bin), python3 with Pillow, and Flutter on PATH.
set -eu
cd "$(dirname "$0")/../.."
python3 tool/brand/render_brand.py
dart pub global activate flutter_launcher_icons 0.14.4 >/dev/null
dart pub global run flutter_launcher_icons -f flutter_launcher_icons.yaml
dart run flutter_native_splash:create --path=flutter_native_splash.yaml

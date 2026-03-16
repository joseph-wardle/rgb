#!/bin/sh
# Build the rgb_web crate as a WASM module.
#
# Prerequisites:
#   cargo install wasm-pack
#
# Outputs:
#   pkg/          — WASM module for the standalone rgb_web page
#   ../../portfolio/static/emulator/
#               — copy of pkg/ served by the Hugo portfolio at /emulator/
#
# Standalone test:
#   python3 -m http.server -d rgb_web 8080
#   # then open http://localhost:8080

set -e
cd "$(dirname "$0")"

wasm-pack build --target web --out-dir pkg

# Copy the generated module into the portfolio's static asset tree so Hugo
# serves it at /emulator/rgb_web.js and /emulator/rgb_web_bg.wasm.
PORTFOLIO_DEST="../../portfolio/static/emulator"
mkdir -p "$PORTFOLIO_DEST"
cp pkg/rgb_web.js pkg/rgb_web_bg.wasm "$PORTFOLIO_DEST/"

echo "Build complete."
echo "  Standalone:  python3 -m http.server -d . 8080  (then open index.html)"
echo "  Portfolio:   files copied to $PORTFOLIO_DEST"

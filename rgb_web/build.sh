#!/bin/sh
# Build the rgb_web crate as a WASM module.
#
# Prerequisites:
#   cargo install wasm-pack
#
# After building, serve the rgb_web/ directory with any static HTTP server:
#   python3 -m http.server -d rgb_web 8080
#   # then open http://localhost:8080

set -e
cd "$(dirname "$0")"

wasm-pack build --target web --out-dir pkg
echo "Build complete.  Serve this directory and open index.html."

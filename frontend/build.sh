#!/bin/bash
set -e

echo "Cleaning previous build..."
trunk clean

echo "Building frontend..."
trunk build --release

echo "Modifying asset paths in index.html for backend compatibility..."
# Replace the asset paths in the HTML file to match our new route patterns
sed -i 's|/frontend-\([^_]*\).js|/js-\1|g' dist/index.html
sed -i 's|/frontend-\([^_]*\)_bg.wasm|/wasm-\1|g' dist/index.html
sed -i 's|/main-\([^\.]*\).css|/css-\1|g' dist/index.html

# Remove integrity hashes to prevent validation issues
sed -i 's| integrity="[^"]*"||g' dist/index.html

echo "Build complete!"
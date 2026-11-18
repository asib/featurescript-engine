#!/usr/bin/env bash
# Builds the WebAssembly engine and assembles its browser package (crates/fs-wasm/package) in OUT_DIR.
#
# Usage: crates/fs-wasm/pack.sh OUT_DIR
#
# Needs Emscripten in the shell (`source emsdk_env.sh`) and OCCT built by build-occt.sh (OCCT_INCLUDE_DIR and
# OCCT_LIB_DIR, defaulting to ~/wasm-build). OUT_DIR gets index.js and index.d.ts, the engine (fs_wasm.js, .wasm and
# .data, which is std), the fonts sketch text uses, the licences and a README, replacing what was there.
set -euo pipefail

out=${1:?usage: pack.sh OUT_DIR}
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../.." && pwd)
export OCCT_INCLUDE_DIR=${OCCT_INCLUDE_DIR:-$HOME/wasm-build/occt-wasm/include/opencascade}
export OCCT_LIB_DIR=${OCCT_LIB_DIR:-$HOME/wasm-build/occt-wasm/lin32/clang/lib}

(cd "$repo" && cargo build --profile wasm -p fs-wasm --target wasm32-unknown-emscripten)
built="$repo/target/wasm32-unknown-emscripten/wasm/deps"

rm -rf "$out"
mkdir -p "$out/fonts"
cp "$here/package/index.js" "$here/package/index.d.ts" "$here/package/package.json" "$here/package/README.md" "$out/"
cp "$built/fs_wasm.js" "$built/fs_wasm.wasm" "$built/fs_wasm.data" "$out/"
cp "$repo"/crates/fs-geom/fonts/*.ttf "$out/fonts/"
cp "$repo/crates/fs-geom/fonts/README.md" "$out/fonts/"
cp "$repo"/crates/fs-geom/fonts/*-OFL.txt "$repo"/crates/fs-geom/fonts/*-COPYRIGHT "$repo"/crates/fs-geom/fonts/LICENSE-Apache-2.0 "$out/fonts/"
cp "$HOME/wasm-build/OCCT-7_9_2/LICENSE_LGPL_21.txt" "$out/OCCT-LICENSE_LGPL_21.txt" 2>/dev/null || true
cp "$HOME/wasm-build/OCCT-7_9_2/OCCT_LGPL_EXCEPTION.txt" "$out/OCCT_LGPL_EXCEPTION.txt" 2>/dev/null || true
git -C "$repo" rev-parse HEAD > "$out/ENGINE_COMMIT"
ls -la "$out"

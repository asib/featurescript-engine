#!/usr/bin/env bash
# Builds OpenCascade 7.9.2's static libraries for WebAssembly with Emscripten, for crates/fs-wasm.
#
# Usage: crates/fs-wasm/build-occt.sh [WORK_DIR]   (default ~/wasm-build)
#
# Needs Emscripten (emsdk, `source emsdk_env.sh`), CMake and Ninja. Writes WORK_DIR/occt-wasm, whose
# include/opencascade and lin32/clang/lib are OCCT_INCLUDE_DIR and OCCT_LIB_DIR for `cargo build --target
# wasm32-unknown-emscripten -p fs-wasm`. Takes a couple of hours on four cores; JOBS (default 3) bounds the memory.
set -euo pipefail

work=${1:-$HOME/wasm-build}
jobs=${JOBS:-3}
mkdir -p "$work"
cd "$work"
if [ ! -d OCCT-7_9_2 ]; then
  curl -sL https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V7_9_2.tar.gz -o occt-7.9.2.tar.gz
  tar xzf occt-7.9.2.tar.gz
fi

# OCCT converts Unix signals to exceptions with setjmp in every OCC_CATCH_SIGNALS block. WebAssembly has no signals,
# and setjmp mixed with WebAssembly exceptions miscompiles (an invalid br_table in ShapeUpgrade_ShapeDivide::Perform),
# so leave it out for Emscripten.
flags=OCCT-7_9_2/adm/cmake/occt_defs_flags.cmake
if ! grep -q "if (NOT EMSCRIPTEN)" "$flags"; then
  python3 - "$flags" <<'PY'
import sys
path = sys.argv[1]
text = open(path).read()
old = "  add_definitions(-DOCC_CONVERT_SIGNALS)\nendif()"
assert text.count(old) == 1, "OCC_CONVERT_SIGNALS isn't where it was"
open(path, "w").write(text.replace(old, "  if (NOT EMSCRIPTEN)\n    add_definitions(-DOCC_CONVERT_SIGNALS)\n  endif()\nendif()"))
PY
fi

# Static libraries, WebAssembly exceptions (as the bridge's), no OpenGL or third-party libraries. XCAF (STEP with
# names) needs the visualization toolkits TKService, TKV3d and TKVCAF, which build without OpenGL.
emcmake cmake -S OCCT-7_9_2 -B occt-wasm -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DBUILD_LIBRARY_TYPE=Static -DINSTALL_DIR="$work/occt-install" \
  -DBUILD_MODULE_Draw=OFF -DBUILD_MODULE_DETools=OFF -DBUILD_MODULE_Visualization=ON \
  -DBUILD_MODULE_ApplicationFramework=ON -DBUILD_MODULE_DataExchange=ON \
  -DUSE_FREETYPE=OFF -DUSE_TK=OFF -DUSE_TCL=OFF -DUSE_OPENGL=OFF -DUSE_GLES2=OFF -DUSE_FREEIMAGE=OFF \
  -DUSE_RAPIDJSON=OFF -DUSE_DRACO=OFF -DUSE_TBB=OFF -DUSE_VTK=OFF -DUSE_OPENVR=OFF -DUSE_XLIB=OFF \
  -DBUILD_DOC_Overview=OFF -DBUILD_SAMPLES_QT=OFF -DBUILD_Inspector=OFF \
  -DCMAKE_CXX_FLAGS="-fwasm-exceptions" -DCMAKE_C_FLAGS="-fwasm-exceptions" \
  -DCMAKE_CXX_FLAGS_RELEASE="-O2" -DCMAKE_C_FLAGS_RELEASE="-O2"

ninja -C occt-wasm -j"$jobs" TKernel TKMath TKG2d TKG3d TKGeomBase TKBRep TKGeomAlgo TKTopAlgo TKPrim TKBO TKBool \
  TKFillet TKOffset TKFeat TKShHealing TKMesh TKXSBase TKDE TKDESTEP TKCDF TKLCAF TKCAF TKXCAF TKService TKV3d TKVCAF \
  TKHLR
echo "OCCT_INCLUDE_DIR=$work/occt-wasm/include/opencascade"
echo "OCCT_LIB_DIR=$work/occt-wasm/lin32/clang/lib"

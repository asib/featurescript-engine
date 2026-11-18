# fs-wasm: the engine for WebAssembly

The FeatureScript engine built for the browser with Emscripten: Rust compiled for `wasm32-unknown-emscripten`, the
C++ bridge and OpenCascade 7.9.2 compiled with `em++`, and std preloaded into the module's virtual file system at
`/std`. `fs_build(path, feature, stem)` runs one feature of a Feature Studio in the virtual file system and writes
its solid parts beside it as `<stem>.stl` (binary, millimetres) and `<stem>.step`, with `<stem>.log` holding what it
printed and each part's volume, or the error.

## Building

1. Emscripten (tested with 7.0.0): `git clone https://github.com/emscripten-core/emsdk && cd emsdk && ./emsdk install
   latest && ./emsdk activate latest`, then `source emsdk_env.sh` in the shell that builds. CMake and Ninja too
   (`pip install --user cmake ninja`).
2. Rust's target: `rustup target add wasm32-unknown-emscripten`.
3. OpenCascade: `crates/fs-wasm/build-occt.sh` (a couple of hours). It leaves out OCCT's signal conversion, whose
   `setjmp` miscompiles beside WebAssembly exceptions.
4. The engine:

   ```sh
   OCCT_INCLUDE_DIR=~/wasm-build/occt-wasm/include/opencascade \
   OCCT_LIB_DIR=~/wasm-build/occt-wasm/lin32/clang/lib \
   cargo build --release -p fs-wasm --target wasm32-unknown-emscripten
   ```

   which writes `target/wasm32-unknown-emscripten/release/deps/fs_wasm.{js,wasm,data}`: the loader, the module
   (42 MB with line tables) and std (7.2 MB).

## Trying it

`node crates/fs-wasm/test.mjs crates/fs-wasm/rounded-cube.fs roundedCube` builds a filleted cube (7855.841 mm³, in
half a second); `node crates/fs-wasm/test.mjs examples/showcase.fs showcase /tmp/showcase` builds the showcase part
(108987.704 mm³ and 65820 triangles, as natively, in about 8 s) and copies its STL out.

## How it differs from the native engine

- No threads: the evaluator's deep recursion runs on the module's own 256 MB stack (`-sSTACK_SIZE`) rather than a
  thread's (`fs_interp::run_with_large_stack`).
- C++ exceptions are WebAssembly's (`-fwasm-exceptions`) throughout: OCCT, the bridge and the link.
- OCCT links statically, and the visualization toolkits XCAF needs (TKService, TKV3d, TKVCAF) with it.
- Fonts are compiled in, as natively.

# fs-wasm: the engine for WebAssembly

The FeatureScript engine built for the browser with Emscripten: Rust compiled for `wasm32-unknown-emscripten`, the
C++ bridge and OpenCascade 7.9.2 compiled with `em++`, and std preloaded into the module's virtual file system at
`/std`. It gives a page what OpenSCAD's engine gives a customizer: a design's editable parameters, and the design
built with edited values.

- `fs_parameters(path, feature, out)` writes the features of the Feature Studio at `path` and the parameters of
  `feature` (or its first) to `out` as JSON in the shape OpenSCAD's Customizer gives (`src/lib.rs`), read from the
  feature's precondition: lengths (millimetres), angles (degrees), integers, reals, booleans, text and enum choices,
  with their bounds, defaults and labels, and when they apply (`fs_interp::features`).
- `fs_build(path, feature, values, stem)` builds the feature with `values` (a JSON object; parameters missing take
  their defaults) and writes `<stem>.stl` (binary, millimetres), `<stem>.step` and `<stem>.log` (what it printed and
  each part's volume, or the error).
- `fs_model(path, feature, values, out)` builds the feature the same way but only keeps it, for `fs_mesh(tolerance,
  out)` (an STL meshed to within `tolerance` mm), `fs_step(out)` and `fs_volumes(out)` to write out one at a time: a
  page shows a coarse mesh as soon as the feature is built, and makes the fine one, the STEP and the volumes after.

The engine and the Feature Studio last loaded stay between calls, so a design built again with other values doesn't
load std again. Nor does it run again the operations that come before the first one its new values change: the
engine keeps the state each operation left (`fs_geom::reuse`) for three builds after the last that used it, and
what each slow measurement of those states returned. The showcase part's label is engraved last, so with a new label
it builds in a third of the time; its pins come halfway, and another count takes two thirds; its plate comes first,
and another length takes as long as ever; and the values before take a fortieth. A kept state is taken up only while
its shapes are as they were when it was made, so a build gives the same whether it started from kept states or from
nothing.

The same interface runs natively (`fs_wasm::parameters_json`, `fs_wasm::build`, `fs_wasm::model`;
tests/customizer.rs), and
`examples/build_native.rs` builds a design with values natively, to compare. `examples/rebuilds.rs` builds a design
with one set of values after another on one engine, checks each against a build on an engine that kept nothing, and
prints how many operations each ran, how many it took from what was kept, and how many measurements it was given
from what was remembered.

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
   cargo build --profile wasm -p fs-wasm --target wasm32-unknown-emscripten
   ```

   which writes `target/wasm32-unknown-emscripten/wasm/deps/fs_wasm.{js,wasm,data}`: the loader (an ES module),
   the module (23 MB, 7.7 MB gzipped: under the 25 MiB a static host such as Cloudflare Pages serves) and std
   (7.2 MB, 1 MB gzipped). The `wasm` profile is release without debug information, optimised for size.

## The browser package

`crates/fs-wasm/pack.sh OUT_DIR` builds the engine and assembles the package in `package/` with it: `index.js`
(`createEngine({ locateFile, loadFont })`, whose `parameters(source)` and `build(source, values)` wrap the entry
points; `index.d.ts` types it), the engine, std, the fonts sketch text uses and the licences. A page serves the
module, std and fonts and passes their URLs; fonts are fetched only for designs whose text uses them.

Releases tagged `wasm-v<version>` (the version in `package/package.json`) carry the package built, so it needn't be
rebuilt to be used: `featurescript-engine-wasm-<version>.tar.gz` unpacks to `featurescript-engine/`, and
`SHA256SUMS` has its checksum. To make one, check out the commit to release, run `pack.sh`, archive its output as a
`featurescript-engine/` folder, tag the commit and attach the archive and checksum with `gh release create`.

## Trying it

`node crates/fs-wasm/test.mjs crates/fs-wasm/rounded-cube.fs roundedCube` builds a filleted cube (7855.841 mm³, in
half a second); `node crates/fs-wasm/test.mjs examples/showcase.fs showcase /tmp/showcase '{"pins": 5}'` prints the
showcase part's parameters and builds it with five pins (in about 3 s), copying its STL out.

## How it differs from the native engine

- No threads: the evaluator's deep recursion runs on the module's own 64 MB stack (`-sSTACK_SIZE`) rather than a
  thread's (`fs_interp::run_with_large_stack`); memory starts at 128 MB and grows.
- C++ exceptions are WebAssembly's (`-fwasm-exceptions`) throughout: OCCT, the bridge and the link.
- OCCT links statically, and the visualization toolkits XCAF needs (TKService, TKV3d, TKVCAF) with it.
- Fonts are read from `/fonts/` in the virtual file system when text first needs them, not compiled in.
- Results match the native engine's to within about a millionth of a part's volume: OpenCascade's sweeps differ that
  little with Emscripten's maths library (the showcase's swept handle, 0.064 mm³ in 109980 mm³); designs without
  such curves match exactly.

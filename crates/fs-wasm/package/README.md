# featurescript-engine (WebAssembly)

The FeatureScript engine (github.com/asib/featurescript-engine) compiled to WebAssembly, packaged for a page or a
worker. `index.js` exports `createEngine({ locateFile, loadFont })`, whose `parameters(source)` reads a Feature
Studio's feature parameters and `build(source, values)` builds the feature into an STL and a STEP; see `index.d.ts`.

`model(source, values)` builds the feature and holds it, for a page that shows it before it's all written out:
`mesh(0.05)` gives a coarse STL in a quarter of the time the fine one takes (`mesh()`, to 0.01 mm), and `step()` and
`volumes()` the rest, each when asked. Keep one engine for a design's rebuilds: it keeps std and the Feature Studio
loaded, and the state each operation left, so a rebuild runs only from the first operation its new values change
(a part whose label is engraved last builds with a new label in under half the time). The first build on a new
engine takes longer than the ones after.

- `fs_wasm.js`, `fs_wasm.wasm`: the engine (Rust, and OpenCascade 7.9.2, LGPL-2.1 with its exception:
  `OCCT-LICENSE_LGPL_21.txt`, `OCCT_LGPL_EXCEPTION.txt`), built by `crates/fs-wasm/pack.sh` at `ENGINE_COMMIT`.
- `fs_wasm.data`: Onshape's FeatureScript standard library (std), which the engine runs.
- `fonts/`: the fonts sketch text draws with (`fonts/README.md` gives their versions and licences), loaded only when
  a design's text uses them.

Lengths are millimetres and angles degrees. Parts match the native engine's to within about a millionth of their
volume (OpenCascade's results differ that little between WebAssembly's maths library and Linux's).

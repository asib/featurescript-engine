// Runs Feature Studios through the WebAssembly engine in Node: each file's feature, its STL and STEP written beside
// it in the virtual file system, and the log and sizes printed.
//
// Usage: node crates/fs-wasm/test.mjs FILE.fs FEATURE [OUT_STEM]
//
// With OUT_STEM, the STL is copied out of the virtual file system to OUT_STEM.stl. The engine is
// target/wasm32-unknown-emscripten/release/deps/fs_wasm.js, beside its .wasm and std's .data (see crates/fs-wasm/README.md).

import { readFileSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const [file, feature, out] = process.argv.slice(2);
if (!file || !feature) {
  console.error('usage: node crates/fs-wasm/test.mjs FILE.fs FEATURE [OUT_STEM]');
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const enginePath = resolve(here, '../../target/wasm32-unknown-emscripten/release/deps/fs_wasm.js');
const createFeatureScriptEngine = createRequire(import.meta.url)(enginePath);
const started = Date.now();
const engine = await createFeatureScriptEngine({ locateFile: (name) => resolve(dirname(enginePath), name) });
const loaded = Date.now();
engine.FS.mkdirTree('/work');
const name = basename(file);
engine.FS.writeFile(`/work/${name}`, readFileSync(file, 'utf8'));
const code = engine.ccall('fs_build', 'number', ['string', 'string', 'string'], [`/work/${name}`, feature, '/work/out']);
const built = Date.now();
console.log(engine.FS.readFile('/work/out.log', { encoding: 'utf8' }).trim());
if (code === 0) {
  const stl = engine.FS.readFile('/work/out.stl');
  const step = engine.FS.readFile('/work/out.step');
  console.log(`STL ${stl.length} bytes (${(stl.length - 84) / 50} triangles), STEP ${step.length} bytes`);
  if (out) writeFileSync(`${out}.stl`, stl);
}
console.log(`engine loaded in ${loaded - started} ms, ${feature} built in ${built - loaded} ms`);
process.exit(code);

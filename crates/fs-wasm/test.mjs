// Runs Feature Studios through the WebAssembly engine in Node: each file's feature, its STL and STEP written beside
// it in the virtual file system, and the log and sizes printed.
//
// Usage: node crates/fs-wasm/test.mjs FILE.fs FEATURE [OUT_STEM] [VALUES]
//
// With OUT_STEM, the STL is copied out of the virtual file system to OUT_STEM.stl. VALUES is a JSON object of
// parameter values. The engine is target/wasm32-unknown-emscripten/wasm/deps/fs_wasm.js (or FS_WASM_PROFILE's
// directory in place of wasm), beside its .wasm and std's .data (see crates/fs-wasm/README.md). Fonts the design
// names are copied from crates/fs-geom/fonts into the virtual file system first, as a page would fetch them.

import { readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const [file, feature, out, values = '{}'] = process.argv.slice(2);
if (!file || !feature) {
  console.error('usage: node crates/fs-wasm/test.mjs FILE.fs FEATURE [OUT_STEM]');
  process.exit(2);
}
const here = dirname(fileURLToPath(import.meta.url));
const enginePath = resolve(here, `../../target/wasm32-unknown-emscripten/${process.env.FS_WASM_PROFILE ?? 'wasm'}/deps/fs_wasm.js`);
const { default: createFeatureScriptEngine } = await import(pathToFileURL(enginePath).href);
const started = Date.now();
const engine = await createFeatureScriptEngine({ locateFile: (name) => resolve(dirname(enginePath), name) });
const loaded = Date.now();
engine.FS.mkdirTree('/work');
engine.FS.mkdirTree('/fonts');
const name = basename(file);
const source = readFileSync(file, 'utf8');
engine.FS.writeFile(`/work/${name}`, source);
// The fonts its sketch text names, and Open Sans, which text uses by default.
const fonts = new Set([...source.matchAll(/"fontName"\s*:\s*"([^"]+)"/g)].map((m) => m[1]));
if (/skText\s*\(/.test(source)) fonts.add('OpenSans-Regular.ttf');
for (const font of fonts) {
  try {
    engine.FS.writeFile(`/fonts/${font}`, readFileSync(resolve(here, '../fs-geom/fonts', font)));
  } catch {}
}
engine.FS.writeFile('/work/parameters.json', '');
engine.ccall('fs_parameters', 'number', ['string', 'string', 'string'], [`/work/${name}`, feature, '/work/parameters.json']);
console.log(engine.FS.readFile('/work/parameters.json', { encoding: 'utf8' }).slice(0, 400));
const code = engine.ccall('fs_build', 'number', ['string', 'string', 'string', 'string'], [`/work/${name}`, feature, values, '/work/out']);
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

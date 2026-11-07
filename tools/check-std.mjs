// Checks a copy of std made by tools/browser/fetch-std.mjs.
//
// Usage: node tools/check-std.mjs <std-dir>
//
// It checks that every file in the manifest exists with the recorded hash, that every file declares the same
// FeatureScript version, and that every std import names a file that is present, at that same version.

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node tools/check-std.mjs <std-dir>');
  process.exit(2);
}

const manifest = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8'));
const expectedVersion = manifest.versionName.replace(/\.0$/, '');
const problems = [];
const names = Object.keys(manifest.files);

for (const name of names) {
  const path = join(dir, name);
  if (!existsSync(path)) {
    problems.push(`${name}: in the manifest but missing`);
    continue;
  }
  const text = readFileSync(path, 'utf8');
  const hash = createHash('sha256').update(text).digest('hex');
  if (hash !== manifest.files[name].sha256) problems.push(`${name}: hash differs from the manifest`);

  const header = text.match(/^FeatureScript (\d+);/);
  if (!header) problems.push(`${name}: no FeatureScript header`);
  else if (header[1] !== expectedVersion) problems.push(`${name}: header says ${header[1]}, expected ${expectedVersion}`);

  for (const match of text.matchAll(/import\s*\(\s*path\s*:\s*"([^"]+)"\s*,\s*version\s*:\s*"([^"]+)"\s*\)/g)) {
    const [, importPath, importVersion] = match;
    if (!importPath.startsWith('onshape/std/')) continue;
    const target = importPath.slice('onshape/std/'.length);
    if (!manifest.files[target]) problems.push(`${name}: imports ${importPath}, which is not in the copy`);
    if (importVersion !== manifest.versionName) problems.push(`${name}: imports ${importPath} at ${importVersion}`);
  }
}

const onDisk = readdirSync(dir).filter((f) => f.endsWith('.fs'));
for (const file of onDisk) if (!manifest.files[file]) problems.push(`${file}: on disk but not in the manifest`);
if (!existsSync(join(dir, 'LICENSE'))) problems.push('LICENSE is missing');

const lines = names.reduce((sum, name) => sum + manifest.files[name].lines, 0);
console.log(`${names.length} files, ${lines} lines, version ${manifest.versionName}`);
if (problems.length) {
  console.log(`${problems.length} problems:\n  ${problems.join('\n  ')}`);
  process.exit(1);
}
console.log('no problems');

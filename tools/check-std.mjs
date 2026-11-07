// Checks a copy of std made by tools/browser/fetch-std.mjs.
//
// Usage: node tools/check-std.mjs <std-dir>
//
// It checks that every file in the manifest exists with the recorded hash, and that the version numbers fit
// together.
//
// Since std 2985, Onshape bumps a file's `FeatureScript` header (and the versions in its imports) only when that
// file or something it imports changes, so headers range from 2985 up to the copied version. The copy is
// self-consistent when every std import names a copied file whose header is no newer than the import's version:
// the file then has not changed since that version, so the copy holds exactly what the import means.

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: node tools/check-std.mjs <std-dir>');
  process.exit(2);
}

const manifest = JSON.parse(readFileSync(join(dir, 'manifest.json'), 'utf8'));
const copiedVersion = Number.parseFloat(manifest.versionName);
const problems = [];
const names = Object.keys(manifest.files);
const texts = new Map();
const headers = new Map();

for (const name of names) {
  const path = join(dir, name);
  if (!existsSync(path)) {
    problems.push(`${name}: in the manifest but missing`);
    continue;
  }
  const text = readFileSync(path, 'utf8');
  texts.set(name, text);
  const hash = createHash('sha256').update(text).digest('hex');
  if (hash !== manifest.files[name].sha256) problems.push(`${name}: hash differs from the manifest`);

  const header = text.match(/^FeatureScript (\d+);/);
  if (!header) problems.push(`${name}: no FeatureScript header`);
  else if (Number(header[1]) > copiedVersion) problems.push(`${name}: header ${header[1]} is newer than the copy`);
  else headers.set(name, Number(header[1]));
}

for (const [name, text] of texts) {
  for (const match of text.matchAll(/import\s*\(\s*path\s*:\s*"([^"]+)"\s*,\s*version\s*:\s*"([^"]+)"\s*\)/g)) {
    const [, importPath, importVersionText] = match;
    if (!importPath.startsWith('onshape/std/')) continue;
    const target = importPath.slice('onshape/std/'.length);
    const importVersion = Number.parseFloat(importVersionText);
    if (!manifest.files[target]) problems.push(`${name}: imports ${importPath}, which is not in the copy`);
    else if (importVersion > copiedVersion) problems.push(`${name}: imports ${importPath} at ${importVersionText}`);
    else if (headers.get(target) > importVersion) {
      problems.push(`${name}: imports ${importPath} at ${importVersionText}, but the copy is from ${headers.get(target)}`);
    }
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

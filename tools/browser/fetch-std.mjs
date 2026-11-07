// Copies every Feature Studio of one version of Onshape's public std document by reading the editor of each tab in
// the logged-in oracle Chrome. This is ordinary browsing of the document: it makes no Onshape API calls.
//
// Usage: node tools/browser/fetch-std.mjs <version-id> <version-name> <out-dir>
//   version-id    the id in the document URL when viewing that version (.../v/<version-id>/...)
//   version-name  the version's name in Versions and history, such as 3112.0
//   out-dir       where to write the files and manifest.json; existing files are kept, so a rerun only fetches
//                 what is missing
//
// Find the version id by opening Versions and history in the std document and clicking the version.

import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { connectToPage, sleep } from './cdp.mjs';

const STD_DOCUMENT_ID = '12312312345abcabcabcdeff';
const PAUSE_BETWEEN_TABS_MS = 1500; // browse at roughly a person's pace
const RELOAD_EVERY_TABS = 60; // reload now and then so loaded tabs don't pile up in the page
const LIMIT = Number(process.env.FETCH_LIMIT ?? Infinity); // fetch at most this many new files (for trying it out)

const [versionId, versionName, outDir] = process.argv.slice(2);
if (!versionId || !versionName || !outDir) {
  console.error('usage: node tools/browser/fetch-std.mjs <version-id> <version-name> <out-dir>');
  process.exit(2);
}
mkdirSync(outDir, { recursive: true });

const versionUrl = `https://cad.onshape.com/documents/${STD_DOCUMENT_ID}/v/${versionId}`;
const page = await connectToPage({ urlIncludes: 'cad.onshape.com' });

async function openVersion() {
  await page.navigate(versionUrl);
  await page.waitFor(`document.querySelectorAll('tab-list-item.os-tab-bar-tab').length > 0`, { timeoutMs: 90000 });
  await sleep(3000);
}

async function listTabs() {
  return page.evaluate(`[...document.querySelectorAll('tab-list-item.os-tab-bar-tab')].map((t) => ({
    id: t.getAttribute('data-id'),
    kind: t.getAttribute('data-icon-src'),
    name: t.querySelector('.os-tab-name')?.textContent.trim(),
  }))`);
}

// Switches to a tab the way a click would. A bare element.click() is ignored by the tab bar; it needs the full
// pointer and mouse event sequence.
async function selectTab(tabId) {
  await page.evaluate(`(() => {
    const tab = document.querySelector('tab-list-item.os-tab-bar-tab[data-id="${tabId}"]');
    const target = tab.querySelector('.tab-content-wrapper') || tab;
    const r = target.getBoundingClientRect();
    const opts = { bubbles: true, cancelable: true, view: window, clientX: r.x + 5, clientY: r.y + 5, button: 0 };
    for (const type of ['pointerdown', 'mousedown', 'pointerup', 'mouseup', 'click']) {
      target.dispatchEvent(type.startsWith('pointer') ? new PointerEvent(type, opts) : new MouseEvent(type, opts));
    }
  })()`);
  await page.waitFor(`location.pathname.endsWith('/e/${tabId}')`, { timeoutMs: 30000 });
}

// Returns the text of the visible Ace editor once it has loaded and stopped changing.
async function readEditor() {
  const expression = `(() => {
    const el = [...document.querySelectorAll('.ace_editor')].find((e) => e.offsetParent !== null);
    return el && window.ace ? window.ace.edit(el).getValue() : null;
  })()`;
  let previous = null;
  for (let attempt = 0; attempt < 120; attempt++) {
    const text = await page.evaluate(expression);
    if (text && text === previous) return text;
    previous = text;
    await sleep(500);
  }
  throw new Error('editor text did not settle');
}

// The UI shows a text tab as an <object> holding the file it already loaded; read the text from that.
async function readLicense() {
  return page.evaluate(`(() => {
    const texts = [...document.querySelectorAll('object')]
      .map((o) => o.contentDocument?.body?.textContent ?? '')
      .filter((t) => /MIT License/.test(t));
    return texts[0] ?? null;
  })()`);
}

const manifestPath = join(outDir, 'manifest.json');
const manifest = existsSync(manifestPath)
  ? JSON.parse(readFileSync(manifestPath, 'utf8'))
  : { document: STD_DOCUMENT_ID, versionId, versionName, files: {} };
const saveManifest = () => writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + '\n');

await openVersion();
const tabs = await listTabs();
const studios = tabs.filter((t) => t.kind === 'feature-studio-element');
console.log(`${tabs.length} tabs, ${studios.length} Feature Studios`);

const license = tabs.find((t) => t.name === 'LICENSE');
if (license && !existsSync(join(outDir, 'LICENSE'))) {
  await selectTab(license.id);
  await sleep(2000);
  const text = await readLicense();
  if (text) writeFileSync(join(outDir, 'LICENSE'), text.endsWith('\n') ? text : text + '\n');
  else console.error('could not read the LICENSE tab');
}

const failures = [];
let sinceReload = 0;
let fetched = 0;
for (const [index, tab] of studios.entries()) {
  const path = join(outDir, tab.name);
  if (existsSync(path) && manifest.files[tab.name]) continue;
  if (fetched >= LIMIT) break;
  if (sinceReload >= RELOAD_EVERY_TABS) {
    await openVersion();
    sinceReload = 0;
  }
  try {
    await selectTab(tab.id);
    const text = await readEditor();
    if (!text.startsWith('FeatureScript ')) throw new Error(`unexpected start: ${JSON.stringify(text.slice(0, 40))}`);
    writeFileSync(path, text);
    manifest.files[tab.name] = {
      elementId: tab.id,
      sha256: createHash('sha256').update(text).digest('hex'),
      lines: text.split('\n').length,
    };
    saveManifest();
    fetched++;
    console.log(`[${index + 1}/${studios.length}] ${tab.name} (${manifest.files[tab.name].lines} lines)`);
  } catch (error) {
    failures.push(tab.name);
    console.error(`[${index + 1}/${studios.length}] ${tab.name} FAILED: ${error.message}`);
  }
  sinceReload++;
  await sleep(PAUSE_BETWEEN_TABS_MS);
}

manifest.fetchedAt = new Date().toISOString();
saveManifest();
page.close();
console.log(failures.length ? `done with ${failures.length} failures: ${failures.join(', ')}` : 'done, no failures');
process.exit(failures.length ? 1 : 0);

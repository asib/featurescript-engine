// Builds a fixture batch in Onshape and exports it as STEP, through the UI of the logged-in oracle Chrome. Makes
// no API calls.
//
// Usage: node tools/browser/build-fixture-batch.mjs <batch-dir> <document-url>
//
//   batch-dir     e.g. fixtures/batch-001, holding cases.fs and generator.fs
//   document-url  the fixture document, e.g. https://cad.onshape.com/documents/<did>/w/<wid>
//
// One-time setup in the document: a Feature Studio tab named "Feature Studio 1", and a Part Studio tab named
// "Part Studio 1" holding one instance of the batch's generator feature. (The first run pastes the generator, so
// insert the feature after it: it then rebuilds on every later paste.)
//
// Steps: paste cases.fs + generator.fs into the Feature Studio and commit; wait for the Part Studio to rebuild and
// report its part count and failed cases, and save the probe lines to <batch-dir>/onshape-probes.txt; export the Part Studio as STEP (AP242, no preprocessing, one file); move
// the download to <batch-dir>/onshape.step.

import { existsSync, readdirSync, readFileSync, renameSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { connectToPage, sleep } from './cdp.mjs';

const [batchDir, documentUrl] = process.argv.slice(2);
if (!batchDir || !documentUrl) {
  console.error('usage: node tools/browser/build-fixture-batch.mjs <batch-dir> <document-url>');
  process.exit(2);
}
const FEATURE_STUDIO = 'Feature Studio 1';
const PART_STUDIO = 'Part Studio 1';
const EXPORT_NAME = `fixture-${Date.now()}`;
// Chrome saves downloads here; Browser.setDownloadBehavior doesn't outlive the connection that sets it.
const DOWNLOADS = join(homedir(), 'Downloads');

const source = readFileSync(join(batchDir, 'cases.fs'), 'utf8') + readFileSync(join(batchDir, 'generator.fs'), 'utf8');
const page = await connectToPage({ urlIncludes: 'cad.onshape.com' });

const tab = (name) =>
  `[...document.querySelectorAll('tab-list-item.os-tab-bar-tab')]
     .find((t) => t.querySelector('.os-tab-name')?.textContent.trim().replace(/^\\*/, '') === ${JSON.stringify(name)})`;
const visibleEditor = `[...document.querySelectorAll('.ace_editor')].find((e) => e.offsetParent !== null)`;

console.log('opening the document');
await page.navigate(documentUrl);
await page.waitFor(`!!(${tab(FEATURE_STUDIO)})`, { timeoutMs: 90000 });
await sleep(3000);

console.log('pasting the cases and generator');
await page.clickElement(`${tab(FEATURE_STUDIO)}.querySelector('.tab-content-wrapper')`);
await page.waitFor(`!!(${visibleEditor})`);
await sleep(1500);
const unchanged = await page.evaluate(`window.ace.edit(${visibleEditor}).getValue() === ${JSON.stringify(source)}`);
if (unchanged) {
  console.log('  the Feature Studio already has this text');
} else {
  await page.evaluate(`(() => { const editor = window.ace.edit(${visibleEditor}); editor.focus(); editor.setValue(${JSON.stringify(source)}, -1); })()`);
  await sleep(3000);
  await page.clickElement(
    `[...document.querySelectorAll('span.tool-label')].find((e) => e.offsetParent !== null && e.textContent.trim() === 'Commit').closest('div.tool')`,
  );
  await page.waitFor(`!${tab(FEATURE_STUDIO)}.querySelector('.os-tab-name').textContent.trim().startsWith('*')`, {
    timeoutMs: 60000,
  });
}

console.log('waiting for the Part Studio to rebuild');
await page.clickElement(`${tab(PART_STUDIO)}.querySelector('.tab-content-wrapper')`);
let parts = null;
for (let stable = 0, last = null; stable < 3; ) {
  await sleep(5000);
  parts = await page.evaluate(
    `[...document.querySelectorAll('*')].find((e) => e.children.length === 0 && /^Parts \\(\\d+\\)$/.test(e.textContent.trim()))?.textContent.trim()`,
  );
  stable = parts && parts === last ? stable + 1 : 0;
  last = parts;
}
console.log(`  ${parts}`);
// The generator prints one "probe <case> ..." line per case and a "batch-NNN failed cases: [...]" line at the end;
// they show in the FeatureScript notices panel, among the stack traces of the cases that threw.
if (!(await page.evaluate(`!!document.querySelector('.notices-content')`))) {
  await page.clickElement(`document.querySelector('.notice-pane-toggle-button .notices-toggle-content')`);
  await page.waitFor(`!!document.querySelector('.notices-content')`);
  await sleep(2000);
}
const notices = await page.evaluate(`document.querySelector('.notices-content').innerText`);
const probes = new Map();
let failed = 'not shown';
for (const line of notices.split('\n').map((l) => l.trim())) {
  const probe = line.match(/^probe (c\d+) /);
  if (probe) probes.set(probe[1], line);
  if (/^batch-\d+ failed cases: /.test(line)) failed = line;
}
console.log(`  ${failed}; ${probes.size} probe lines`);
const probeFile = join(batchDir, 'onshape-probes.txt');
writeFileSync(probeFile, [...[...probes.keys()].sort().map((k) => probes.get(k)), failed].join('\n') + '\n');
console.log(`  saved ${probeFile}`);

console.log('exporting STEP');
const box = await page.evaluate(`(() => { const r = ${tab(PART_STUDIO)}.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
await page.clickAt(box.x, box.y, { button: 'right' });
await sleep(1000);
await page.clickElement(`[...document.querySelectorAll('li, [role=menuitem], .dropdown-item, a')].find((e) => e.offsetParent !== null && e.textContent.trim() === 'Export…')`);
await page.waitFor(
  `[...(document.querySelector('#export-format-dropdown')?.options ?? [])].some((o) => o.label.trim() === 'STEP')`,
);
const chooseOption = (selector, label) =>
  page.evaluate(`(() => {
    const s = document.querySelector(${JSON.stringify(selector)});
    const i = [...s.options].findIndex((o) => o.label.trim() === ${JSON.stringify(label)});
    if (i < 0) throw new Error('no option ' + ${JSON.stringify(label)});
    s.selectedIndex = i;
    s.dispatchEvent(new Event('change', { bubbles: true }));
  })()`);
await chooseOption('#export-format-dropdown', 'STEP');
await sleep(1000);
await chooseOption('#step-parasolid-preprocessing-on-read', 'None');
await chooseOption('#step-export-version-dropdown', 'AP242');
await chooseOption('#export-options-dropdown', 'Download');
// One file for the whole Part Studio: Onshape sometimes offers each part as its own file (in a zip).
await page.evaluate(`(() => {
  const c = [...document.querySelectorAll('input[type=checkbox]')].find((c) => c.offsetParent !== null &&
    (c.closest('label')?.innerText || c.parentElement?.innerText || '').includes('individual files'));
  if (c && c.checked) c.click();
})()`);
await sleep(500);
await page.evaluate(`(() => { const i = document.querySelector('#export-filename-input'); i.focus(); i.select(); })()`);
await page.send('Input.insertText', { text: EXPORT_NAME });
await page.clickElement(`[...document.querySelectorAll('button')].find((b) => b.offsetParent !== null && b.textContent.trim() === 'Export')`);

const downloaded = join(DOWNLOADS, `${EXPORT_NAME}.step`);
const deadline = Date.now() + 5 * 60 * 1000;
while (!(existsSync(downloaded) && statSync(downloaded).size > 0)) {
  if (Date.now() > deadline) throw new Error(`no download at ${downloaded}; files: ${readdirSync(DOWNLOADS).join(', ')}`);
  await sleep(2000);
}
await sleep(1000);
const target = join(batchDir, 'onshape.step');
renameSync(downloaded, target);
console.log(`saved ${target} (${statSync(target).size} bytes)`);
page.close();

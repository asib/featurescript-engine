// Exports a fixture document's Part Studio as STEP, through the UI of the logged-in oracle Chrome, without pasting or
// rebuilding anything: for when build-fixture-batch.mjs saved the probes but its export failed. Makes no API calls.
//
// Usage: node tools/browser/export-step.mjs <batch-dir> <document-url> [<part-studio> <file>]
//
// Moves the download to <batch-dir>/onshape.step, or with a Part Studio's tab name and a file name, exports that
// Part Studio to <batch-dir>/<file> (Onshape's own import of an uploaded file, fixtures/blobs).

import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { connectToPage, sleep } from './cdp.mjs';

const [batchDir, documentUrl, partStudioName, fileName] = process.argv.slice(2);
if (!batchDir || !documentUrl) {
  console.error('usage: node tools/browser/export-step.mjs <batch-dir> <document-url>');
  process.exit(2);
}
const FEATURE_STUDIO = 'Feature Studio 1';
const PART_STUDIO = partStudioName ?? 'Part Studio 1';
const EXPORT_NAME = `fixture-${Date.now()}`;
// Chrome saves downloads here; Browser.setDownloadBehavior doesn't outlive the connection that sets it.
const DOWNLOADS = join(homedir(), 'Downloads');

// While this runs the keep-alive (keep-alive.mjs) leaves the tab alone, so its nudges can't land inside a click.
const BUSY = join(homedir(), '.local/share/fs-engine-oracle/busy');
mkdirSync(join(homedir(), '.local/share/fs-engine-oracle'), { recursive: true });
writeFileSync(BUSY, String(process.pid));
process.on('exit', () => rmSync(BUSY, { force: true }));

const page = await connectToPage({ urlIncludes: 'cad.onshape.com' });

const tab = (name) =>
  `[...document.querySelectorAll('tab-list-item.os-tab-bar-tab')]
     .find((t) => t.querySelector('.os-tab-name')?.textContent.trim().replace(/^\\*/, '') === ${JSON.stringify(name)})`;

console.log('opening the document');
await page.navigate(documentUrl);
await page.waitFor(`!!(${tab(PART_STUDIO)})`, { timeoutMs: 90000 });
await sleep(5000);
await page.clickElement(`${tab(PART_STUDIO)}.querySelector('.tab-content-wrapper')`);
await sleep(5000);

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
const target = join(batchDir, fileName ?? 'onshape.step');
renameSync(downloaded, target);
console.log(`saved ${target} (${statSync(target).size} bytes)`);
page.close();

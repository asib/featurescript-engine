// Keeps the oracle Chrome's Onshape session alive: every few minutes it moves the mouse over the Onshape tab and
// taps Shift, as a person resting at the keyboard would, and logs whether the tab is still signed in. It sends no
// requests of its own; whatever Onshape's page does when it sees a user active is all that reaches Onshape.
//
// Usage: node tools/browser/keep-alive.mjs [minutes between nudges, default 5]
// Runs as the sprite service `onshape-keepalive`.

import { existsSync, statSync } from 'node:fs';
import { homedir } from 'node:os';
import { join } from 'node:path';
import { connectToPage, sleep } from './cdp.mjs';

const minutes = Number(process.argv[2] ?? 5);
// A fixture build holds this while it drives the tab (build-fixture-batch.mjs); one older than an hour is stale.
const BUSY = join(homedir(), '.local/share/fs-engine-oracle/busy');
let step = 0;

for (;;) {
  const busy = existsSync(BUSY) && Date.now() - statSync(BUSY).mtimeMs < 60 * 60 * 1000;
  if (busy) {
    console.log(`${new Date().toISOString()} a fixture build is using the tab: skipped`);
    await sleep(minutes * 60 * 1000);
    continue;
  }
  // Each nudge gets a minute: a CDP call that never answers (the loop once hung that way for hours, logging nothing)
  // must not stall the loop.
  let page;
  try {
    const nudge = async () => {
      page = await connectToPage({ urlIncludes: 'onshape' });
      // A small move inside the page's lower right corner, away from the toolbars, then a Shift tap.
      const x = 900 + (step % 2) * 20;
      const y = 700 + (step % 3) * 10;
      await page.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
      await page.send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Shift', code: 'ShiftLeft', windowsVirtualKeyCode: 16 });
      await page.send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Shift', code: 'ShiftLeft', windowsVirtualKeyCode: 16 });
      return { title: await page.evaluate('document.title'), href: await page.evaluate('location.href') };
    };
    let timer;
    const timeout = new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('timed out after 60 s')), 60 * 1000);
    });
    const { title, href } = await Promise.race([nudge(), timeout]).finally(() => clearTimeout(timer));
    // Any Onshape tab on the sign-in page means the session is gone: a tab opened before it went keeps showing the
    // document it had, so the nudged tab alone can look signed in when it isn't.
    const endpoint = process.env.CDP_ENDPOINT ?? 'http://127.0.0.1:9222';
    const tabs = (await (await fetch(`${endpoint}/json/list`)).json()).filter((t) => t.type === 'page' && t.url.includes('onshape'));
    const signedOut = href.includes('/signin') || tabs.some((t) => t.url.includes('/signin'));
    console.log(`${new Date().toISOString()} ${signedOut ? 'SIGNED OUT' : 'signed in'}: ${title} (${tabs.length} Onshape tabs)`);
  } catch (error) {
    console.log(`${new Date().toISOString()} no Onshape tab reachable: ${error.message}`);
  } finally {
    try {
      page?.close();
    } catch {}
  }
  step += 1;
  await sleep(minutes * 60 * 1000);
}

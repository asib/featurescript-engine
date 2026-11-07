// Minimal Chrome DevTools Protocol client for the logged-in oracle Chrome on this machine.
// Needs no npm packages: Node's built-in fetch and WebSocket are enough.
//
// The Chrome debugging port only listens on 127.0.0.1. Nothing here calls Onshape's API directly; scripts drive
// the Onshape web UI the same way a person would.

import { writeFileSync } from 'node:fs';

const DEFAULT_ENDPOINT = process.env.CDP_ENDPOINT ?? 'http://127.0.0.1:9222';

export async function connectToPage({ endpoint = DEFAULT_ENDPOINT, urlIncludes } = {}) {
  const targets = await (await fetch(`${endpoint}/json/list`)).json();
  const pages = targets.filter((t) => t.type === 'page');
  const target = urlIncludes ? pages.find((t) => t.url.includes(urlIncludes)) : pages[0];
  if (!target) throw new Error(`no page target${urlIncludes ? ` with URL containing ${urlIncludes}` : ''}`);
  return Page.connect(target.webSocketDebuggerUrl);
}

// Makes Chrome save downloads into `dir` without asking, and report their progress.
export async function setDownloadDir(dir, { endpoint = DEFAULT_ENDPOINT } = {}) {
  const { webSocketDebuggerUrl } = await (await fetch(`${endpoint}/json/version`)).json();
  const browser = await Page.connect(webSocketDebuggerUrl);
  await browser.send('Browser.setDownloadBehavior', { behavior: 'allow', downloadPath: dir, eventsEnabled: true });
  browser.close();
}

export class Page {
  static async connect(wsUrl) {
    const ws = new WebSocket(wsUrl);
    await new Promise((resolve, reject) => {
      ws.onopen = resolve;
      ws.onerror = reject;
    });
    return new Page(ws);
  }

  constructor(ws) {
    this.ws = ws;
    this.nextId = 1;
    this.pending = new Map();
    this.listeners = new Map();
    ws.onmessage = (message) => {
      const data = JSON.parse(message.data);
      if (data.id && this.pending.has(data.id)) {
        const { resolve, reject } = this.pending.get(data.id);
        this.pending.delete(data.id);
        if (data.error) reject(new Error(`${data.error.message} (${data.error.code})`));
        else resolve(data.result);
      } else if (data.method) {
        for (const listener of this.listeners.get(data.method) ?? []) listener(data.params);
      }
    };
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  once(method, timeoutMs = 60000) {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`timed out waiting for ${method}`)), timeoutMs);
      const listener = (params) => {
        clearTimeout(timer);
        this.listeners.set(method, (this.listeners.get(method) ?? []).filter((l) => l !== listener));
        resolve(params);
      };
      this.listeners.set(method, [...(this.listeners.get(method) ?? []), listener]);
    });
  }

  // Evaluates an expression in the page and returns its value (awaiting promises).
  async evaluate(expression) {
    const result = await this.send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) {
      const description = result.exceptionDetails.exception?.description ?? result.exceptionDetails.text;
      throw new Error(`page evaluation failed: ${description}`);
    }
    return result.result.value;
  }

  async navigate(url) {
    await this.send('Page.enable');
    const loaded = this.once('Page.loadEventFired');
    await this.send('Page.navigate', { url });
    await loaded;
  }

  // Polls an expression in the page until it returns a truthy value.
  async waitFor(expression, { timeoutMs = 60000, intervalMs = 250 } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      const value = await this.evaluate(expression);
      if (value) return value;
      if (Date.now() > deadline) throw new Error(`timed out waiting for: ${expression}`);
      await sleep(intervalMs);
    }
  }

  // Clicks at page coordinates with real (trusted) mouse events, for controls that ignore synthetic ones.
  async clickAt(x, y, { button = 'left', clickCount = 1 } = {}) {
    await this.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y });
    await this.send('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button, clickCount });
    await this.send('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button, clickCount });
  }

  // Clicks the centre of the first visible element an in-page expression returns.
  async clickElement(expression, options) {
    const box = await this.evaluate(`(() => {
      const el = (${expression});
      if (!el) return null;
      el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      const r = el.getBoundingClientRect();
      return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
    })()`);
    if (!box) throw new Error(`nothing to click for: ${expression}`);
    await this.clickAt(box.x, box.y, options);
  }

  async pressKey(key, code = key, keyCode = 0) {
    for (const type of ['keyDown', 'keyUp']) {
      await this.send('Input.dispatchKeyEvent', { type, key, code, windowsVirtualKeyCode: keyCode });
    }
  }

  async screenshot(path) {
    const { data } = await this.send('Page.captureScreenshot', { format: 'png' });
    writeFileSync(path, Buffer.from(data, 'base64'));
  }

  close() {
    this.ws.close();
  }
}

export function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

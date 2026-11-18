// Renders a binary STL (in millimetres, z up) to a PNG with three.js in the oracle Chrome, as a picture of a part the
// engine built (examples/run_part writes the STL).
//
// Usage: node tools/render/render-stl.mjs PART.stl OUT.png
//
// Writes a page and the mesh beside OUT.png, opens it in a new tab of the Chrome on 127.0.0.1:9222, screenshots it at
// 1600 x 1100 and closes the tab. three.js comes from jsDelivr.

import { Page } from '../browser/cdp.mjs';
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';

const PAGE = "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>Desk tidy</title>\n<style>html,body{margin:0;height:100%;background:#eef1f5;overflow:hidden}canvas{display:block}</style>\n<script src=\"https://cdn.jsdelivr.net/npm/three@0.147.0/build/three.min.js\"></script>\n<script src=\"https://cdn.jsdelivr.net/npm/three@0.147.0/examples/js/loaders/STLLoader.js\"></script>\n<script src=\"mesh.js\"></script>\n</head><body>\n<script>\nconst bytes = Uint8Array.from(atob(window.MESH_B64), c => c.charCodeAt(0));\nconst geometry = new THREE.STLLoader().parse(bytes.buffer);\ngeometry.computeBoundingBox();\nconst box = geometry.boundingBox, size = new THREE.Vector3(), centre = new THREE.Vector3();\nbox.getSize(size); box.getCenter(centre);\nconst renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });\nrenderer.setPixelRatio(1);\nrenderer.setSize(window.innerWidth, window.innerHeight);\nrenderer.shadowMap.enabled = true;\nrenderer.shadowMap.type = THREE.PCFSoftShadowMap;\nrenderer.outputEncoding = THREE.sRGBEncoding;\ndocument.body.appendChild(renderer.domElement);\nconst scene = new THREE.Scene();\nscene.background = new THREE.Color(0xeef1f5);\nscene.add(new THREE.HemisphereLight(0xffffff, 0x9aa7b8, 0.65));\nconst sun = new THREE.DirectionalLight(0xffffff, 0.85);\nsun.position.set(centre.x - 60, centre.y - 110, centre.z + 160);\nsun.target.position.copy(centre);\nsun.castShadow = true;\nsun.shadow.mapSize.set(4096, 4096);\nconst r = Math.max(size.x, size.y) * 0.9;\nObject.assign(sun.shadow.camera, { left: -r, right: r, top: r, bottom: -r, near: 1, far: 600 });\nsun.shadow.bias = -0.0005;\nscene.add(sun, sun.target);\nconst fill = new THREE.DirectionalLight(0xdfe8ff, 0.35);\nfill.position.set(centre.x + 150, centre.y + 60, centre.z + 40);\nscene.add(fill);\nconst material = new THREE.MeshStandardMaterial({ color: 0x4f7cac, metalness: 0.2, roughness: 0.42 });\nconst mesh = new THREE.Mesh(geometry, material);\nmesh.castShadow = true; mesh.receiveShadow = true;\nscene.add(mesh);\nconst edges = new THREE.LineSegments(new THREE.EdgesGeometry(geometry, 28),\n  new THREE.LineBasicMaterial({ color: 0x1c2a3a, transparent: true, opacity: 0.55 }));\nscene.add(edges);\nconst ground = new THREE.Mesh(new THREE.PlaneGeometry(2000, 2000), new THREE.ShadowMaterial({ opacity: 0.18 }));\nground.position.set(centre.x, centre.y, box.min.z - 0.05);\nground.receiveShadow = true;\nscene.add(ground);\nconst camera = new THREE.PerspectiveCamera(30, window.innerWidth / window.innerHeight, 1, 5000);\ncamera.up.set(0, 0, 1);\nconst d = Math.max(size.x, size.y, size.z);\ncamera.position.set(centre.x + 0.55 * d, centre.y - 1.85 * d, centre.z + 1.45 * d);\ncamera.lookAt(centre.x, centre.y, centre.z - 0.05 * d);\nrenderer.render(scene, camera);\nwindow.renderDone = true;\n</script></body></html>";

const [stl, out] = process.argv.slice(2).map((p) => resolve(p));
if (!stl || !out) {
  console.error('usage: node tools/render/render-stl.mjs PART.stl OUT.png');
  process.exit(2);
}
const dir = dirname(out);
writeFileSync(join(dir, 'mesh.js'), `window.MESH_B64="${readFileSync(stl).toString('base64')}";`);
const html = join(dir, 'render.html');
writeFileSync(html, PAGE);

const endpoint = 'http://127.0.0.1:9222';
const { webSocketDebuggerUrl } = await (await fetch(`${endpoint}/json/version`)).json();
const browser = await Page.connect(webSocketDebuggerUrl);
const { targetId } = await browser.send('Target.createTarget', { url: 'about:blank' });
try {
  const target = (await (await fetch(`${endpoint}/json/list`)).json()).find((t) => t.id === targetId);
  const page = await Page.connect(target.webSocketDebuggerUrl);
  await page.send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 1100, deviceScaleFactor: 1, mobile: false });
  await page.navigate(`file://${html}`);
  await page.waitFor('window.renderDone === true', { timeoutMs: 60000 });
  const { data } = await page.send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(out, Buffer.from(data, 'base64'));
  page.ws.close();
  console.log(`saved ${out}`);
} finally {
  await fetch(`${endpoint}/json/close/${targetId}`);
  browser.ws.close();
}

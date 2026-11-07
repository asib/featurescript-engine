// Summarises the parts in a STEP file without any CAD library: for each named part, its bounding box (from its
// vertices) and counts of faces by surface type, edges and vertices.
//
// Usage: node tools/fixtures/step-summary.mjs <file.step> [--json]
//
// This is a quick check of a fixture, not the real comparison (which reads STEP with OpenCascade). It understands
// enough of STEP (ISO 10303-21) to follow references; it does not evaluate geometry.

import { readFileSync } from 'node:fs';

const [path, flag] = process.argv.slice(2);
if (!path) {
  console.error('usage: node tools/fixtures/step-summary.mjs <file.step> [--json]');
  process.exit(2);
}

// Parse "#id=TYPE(args);" records. Records can span lines; strings may contain ';' or '#'.
const text = readFileSync(path, 'utf8');
const data = text.slice(text.indexOf('DATA;') + 5, text.lastIndexOf('ENDSEC;'));
const entities = new Map();
let i = 0;
while (i < data.length) {
  const start = data.indexOf('#', i);
  if (start < 0) break;
  let j = start;
  let inString = false;
  while (j < data.length) {
    const c = data[j];
    if (c === "'") inString = !inString;
    else if (c === ';' && !inString) break;
    j++;
  }
  const record = data.slice(start, j);
  i = j + 1;
  const eq = record.indexOf('=');
  if (eq < 0) continue;
  const id = Number(record.slice(1, eq));
  const body = record.slice(eq + 1).trim();
  const withoutStrings = body.replace(/'(?:[^']|'')*'/g, "''");
  const type = body.startsWith('(') ? 'COMPLEX' : body.slice(0, body.indexOf('(')).trim();
  const refs = [...withoutStrings.matchAll(/#(\d+)/g)].map((m) => Number(m[1]));
  const strings = [...body.matchAll(/'((?:[^']|'')*)'/g)].map((m) => m[1].replace(/''/g, "'"));
  const numbers = [...withoutStrings.matchAll(/[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?/g)].map((m) => Number(m[0]));
  entities.set(id, { id, type, body, refs, strings, numbers });
}

const referencedBy = new Map();
for (const entity of entities.values()) {
  for (const ref of entity.refs) {
    if (!referencedBy.has(ref)) referencedBy.set(ref, []);
    referencedBy.get(ref).push(entity.id);
  }
}

function forward(fromId) {
  const seen = new Set([fromId]);
  const stack = [fromId];
  while (stack.length) {
    const entity = entities.get(stack.pop());
    for (const ref of entity?.refs ?? []) {
      if (!seen.has(ref)) {
        seen.add(ref);
        stack.push(ref);
      }
    }
  }
  return [...seen].map((id) => entities.get(id)).filter(Boolean);
}

// The part name for a solid: go up to the SHAPE_DEFINITION_REPRESENTATION, then down to its PRODUCT.
function productNameFor(solidId) {
  const seen = new Set([solidId]);
  let frontier = [solidId];
  for (let depth = 0; depth < 6 && frontier.length; depth++) {
    const next = [];
    for (const id of frontier) {
      for (const parent of referencedBy.get(id) ?? []) {
        if (seen.has(parent)) continue;
        seen.add(parent);
        const entity = entities.get(parent);
        if (entity.type === 'SHAPE_DEFINITION_REPRESENTATION') {
          const product = forward(parent).find((e) => e.type === 'PRODUCT');
          if (product) return product.strings[0];
        }
        if (['ADVANCED_BREP_SHAPE_REPRESENTATION', 'SHAPE_REPRESENTATION', 'SHAPE_REPRESENTATION_RELATIONSHIP', 'COMPLEX'].includes(entity.type)) {
          next.push(parent);
        }
      }
    }
    frontier = next;
  }
  return `#${solidId}`;
}

const parts = [];
for (const solid of entities.values()) {
  if (solid.type !== 'MANIFOLD_SOLID_BREP') continue;
  const closure = forward(solid.id);
  const vertices = closure.filter((e) => e.type === 'VERTEX_POINT');
  const points = vertices.map((v) => entities.get(v.refs[0])).filter((p) => p?.type === 'CARTESIAN_POINT');
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (const p of points) {
    const coords = p.numbers.slice(-3);
    for (let k = 0; k < 3; k++) {
      min[k] = Math.min(min[k], coords[k]);
      max[k] = Math.max(max[k], coords[k]);
    }
  }
  const faces = closure.filter((e) => e.type === 'ADVANCED_FACE');
  const surfaceTypes = {};
  for (const face of faces) {
    const surface = entities.get(face.refs[face.refs.length - 1]);
    const type = surface?.type ?? '?';
    surfaceTypes[type] = (surfaceTypes[type] ?? 0) + 1;
  }
  parts.push({
    // Onshape writes the part name on the solid itself; otherwise find it through the product structure.
    name: solid.strings[0] || productNameFor(solid.id),
    faces: faces.length,
    edges: closure.filter((e) => e.type === 'EDGE_CURVE').length,
    vertices: vertices.length,
    surfaceTypes,
    bboxMeters: points.length ? { min, max } : null,
  });
}
parts.sort((a, b) => a.name.localeCompare(b.name));

if (flag === '--json') {
  console.log(JSON.stringify(parts, null, 2));
} else {
  const mm = (v) => (v * 1000).toFixed(3);
  for (const p of parts) {
    const size = p.bboxMeters ? p.bboxMeters.max.map((v, k) => mm(v - p.bboxMeters.min[k])).join(' x ') : '-';
    const types = Object.entries(p.surfaceTypes).map(([t, n]) => `${n} ${t.toLowerCase().replace(/_surface$/, '')}`).join(', ');
    console.log(`${p.name.padEnd(8)} ${String(p.faces).padStart(3)} faces ${String(p.edges).padStart(3)} edges ${String(p.vertices).padStart(3)} vertices  size ${size} mm  (${types})`);
  }
  console.log(`${parts.length} parts`);
}

// Generates fixture batch 001: about 100 sketch-and-extrude cases.
//
// Usage: node tools/fixtures/gen-batch-001.mjs
//
// Writes into fixtures/batch-001/:
//   cases.fs      one FeatureScript function per case. Runs unchanged in Onshape and on our engine.
//   generator.fs  the Onshape-only part: a feature that builds every case, moves each case's solids to its grid
//                 slot and names them after the case. Pasted into a Feature Studio after cases.fs.
//   cases.json    the manifest: each case's name, kind, description and grid slot.
//
// The output only depends on SEED, so rerunning gives the same files.

import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const SEED = 20261107;
const STD_VERSION = '3112.0';
const SLOT_SPACING_M = 0.4;
const SLOTS_PER_ROW = 10;
const OUT_DIR = new URL('../../fixtures/batch-001/', import.meta.url).pathname;

// mulberry32: small, deterministic PRNG.
let state = SEED >>> 0;
function random() {
  state = (state + 0x6d2b79f5) >>> 0;
  let t = state;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
const between = (min, max) => min + (max - min) * random();
const pick = (items) => items[Math.floor(random() * items.length)];
const mm = (value) => `${num(value)} * millimeter`;
const num = (value) => {
  const rounded = Math.round(value * 100) / 100;
  return Object.is(rounded, -0) ? '0' : String(rounded);
};
const vec2 = (x, y) => `vector(${num(x)}, ${num(y)}) * millimeter`;
const vec3 = (x, y, z) => `vector(${num(x)}, ${num(y)}, ${num(z)}) * millimeter`;

// Sketch planes through the origin, written out explicitly so neither side depends on default plane conventions.
const PLANES = {
  xy: 'plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0))',
  xz: 'plane(vector(0, 0, 0) * millimeter, vector(0, -1, 0), vector(1, 0, 0))',
  yz: 'plane(vector(0, 0, 0) * millimeter, vector(1, 0, 0), vector(0, 1, 0))',
  tilted: 'plane(vector(0, 0, 0) * millimeter, vector(0, -0.5, 0.8660254037844386), vector(1, 0, 0))',
};
const planeAtZ = (z) => `plane(${vec3(0, 0, z)}, vector(0, 0, 1), vector(1, 0, 0))`;

const cases = [];
function addCase(kind, description, body) {
  const index = cases.length;
  const name = `c${String(index + 1).padStart(3, '0')}`;
  const slot = [(index % SLOTS_PER_ROW) * SLOT_SPACING_M, Math.floor(index / SLOTS_PER_ROW) * SLOT_SPACING_M, 0];
  cases.push({ name, kind, description, slot, body });
}

// ----- Building blocks that emit FeatureScript statements -----

const sketch = (n, plane) =>
  `const sketch${n} = newSketchOnPlane(context, id + "sketch${n}", { "sketchPlane" : ${plane} });`;
const solve = (n) => `skSolve(sketch${n});`;
const rect = (n, name, x1, y1, x2, y2) =>
  `skRectangle(sketch${n}, "${name}", { "firstCorner" : ${vec2(x1, y1)}, "secondCorner" : ${vec2(x2, y2)} });`;
const circle = (n, name, cx, cy, r) =>
  `skCircle(sketch${n}, "${name}", { "center" : ${vec2(cx, cy)}, "radius" : ${mm(r)} });`;
const line = (n, name, [x1, y1], [x2, y2]) =>
  `skLineSegment(sketch${n}, "${name}", { "start" : ${vec2(x1, y1)}, "end" : ${vec2(x2, y2)} });`;
const arc = (n, name, [x1, y1], [xm, ym], [x2, y2]) =>
  `skArc(sketch${n}, "${name}", { "start" : ${vec2(x1, y1)}, "mid" : ${vec2(xm, ym)}, "end" : ${vec2(x2, y2)} });`;

function extrude(n, fields) {
  const entries = Object.entries(fields).map(([key, value]) => `"${key}" : ${value}`);
  return `extrude(context, id + "extrude${n}", { ${entries.join(', ')} });`;
}
const regions = (n, outerOnly = false) => `qSketchRegion(id + "sketch${n}"${outerOnly ? ', true' : ''})`;
const scopeOf = (n) => `qCreatedBy(id + "extrude${n}", EntityType.BODY)`;
const booleanFields = (operation, scopeExtrude) => ({
  operationType: `NewBodyOperationType.${operation}`,
  defaultScope: 'false',
  booleanScope: scopeOf(scopeExtrude),
});

// A random rectangle, as corner coordinates, roughly centred on the origin.
function randomRect(maxSize = 80) {
  const w = between(5, maxSize);
  const h = between(5, maxSize);
  const x = between(-w, 0);
  const y = between(-h, 0);
  return [x, y, x + w, y + h];
}

// A star-shaped simple polygon: vertices at increasing angles with random radii.
function randomPolygon() {
  const count = 3 + Math.floor(random() * 6);
  const offset = between(0, 2 * Math.PI);
  const points = [];
  for (let i = 0; i < count; i++) {
    const angle = offset + (2 * Math.PI * (i + between(0.15, 0.85))) / count;
    const radius = between(8, 45);
    points.push([radius * Math.cos(angle), radius * Math.sin(angle)]);
  }
  return points;
}

function polygonLines(n, points) {
  return points.map((p, i) => line(n, `line${i + 1}`, p, points[(i + 1) % points.length]));
}

// A slot: two straight sides joined by semicircular ends, along the x axis.
function slotEntities(n, length, width, cx, cy) {
  const r = width / 2;
  const x1 = cx - length / 2;
  const x2 = cx + length / 2;
  return [
    line(n, 'line1', [x1, cy - r], [x2, cy - r]),
    arc(n, 'arc1', [x2, cy - r], [x2 + r, cy], [x2, cy + r]),
    line(n, 'line2', [x2, cy + r], [x1, cy + r]),
    arc(n, 'arc2', [x1, cy + r], [x1 - r, cy], [x1, cy - r]),
  ];
}

// ----- Cases -----

for (let i = 0; i < 12; i++) {
  const planeName = pick(Object.keys(PLANES));
  const [x1, y1, x2, y2] = randomRect();
  const depth = between(1, 60);
  addCase('rect-blind', `rectangle on the ${planeName} plane, blind ${num(depth)} mm`, [
    sketch(1, PLANES[planeName]),
    rect(1, 'rect1', x1, y1, x2, y2),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 10; i++) {
  const planeName = pick(Object.keys(PLANES));
  const r = between(2, 40);
  const depth = between(1, 60);
  addCase('circle-blind', `circle r ${num(r)} on the ${planeName} plane, blind ${num(depth)} mm`, [
    sketch(1, PLANES[planeName]),
    circle(1, 'circle1', between(-20, 20), between(-20, 20), r),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 10; i++) {
  const points = randomPolygon();
  const depth = between(1, 50);
  addCase('polygon-blind', `${points.length}-sided star-shaped polygon, blind ${num(depth)} mm`, [
    sketch(1, PLANES.xy),
    ...polygonLines(1, points),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 8; i++) {
  const length = between(10, 60);
  const width = between(4, 30);
  const depth = between(1, 40);
  addCase('slot-blind', `slot ${num(length)} x ${num(width)} made of lines and arcs, blind ${num(depth)} mm`, [
    sketch(1, pick([PLANES.xy, PLANES.xz])),
    ...slotEntities(1, length, width, between(-10, 10), between(-10, 10)),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 8; i++) {
  const w = between(30, 90);
  const h = between(30, 90);
  const r = between(2, Math.min(w, h) / 2 - 4);
  const cx = between(-w / 2 + r + 2, w / 2 - r - 2);
  const cy = between(-h / 2 + r + 2, h / 2 - r - 2);
  const depth = between(2, 30);
  addCase('plate-with-hole', 'rectangle with a circular hole; only the outer region (filterInnerLoops) is extruded', [
    sketch(1, PLANES.xy),
    rect(1, 'rect1', -w / 2, -h / 2, w / 2, h / 2),
    circle(1, 'circle1', cx, cy, r),
    solve(1),
    extrude(1, { entities: regions(1, true), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 5; i++) {
  const r1 = between(10, 30);
  const r2 = between(10, 30);
  const gap = between(0.3, 0.9) * (r1 + r2);
  const depth = between(2, 30);
  addCase('overlapping-regions', 'two overlapping circles (three regions); every region is extruded', [
    sketch(1, PLANES.xy),
    circle(1, 'circle1', -gap / 2, 0, r1),
    circle(1, 'circle2', gap / 2, between(-5, 5), r2),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

for (let i = 0; i < 8; i++) {
  const useCircle = random() < 0.5;
  const depth = between(2, 60);
  const profile = useCircle ? circle(1, 'circle1', between(-10, 10), between(-10, 10), between(3, 30)) : rect(1, 'rect1', ...randomRect(60));
  addCase('symmetric', `${useCircle ? 'circle' : 'rectangle'} extruded symmetrically, total ${num(depth)} mm`, [
    sketch(1, pick([PLANES.xy, PLANES.yz, PLANES.tilted])),
    profile,
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth), symmetric: 'true' }),
  ]);
}

for (let i = 0; i < 8; i++) {
  const depth = between(2, 40);
  const second = between(2, 40);
  const opposite = i % 4 === 3;
  addCase('second-direction', `rectangle, ${num(depth)} mm one way and ${num(second)} mm the other${opposite ? ', first direction flipped' : ''}`, [
    sketch(1, pick([PLANES.xy, PLANES.xz])),
    rect(1, 'rect1', ...randomRect(50)),
    solve(1),
    extrude(1, {
      entities: regions(1),
      endBound: 'BoundingType.BLIND',
      depth: mm(depth),
      oppositeDirection: String(opposite),
      hasSecondDirection: 'true',
      secondDirectionBound: 'BoundingType.BLIND',
      secondDirectionDepth: mm(second),
      secondDirectionOppositeDirection: 'true',
    }),
  ]);
}

for (let i = 0; i < 4; i++) {
  const depth = between(2, 40);
  addCase('opposite-direction', `circle extruded ${num(depth)} mm in the opposite direction`, [
    sketch(1, pick([PLANES.xy, PLANES.tilted])),
    circle(1, 'circle1', between(-15, 15), between(-15, 15), between(3, 25)),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth), oppositeDirection: 'true' }),
  ]);
}

for (let i = 0; i < 2; i++) {
  const depth = -between(2, 30);
  addCase('negative-depth', `rectangle with a negative blind depth (${num(depth)} mm)`, [
    sketch(1, PLANES.xy),
    rect(1, 'rect1', ...randomRect(40)),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  ]);
}

// Booleans: a base block from the XY plane up to z = baseDepth, then a second extrude from its top face plane.
function baseBlock() {
  const w = between(30, 80);
  const h = between(30, 80);
  const baseDepth = between(5, 30);
  return {
    w,
    h,
    baseDepth,
    statements: [
      sketch(1, PLANES.xy),
      rect(1, 'rect1', -w / 2, -h / 2, w / 2, h / 2),
      solve(1),
      extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(baseDepth) }),
    ],
  };
}

for (let i = 0; i < 8; i++) {
  const base = baseBlock();
  const r = between(3, Math.min(base.w, base.h) / 2 - 1);
  const height = between(2, 40);
  const overhang = i % 4 === 3; // the boss sticks out past the block's edge
  const cx = overhang ? base.w / 2 : between(-base.w / 2 + r, base.w / 2 - r);
  addCase('boolean-add', `block plus a cylindrical boss r ${num(r)}, h ${num(height)}${overhang ? ', overhanging the edge' : ''}`, [
    ...base.statements,
    sketch(2, planeAtZ(base.baseDepth)),
    circle(2, 'circle1', cx, between(-base.h / 4, base.h / 4), r),
    solve(2),
    extrude(2, { entities: regions(2), endBound: 'BoundingType.BLIND', depth: mm(height), ...booleanFields('ADD', 1) }),
  ]);
}

for (let i = 0; i < 8; i++) {
  const base = baseBlock();
  const through = i % 2 === 1;
  const depth = through ? base.baseDepth + between(1, 10) : between(1, base.baseDepth - 1);
  const pocket = rect(2, 'rect1', ...randomRect(Math.min(base.w, base.h) - 4).map((v) => v * 0.8));
  addCase('boolean-remove', `block with a rectangular ${through ? 'through cut' : `pocket ${num(depth)} mm deep`}`, [
    ...base.statements,
    sketch(2, planeAtZ(base.baseDepth)),
    pocket,
    solve(2),
    extrude(2, {
      entities: regions(2),
      endBound: 'BoundingType.BLIND',
      depth: mm(depth),
      oppositeDirection: 'true',
      ...booleanFields('REMOVE', 1),
    }),
  ]);
}

for (let i = 0; i < 4; i++) {
  const base = baseBlock();
  const r = between(10, 45);
  addCase('boolean-intersect', `block intersected with a cylinder r ${num(r)} through it`, [
    ...base.statements,
    sketch(2, planeAtZ(-5)),
    circle(2, 'circle1', between(-10, 10), between(-10, 10), r),
    solve(2),
    extrude(2, {
      entities: regions(2),
      endBound: 'BoundingType.BLIND',
      depth: mm(base.baseDepth + 10),
      ...booleanFields('INTERSECT', 1),
    }),
  ]);
}

// Language probes: a 10 mm square extruded to a depth that depends on how an expression evaluates. Some
// alternative readings throw instead, which leaves no part.
const probes = [
  ['-2^2 + 5', 'unary minus vs ^: 1 if ^ binds tighter, 9 if minus does'],
  ['("a" ~ 1 + 2 == "a3") ? 2 : 1', '~ vs +: 2 if + binds tighter; throws or 1 otherwise'],
  ['(true ?? false && false) ? 2 : 1', '?? vs &&: 2 if ?? binds looser, 1 if tighter'],
  ['("x" is string == true) ? 2 : 1', 'is vs ==: 2 if is binds tighter'],
  ['2 * 3 % 4', '* and % at the same level, left to right: 2; 6 if % binds tighter'],
  ['2^3^2 / 64', '^ associativity: 8 if right-associative, 1 if left'],
  ['true ? 1 : false ? 2 : 3', 'ternary associativity: 1 if right-associative; throws if left'],
  ['(1 < 2 == true) ? 2 : 1', '< vs ==: 2 if < binds tighter; throws otherwise'],
  ['-7 % 3 + 4', '% sign: 6 if the result takes the divisor\'s sign, 3 if the dividend\'s'],
];
for (const [expression, description] of probes) {
  addCase('language-probe', `depth (mm) = ${expression}; ${description}`, [
    `const probeValue = ${expression};`,
    sketch(1, PLANES.xy),
    rect(1, 'rect1', 0, 0, 10, 10),
    solve(1),
    extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: 'probeValue * millimeter' }),
  ]);
}

addCase('expected-failure', 'zero blind depth', [
  sketch(1, PLANES.xy),
  rect(1, 'rect1', 0, 0, 10, 10),
  solve(1),
  extrude(1, { entities: regions(1), endBound: 'BoundingType.BLIND', depth: mm(0) }),
]);
addCase('expected-failure', 'nothing selected to extrude', [
  extrude(1, { entities: 'qNothing()', endBound: 'BoundingType.BLIND', depth: mm(10) }),
]);

// ----- Output -----

const header = `FeatureScript ${STD_VERSION.replace(/\.0$/, '')};\nimport(path : "onshape/std/geometry.fs", version : "${STD_VERSION}");\n`;

let casesFs = `${header}
// Fixture batch 001: sketch-and-extrude cases. Generated by tools/fixtures/gen-batch-001.mjs (seed ${SEED}); do not edit.
// Each case builds its geometry at the origin. Booleans are scoped to the case's own bodies.
`;
for (const c of cases) {
  casesFs += `\n// ${c.kind}: ${c.description}\nexport function ${c.name}(context is Context, id is Id)\n{\n`;
  casesFs += c.body.map((s) => `    ${s}\n`).join('');
  casesFs += '}\n';
}

let generatorFs = `
// ----- Onshape only: builds every case of batch 001 for the STEP export. Not used by our engine. -----

annotation { "Feature Type Name" : "Build fixture batch 001" }
export const buildFixtureBatch001 = defineFeature(function(context is Context, id is Id, definition is map)
    precondition
    {
    }
    {
        var failed = [];
`;
for (const c of cases) {
  generatorFs += `        if (!runFixtureCase(context, id, "${c.name}", vector(${num(c.slot[0])}, ${num(c.slot[1])}, ${num(c.slot[2])}) * meter,\n                function(context is Context, id is Id) { ${c.name}(context, id); }))\n            failed = append(failed, "${c.name}");\n`;
}
generatorFs += `        println("batch-001 failed cases: " ~ toString(failed));
    });

// Builds one case, then moves its solids to the case's slot and names them "<case>.<n>". Returns false if the
// case threw or made no solid.
function runFixtureCase(context is Context, id is Id, name is string, slot is Vector, build is function) returns boolean
{
    const caseId = id + name;
    try
    {
        build(context, caseId);
    }
    catch
    {
        clearFeatureStatus(context, id, { "withDisplayData" : true });
        return false;
    }
    const solids = qBodyType(qCreatedBy(caseId, EntityType.BODY), BodyType.SOLID);
    const bodies = evaluateQuery(context, solids);
    if (size(bodies) == 0)
        return false;
    opTransform(context, caseId + "move", { "bodies" : solids, "transform" : transform(slot) });
    for (var i, body in bodies)
    {
        setProperty(context, { "entities" : body, "propertyType" : PropertyType.NAME, "value" : name ~ "." ~ (i + 1) });
    }
    return true;
}
`;

const manifest = {
  batch: '001',
  seed: SEED,
  stdVersion: STD_VERSION,
  slotSpacingMeters: SLOT_SPACING_M,
  cases: cases.map(({ name, kind, description, slot }) => ({ name, kind, description, slotMeters: slot })),
};

mkdirSync(OUT_DIR, { recursive: true });
writeFileSync(join(OUT_DIR, 'cases.fs'), casesFs);
writeFileSync(join(OUT_DIR, 'generator.fs'), generatorFs);
writeFileSync(join(OUT_DIR, 'cases.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`wrote ${cases.length} cases to ${OUT_DIR}`);

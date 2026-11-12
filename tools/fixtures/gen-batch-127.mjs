// Generates fixture batch 127: features one after another, as models are made. A block with filleted upright
// edges and a hole shelled open at its top; a shelled box's inner bottom edges filleted; a rib across a shelled box;
// a shelled box mirrored about its side and merged, and split through its middle; a block's top and the fillet on
// its edge thickened together; two balls united, the circle where they meet filleted; a block's side with a hole
// in it drafted.
//
// Usage: node tools/fixtures/gen-batch-127.mjs
//
// Writes fixtures/batch-127/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, deg, edgeAt, faceAt, line, mm, planeAt, planeBody, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 127, title: 'features in sequence: shells with fillets and holes, ribs, mirrors and splits of shells, thickened fillets', helpers: '' });
const base = solidsOf('base');

const hole = (name, [cx, cy], r, z) => [
  sketch(name + 'Sketch', planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])), circle(name + 'Sketch', 'circle1', [cx, cy], r), solve(name + 'Sketch'),
  call('extrude', name, {
    entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
];
const uprights = (x1, y1, x2, y2, z) => `qUnion([${[[x1, y1], [x2, y1], [x2, y2], [x1, y2]].map(([x, y]) => edgeAt(base, [x, y, z])).join(', ')}])`;
// A 40 × 30 × 20 box shelled 2 mm with its top open.
const shelledBox = [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 20]), thickness: mm(2) }),
];

batch.add('shell', 'a 40 × 30 × 20 block with its upright edges filleted 5 mm and a hole (radius 4) through it: shelled 2 mm open at its top', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: uprights(0, 0, 40, 30, 10), radius: mm(5) }),
  hole('hole', [20, 15], 4, 20),
  call('shell', 'shell1', { entities: faceAt(base, [10, 10, 20]), thickness: mm(2) }),
]);
batch.add('fillet', "a 40 × 30 × 20 box shelled 2 mm open at its top: its inner bottom edges filleted 1 mm", [
  shelledBox,
  call('fillet', 'fillet1', { entities: `qUnion([${[[20, 2, 2], [38, 15, 2], [20, 28, 2], [2, 15, 2]].map((p) => edgeAt(base, p)).join(', ')}])`, radius: mm(1) }),
]);
batch.add('rib', 'a 40 × 30 × 20 box shelled 2 mm open at its top: a rib 2 thick across it, from a line along its middle', [
  shelledBox,
  sketch('profile', planeAt([0, 15, 0], [0, -1, 0], [1, 0, 0])), line('profile', 'line1', [0, 15], [40, 15]), solve('profile'),
  call('rib', 'rib1', {
    profiles: `qOwnedByBody(qBodyType(qCreatedBy(id + "profile", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`, parts: base,
    thickness: mm(2), ribExtrusionDirection: 'RibExtrusionDirection.NORMAL_TO_SKETCH_PLANE', oppositeDirection: 'true',
  }),
]);
batch.add('mirror', 'a 40 × 30 × 20 box shelled 2 mm open at its top, mirrored about its right side and merged', [
  shelledBox,
  call('mirror', 'mirror1', { entities: base, mirrorPlane: faceAt(base, [40, 15, 10]), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
]);
batch.add('split-part', 'a 40 × 30 × 20 box shelled 2 mm open at its top, split through its middle', [
  shelledBox, planeBody('cut', [20, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('splitPart', 'split1', { targets: base, tool: `qCreatedBy(id + "cut", EntityType.FACE)` }),
]);
batch.add('thicken', "a 40 × 30 × 20 block with its top back edge filleted 5 mm: its top and the fillet thickened 2 mm together", [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 30, 20]), radius: mm(5) }),
  call('thicken', 'thicken1', { entities: `qUnion([${faceAt(base, [20, 10, 20])}, qCreatedBy(id + "fillet1", EntityType.FACE)])`, thickness1: mm(2), thickness2: mm(0) }),
]);
batch.add('fillet', 'balls of radius 10 and 8 with centres 12 apart united: the circle where they meet filleted 1 mm', [
  call('opSphere', 'base', { center: vec3(0, 0, 0), radius: mm(10) }),
  call('opSphere', 'other', { center: vec3(12, 0, 0), radius: mm(8) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('other')}])`, operationType: 'BooleanOperationType.UNION' }),
  // Where the spheres meet: x = (100 - 64 + 144) / 24 = 7.5, radius sqrt(100 - 56.25).
  call('fillet', 'fillet1', { entities: edgeAt(base, [7.5, Math.sqrt(43.75), 0]), radius: mm(1) }),
]);
batch.add('draft', 'a 30 × 20 × 10 block with a hole (radius 3) through its front: the front drafted 5° from the bottom', [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('holeSketch', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), circle('holeSketch', 'circle1', [15, 5], 3), solve('holeSketch'),
  call('extrude', 'hole', {
    entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  call('draft', 'draft1', { neutralPlane: faceAt(base, [15, 10, 0]), draftFaces: faceAt(base, [5, 0, 5]), angle: deg(5) }),
]);

batch.write('tools/fixtures/gen-batch-127.mjs');

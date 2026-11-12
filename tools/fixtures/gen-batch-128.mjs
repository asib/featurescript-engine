// Generates fixture batch 128: joints of curved faces, as in pipework and housings. A tee of a pipe of radius 10
// and a branch of radius 3 (batch 124's tee had its fillet reach the pipe's side exactly): the joint filleted,
// chamfered, and the tee shelled open at its three ends; a capsule (a cylinder capped by a hemisphere) shelled open
// at its base; a shelled box with a window cut through its front, the window's outer edges filleted; a boss rising
// from a lying cylinder, its foot filleted; two pipes of radius 5 and 4 crossing, united, their joints filleted.
//
// Usage: node tools/fixtures/gen-batch-128.mjs
//
// Writes fixtures/batch-128/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 128, title: 'joints of curved faces: tees, a capsule, a window, a boss on a pipe, crossing pipes', helpers: '' });
const base = solidsOf('base');

// A cylinder lying along x from x = -l/2 to l/2, its axis at height z (or along y, turned).
const lying = (name, r, l, z, alongY = false) => [
  sketch(name + 'Sketch', alongY ? planeAt([0, -l / 2, z], [0, 1, 0], [0, 0, 1]) : planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])),
  circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
const added = (name, sketchName) => call('extrude', name, {
  entities: regions(sketchName), endBound: 'BoundingType.BLIND', depth: mm(20),
  operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
});
// The tee: a pipe of radius 10 along x (from -25 to 25) and a branch of radius 3 up z from its axis to z = 20.
const tee = [
  lying('base', 10, 50, 0),
  sketch('branchSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('branchSketch', 'circle1', [0, 0], 3), solve('branchSketch'),
  added('branch', 'branchSketch'),
];
// The joint where the branch meets the pipe: at y = 0, x = 3, z = sqrt(100 - 0) = 10.
const joint = edgeAt(base, [3, 0, 10]);

batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted 1 mm', [
  tee, call('fillet', 'fillet1', { entities: joint, radius: mm(1) }),
]);
batch.add('chamfer', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint chamfered 1 mm', [
  tee, call('chamfer', 'chamfer1', { entities: joint, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1) }),
]);
batch.add('shell', 'a tee of a pipe of radius 10 and a branch of radius 3: shelled 1 mm inwards, open at its three ends', [
  tee,
  call('shell', 'shell1', { entities: `qUnion([${faceAt(base, [-25, 0, 0])}, ${faceAt(base, [25, 0, 0])}, ${faceAt(base, [0, 0, 20])}])`, thickness: mm(1) }),
]);
batch.add('shell', 'a capsule (a cylinder of radius 10, 20 tall, capped by a hemisphere) shelled 1 mm inwards, open at its base', [
  sketch('profile', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])),
  line('profile', 'line1', [0, 0], [10, 0]), line('profile', 'line2', [10, 0], [10, 20]),
  arc('profile', 'arc1', [10, 20], [7.071068, 27.071068], [0, 30]), line('profile', 'line3', [0, 30], [0, 0]),
  solve('profile'),
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [30, 0]), solve('axis1'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis1", EntityType.EDGE)` }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 0]), thickness: mm(1) }),
]);
batch.add('fillet', 'a 40 × 30 × 20 box shelled 2 mm open at its top, a 16 × 8 window cut through its front: the window\'s outer edges filleted 0.5 mm', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 20]), thickness: mm(2) }),
  sketch('window', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('window', 'rect1', [12, 6], [28, 14]), solve('window'),
  call('extrude', 'cut', {
    entities: regions('window'), endBound: 'BoundingType.BLIND', depth: mm(2), oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  call('fillet', 'fillet1', { entities: `qUnion([${[[20, 0, 6], [20, 0, 14], [12, 0, 10], [28, 0, 10]].map((p) => edgeAt(base, p)).join(', ')}])`, radius: mm(0.5) }),
]);
batch.add('fillet', 'a cylinder of radius 10 lying along x with a boss of radius 3 rising from its axis to z = 20: the boss\'s foot filleted 1 mm', [
  lying('base', 10, 40, 0),
  sketch('bossSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('bossSketch', 'circle1', [5, 0], 3), solve('bossSketch'),
  added('boss', 'bossSketch'),
  call('fillet', 'fillet1', { entities: edgeAt(base, [8, 0, 10]), radius: mm(1) }),
]);
batch.add('fillet', 'pipes of radius 5 along x and radius 4 along y crossing, united: their joints filleted 1 mm', [
  lying('base', 5, 40, 0), lying('other', 4, 40, 0, true),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('other')}])`, operationType: 'BooleanOperationType.UNION' }),
  // The smaller pipe passes through the larger: two joints, one each side. At x = 0 the smaller pipe's top is at
  // z = 4, on the larger at y = ±sqrt(25 - 16) = ±3.
  call('fillet', 'fillet1', { entities: `qUnion([${edgeAt(base, [0, 3, 4])}, ${edgeAt(base, [0, -3, 4])}])`, radius: mm(1) }),
]);

batch.write('tools/fixtures/gen-batch-128.mjs');

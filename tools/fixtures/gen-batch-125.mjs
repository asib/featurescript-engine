// Generates fixture batch 125: batch 124's follow-ups. Sweeps along helices with the profile truly square to the
// path (batch 124's lay 18° off: `Direction.CCW` turns clockwise as it rises), both hands, a circle and a square;
// rims of spheres meeting planes, a dome's concave foot filleted and a pocket's rim chamfered; a cone's top edge
// filleted; a hemisphere shelled open at its flat face; a cross hole's mouths through a lying cylinder filleted.
//
// Usage: node tools/fixtures/gen-batch-125.mjs
//
// Writes fixtures/batch-125/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, deg, dir3, edgeAt, faceAt, line, mm, planeAt, planeBody, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 125, title: 'helix sweeps square to the path, sphere rims, a cone edge, a hemisphere shelled, a cross hole filleted', helpers: '' });
const base = solidsOf('base');

// A helix of radius r about z from (r, 0, 0), `turns` turns `height` high: `Direction.CCW` turns clockwise seen
// from above as it rises (left-handed), `Direction.CW` anticlockwise.
const helix = (r, turns, height, hand) => [
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [height, 0]), solve('axis1'),
  call('opPoint', 'start', { point: vec3(r, 0, 0) }),
  call('helix', 'helix1', {
    startType: 'StartType.START_POINT', startAngle: deg(0), handedness: `Direction.${hand}`, oppositeDirection: 'false',
    axisType: 'AxisType.AXIS', axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, pathType: 'PathType.TURNS',
    revolutions: turns, startPoint: `qCreatedBy(id + "start", EntityType.VERTEX)`, endType: 'EndType.HEIGHT', height: mm(height),
  }),
];
// A profile on the plane square to the helix at its start (x radial).
const helixProfile = (r, turns, height, hand, draw) => {
  const sign = hand === 'CCW' ? -1 : 1;
  const t = [0, sign * 2 * Math.PI * r, height / turns];
  const n = Math.hypot(...t);
  return [
    `const profile = newSketchOnPlane(context, id + "profile", { "sketchPlane" : plane(${vec3(r, 0, 0)}, ${dir3(0, t[1] / n, t[2] / n)}, ${dir3(1, 0, 0)}) });`,
    ...draw('profile'), solve('profile'),
  ];
};
const helixSweep = (r, turns, height, hand, draw) => [
  helix(r, turns, height, hand), helixProfile(r, turns, height, hand, draw),
  call('sweep', 'base', { profiles: regions('profile'), path: `qCreatedBy(id + "helix1", EntityType.EDGE)` }),
];
const disc = (s) => [circle(s, 'circle1', [0, 0], 1)];
const square = (s) => [rect(s, 'rect1', [-1, -1], [1, 1])];

// A cylinder lying along x from x = -l/2 to l/2, its axis at height z.
const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];

batch.add('sweep', 'a spring: a circle of radius 1 square to a left-handed helix (Direction.CCW) of radius 10, 3 turns 15 high', [
  helixSweep(10, 3, 15, 'CCW', disc),
]);
batch.add('sweep', 'a 2 × 2 square square to a left-handed helix of radius 10, 2 turns 20 high', [helixSweep(10, 2, 20, 'CCW', square)]);
batch.add('sweep', 'a 2 × 2 square square to a right-handed helix (Direction.CW) of radius 10, 2 turns 20 high', [helixSweep(10, 2, 20, 'CW', square)]);
batch.add('fillet', 'a 30 × 20 × 10 block with a ball (radius 6) centred 2 below its top united on it: the dome\'s foot filleted 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 8), radius: mm(6) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('ball')}])`, operationType: 'BooleanOperationType.UNION' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [15 + Math.sqrt(32), 10, 10]), radius: mm(1) }),
]);
batch.add('chamfer', 'a 30 × 20 × 10 block with a pocket of a sphere (radius 6) centred 2 above its top: the rim chamfered 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 12), radius: mm(6) }),
  call('booleanBodies', 'pocket', { tools: solidsOf('ball'), targets: base, operationType: 'BooleanOperationType.SUBTRACTION' }),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [15 + Math.sqrt(32), 10, 10]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1) }),
]);
batch.add('fillet', 'a cone frustum (radius 10 to 5, 20 tall): its top edge filleted 1 mm', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 20), bottomRadius: mm(10), topRadius: mm(5) }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [5, 0, 20]), radius: mm(1) }),
]);
batch.add('shell', 'a hemisphere (radius 10, flat face down) shelled 1 mm inwards, open at its flat face', [
  call('opSphere', 'base', { center: vec3(0, 0, 0), radius: mm(10) }),
  planeBody('cut', [0, 0, 0], [0, 0, 1], [1, 0, 0]),
  call('splitPart', 'half', { targets: base, tool: `qCreatedBy(id + "cut", EntityType.FACE)`, keepBothSides: 'false', keepFront: 'true' }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 0]), thickness: mm(1) }),
]);
batch.add('fillet', 'a cylinder (radius 10, 40 long) lying along x with a hole of radius 2 cut down through it: the hole\'s top mouth filleted 0.5 mm', [
  lying('base', 10, 40, 0),
  sketch('holeSketch', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'circle1', [5, 0], 2), solve('holeSketch'),
  call('extrude', 'hole', {
    entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [7, 0, 10]), radius: mm(0.5) }),
]);

batch.write('tools/fixtures/gen-batch-125.mjs');

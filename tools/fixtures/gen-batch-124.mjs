// Generates fixture batch 124: everyday modelling past the plain cases. A spring (a circle swept along a helix) and
// a square wire swept along one; a boss extruded up to a lying cylinder's side; a block's spherical pocket with its
// rim filleted; a cross hole cut through a lying cylinder; a block with filleted corners drafted all round (its
// sides and fillets); a tee of two pipes, its joint filleted, chamfered, and the tee shelled open at its three ends;
// a bowl revolved from a profile with an arc.
//
// Usage: node tools/fixtures/gen-batch-124.mjs
//
// Writes fixtures/batch-124/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, deg, dir3, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 124, title: 'everyday modelling: springs, bosses up to curved faces, pockets, drafts round fillets, a tee', helpers: '' });
const base = solidsOf('base');

// A helix of radius r about z from (r, 0, 0), `turns` turns `height` high, anticlockwise.
const helix = (r, turns, height) => [
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [height, 0]), solve('axis1'),
  call('opPoint', 'start', { point: vec3(r, 0, 0) }),
  call('helix', 'helix1', {
    startType: 'StartType.START_POINT', startAngle: deg(0), handedness: 'Direction.CCW', oppositeDirection: 'false',
    axisType: 'AxisType.AXIS', axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, pathType: 'PathType.TURNS',
    revolutions: turns, startPoint: `qCreatedBy(id + "start", EntityType.VERTEX)`, endType: 'EndType.HEIGHT', height: mm(height),
  }),
];
// A profile meant to be square to the helix at its start. It isn't: this takes the tangent of a helix turning
// anticlockwise about z, but `Direction.CCW` turns clockwise as it rises (left-handed, batch 004), so the profile
// lies 18° off square. The fixture was built so; both kernels sweep it alike.
const helixProfile = (r, turns, height, draw) => {
  const t = [0, 2 * Math.PI * r, height / turns];
  const n = Math.hypot(...t);
  return [
    `const profile = newSketchOnPlane(context, id + "profile", { "sketchPlane" : plane(${vec3(r, 0, 0)}, ${dir3(0, t[1] / n, t[2] / n)}, ${dir3(1, 0, 0)}) });`,
    ...draw('profile'), solve('profile'),
  ];
};
const sweepAlongHelix = call('sweep', 'base', { profiles: regions('profile'), path: `qCreatedBy(id + "helix1", EntityType.EDGE)` });

// A cylinder lying along x from x = -l/2 to l/2, its axis at height z.
const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
// The tee: a pipe of radius 5 along x (from -20 to 20) and one of radius 4 up z from its axis to z = 20, united.
const tee = [
  lying('base', 5, 40, 0),
  sketch('branchSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('branchSketch', 'circle1', [0, 0], 4), solve('branchSketch'),
  call('extrude', 'branch', {
    entities: regions('branchSketch'), endBound: 'BoundingType.BLIND', depth: mm(20),
    operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
  }),
];
const joint = edgeAt(base, [4, 0, 5]);

batch.add('sweep', 'a spring: a circle of radius 1 swept along a helix of radius 10, 3 turns 15 high', [
  helix(10, 3, 15), helixProfile(10, 3, 15, (s) => [circle(s, 'circle1', [0, 0], 1)]), sweepAlongHelix,
]);
batch.add('sweep', 'a 2 × 2 square swept along a helix of radius 10, 2 turns 20 high', [
  helix(10, 2, 20), helixProfile(10, 2, 20, (s) => [rect(s, 'rect1', [-1, -1], [1, 1])]), sweepAlongHelix,
]);
batch.add('extrude', 'a circle of radius 3 extruded up to the side of a cylinder (radius 10) lying along x 20 above it', [
  lying('wall', 10, 30, 20),
  sketch('bossSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('bossSketch', 'circle1', [2, 0], 3), solve('bossSketch'),
  call('extrude', 'base', {
    entities: regions('bossSketch'), endBound: 'BoundingType.UP_TO_SURFACE',
    endBoundEntityFace: `qGeometry(qOwnedByBody(qCreatedBy(id + "wall", EntityType.BODY), EntityType.FACE), GeometryType.CYLINDER)`,
  }),
]);
batch.add('fillet', 'a 30 × 20 × 10 block with a pocket of a sphere (radius 6) centred 2 above its top: the rim filleted 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 12), radius: mm(6) }),
  call('booleanBodies', 'pocket', { tools: solidsOf('ball'), targets: base, operationType: 'BooleanOperationType.SUBTRACTION' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [15 + Math.sqrt(32), 10, 10]), radius: mm(1) }),
]);
batch.add('extrude', 'a cylinder (radius 10, 40 long) lying along x with a circle of radius 2 cut down through it at x = 5', [
  lying('base', 10, 40, 0),
  sketch('holeSketch', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'circle1', [5, 0], 2), solve('holeSketch'),
  call('extrude', 'hole', {
    entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
]);
batch.add('draft', 'a 30 × 20 × 10 block with its upright edges filleted 3 mm: its sides and fillets drafted 5° from the bottom', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fillet', 'fillet1', { entities: `qUnion([${[[0, 0], [30, 0], [30, 20], [0, 20]].map(([x, y]) => edgeAt(base, [x, y, 5])).join(', ')}])`, radius: mm(3) }),
  call('draft', 'draft1', {
    neutralPlane: faceAt(base, [15, 10, 0]),
    draftFaces: `qUnion([${[[15, 0, 5], [30, 10, 5], [15, 20, 5], [0, 10, 5]].map((p) => faceAt(base, p)).join(', ')}, qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CYLINDER)])`,
    angle: deg(5),
  }),
]);
batch.add('fillet', 'a tee of pipes (radius 5 along x, radius 4 up z): the joint filleted 1 mm', [
  tee, call('fillet', 'fillet1', { entities: joint, radius: mm(1) }),
]);
batch.add('chamfer', 'a tee of pipes (radius 5 along x, radius 4 up z): the joint chamfered 1 mm', [
  tee, call('chamfer', 'chamfer1', { entities: joint, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1) }),
]);
batch.add('shell', 'a tee of pipes (radius 5 along x, radius 4 up z): shelled 1 mm inwards, open at its three ends', [
  tee,
  call('shell', 'shell1', { entities: `qUnion([${faceAt(base, [-20, 0, 0])}, ${faceAt(base, [20, 0, 0])}, ${faceAt(base, [0, 0, 20])}])`, thickness: mm(1) }),
]);
batch.add('revolve', 'a bowl: arcs of radius 20 and 18 about (0, 20) closed at the rim and the axis, revolved about z', [
  sketch('bowlSketch', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])),
  line('bowlSketch', 'line1', [0, 0], [0, 2]),
  arc('bowlSketch', 'arc1', [0, 0], [14.142136, 5.857864], [20, 20]),
  line('bowlSketch', 'line2', [20, 20], [18, 20]),
  arc('bowlSketch', 'arc2', [0, 2], [12.727922, 7.272078], [18, 20]),
  solve('bowlSketch'),
  sketch('axisSketch', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axisSketch', 'line1', [0, 0], [20, 0]), solve('axisSketch'),
  call('revolve', 'base', { entities: regions('bowlSketch'), axis: `qCreatedBy(id + "axisSketch", EntityType.EDGE)` }),
]);

batch.write('tools/fixtures/gen-batch-124.mjs');

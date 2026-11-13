// Generates fixture batch 147: everyday parts, an eighth round: a cam (two circles joined by lines held tangent to
// both, with bores), a plate with a boss extruded up to a tilted plane, a ring revolved through 270° with a cap's
// outer edge filleted, a tray (a pocket cut with a draft, its floor's edges filleted), a shaft collar (a ring split
// by a slot, a clamp hole across the slot), a handle (a circle swept along lines joined by arcs), a wedge with holes
// drilled square to its slanted face.
//
// Usage: node tools/fixtures/gen-batch-147.mjs
//
// Writes fixtures/batch-147/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, createdBy, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 147, title: 'everyday parts, an eighth round: a cam, a boss up to a tilted plane, a 270° ring, a drafted tray, a shaft collar, a swept handle, holes in a slanted face', helpers: '' });
const base = solidsOf('base');
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = (name) => [sketch(name, planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line(name, 'line1', [-100, 0], [100, 0]), solve(name)];
const cut = (name, opposite = 'true') => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: opposite, operationType: 'NewBodyOperationType.REMOVE', ...scoped });
const tangent = (s, name, a, b) => `skConstraint(${s}, "${name}", { "constraintType" : ConstraintType.TANGENT, "localFirst" : "${a}", "localSecond" : "${b}" });`;

// The cam's tangent lines, drawn a little off and held tangent by the solver: circles of radius 15 and 8, 30 apart.
const alpha = Math.asin(7 / 30);
const [s, c] = [Math.sin(alpha), Math.cos(alpha)];
batch.add('extrude', 'a cam: circles of radius 15 at the origin and 8 at (30, 0) joined by two lines held tangent to both, extruded 6, bores of radius 5 and 3', [
  sketch('baseSketch', top(0)), circle('baseSketch', 'c1', [0, 0], 15), circle('baseSketch', 'c2', [30, 0], 8),
  line('baseSketch', 'l1', [15 * s + 0.2, 15 * c - 0.1], [30 + 8 * s - 0.2, 8 * c + 0.1]), line('baseSketch', 'l2', [15 * s + 0.2, -15 * c + 0.1], [30 + 8 * s - 0.2, -8 * c - 0.1]),
  tangent('baseSketch', 't1', 'l1', 'c1'), tangent('baseSketch', 't2', 'l1', 'c2'), tangent('baseSketch', 't3', 'l2', 'c1'), tangent('baseSketch', 't4', 'l2', 'c2'),
  `skConstraint(baseSketch, "k1", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l1.start", "localSecond" : "c1" });`,
  `skConstraint(baseSketch, "k2", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l1.end", "localSecond" : "c2" });`,
  `skConstraint(baseSketch, "k3", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l2.start", "localSecond" : "c1" });`,
  `skConstraint(baseSketch, "k4", { "constraintType" : ConstraintType.COINCIDENT, "localFirst" : "l2.end", "localSecond" : "c2" });`,
  solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(6) }),
  sketch('boreSketch', top(6)), circle('boreSketch', 'c1', [0, 0], 5), circle('boreSketch', 'c2', [30, 0], 3), solve('boreSketch'), cut('bore'),
]);
batch.add('extrude', 'a 40 × 20 × 5 plate with a boss (radius 5) at its middle extruded up to a plane through (20, 10, 25) tilted 11.3° about y', [
  box('base', [0, 0, 0], [40, 20, 5]),
  call('opPlane', 'limit', { plane: `plane(${vec3(20, 10, 25)}, vector(0.2, 0, 1) / sqrt(1.04))`, width: mm(100), height: mm(100) }),
  sketch('bossSketch', top(5)), circle('bossSketch', 'c1', [20, 10], 5), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.UP_TO_SURFACE', endBoundEntityFace: createdBy('limit', 'FACE'), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('revolve', 'a ring (radii 10 to 20, 8 tall) revolved 270° about z, its start cap\'s outer edge filleted 2', [
  sketch('profile', PLANES.xz), polygon('profile', [[10, 0], [20, 0], [20, 8], [10, 8]]), solve('profile'),
  zAxis('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', angle: deg(270), oppositeDirection: 'false', symmetric: 'false' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 0, 4]), radius: mm(2) }),
]);
batch.add('extrude', 'a tray: a 60 × 40 × 15 block with a 50 × 30 pocket 10 deep cut with a 5° draft, its floor\'s edges filleted 2', [
  box('base', [0, 0, 0], [60, 40, 15]),
  sketch('pocketSketch', top(15)), rect('pocketSketch', 'r1', [5, 5], [55, 35]), solve('pocketSketch'),
  call('extrude', 'pocket', { entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), oppositeDirection: 'true', hasDraft: 'true', draftAngle: deg(5), draftPullDirection: 'false', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('fillet', 'fillet1', { entities: `qAdjacent(${faceAt(base, [30, 20, 5])}, AdjacencyType.EDGE, EntityType.EDGE)`, radius: mm(2) }),
]);
batch.add('extrude', 'a shaft collar: a ring (radii 8 to 15, 10 tall) split by a slot 2 wide on its +x side, a clamp hole (radius 1.5) across the slot at x = 11.5, half way up', [
  cylinder('base', [0, 0], 15, 0, 10),
  sketch('boreSketch', top(10)), circle('boreSketch', 'c1', [0, 0], 8), solve('boreSketch'), cut('bore'),
  sketch('slotSketch', top(10)), rect('slotSketch', 'r1', [7, -1], [16, 1]), solve('slotSketch'), cut('slot'),
  sketch('clampSketch', planeAt([0, -20, 0], [0, 1, 0], [1, 0, 0])), circle('clampSketch', 'c1', [11.5, -5], 1.5), solve('clampSketch'), cut('clamp', 'false'),
]);
const r = 5;
batch.add('sweep', 'a handle: a circle (radius 3) swept up 20, round an arc (radius 5), along 30, round another and down 20', [
  sketch('path', PLANES.xz),
  line('path', 'l1', [0, 0], [0, 20]), arc('path', 'a1', [0, 20], [r - r * Math.SQRT1_2, 20 + r * Math.SQRT1_2], [r, 25]), line('path', 'l2', [r, 25], [40 - r, 25]),
  arc('path', 'a2', [40 - r, 25], [40 - r + r * Math.SQRT1_2, 20 + r * Math.SQRT1_2], [40, 20]), line('path', 'l3', [40, 20], [40, 0]), solve('path'),
  sketch('profile', top(0)), circle('profile', 'c1', [0, 0], 3), solve('profile'),
  call('sweep', 'base', { profiles: regions('profile'), path: `qUnion([qOwnedByBody(qBodyType(qCreatedBy(id + "path", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)])` }),
]);
const n = [10 / Math.hypot(10, 40), 0, 40 / Math.hypot(10, 40)];
const x = [40 / Math.hypot(10, 40), 0, -10 / Math.hypot(10, 40)];
batch.add('hole', 'a wedge (sloping from 20 tall to 10 over 40, 20 deep) with two holes (diameter 5, 10 deep) drilled square to its slanted face', [
  sketch('baseSketch', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), polygon('baseSketch', [[0, 0], [40, 0], [40, 10], [0, 20]]), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
  sketch('pts', planeAt([0, 0, 20], n, x)), `skPoint(pts, "p1", { "position" : vector(10, 10) * millimeter });`, `skPoint(pts, "p2", { "position" : vector(30, 10) * millimeter });`, solve('pts'),
  call('hole', 'hole1', { locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`, scope: base, style: 'HoleStyle.SIMPLE', holeDiameter: mm(5), holeDepth: mm(10), endStyle: 'HoleEndStyle.BLIND' }),
]);

batch.write('tools/fixtures/gen-batch-147.mjs');

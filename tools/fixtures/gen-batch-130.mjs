// Generates fixture batch 130: chamfers where the two rules batch 129 found differ. Along straight edges at corners
// other than square (a 60° prism edge, a 120° one, a 120° concave one), the chamfer's feet are its width from the
// edge (as along circles) or where a ball of that radius touches (as along intersection curves): the volumes tell.
// And uneven chamfers along curved edges: a tee's joint with offsets 1 and 2 and with 1 mm at 30°, a cone's foot with
// offsets 1 and 2.
//
// Usage: node tools/fixtures/gen-batch-130.mjs
//
// Writes fixtures/batch-130/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, deg, edgeAt, line, mm, planeAt, polygon, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 130, title: 'chamfers where the two rules differ: corners other than square, uneven chamfers of curved edges', helpers: '' });
const base = solidsOf('base');

// A prism of a polygon in the xy plane extruded 20 up.
const prism = (points) => [
  sketch('baseSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), polygon('baseSketch', points), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(20) }),
];
const s3 = Math.sqrt(3);
const equal = (width) => ({ chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(width) });
const chamfer = (edge, fields) => call('chamfer', 'chamfer1', { entities: edge, ...fields });

batch.add('chamfer', 'an equilateral triangular prism (side 20, 20 tall): an upright 60° edge chamfered 2 mm', [
  prism([[0, 0], [20, 0], [10, 10 * s3]]), chamfer(edgeAt(base, [0, 0, 10]), equal(2)),
]);
batch.add('chamfer', 'a regular hexagonal prism (side 10, 20 tall): an upright 120° edge chamfered 2 mm', [
  prism([[10, 0], [5, 5 * s3], [-5, 5 * s3], [-10, 0], [-5, -5 * s3], [5, -5 * s3]]), chamfer(edgeAt(base, [10, 0, 10]), equal(2)),
]);
batch.add('chamfer', 'a prism of a V (its inner corner 120°, 20 tall): the concave upright edge chamfered 2 mm', [
  prism([[0, 0], [30, 0], [30, 10], [10, 10], [0, 10 + 10 * s3]]),
  chamfer(edgeAt(base, [10, 10, 10]), equal(2)),
]);
const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
const tee = [
  lying('base', 10, 50, 0),
  sketch('branchSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('branchSketch', 'circle1', [0, 0], 3), solve('branchSketch'),
  call('extrude', 'branch', {
    entities: regions('branchSketch'), endBound: 'BoundingType.BLIND', depth: mm(20),
    operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
  }),
];
batch.add('chamfer', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint chamfered with offsets 1 and 2', [
  tee, chamfer(edgeAt(base, [3, 0, 10]), { chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(1), width2: mm(2) }),
]);
batch.add('chamfer', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint chamfered 1 mm at 30°', [
  tee, chamfer(edgeAt(base, [3, 0, 10]), { chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(1), angle: deg(30) }),
]);
batch.add('chamfer', 'a cone frustum (radius 8 to 4, 10 tall) standing on a 30 × 30 × 5 plate: its foot chamfered with offsets 1 and 2', [
  box('base', [-15, -15, -5], [15, 15, 0]),
  call('fCone', 'cone', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 10), bottomRadius: mm(8), topRadius: mm(4) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('cone')}])`, operationType: 'BooleanOperationType.UNION' }),
  chamfer(edgeAt(base, [8, 0, 0]), { chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(1), width2: mm(2) }),
]);

batch.write('tools/fixtures/gen-batch-130.mjs');

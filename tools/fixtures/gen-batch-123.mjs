// Generates fixture batch 123: which face an uneven chamfer measures its first distance on. Batch 003's straight
// edges put it on the face left of the edge as it runs (with that face's normal up), but batch 122's cylinder top
// edge put it on the right. Each case chamfers one edge with offsets 2 and 1 and returns the edge's direction
// (before the chamfer) and the chamfer face's box: a block's twelve edges, a block from a rectangle drawn the other
// way, extruded down, and tall and thin; a cylinder's top and bottom edges, a flat cylinder's, one extruded down;
// a hole's mouth and a boss's foot; and two offset-and-angle chamfers.
//
// Usage: node tools/fixtures/gen-batch-123.mjs
//
// Writes fixtures/batch-123/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, deg, edgeAt, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// The edge's direction at its middle, as Onshape runs it.
function edgeDirection(context is Context, edge is Query) returns map
{
    const line = evEdgeTangentLine(context, { "edge" : edge, "parameter" : 0.5 });
    return { "at" : round6(line.origin), "direction" : [roundToPrecision(line.direction[0], 6), roundToPrecision(line.direction[1], 6), roundToPrecision(line.direction[2], 6)] };
}

function faceBox(context is Context, faces is Query) returns map
{
    const bounds = evBox3d(context, { "topology" : faces, "tight" : true });
    return { "min" : round6(bounds.minCorner), "max" : round6(bounds.maxCorner) };
}
`;

const batch = new Batch({ number: 123, title: 'uneven chamfers: which face takes the first distance', helpers });
const base = solidsOf('base');

// A solid from a sketch on the plane z = z0 extruded `depth` up (down if negative).
const extruded = (draw, z0, depth) => [
  sketch('baseSketch', planeAt([0, 0, z0], [0, 0, 1], [1, 0, 0])), ...draw('baseSketch'), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(Math.abs(depth)), oppositeDirection: depth < 0 ? 'true' : 'false' }),
];
const block = (a = [0, 0], b = [30, 20], z0 = 0, depth = 10) => extruded((s) => [rect(s, 'rect1', a, b)], z0, depth);
const cyl = (r, z0, depth) => extruded((s) => [circle(s, 'circle1', [0, 0], r)], z0, depth);

const probed = (edge, fields = { chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(1) }) => [
  `const direction = edgeDirection(context, ${edge});`,
  call('chamfer', 'chamfer1', { entities: edge, ...fields }),
  `return { "edge" : direction, "chamfer" : faceBox(context, qCreatedBy(id + "chamfer1", EntityType.FACE)) };`,
];

const blockEdges = [
  ['top front (y = 0)', [15, 0, 10]], ['top right (x = 30)', [30, 10, 10]], ['top back (y = 20)', [15, 20, 10]], ['top left (x = 0)', [0, 10, 10]],
  ['bottom front', [15, 0, 0]], ['bottom right', [30, 10, 0]], ['bottom back', [15, 20, 0]], ['bottom left', [0, 10, 0]],
  ['upright at (0, 0)', [0, 0, 5]], ['upright at (30, 0)', [30, 0, 5]], ['upright at (30, 20)', [30, 20, 5]], ['upright at (0, 20)', [0, 20, 5]],
];
for (const [name, p] of blockEdges)
  batch.add('chamfer', `a 30 × 20 × 10 block's ${name} edge chamfered with offsets 2 and 1`, [block(), probed(edgeAt(base, p))]);
batch.add('chamfer', "a 30 × 20 × 10 block from a rectangle drawn from (30, 20) to (0, 0): its top front edge, offsets 2 and 1", [
  block([30, 20], [0, 0]), probed(edgeAt(base, [15, 0, 10])),
]);
batch.add('chamfer', 'a 30 × 20 × 10 block extruded down from z = 10: its top front edge, offsets 2 and 1', [
  block([0, 0], [30, 20], 10, -10), probed(edgeAt(base, [15, 0, 10])),
]);
batch.add('chamfer', 'a 30 × 5 × 20 block (front larger than top): its top front edge, offsets 2 and 1', [
  block([0, 0], [30, 5], 0, 20), probed(edgeAt(base, [15, 0, 20])),
]);
batch.add('chamfer', "a cylinder (radius 10, 20 tall)'s top edge, offsets 2 and 1", [cyl(10, 0, 20), probed(edgeAt(base, [10, 0, 20]))]);
batch.add('chamfer', "a cylinder (radius 10, 20 tall)'s bottom edge, offsets 2 and 1", [cyl(10, 0, 20), probed(edgeAt(base, [10, 0, 0]))]);
batch.add('chamfer', "a flat cylinder (radius 10, 4 tall)'s top edge, offsets 2 and 1", [cyl(10, 0, 4), probed(edgeAt(base, [10, 0, 4]))]);
batch.add('chamfer', "a cylinder (radius 10, 20 tall) extruded down from z = 20: its top edge, offsets 2 and 1", [cyl(10, 20, -20), probed(edgeAt(base, [10, 0, 20]))]);
batch.add('chamfer', "a 30 × 20 × 10 block with a hole (radius 4) through it: the hole's mouth, offsets 2 and 1", [
  block(),
  sketch('holeSketch', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'circle1', [15, 10], 4), solve('holeSketch'),
  call('extrude', 'hole', {
    entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  probed(edgeAt(base, [19, 10, 10])),
]);
batch.add('chamfer', "a 30 × 20 × 10 block with a boss (radius 5, 8 tall): the boss's foot, offsets 2 and 1", [
  block(),
  sketch('bossSketch', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('bossSketch', 'circle1', [15, 10], 5), solve('bossSketch'),
  call('extrude', 'boss', {
    entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(8),
    operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
  }),
  probed(edgeAt(base, [20, 10, 10])),
]);
batch.add('chamfer', "a 30 × 20 × 10 block's top front edge chamfered 2 mm at 30°", [
  block(), probed(edgeAt(base, [15, 0, 10]), { chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(2), angle: deg(30) }),
]);
batch.add('chamfer', "a 30 × 20 × 10 block's upright edge at (30, 20) chamfered 2 mm at 30°", [
  block(), probed(edgeAt(base, [30, 20, 5]), { chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(2), angle: deg(30) }),
]);

batch.write('tools/fixtures/gen-batch-123.mjs');

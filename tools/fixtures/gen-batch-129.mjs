// Generates fixture batch 129: where an equal chamfer of a curved edge meets its faces. Batch 125's spherical
// pocket's rim chamfered 1 mm put its feet 1 mm from the edge in a straight line square to it, but batch 128's tee
// joint chamfered 1 mm adds 7.27 mm³ where that rule (and OCCT) add 10.18. Each case chamfers one closed edge and
// returns the chamfer face's area and its edges' points at eight places round: a tee's joint (a pipe of radius 10, a
// branch of radius 3) at 1 and 2 mm; a boss rising from a lying cylinder; a cross hole's mouth; the spherical
// pocket's rim; a cone frustum standing on a plate (its foot, concave) and its top edge (convex).
//
// Usage: node tools/fixtures/gen-batch-129.mjs
//
// Writes fixtures/batch-129/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, edgeAt, mm, planeAt, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// The chamfer's faces: each one's area, and each of its edges' points at eight places round.
function chamferProbe(context is Context, faces is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, faces))
    {
        var edges = [];
        for (var edge in evaluateQuery(context, qAdjacent(face, AdjacencyType.EDGE, EntityType.EDGE)))
        {
            const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875], "arcLengthParameterization" : false });
            var points = [];
            for (var l in lines)
                points = append(points, round6(l.origin));
            edges = append(edges, points);
        }
        out = append(out, { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6), "edges" : edges });
    }
    return out;
}
`;

const batch = new Batch({ number: 129, title: 'where equal chamfers of curved edges meet their faces', helpers });
const base = solidsOf('base');

const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
const upFrom = (name, [cx, cy], r, z0, depth, op) => [
  sketch(name + 'Sketch', planeAt([0, 0, z0], [0, 0, 1], [1, 0, 0])), circle(name + 'Sketch', 'circle1', [cx, cy], r), solve(name + 'Sketch'),
  call('extrude', name, {
    entities: regions(name + 'Sketch'), ...(op === 'REMOVE' ? { endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true' } : { endBound: 'BoundingType.BLIND', depth: mm(depth) }),
    operationType: `NewBodyOperationType.${op}`, defaultScope: 'false', booleanScope: base,
  }),
];
const chamfered = (edge, width) => [
  call('chamfer', 'chamfer1', { entities: edge, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(width) }),
  `return { "chamfer" : chamferProbe(context, qCreatedBy(id + "chamfer1", EntityType.FACE)) };`,
];

for (const width of [1, 2]) {
  batch.add('chamfer', `a tee of a pipe of radius 10 and a branch of radius 3: the joint chamfered ${width} mm`, [
    lying('base', 10, 50, 0), upFrom('branch', [0, 0], 3, 0, 20, 'ADD'), chamfered(edgeAt(base, [3, 0, 10]), width),
  ]);
}
batch.add('chamfer', 'a cylinder of radius 10 lying along x with a boss of radius 3 rising from its axis at x = 5: the foot chamfered 1 mm', [
  lying('base', 10, 40, 0), upFrom('boss', [5, 0], 3, 0, 20, 'ADD'), chamfered(edgeAt(base, [8, 0, 10]), 1),
]);
batch.add('chamfer', 'a cylinder of radius 10 lying along x with a hole of radius 2 cut down through it at x = 5: the top mouth chamfered 0.5 mm', [
  lying('base', 10, 40, 0), upFrom('hole', [5, 0], 2, 20, 0, 'REMOVE'), chamfered(edgeAt(base, [7, 0, 10]), 0.5),
]);
batch.add('chamfer', 'a 30 × 20 × 10 block with a pocket of a sphere (radius 6) centred 2 above its top: the rim chamfered 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 12), radius: mm(6) }),
  call('booleanBodies', 'pocket', { tools: solidsOf('ball'), targets: base, operationType: 'BooleanOperationType.SUBTRACTION' }),
  chamfered(edgeAt(base, [15 + Math.sqrt(32), 10, 10]), 1),
]);
const coneOnPlate = [
  box('base', [-15, -15, -5], [15, 15, 0]),
  call('fCone', 'cone', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 10), bottomRadius: mm(8), topRadius: mm(4) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('cone')}])`, operationType: 'BooleanOperationType.UNION' }),
];
batch.add('chamfer', 'a cone frustum (radius 8 to 4, 10 tall) standing on a 30 × 30 × 5 plate: its foot chamfered 1 mm', [
  coneOnPlate, chamfered(edgeAt(base, [8, 0, 0]), 1),
]);
batch.add('chamfer', 'a cone frustum (radius 8 to 4, 10 tall) standing on a 30 × 30 × 5 plate: its top edge chamfered 1 mm', [
  coneOnPlate, chamfered(edgeAt(base, [4, 0, 10]), 1),
]);

batch.write('tools/fixtures/gen-batch-129.mjs');

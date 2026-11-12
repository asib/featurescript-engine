// Generates fixture batch 132: fillets of curved edges with sections other than circular, which batches 053 and 082
// cover only along straight edges and a cylinder's rim. A tee's joint (a pipe of radius 10, a branch of radius 3)
// filleted with conic sections (rho 0.5 and 0.7), a curvature section (magnitude 0.5) and uneven radii both ways;
// a boss rising from a lying cylinder, its foot conic; a spherical pocket's rim conic; a cone standing on a plate,
// its foot conic and its top edge uneven; two balls united, the circle where they meet conic. Each returns the
// fillet's faces' areas and their edges' points at eight places round.
//
// Usage: node tools/fixtures/gen-batch-132.mjs
//
// Writes fixtures/batch-132/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, edgeAt, mm, planeAt, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// The blend's faces: each one's area, and each of its edges' points at eight places round.
function blendProbe(context is Context, faces is Query) returns array
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

const batch = new Batch({ number: 132, title: 'conic, curvature and uneven fillets of curved edges', helpers });
const base = solidsOf('base');

const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
const upFrom = (name, [cx, cy], r, z0, depth) => [
  sketch(name + 'Sketch', planeAt([0, 0, z0], [0, 0, 1], [1, 0, 0])), circle(name + 'Sketch', 'circle1', [cx, cy], r), solve(name + 'Sketch'),
  call('extrude', name, {
    entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(depth),
    operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base,
  }),
];
const conic = (radius, rho) => ({ radius: mm(radius), crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(radius), rho: String(rho) });
const curvature = (radius, magnitude) => ({ radius: mm(radius), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(radius), magnitude: String(magnitude) });
const uneven = (radius, other, flip = false) => ({ radius: mm(radius), isAsymmetric: 'true', otherRadius: mm(other), flipAsymmetric: String(flip) });
const filleted = (edge, fields) => [
  call('fillet', 'fillet1', { entities: edge, ...fields }),
  `return { "fillet" : blendProbe(context, qCreatedBy(id + "fillet1", EntityType.FACE)) };`,
];

const tee = [lying('base', 10, 50, 0), upFrom('branch', [0, 0], 3, 0, 20)];
const joint = edgeAt(base, [3, 0, 10]);
batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted 1 mm with a conic section, rho 0.5', [tee, filleted(joint, conic(1, 0.5))]);
batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted 1 mm with a conic section, rho 0.7', [tee, filleted(joint, conic(1, 0.7))]);
batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted 1 mm with a curvature section, magnitude 0.5', [tee, filleted(joint, curvature(1, 0.5))]);
batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted unevenly, 1 and 2 mm', [tee, filleted(joint, uneven(1, 2))]);
batch.add('fillet', 'a tee of a pipe of radius 10 and a branch of radius 3: the joint filleted unevenly, 1 and 2 mm, flipped', [tee, filleted(joint, uneven(1, 2, true))]);
batch.add('fillet', 'a cylinder of radius 10 lying along x with a boss of radius 3 rising from its axis at x = 5: the foot filleted 1 mm, conic, rho 0.5', [
  lying('base', 10, 40, 0), upFrom('boss', [5, 0], 3, 0, 20), filleted(edgeAt(base, [8, 0, 10]), conic(1, 0.5)),
]);
batch.add('fillet', 'a 30 × 20 × 10 block with a pocket of a sphere (radius 6) centred 2 above its top: the rim filleted 1 mm, conic, rho 0.5', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 12), radius: mm(6) }),
  call('booleanBodies', 'pocket', { tools: solidsOf('ball'), targets: base, operationType: 'BooleanOperationType.SUBTRACTION' }),
  filleted(edgeAt(base, [15 + Math.sqrt(32), 10, 10]), conic(1, 0.5)),
]);
const coneOnPlate = [
  box('base', [-15, -15, -5], [15, 15, 0]),
  call('fCone', 'cone', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 10), bottomRadius: mm(8), topRadius: mm(4) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('cone')}])`, operationType: 'BooleanOperationType.UNION' }),
];
batch.add('fillet', 'a cone frustum (radius 8 to 4, 10 tall) standing on a 30 × 30 × 5 plate: its foot filleted 1 mm, conic, rho 0.5', [
  coneOnPlate, filleted(edgeAt(base, [8, 0, 0]), conic(1, 0.5)),
]);
batch.add('fillet', 'a cone frustum (radius 8 to 4, 10 tall) standing on a 30 × 30 × 5 plate: its top edge filleted unevenly, 1 and 2 mm', [
  coneOnPlate, filleted(edgeAt(base, [4, 0, 10]), uneven(1, 2)),
]);
batch.add('fillet', 'balls of radius 10 and 8 with centres 12 apart united: the circle where they meet filleted 1 mm, conic, rho 0.5', [
  call('opSphere', 'base', { center: vec3(0, 0, 0), radius: mm(10) }),
  call('opSphere', 'other', { center: vec3(12, 0, 0), radius: mm(8) }),
  call('booleanBodies', 'unite', { tools: `qUnion([${base}, ${solidsOf('other')}])`, operationType: 'BooleanOperationType.UNION' }),
  filleted(edgeAt(base, [7.5, Math.sqrt(43.75), 0]), conic(1, 0.5)),
]);

batch.write('tools/fixtures/gen-batch-132.mjs');

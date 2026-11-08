// Generates fixture batch 052: conic and asymmetric fillets beyond batch 003's one of each (c013, c014): other rho
// values and sizes (whose B-spline sections show how many spans Onshape gives a conic), edges along other axes,
// corners of 60° and 120°, concave edges, flipped and uneven radii, an asymmetric conic, and two edges at once.
//
// Usage: node tools/fixtures/gen-batch-052.mjs
//
// Writes fixtures/batch-052/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, edgeAt, mm, polygon, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

const batch = new Batch({ number: 52, title: 'conic and asymmetric fillets: sizes, angles, concave edges, sides' });

const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const union = (...q) => `qUnion([${q.join(', ')}])`;
const fillet = (edges, fields) => call('fillet', 'fillet1', { entities: union(...edges.map((p) => edgeAt(base, p))), ...fields });
const conic = (radius, rho) => ({ crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(radius), rho: String(rho) });
const asymmetric = (radius, other, flip = false) => ({ radius: mm(radius), isAsymmetric: 'true', otherRadius: mm(other), flipAsymmetric: String(flip) });
// A trapezoid prism: corners of 60° along the bottom (at (0, 0) and (40, 0)) and 120° along the top.
const h = 17.320508075688775;
const trapezoid = [
  sketch('s1', PLANES.xy), ...polygon('s1', [[0, 0], [40, 0], [30, h], [10, h]]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
];
// An L block: the block with a 10 × 20 × 10 block on its top at x 0..10; the inside edge along y at (10, y, 10) is
// concave.
const lBlock = [
  block, box('riser', [0, 0, 10], [10, 20, 20]),
  call('booleanBodies', 'join', { tools: union(base, solidsOf('riser')), operationType: 'BooleanOperationType.UNION' }),
];
const upright = [0, 0, 5];

for (const rho of [0.5, 0.9, 0.2]) {
  batch.add('fillet', `a block's upright edge, conic, 4 mm, rho ${rho}`, [block, fillet([upright], conic(4, rho))]);
}
batch.add('fillet', 'a 60 × 40 × 10 block\'s upright edge, conic, 10 mm, rho 0.7', [
  box('base', [0, 0, 0], [60, 40, 10]), fillet([upright], conic(10, 0.7)),
]);
batch.add('fillet', 'a block\'s upright edge, conic, 1 mm, rho 0.7', [block, fillet([upright], conic(1, 0.7))]);
batch.add('fillet', 'a block\'s top edge along x at y = 0, conic, 4 mm, rho 0.7', [block, fillet([[15, 0, 10]], conic(4, 0.7))]);
batch.add('fillet', 'a trapezoid prism\'s 60° upright edge, conic, 4 mm, rho 0.7', [trapezoid, fillet([upright], conic(4, 0.7))]);
batch.add('fillet', 'a trapezoid prism\'s 120° upright edge, conic, 4 mm, rho 0.7', [trapezoid, fillet([[10, h, 5]], conic(4, 0.7))]);
batch.add('fillet', 'an L block\'s concave inside edge, conic, 4 mm, rho 0.7', [lBlock, fillet([[10, 10, 10]], conic(4, 0.7))]);
batch.add('fillet', 'a block\'s upright edge, asymmetric 2 and 5 mm, flipped', [block, fillet([upright], asymmetric(2, 5, true))]);
batch.add('fillet', 'a block\'s top edge along x at y = 0, asymmetric 2 and 5 mm', [block, fillet([[15, 0, 10]], asymmetric(2, 5))]);
batch.add('fillet', 'a block\'s upright edge at (30, 20), asymmetric 2 and 5 mm', [block, fillet([[30, 20, 5]], asymmetric(2, 5))]);
batch.add('fillet', 'a trapezoid prism\'s 60° upright edge, asymmetric 2 and 5 mm', [trapezoid, fillet([upright], asymmetric(2, 5))]);
batch.add('fillet', 'a trapezoid prism\'s 120° upright edge, asymmetric 2 and 5 mm', [trapezoid, fillet([[10, h, 5]], asymmetric(2, 5))]);
batch.add('fillet', 'an L block\'s concave inside edge, asymmetric 2 and 5 mm', [lBlock, fillet([[10, 10, 10]], asymmetric(2, 5))]);
batch.add('fillet', 'a block\'s upright edge, asymmetric 1 and 8 mm', [block, fillet([upright], asymmetric(1, 8))]);
batch.add('fillet', 'a block\'s upright edge, asymmetric 3 and 3 mm', [block, fillet([upright], asymmetric(3, 3))]);
batch.add('fillet', 'a block\'s upright edge, conic and asymmetric, 3 and 6 mm, rho 0.6', [
  block, fillet([upright], { ...conic(3, 0.6), isAsymmetric: 'true', otherRadius: mm(6), flipAsymmetric: 'false' }),
]);
batch.add('fillet', 'a block\'s two top edges along x, conic, 4 mm, rho 0.7', [block, fillet([[15, 0, 10], [15, 20, 10]], conic(4, 0.7))]);

batch.write('tools/fixtures/gen-batch-052.mjs');

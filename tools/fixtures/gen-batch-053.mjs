// Generates fixture batch 053: curvature-continuous fillets (`FilletCrossSection.CURVATURE`), whose B-spline sections
// show how Onshape builds them — other magnitudes and sizes, edges along other axes, 60° and 120° corners, a
// concave edge, uneven radii — and conic and asymmetric fillets of edges meeting at a corner.
//
// Usage: node tools/fixtures/gen-batch-053.mjs
//
// Writes fixtures/batch-053/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, edgeAt, mm, polygon, regions, sketch, solidsOf, solve, PLANES } from './lib.mjs';

const batch = new Batch({ number: 53, title: 'curvature fillets; conic and asymmetric fillets meeting at a corner' });

const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const union = (...q) => `qUnion([${q.join(', ')}])`;
const fillet = (edges, fields) => call('fillet', 'fillet1', { entities: union(...edges.map((p) => edgeAt(base, p))), ...fields });
const curvature = (radius, magnitude) => ({ crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(radius), magnitude: String(magnitude) });
const conic = (radius, rho) => ({ crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(radius), rho: String(rho) });
const asymmetric = (radius, other) => ({ radius: mm(radius), isAsymmetric: 'true', otherRadius: mm(other), flipAsymmetric: 'false' });
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

for (const magnitude of [0.5, 0.2, 0.8, 0.95]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [block, fillet([upright], curvature(4, magnitude))]);
}
batch.add('fillet', 'a 60 × 40 × 10 block\'s upright edge, curvature, 10 mm, magnitude 0.5', [
  box('base', [0, 0, 0], [60, 40, 10]), fillet([upright], curvature(10, 0.5)),
]);
batch.add('fillet', 'a block\'s upright edge, curvature, 1 mm, magnitude 0.5', [block, fillet([upright], curvature(1, 0.5))]);
batch.add('fillet', 'a block\'s top edge along x at y = 0, curvature, 4 mm, magnitude 0.5', [block, fillet([[15, 0, 10]], curvature(4, 0.5))]);
batch.add('fillet', 'a trapezoid prism\'s 60° upright edge, curvature, 4 mm, magnitude 0.5', [trapezoid, fillet([upright], curvature(4, 0.5))]);
batch.add('fillet', 'a trapezoid prism\'s 120° upright edge, curvature, 4 mm, magnitude 0.5', [trapezoid, fillet([[10, h, 5]], curvature(4, 0.5))]);
batch.add('fillet', 'an L block\'s concave inside edge, curvature, 4 mm, magnitude 0.5', [lBlock, fillet([[10, 10, 10]], curvature(4, 0.5))]);
batch.add('fillet', 'a block\'s upright edge, curvature and asymmetric, 2 and 5 mm, magnitude 0.5', [
  block, fillet([upright], { ...curvature(2, 0.5), isAsymmetric: 'true', otherRadius: mm(5), flipAsymmetric: 'false' }),
]);
batch.add('fillet', 'a block\'s three edges at the corner (30, 20, 10), conic, 3 mm, rho 0.7', [
  block, fillet([[30, 10, 10], [15, 20, 10], [30, 20, 5]], conic(3, 0.7)),
]);
batch.add('fillet', 'a block\'s two top edges meeting at (30, 20, 10), conic, 3 mm, rho 0.7', [block, fillet([[30, 10, 10], [15, 20, 10]], conic(3, 0.7))]);
batch.add('fillet', 'a block\'s two top edges meeting at (30, 20, 10), asymmetric 2 and 4 mm', [block, fillet([[30, 10, 10], [15, 20, 10]], asymmetric(2, 4))]);
batch.add('fillet', 'a block\'s top edge and upright edge meeting at (30, 20, 10), asymmetric 2 and 4 mm', [block, fillet([[30, 10, 10], [30, 20, 5]], asymmetric(2, 4))]);

batch.write('tools/fixtures/gen-batch-053.mjs');

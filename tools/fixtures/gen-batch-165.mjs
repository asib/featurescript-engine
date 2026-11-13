// Generates fixture batch 165: variable fillets with radii at points along the edge (which the engine refused): a
// block's top edge filleted from its ends' radii through radii at points between, smoothly and straight, the probe
// the fillet's radius at every 2 mm along the edge (from how far it lies from the edge along the faces' bisector).
//
// Usage: node tools/fixtures/gen-batch-165.mjs
//
// Writes fixtures/batch-165/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, edgeAt, mm, solidsOf, vec3, vertexAt } from './lib.mjs';

const helpers = `
// The fillet's radius along the block's top edge (y = 20, z = 10, x from 0 to 40), every 2 mm: a circular fillet
// between the perpendicular top and side lies r(sqrt 2 - 1) from the edge along their bisector. Rounded to 1e-6.
function radiusProbe(context is Context, faces is Query) returns array
{
    var out = [];
    for (var i = 0; i <= 20; i += 1)
    {
        const x = 2 * i;
        const ray = line(vector(x, 20, 10) * millimeter, vector(0, -1, -1) / sqrt(2));
        const d = evDistance(context, { "side0" : faces, "side1" : ray });
        const p = d.sides[0].point;
        const along = norm(p - vector(x, 20, 10) * millimeter);
        out = append(out, roundToPrecision(along / millimeter / (sqrt(2) - 1), 6));
    }
    return out;
}
`;
const batch = new Batch({ number: 165, title: 'variable fillets with radii at points along the edge', helpers });
const base = solidsOf('base');
const edge = edgeAt(base, [20, 20, 10]);
const fillet = (ends, points, smooth) => call('fillet', 'fillet1', {
  entities: edge, radius: mm(2), isVariable: 'true', crossSection: 'FilletCrossSection.CIRCULAR',
  vertexSettings: `[${ends.map(([x, r]) => `{ "vertex" : ${vertexAt(base, [x, 20, 10])}, "vertexRadius" : ${mm(r)} }`).join(', ')}]`,
  pointOnEdgeSettings: `[${points.map(([t, r]) => `{ "edge" : ${edge}, "edgeParameter" : ${t}, "pointOnEdgeRadius" : ${mm(r)} }`).join(', ')}]`,
  smoothTransition: String(smooth),
});
const probe = `return { "radii" : radiusProbe(context, qCreatedBy(id + "fillet1", EntityType.FACE)) };`;
const block = box('base', [0, 0, 0], [40, 20, 10]);

batch.add('fillet', 'a variable fillet, 2 at both ends and 4 halfway, smooth', [...block, fillet([[0, 2], [40, 2]], [[0.5, 4]], true), probe]);
batch.add('fillet', 'the same, straight', [...block, fillet([[0, 2], [40, 2]], [[0.5, 4]], false), probe]);
batch.add('fillet', 'a variable fillet, 2 and 3 at the ends, 4 a quarter along and 1.5 at 0.6, smooth', [...block, fillet([[0, 2], [40, 3]], [[0.25, 4], [0.6, 1.5]], true), probe]);
batch.add('fillet', 'the same, straight', [...block, fillet([[0, 2], [40, 3]], [[0.25, 4], [0.6, 1.5]], false), probe]);
batch.add('fillet', 'a variable fillet, no radii at its ends (2 there), 5 at 0.3, smooth', [...block, fillet([], [[0.3, 5]], true), probe]);

batch.write('tools/fixtures/gen-batch-165.mjs');

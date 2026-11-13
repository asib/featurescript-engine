// Generates fixture batch 158: probes of which way Onshape faces a flat fill (batch 157 c004 faced up where the engine
// faced down): a rectangle of lines in the top plane drawn anticlockwise, the same drawn clockwise, the same listed
// starting from another side, a rectangle in an upright plane, a triangle, and a rectangle in a tilted plane.
//
// Usage: node tools/fixtures/gen-batch-158.mjs
//
// Writes fixtures/batch-158/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, planeAt, sketch, solve } from './lib.mjs';

const helpers = `
// The fill's area and its normal at the middle of its parameters, rounded to 1e-6.
function facing(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return round(x * 1e6) / 1e6; };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        out = append(out, [r(evArea(context, { "entities" : f }) / millimeter ^ 2), [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])]]);
    }
    return out;
}
`;
const batch = new Batch({ number: 158, title: 'probes: which way flat fills face', helpers });
const sheets = `qBodyType(qCreatedBy(id + "fill1", EntityType.BODY), BodyType.SHEET)`;
const edgesOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const fill = (names) => call('fill', 'fill1', {
  edges: `[${names.map((n) => `{ "entities" : ${n}, "continuity" : GeometricContinuity.G0 }`).join(', ')}]`,
  surfaceOperationType: 'NewSurfaceOperationType.NEW',
});
const probe = `return { "faces" : facing(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`;
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
// Each side its own sketch, so the fill takes them one by one in the order given.
const sides = (plane, points) => points.map((p, i) => [sketch(`e${i}`, plane), line(`e${i}`, 'l1', p, points[(i + 1) % points.length]), solve(`e${i}`)]).flat();
const rect = [[0, 0], [30, 0], [30, 20], [0, 20]];

batch.add('fill', 'a 30 × 20 rectangle of lines in the top plane drawn anticlockwise', [
  ...sides(top, rect), fill(rect.map((_, i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'the rectangle drawn clockwise', [
  ...sides(top, [...rect].reverse()), fill(rect.map((_, i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'the rectangle drawn anticlockwise, listed from its third side', [
  ...sides(top, rect), fill([2, 3, 0, 1].map((i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'the rectangle drawn anticlockwise, its sides listed alternately', [
  ...sides(top, rect), fill([0, 2, 1, 3].map((i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'a 30 × 20 rectangle in the upright plane y = 0 drawn anticlockwise in its sketch', [
  ...sides(planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0]), rect), fill(rect.map((_, i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'a triangle in the top plane drawn clockwise', [
  ...sides(top, [[0, 0], [15, 25], [30, 0]]), fill([0, 1, 2].map((i) => edgesOf(`e${i}`))), probe,
]);
batch.add('fill', 'a rectangle in a plane tilted 30° about x, drawn anticlockwise in its sketch', [
  ...sides(planeAt([0, 0, 0], [0, -0.5, Math.sqrt(3) / 2], [1, 0, 0]), rect), fill(rect.map((_, i) => edgesOf(`e${i}`))), probe,
]);

batch.write('tools/fixtures/gen-batch-158.mjs');

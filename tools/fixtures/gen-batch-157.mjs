// Generates fixture batch 157: probes of fills with tangency conditions: a gap between a level sheet and a sheet
// rising at 26.6° bridged by a fill tangent to both along their edges (lines at the sides), the same tangent to the
// level sheet only, and tangent to the rising sheet only.
//
// Usage: node tools/fixtures/gen-batch-157.mjs
//
// Writes fixtures/batch-157/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
// The fill's area, its point and normal at the middle of its parameters, and how far above (along z) it lies over
// points across the gap, rounded to 1e-6.
function fillProbe(context is Context, faces is Query) returns array
{
    var out = [];
    const r = function(x) { return round(x * 1e6) / 1e6; };
    for (var f in evaluateQuery(context, faces))
    {
        const p = evFaceTangentPlane(context, { "face" : f, "parameter" : vector(0.5, 0.5) });
        var heights = [];
        for (var x in [2.5, 5, 7.5])
        {
            for (var y in [5, 10])
            {
                const d = evDistance(context, { "side0" : f, "side1" : line(vector(x, y, 0) * millimeter, vector(0, 0, 1)) });
                heights = append(heights, r(d.sides[0].point[2] / millimeter));
            }
        }
        out = append(out, [r(evArea(context, { "entities" : f }) / millimeter ^ 2),
                [r(p.origin[0] / millimeter), r(p.origin[1] / millimeter), r(p.origin[2] / millimeter)],
                [r(p.normal[0]), r(p.normal[1]), r(p.normal[2])], heights]);
    }
    return out;
}
`;
const batch = new Batch({ number: 157, title: 'probes: fills with tangency conditions across a gap between sheets', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheets = `qBodyType(qCreatedBy(id + "fill1", EntityType.BODY), BodyType.SHEET)`;
const edgeOf = (body, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${body}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE), ${vec3(...p)})`;
// A level sheet (x from -20 to 0, y from 0 to 20, z = 0) and a sheet rising from x = 10, z = 5 along (2, 0, 1); lines
// along y = 0 and y = 20 from one sheet's edge to the other's.
const setup = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('bSketch', planeAt([10, 0, 5], [2 / Math.sqrt(5), 0, 1 / Math.sqrt(5)], [0, 1, 0])), line('bSketch', 'l1', [0, 0], [20, 0]), solve('bSketch'),
  call('extrude', 'b', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('bSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('s1', 'l1', [0, 0], [10, 5]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), line('s2', 'l1', [0, 0], [10, 5]), solve('s2'),
];
const fill = (ca, cb) => call('fill', 'fill1', {
  edges: `[{ "entities" : ${edgeOf('a', [0, 10, 0])}, "continuity" : GeometricContinuity.${ca} }, { "entities" : ${edgeOf('b', [10, 10, 5])}, "continuity" : GeometricContinuity.${cb} }, { "entities" : qUnion([${wires('s1')}, ${wires('s2')}]), "continuity" : GeometricContinuity.G0 }]`,
  surfaceOperationType: 'NewSurfaceOperationType.NEW',
});
const probe = `return { "faces" : fillProbe(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`;

batch.add('fill', 'a gap between a level sheet and a rising one bridged by a fill tangent to both', [...setup, fill('G1', 'G1'), probe]);
batch.add('fill', 'the gap bridged by a fill tangent to the level sheet only', [...setup, fill('G1', 'G0'), probe]);
batch.add('fill', 'the gap bridged by a fill tangent to the rising sheet only', [...setup, fill('G0', 'G1'), probe]);
batch.add('fill', 'the gap bridged by a fill with no tangency', [...setup, fill('G0', 'G0'), probe]);

batch.write('tools/fixtures/gen-batch-157.mjs');

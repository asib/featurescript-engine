// Generates fixture batch 159: boundary surfaces matching the tangency of a flat face next to a profile: from a level
// sheet's edge up to a line above it, from the edge of a sheet rising at 26.6°, from a level sheet's edge to an arc,
// both ends matching two sheets, and with a magnitude of 2.
//
// Usage: node tools/fixtures/gen-batch-159.mjs
//
// Writes fixtures/batch-159/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, planeAt, sketch, solve, vec3 } from './lib.mjs';

const helpers = `
// The surface's area, its point and normal at the middle of its parameters, and its heights (along z) over points
// across it, rounded to 1e-6.
function surfaceProbe(context is Context, faces is Query) returns array
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
const batch = new Batch({ number: 159, title: 'boundary surfaces matching the tangency of flat faces next to their profiles', helpers });
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetEdge = (body, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${body}", EntityType.BODY), BodyType.SHEET), EntityType.EDGE), ${vec3(...p)})`;
const sheets = `qBodyType(qCreatedBy(id + "surface1", EntityType.BODY), BodyType.SHEET)`;
const profile = (entities, condition, magnitude = 1) =>
  `{ "uProfileEntities" : ${entities}, "uCondition" : BSurfEndDerivativeType.${condition}, "uMagnitude" : ${magnitude}, "uReference" : qNothing(), "uDirection" : qNothing() }`;
const surface = (first, second) => call('boundarySurface', 'surface1', { uProfilesArray: `[${first}, ${second}]`, vProfilesArray: '[]' });
const probe = `return { "faces" : surfaceProbe(context, qOwnedByBody(${sheets}, EntityType.FACE)) };`;
// A level sheet (x from -20 to 0, y from 0 to 20); a sheet rising from x = 10, z = 5 along (2, 0, 1).
const level = [
  sketch('aSketch', planeAt([0, 0, 0], [-1, 0, 0], [0, 1, 0])), line('aSketch', 'l1', [0, 0], [20, 0]), solve('aSketch'),
  call('extrude', 'a', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('aSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
const rising = [
  sketch('bSketch', planeAt([10, 0, 5], [2 / Math.sqrt(5), 0, 1 / Math.sqrt(5)], [0, 1, 0])), line('bSketch', 'l1', [0, 0], [20, 0]), solve('bSketch'),
  call('extrude', 'b', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wires('bSketch'), endBound: 'BoundingType.BLIND', depth: '20 * millimeter' }),
];
// A line along y at x = 10, z = 5, drawn in the upright plane x = 10.
const target = [sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 5], [20, 5]), solve('t')];

batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency, to a line 10 across and 5 up', [
  ...level, ...target, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency with magnitude 2, to the line', [
  ...level, ...target, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT', 2), profile(wires('t'), 'DEFAULT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge to a rising sheet\'s edge, matching both', [
  ...level, ...rising, surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(sheetEdge('b', [10, 10, 5]), 'MATCH_TANGENT')), probe,
]);
batch.add('boundary-surface', 'from the line to a rising sheet\'s edge, matching its tangency', [
  sketch('t', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), line('t', 'l1', [0, 0], [20, 0]), solve('t'), ...rising,
  surface(profile(wires('t'), 'DEFAULT'), profile(sheetEdge('b', [10, 10, 5]), 'MATCH_TANGENT')), probe,
]);
batch.add('boundary-surface', 'from a level sheet\'s edge, matching its tangency, to an arc over x = 10 rising 4 at its middle', [
  ...level, sketch('t', planeAt([10, 0, 0], [1, 0, 0], [0, 1, 0])), arc('t', 'a1', [0, 5], [10, 9], [20, 5]), solve('t'),
  surface(profile(sheetEdge('a', [0, 10, 0]), 'MATCH_TANGENT'), profile(wires('t'), 'DEFAULT')), probe,
]);

batch.write('tools/fixtures/gen-batch-159.mjs');

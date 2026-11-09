// Generates fixture batch 074: lofts with connections (a square to a square turned 45°, connected corner to
// corner either way; a square to a square, connected a quarter turn round), with straight guides along a square
// loft's corners, and along a path (two circles in planes square to a quarter circle).
//
// Usage: node tools/fixtures/gen-batch-074.mjs
//
// Writes fixtures/batch-074/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Each edge of a body that isn't straight: three points along it.
function curvedEdges(context is Context, body is Query) returns array
{
    var out = [];
    const edges = qOwnedByBody(body, EntityType.EDGE);
    for (var edge in evaluateQuery(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}
`;

const batch = new Batch({ number: 74, title: 'lofts with connections, guides and a path', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const h = 7.0710678118654755;
const square = (s) => [sketch(s, at(0)), rect(s, 'rect1', [-10, -10], [10, 10]), solve(s)];
const turned = (s) => [
  sketch(s, at(20)), line(s, 'l1', [h, 0], [0, h]), line(s, 'l2', [0, h], [-h, 0]), line(s, 'l3', [-h, 0], [0, -h]), line(s, 'l4', [0, -h], [h, 0]),
  solve(s),
];
const small = (s) => [sketch(s, at(20)), rect(s, 'rect1', [-5, -5], [5, 5]), solve(s)];
const profiles = (names) => `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`;
const vertex = (p) => `qContainsPoint(qUnion([qCreatedBy(id + "s1", EntityType.VERTEX), qCreatedBy(id + "s2", EntityType.VERTEX)]), ${vec3(...p)})`;
const connect = (a, b) => ({
  matchConnections: 'true',
  connections: `[{ "connectionEntities" : qUnion([${vertex(a)}, ${vertex(b)}]), "connectionEdgeQueries" : qNothing(), "connectionEdgeParameters" : [] }]`,
});
const probe = returns({ faces: `faceProbe(context, ${loftBody})`, curves: `curvedEdges(context, ${loftBody})` });
const loft = (fields) => call('loft', 'loft1', { sheetProfilesArray: profiles(['s1', 's2']), ...fields });

batch.add('loft', 'a 20 mm square to a 10 mm one turned 45°, 20 mm above', [square('s1'), turned('s2'), loft({}), probe]);
batch.add('loft', 'the same, the corner at (10, 10) connected to the one at (7.07, 0)', [square('s1'), turned('s2'), loft(connect([10, 10, 0], [h, 0, 20])), probe]);
batch.add('loft', 'the same, the corner at (10, 10) connected to the one at (0, 7.07)', [square('s1'), turned('s2'), loft(connect([10, 10, 0], [0, h, 20])), probe]);
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, the corner at (10, 10) connected to the one at (-5, 5)', [
  square('s1'), small('s2'), loft(connect([10, 10, 0], [-5, 5, 20])), probe,
]);
// Four guides, straight from each corner of the bottom square to the one above it.
const guides = [[10, 10], [-10, 10], [-10, -10], [10, -10]].map(([x, y], i) => [
  sketch(`g${i}`, planeAt([x, y, 0], [y === x ? 1 : -1, y === x ? -1 : -1, 0].map((c) => c / Math.SQRT2), [x / Math.abs(x) * 0.5 / 1.118033988749895, y / Math.abs(y) * 0.5 / 1.118033988749895, 1 / 1.118033988749895])),
  line(`g${i}`, 'l1', [0, 0], [11.180339887498949, 0]), solve(`g${i}`),
]);
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, with straight guides from corner to corner', [
  square('s1'), small('s2'), guides,
  loft({
    addGuides: 'true',
    guidesArray: `[${[0, 1, 2, 3].map((i) => `{ "guideEntities" : qUnion([qCreatedBy(id + "g${i}", EntityType.EDGE)]), "guideDerivativeType" : LoftGuideDerivativeType.DEFAULT, "guideDerivativeMagnitude" : 1 }`).join(', ')}]`,
  }),
  probe,
]);
// Circles of radius 3 about (20, 0, 0) in the plane z = 0 and about (0, 0, 20) in the plane x = 0, along the quarter
// circle of radius 20 about the origin in the xz plane between them.
batch.add('loft', 'circles of radius 3 at the ends of a quarter circle of radius 20, lofted along it (5 sections)', [
  sketch('s1', planeAt([20, 0, 0], [0, 0, 1], [1, 0, 0])), circle('s1', 'c1', [0, 0], 3), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [1, 0, 0], [0, 1, 0])), circle('s2', 'c1', [0, 0], 3), solve('s2'),
  sketch('path', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('path', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('path'),
  loft({ addSections: 'true', spine: `qCreatedBy(id + "path", EntityType.EDGE)`, sectionCount: '5' }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
]);

batch.write('tools/fixtures/gen-batch-074.mjs');

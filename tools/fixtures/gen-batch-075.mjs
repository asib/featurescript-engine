// Generates fixture batch 075: lofts with valid connections (each the region's vertex on each profile), straight and
// bulging guides, and paths (straight, and a quarter circle with its far circle facing along it). Batch 074's
// connections took every vertex at a corner and its guides missed the profiles.
//
// Usage: node tools/fixtures/gen-batch-075.mjs
//
// Writes fixtures/batch-075/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, circle, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 75, title: 'lofts with valid connections, guides and paths', helpers });

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
// A region's own vertex: the corner of the sheet the sketch's regions make.
const regionVertex = (s, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.SHEET), EntityType.VERTEX), ${vec3(...p)})`;
const connect = (a, b) => ({
  matchConnections: 'true',
  connections: `[{ "connectionEntities" : qUnion([${regionVertex('s1', a)}, ${regionVertex('s2', b)}]), "connectionEdgeQueries" : qNothing(), "connectionEdgeParameters" : [] }]`,
});
const probe = returns({
  faces: `faceProbe(context, ${loftBody})`, curves: `curvedEdges(context, ${loftBody})`,
  connectionCounts: `[evaluateQueryCount(context, ${regionVertex('s1', [10, 10, 0])}), evaluateQueryCount(context, ${regionVertex('s2', [h, 0, 20])})]`,
});
const loft = (fields) => call('loft', 'loft1', { sheetProfilesArray: profiles(['s1', 's2']), ...fields });
const guided = (names) => ({
  addGuides: 'true',
  guidesArray: `[${names.map((n) => `{ "guideEntities" : qUnion([qCreatedBy(id + "${n}", EntityType.EDGE)]), "guideDerivativeType" : LoftGuideDerivativeType.DEFAULT, "guideDerivativeMagnitude" : 1 }`).join(', ')}]`,
});

// ----- connections -----

batch.add('loft', 'a 20 mm square to a 10 mm one turned 45°, the corner at (10, 10) connected to the one at (7.07, 0)', [
  square('s1'), turned('s2'), loft(connect([10, 10, 0], [h, 0, 20])), probe,
]);
batch.add('loft', 'the same, the corner at (10, 10) connected to the one at (0, 7.07)', [square('s1'), turned('s2'), loft(connect([10, 10, 0], [0, h, 20])), probe]);
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, the corner at (10, 10) connected to the one at (-5, 5)', [
  square('s1'), small('s2'), loft(connect([10, 10, 0], [-5, 5, 20])), probe,
]);

// ----- guides -----

const corners = [[10, 10], [-10, 10], [-10, -10], [10, -10]];
batch.add('loft', 'a 20 mm square to a 10 mm one 20 mm above, with straight guides from corner to corner', [
  square('s1'), small('s2'), corners.map(([x, y], i) => axisLine(`g${i}`, [x, y, 0], [x / 2, y / 2, 20])),
  loft(guided(['g0', 'g1', 'g2', 'g3'])), probe,
]);
// One guide in the xz plane from the middle of the bottom square's edge at x = 10 bulging out to the top's at x = 5.
batch.add('loft', 'the same squares with one guide bulging out, an arc from (10, 0, 0) through (13, 0, 10) to (5, 0, 20)', [
  square('s1'), small('s2'),
  sketch('g0', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('g0', 'a1', [10, 0], [13, 10], [5, 20]), solve('g0'),
  loft(guided(['g0'])), probe,
]);

// ----- paths -----

batch.add('loft', 'circles of radius 10 at z = 0 and 5 at z = 30, lofted along the straight line between their centres (3 sections)', [
  sketch('s1', at(0)), circle('s1', 'c1', [0, 0], 10), solve('s1'), sketch('s2', at(30)), circle('s2', 'c1', [0, 0], 5), solve('s2'),
  axisLine('path', [0, 0, 0], [0, 0, 30]),
  loft({ addSections: 'true', spine: `qCreatedBy(id + "path", EntityType.EDGE)`, sectionCount: '3' }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
]);
// As batch 074 c006, the far circle's plane facing along the path (-x).
batch.add('loft', 'circles of radius 3 at the ends of a quarter circle of radius 20, the far one facing along it, lofted along it', [
  sketch('s1', planeAt([20, 0, 0], [0, 0, 1], [1, 0, 0])), circle('s1', 'c1', [0, 0], 3), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [-1, 0, 0], [0, 1, 0])), circle('s2', 'c1', [0, 0], 3), solve('s2'),
  sketch('path', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('path', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('path'),
  loft({ addSections: 'true', spine: `qCreatedBy(id + "path", EntityType.EDGE)`, sectionCount: '5' }),
  returns({ faces: `faceProbe(context, ${loftBody})` }),
]);

batch.write('tools/fixtures/gen-batch-075.mjs');

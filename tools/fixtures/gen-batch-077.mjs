// Generates fixture batch 077: how Onshape matches sheet metal loft profiles (evTessellatedLoftMatches for an L to
// a line, two arcs and a line to a skew line), the end faces of a slanted sheet metal loft bend (their surfaces and
// points along their edges), and a block's top replaced by a sphere's surface.
//
// Usage: node tools/fixtures/gen-batch-077.mjs
//
// Writes fixtures/batch-077/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, faceAt, line, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// A sheet metal loft's matched points: each pair's positions, and for a point inside an edge its parameter.
function matchProbe(matches is array) returns array
{
    var out = [];
    for (var m in matches)
    {
        var pair = [];
        for (var item in m.match)
        {
            var entry = { "position" : round6(item.position) };
            if (item.parameter != undefined)
                entry.parameter = roundToPrecision(item.parameter, 9);
            pair = append(pair, entry);
        }
        out = append(out, pair);
    }
    return out;
}

function knotsProbe(knots) returns array
{
    var out = [];
    for (var i = 0; i < size(knots); i += 1)
        out = append(out, roundToPrecision(knots[i], 9));
    return out;
}

// A part's B-spline faces: their surfaces (degrees, knots, poles) and nine points along each of their edges.
function splineFaces(context is Context, part is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qGeometry(qOwnedByBody(part, EntityType.FACE), GeometryType.OTHER_SURFACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            const s = evSurfaceDefinition(context, { "face" : face });
            if (s is BSplineSurface)
            {
                var rows = [];
                for (var i = 0; i < size(s.controlPoints); i += 1)
                {
                    var points = [];
                    for (var j = 0; j < size(s.controlPoints[i]); j += 1)
                        points = append(points, round6(s.controlPoints[i][j]));
                    rows = append(rows, points);
                }
                entry.bspline = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational,
                                  "uKnots" : knotsProbe(s.uKnots), "vKnots" : knotsProbe(s.vKnots), "poles" : rows };
            }
        }
        catch (error)
        {
            entry.bspline = "failed";
        }
        var edges = [];
        for (var edge in evaluateQuery(context, qAdjacent(face, AdjacencyType.EDGE, EntityType.EDGE)))
        {
            const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1], "arcLengthParameterization" : false });
            edges = append(edges, mapArray(lines, function(l) { return round6(l.origin); }));
        }
        entry.edges = edges;
        out = append(out, entry);
    }
    return out;
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
`;

const batch = new Batch({ number: 77, title: 'sheet metal loft matches and slanted bend ends, a top replaced by a sphere', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const up = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const matches = returns({
  matches: `matchProbe(evTessellatedLoftMatches(context, { "profileSubqueries" : [${wireEdges('p1')}, ${wireEdges('p2')}], "chordalTolerance" : 0.1 * millimeter, "connections" : [] }).matches)`,
});

// ----- loft matches -----

batch.add('sheet-metal-loft', 'the matches of an L (0, 0)-(30, 0)-(30, 20) to a line (0, 10)-(40, 10) 20 mm up', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), line('p1', 'l2', [30, 0], [30, 20]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [40, 10]), solve('p2'), matches,
]);
batch.add('sheet-metal-loft', 'the matches of a quarter circle of radius 20 to one of radius 10 20 mm up', [
  sketch('p1', xy), arc('p1', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('p1'),
  sketch('p2', up(20)), arc('p2', 'a1', [10, 0], [7.0710678118654755, 7.0710678118654755], [0, 10]), solve('p2'), matches,
]);
batch.add('sheet-metal-loft', 'the matches of a line (0, 0)-(30, 0) to a line 20 mm up turned 10°', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [29.544232590366242, 15.209445330007912]), solve('p2'), matches,
]);

// ----- a slanted bend's ends -----

batch.add('sheet-metal-loft', 'batch 044 c002\'s loft (a line to a line turned 10°): its B-spline faces and their edges', [
  sketch('p1', xy), line('p1', 'l1', [0, 0], [30, 0]), solve('p1'),
  sketch('p2', up(20)), line('p2', 'l1', [0, 10], [29.544232590366242, 15.209445330007912]), solve('p2'),
  call('sheetMetalLoft', 'sm1', {
    surfaceOperationType: 'NewSurfaceOperationType.NEW', profile1: wireEdges('p1'), profile2: wireEdges('p2'), matchConnections: 'false',
    chordalTolerance: mm(0.1), thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
  }),
  returns({ ends: `splineFaces(context, qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID))` }),
]);

// ----- a top replaced by a sphere -----

const base = solidsOf('base');
batch.add('replace-face', 'a 40 x 30 x 10 block\'s top replaced by a sphere of radius 50 about (20, 15, -32)', [
  box('base', [0, 0, 0], [40, 30, 10]),
  call('opSphere', 'tool', { center: 'vector(20, 15, -32) * millimeter', radius: mm(50) }),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [20, 15, 10]), templateFace: `qOwnedByBody(${solidsOf('tool')}, EntityType.FACE)`, oppositeSense: 'false', offset: mm(0) }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

batch.write('tools/fixtures/gen-batch-077.mjs');

// Generates fixture batch 081: what sets how far Onshape tilts a slanted sheet metal bend's far end (batch 044
// c001's L to an L shifted 10 mm along y, its far end tilted 0.48 mm into the bend): the same with a bend radius of
// 2, 1 mm thick, shifted 5 mm and 20 mm, and with the material on the other side.
//
// Usage: node tools/fixtures/gen-batch-081.mjs
//
// Writes fixtures/batch-081/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

function knotsProbe(knots) returns array
{
    var out = [];
    for (var i = 0; i < size(knots); i += 1)
        out = append(out, roundToPrecision(knots[i], 9));
    return out;
}

// A part's B-spline faces: their surfaces (degrees, knots, poles) and their vertices.
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
        var vertices = [];
        for (var v in evaluateQuery(context, qAdjacent(face, AdjacencyType.VERTEX, EntityType.VERTEX)))
            vertices = append(vertices, round6(evVertexPoint(context, { "vertex" : v })));
        entry.vertices = vertices;
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

const batch = new Batch({ number: 81, title: 'how far a slanted sheet metal bend\'s far end tilts', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const ell = (s, z, dy) => [sketch(s, at(z)), line(s, 'l1', [0, dy], [30, dy]), line(s, 'l2', [30, dy], [30, 20 + dy]), solve(s)];
const smPart = solidsOf('sm1');
const loftCase = (shift, { radius = 1, thickness = 2, opposite = false } = {}) => [
  ell('p1', 0, 0), ell('p2', 20, shift),
  call('sheetMetalLoft', 'sm1', {
    surfaceOperationType: 'NewSurfaceOperationType.NEW', profile1: wireEdges('p1'), profile2: wireEdges('p2'), matchConnections: 'false',
    chordalTolerance: mm(0.1), thickness: mm(thickness), oppositeDirection: opposite ? 'true' : 'false', radius: mm(radius), flipDirectionUp: 'false',
  }),
  returns({ faces: `faceProbe(context, ${smPart})`, ends: `splineFaces(context, ${smPart})` }),
];

batch.add('sheet-metal-loft', 'batch 044 c001\'s L to an L (shifted 10 mm along y, 20 mm up), the bend radius 2', loftCase(10, { radius: 2 }));
batch.add('sheet-metal-loft', 'the same L to an L, radius 1, 1 mm thick', loftCase(10, { thickness: 1 }));
batch.add('sheet-metal-loft', 'the L to the same L shifted 5 mm along y, 20 mm up, radius 1, 2 mm thick', loftCase(5));
batch.add('sheet-metal-loft', 'the L to the same L shifted 20 mm along y, 20 mm up, radius 1, 2 mm thick', loftCase(20));
batch.add('sheet-metal-loft', 'batch 044 c001\'s L to an L, the material on the other side', loftCase(10, { opposite: true }));

batch.write('tools/fixtures/gen-batch-081.mjs');

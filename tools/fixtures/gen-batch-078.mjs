// Generates fixture batch 078: where Onshape tilts a slanted sheet metal bend's far end (batch 044 c001's L to an
// L, mirrored, with its profiles swapped, and shifted along the first leg instead), a sheet metal loft with a
// connection and one between closed profiles, and variable fillets of a concave edge, of an arc and of two edges.
//
// Usage: node tools/fixtures/gen-batch-078.mjs
//
// Writes fixtures/batch-078/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, edgeAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vertexAt } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// A sheet metal loft's matched points: each pair's positions, and for a point on an edge its parameter.
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

const batch = new Batch({ number: 78, title: 'slanted sheet metal bends\' far ends, a loft connection and closed profiles, variable fillets', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wires = (s) => `qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE)`;
const wireEdges = (s) => `qOwnedByBody(${wires(s)}, EntityType.EDGE)`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const smLoft = (fields = {}) =>
  call('sheetMetalLoft', 'sm1', {
    surfaceOperationType: 'NewSurfaceOperationType.NEW', profile1: wireEdges('p1'), profile2: wireEdges('p2'), matchConnections: 'false',
    chordalTolerance: mm(0.1), thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', ...fields,
  });
const smPart = solidsOf('sm1');
const ell = (s, z, [dx, dy]) => [
  sketch(s, at(z)), line(s, 'l1', [dx, dy], [30 + dx, dy]), line(s, 'l2', [30 + dx, dy], [30 + dx, 20 + dy]), solve(s),
];
const endsProbe = returns({ faces: `faceProbe(context, ${smPart})`, ends: `splineFaces(context, ${smPart})` });

// ----- a slanted bend's far end -----

// Batch 044 c001 was the L (0, 0)-(30, 0)-(30, 20) at z = 0 to the same L at z = 20 shifted 10 along y.
batch.add('sheet-metal-loft', 'an L (0, 0)-(30, 0)-(30, 20) to the same L 20 mm up shifted 10 mm back along y', [
  ell('p1', 0, [0, 0]), ell('p2', 20, [0, -10]), smLoft(), endsProbe,
]);
batch.add('sheet-metal-loft', 'batch 044 c001\'s L to an L with the profiles swapped: from the L 20 mm up, shifted 10 mm along y, to the one at z = 0', [
  ell('p1', 20, [0, 10]), ell('p2', 0, [0, 0]), smLoft(), endsProbe,
]);
batch.add('sheet-metal-loft', 'an L (0, 0)-(30, 0)-(30, 20) to the same L 20 mm up shifted 10 mm along x', [
  ell('p1', 0, [0, 0]), ell('p2', 20, [10, 0]), smLoft(), endsProbe,
]);

// ----- a connection, closed profiles -----

// The L's corner (one of the two coincident vertices of its lines) connected to the line's start.
const corner = `qNthElement(${vertexAt(wires('p1'), [30, 0, 0])}, 0)`;
const start = vertexAt(wires('p2'), [0, 10, 20]);
batch.add('sheet-metal-loft', 'an L (0, 0)-(30, 0)-(30, 20) to a line (0, 10)-(40, 10) 20 mm up, the L\'s corner connected to the line\'s start', [
  ell('p1', 0, [0, 0]), sketch('p2', at(20)), line('p2', 'l1', [0, 10], [40, 10]), solve('p2'),
  smLoft({
    matchConnections: 'true',
    connections: `[{ "connectionEntities" : qUnion([${corner}, ${start}]), "isRip" : false, "connectionEdgeQueries" : qNothing(), "connectionEdgeParameters" : [] }]`,
  }),
  returns({
    matches: `matchProbe(evTessellatedLoftMatches(context, { "profileSubqueries" : [${wireEdges('p1')}, ${wireEdges('p2')}], "chordalTolerance" : 0.1 * millimeter, "connections" : [{ "connectionEntities" : qUnion([${corner}, ${start}]), "connectionEdges" : [], "connectionEdgeParameters" : [], "removeRedundantEdge" : true }] }).matches)`,
    faces: `faceProbe(context, ${smPart})`,
  }),
]);
batch.add('sheet-metal-loft', 'a 20 mm square at z = 0 to a 10 mm square 20 mm up, both closed', [
  sketch('p1', at(0)), rect('p1', 'r', [-10, -10], [10, 10]), solve('p1'), sketch('p2', at(20)), rect('p2', 'r', [-5, -5], [5, 5]), solve('p2'),
  smLoft(), returns({ faces: `faceProbe(context, ${smPart})` }),
]);

// ----- variable fillets -----

const base = solidsOf('base');
const filletProbe = returns({ faces: `faceProbe(context, ${base})` });
const conic = (settings) => ({
  radius: mm(3), nonCircularRadius: mm(3), isVariable: 'true', crossSection: 'FilletCrossSection.CONIC', rho: '0.7', smoothTransition: 'true',
  vertexSettings: `[${settings.map(([p, r]) => `{ "vertex" : ${vertexAt(base, p)}, "vertexRadius" : ${mm(r)}, "variableRho" : 0.7 }`).join(', ')}]`,
});
// An L section in the yz plane, extruded 40 mm along x: its inner edge runs along x at y = 10, z = 10.
batch.add('fillet', 'a variable conic fillet (rho 0.7), 2 mm to 4 mm, along the concave edge of an L-shaped block', [
  sketch('s1', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])),
  line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 10]), line('s1', 'l3', [30, 10], [10, 10]),
  line('s1', 'l4', [10, 10], [10, 30]), line('s1', 'l5', [10, 30], [0, 30]), line('s1', 'l6', [0, 30], [0, 0]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(40) }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 10, 10]), ...conic([[[0, 10, 10], 2], [[40, 10, 10], 4]]) }),
  filletProbe,
]);
// A half disc of radius 15 extruded 10 mm: its top arc runs from (15, 0, 10) round to (-15, 0, 10).
batch.add('fillet', 'a variable conic fillet (rho 0.7), 2 mm to 4 mm, along the top arc of a half disc 10 mm thick', [
  sketch('s1', at(0)), line('s1', 'l1', [-15, 0], [15, 0]), arc('s1', 'a1', [15, 0], [0, 15], [-15, 0]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [0, 15, 10]), ...conic([[[15, 0, 10], 2], [[-15, 0, 10], 4]]) }),
  filletProbe,
]);
batch.add('fillet', 'a variable curvature fillet (magnitude 0.5), 2 mm to 5 mm, along a block\'s top front and back edges at once', [
  sketch('s1', at(0)), rect('s1', 'r', [0, 0], [40, 30]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(12) }),
  call('fillet', 'fillet1', {
    entities: `qUnion([${edgeAt(base, [20, 0, 12])}, ${edgeAt(base, [20, 30, 12])}])`, radius: mm(3), nonCircularRadius: mm(3), isVariable: 'true',
    crossSection: 'FilletCrossSection.CURVATURE', magnitude: '0.5', smoothTransition: 'true',
    vertexSettings: `[${[[0, 0, 2], [40, 0, 5], [0, 30, 2], [40, 30, 5]].map(([x, y, r]) => `{ "vertex" : ${vertexAt(base, [x, y, 12])}, "vertexRadius" : ${mm(r)}, "variableMagnitude" : 0.5 }`).join(', ')}]`,
  }),
  filletProbe,
]);

batch.write('tools/fixtures/gen-batch-078.mjs');

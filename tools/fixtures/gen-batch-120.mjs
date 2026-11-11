// Generates fixture batch 120: geodesic offset curves across a sphere and a cone from edges that aren't circles round
// their axes (offsetCurveOnFace's untested curved faces; batch 067 found Onshape makes no curve for such circles): a
// sphere band's meridian edge offset 2 mm round it, and a half cone's straight edge offset 3 mm round it.
//
// Usage: node tools/fixtures/gen-batch-120.mjs
//
// Writes fixtures/batch-120/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, axisLine, call, edgeAt, edgeOf, line, mm, sketch, solve, regions } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Knot arrays, weights and control point matrices are typed: indexed, as std does, rather than iterated.
function numbersProbe(numbers) returns array
{
    var out = [];
    for (var i = 0; i < size(numbers); i += 1)
        out = append(out, roundToPrecision(numbers[i], 9));
    return out;
}

function bsplineProbe(s) returns map
{
    var b = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational };
    try
    {
        b.uKnots = numbersProbe(s.uKnots);
        b.vKnots = numbersProbe(s.vKnots);
    }
    catch (error)
    {
        b.knots = "failed";
    }
    try
    {
        const matrix = s.controlPoints;
        var rows = [];
        for (var i = 0; i < size(matrix); i += 1)
        {
            var points = [];
            for (var j = 0; j < size(matrix[i]); j += 1)
                points = append(points, round6(matrix[i][j]));
            rows = append(rows, points);
        }
        b.poles = rows;
    }
    catch (error)
    {
        b.poles = "failed";
    }
    return b;
}

// Each face of a body: its area, its surface's type, and for a B-spline its degrees, knots and poles.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            const s = evSurfaceDefinition(context, { "face" : face });
            entry["type"] = s.surfaceType;
            if (s is BSplineSurface)
                entry.bspline = bsplineProbe(s);
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Each edge of a body that isn't straight: its ends and middle, and for a B-spline its degree, knots, poles and
// weights.
function curveProbe(context is Context, body is Query) returns array
{
    var out = [];
    const edges = qOwnedByBody(body, EntityType.EDGE);
    for (var edge in evaluateQuery(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))))
    {
        var entry = {};
        try
        {
            const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
            var points = [];
            for (var l in lines)
                points = append(points, round6(l.origin));
            entry.points = points;
        }
        catch (error)
        {
            entry.points = "failed";
        }
        try
        {
            const c = evCurveDefinition(context, { "edge" : edge });
            if (c is BSplineCurve)
            {
                entry.degree = c.degree;
                entry.rational = c.isRational;
                entry.knots = numbersProbe(c.knots);
                var poles = [];
                for (var i = 0; i < size(c.controlPoints); i += 1)
                    poles = append(poles, round6(c.controlPoints[i]));
                entry.poles = poles;
                if (c.isRational)
                    entry.weights = numbersProbe(c.weights);
            }
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;


const batch = new Batch({ number: 120, title: "geodesic offset curves across a sphere from a meridian and across a cone from a ruling", helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const offsetWires = `qBodyType(qCreatedBy(id + "offset1", EntityType.BODY), BodyType.WIRE)`;
const offset = (edges, targets, distance) => call('offsetCurveOnFace', 'offset1', {
  edges, distance: mm(distance), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false', gapFill: 'GapFill.LINEAR',
  scope: 'OffsetCurveScope.OFFSET', targets,
});
const probe = returns({ curves: `curveProbe(context, ${offsetWires})` });

// Batch 055's band of a sphere: the arc of radius 10 from 0° to 60° in the xz plane swept 90° about z.
batch.add('offset-curve-on-face', "a sphere band's meridian edge in the xz plane offset 2 mm round the sphere", [
  sketch('profile', PLANES.xz), arc('profile', 'arc1', [10, 0], [8.660254037844387, 5], [5, 8.660254037844386]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
  offset(edgeAt(sheetsOf('base'), [8.660254037844387, 0, 5]), `qOwnedByBody(${sheetsOf('base')}, EntityType.FACE)`, 2), probe,
]);
// Half a cone: the line from (10, 0, 0) to (5, 0, 20) in the xz plane revolved 180° about z.
batch.add('offset-curve-on-face', "a half cone's straight edge in the xz plane offset 3 mm round the cone", [
  sketch('profile', PLANES.xz), line('profile', 'l1', [10, 0], [5, 20]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '180 * degree',
  }),
  offset(edgeAt(sheetsOf('base'), [7.5, 0, 10]), `qOwnedByBody(${sheetsOf('base')}, EntityType.FACE)`, 3), probe,
]);

batch.write('tools/fixtures/gen-batch-120.mjs');

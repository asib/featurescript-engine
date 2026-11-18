// Generates fixture batch 195: rational B-spline curves (opCreateBSplineCurve with weights) trimmed and extended
// (trimCurve's untested rational B-splines): a quarter circle as a rational quadratic extended straight on and curving
// on, a rational cubic extended both ways by a distance and up to a plane, and trimmed by a plane; and geodesic
// offset curves across other curved faces (offsetCurveOnFace): an extruded elliptical arc's straight edge round it
// and its elliptical edge down it, an extruded fit spline's straight edge round it, a revolved fit spline's meridian
// round it and its bottom circle up it.
//
// Usage: node tools/fixtures/gen-batch-195.mjs
//
// Writes fixtures/batch-195/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, call, createdBy, deg, edgeAt, edgeOf, mm, planeBody, sketch, solve, vec2, vertexAt, PLANES } from './lib.mjs';

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
        var entry = { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
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


const batch = new Batch({ number: 195, title: 'rational B-spline curves trimmed and extended; offset curves across other curved faces', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const curve1 = createdBy('curve1');
const v3 = (p) => `vector(${p[0]}, ${p[1]}, ${p[2]}) * millimeter`;
const bspline = (degree, poles, weights) => `opCreateBSplineCurve(context, id + "curve1", { "bSplineCurve" : bSplineCurve({ "degree" : ${degree}, "isPeriodic" : false, "controlPoints" : [${poles.map(v3).join(', ')}], "weights" : [${weights.join(', ')}], "knots" : knotArray([${[...Array(degree + 1).fill(0), ...Array(degree + 1).fill(1)].join(', ')}]) }) });`;
const probe = returns({ curves: `curveProbe(context, ${curve1})` });
const extend = (end, fields) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, end), ...fields,
  });
const blind = (distance, shape) => ({ endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(distance), extensionShape: `CurveExtensionShape.${shape}` });

const QUARTER = bspline(2, [[10, 0, 0], [10, 10, 0], [0, 10, 0]], [1, 0.7071067811865476, 1]);
const CUBIC = bspline(3, [[0, 0, 0], [5, 10, 0], [15, 10, 0], [20, 0, 0]], [1, 2, 0.5, 1]);

batch.add('trim-curve', 'a quarter circle of radius 10 as a rational quadratic B-spline extended 5 mm at its end, straight on', [QUARTER, extend([0, 10, 0], blind(5, 'TANGENT')), probe]);
batch.add('trim-curve', 'the rational quarter circle extended 5 mm at its end, curving on', [QUARTER, extend([0, 10, 0], blind(5, 'CURVATURE')), probe]);
batch.add('trim-curve', 'a rational cubic B-spline (weights 1, 2, 0.5, 1) extended 8 mm at its end, curving on', [CUBIC, extend([20, 0, 0], blind(8, 'CURVATURE')), probe]);
batch.add('trim-curve', 'the rational cubic extended 8 mm at its end, straight on', [CUBIC, extend([20, 0, 0], blind(8, 'TANGENT')), probe]);
batch.add('trim-curve', 'the rational cubic extended 6 mm at its start, curving on', [CUBIC, extend([0, 0, 0], blind(6, 'CURVATURE')), probe]);
batch.add('trim-curve', 'the rational cubic extended up to the plane x = 24, curving on', [
  CUBIC, planeBody('limit', [24, 0, 0], [1, 0, 0], [0, 1, 0]),
  extend([20, 0, 0], { endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY', extendTo: createdBy('limit', 'FACE'), extensionShape: 'CurveExtensionShape.CURVATURE' }),
  probe,
]);
batch.add('trim-curve', 'the rational cubic trimmed by the plane x = 12, keeping its end at (20, 0)', [
  CUBIC, planeBody('limit', [12, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: curve1, trimTo: createdBy('limit', 'FACE'), helpPoint: vertexAt(curve1, [20, 0, 0]),
    flipHeuristics: 'false',
  }),
  probe,
]);

// ----- offset curves across other curved faces -----

const offsetWires = `qBodyType(qCreatedBy(id + "offset1", EntityType.BODY), BodyType.WIRE)`;
const offset = (edges, targets, distance) => call('offsetCurveOnFace', 'offset1', {
  edges, distance: mm(distance), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false', gapFill: 'GapFill.LINEAR',
  scope: 'OffsetCurveScope.OFFSET', targets,
});
const offsetProbe = returns({ curves: `curveProbe(context, ${offsetWires})` });
const faces = (name) => `qOwnedByBody(${sheetsOf(name)}, EntityType.FACE)`;
const extruded = (curveCall) => [
  sketch('s1', PLANES.xy), curveCall, solve('s1'),
  call('extrude', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(20),
    oppositeDirection: 'false',
  }),
];
// An elliptical arc of radii 10 and 6 from (10, 0) a third of the way round, extruded 20 up.
const ELLIPTIC = extruded(`skEllipticalArc(s1, "e1", { "center" : ${vec2(0, 0)}, "majorAxis" : vector(1, 0), "majorRadius" : 10 * millimeter, "minorRadius" : 6 * millimeter, "startParameter" : 0, "endParameter" : 0.3333333333333333 });`);
const SPLINE = extruded(`skFitSpline(s1, "sp", { "points" : [${vec2(0, 0)}, ${vec2(10, 6)}, ${vec2(20, 0)}] });`);
const revolved = [
  sketch('s1', PLANES.xz), `skFitSpline(s1, "spline1", { "points" : [${vec2(10, 0)}, ${vec2(15, 10)}, ${vec2(12, 20)}] });`, solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', angle: deg(90), oppositeDirection: 'false',
  }),
];

batch.add('offset-curve-on-face', "an extruded elliptical arc's straight edge at (10, 0) offset 3 mm round it", [
  ELLIPTIC, offset(edgeAt(sheetsOf('base'), [10, 0, 10]), faces('base'), 3), offsetProbe,
]);
batch.add('offset-curve-on-face', "the extruded elliptical arc's top edge offset 4 mm down it", [
  ELLIPTIC, offset(edgeAt(sheetsOf('base'), [8.090169943749475, 3.526711513754839, 20]), faces('base'), 4), offsetProbe,
]);
batch.add('offset-curve-on-face', "an extruded fit spline's straight edge at (0, 0) offset 4 mm round it", [
  SPLINE, offset(edgeAt(sheetsOf('base'), [0, 0, 10]), faces('base'), 4), offsetProbe,
]);
batch.add('offset-curve-on-face', "a revolved fit spline's meridian in the xz plane offset 2 mm round it", [
  revolved, offset(edgeAt(sheetsOf('base'), [15, 0, 10]), faces('base'), 2), offsetProbe,
]);
batch.add('offset-curve-on-face', "the revolved fit spline's bottom circle offset 3 mm up it", [
  revolved, offset(edgeAt(sheetsOf('base'), [7.0710678118654755, 7.0710678118654755, 0]), faces('base'), 3), offsetProbe,
]);

batch.write('tools/fixtures/gen-batch-195.mjs');

// Generates fixture batch 119: sweeps locked to a sphere along great circles that aren't round its own axis (sweep's
// untested paths), and a sheet extruded from an elliptical arc extended past the ellipse's end keeping its curvature
// and straight on (extendSurface's untested curved sheets).
//
// Usage: node tools/fixtures/gen-batch-119.mjs
//
// Writes fixtures/batch-119/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, edgeAt, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

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


const batch = new Batch({ number: 119, title: 'sweeps locked to a sphere along great circles, an extruded elliptical sheet extended', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const sphereFace = `qOwnedByBody(${solidsOf('base')}, EntityType.FACE)`;
const sweepLocked = (pathSketch) => call('sweep', 'sweep1', {
  profiles: regions('prof'), path: wireEdges(pathSketch), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: sphereFace,
});
const sphere = call('opSphere', 'base', { center: 'vector(0, 0, 0) * millimeter', radius: mm(10) });

batch.add('sweep', "a 2 × 3 rectangle swept round a sphere of radius 10 along its great circle in the xz plane, locked to it", [
  sphere,
  sketch('path', PLANES.xz), `skCircle(path, "c1", { "center" : vector(0, 0) * millimeter, "radius" : 10 * millimeter });`, solve('path'),
  sketch('prof', planeAt([10, 0, 0], [0, 0, 1], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  sweepLocked('path'), returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
// The great circle in the plane through the origin with normal (0, -0.6, 0.8): through (10, 0, 0), its tangent there
// (0, 0.8, 0.6).
batch.add('sweep', "a 2 × 3 rectangle swept round the sphere along a tilted great circle, locked to it", [
  sphere,
  sketch('path', planeAt([0, 0, 0], [0, -0.6, 0.8], [1, 0, 0])), `skCircle(path, "c1", { "center" : vector(0, 0) * millimeter, "radius" : 10 * millimeter });`, solve('path'),
  sketch('prof', planeAt([10, 0, 0], [0, 0.8, 0.6], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  sweepLocked('path'), returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
const ellipseSheet = [
  sketch('s1', PLANES.xy),
  `skEllipticalArc(s1, "e1", { "center" : vector(0, 0) * millimeter, "majorAxis" : vector(1, 0), "majorRadius" : 20 * millimeter, "minorRadius" : 10 * millimeter, "startParameter" : 0, "endParameter" : 0.25 });`,
  solve('s1'),
  call('extrude', 'sheet', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(10), oppositeDirection: 'false' }),
];
for (const [curving, what] of [['true', 'keeping its curvature'], ['false', 'straight on']]) {
  batch.add('extend-surface', `a quarter of a 20 × 10 ellipse extruded 10 mm, extended 5 mm past the ellipse's end at (0, 10), ${what}`, [
    ellipseSheet,
    call('extendSurface', 'extend1', {
      entities: edgeAt(sheetsOf('sheet'), [0, 10, 5]), endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(5), oppositeDirection: 'false',
      maintainCurvature: curving, tangentPropagation: 'false',
    }),
    returns({ faces: `faceProbe(context, ${sheetsOf('sheet')})`, curves: `curveProbe(context, ${sheetsOf('sheet')})` }),
  ]);
}

batch.write('tools/fixtures/gen-batch-119.mjs');

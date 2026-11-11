// Generates fixture batch 118: faces replaced where the replaced face is curved (replaceFace's untested variants):
// a sphere's face up to a larger concentric sphere, a cylinder's side up to itself offset 2 mm, a cylinder's side up
// to a sphere about its middle (a barrel), and a sphere's face up to a smaller concentric one.
//
// Usage: node tools/fixtures/gen-batch-118.mjs
//
// Writes fixtures/batch-118/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, circle, mm, sketch, solidsOf, solve, regions } from './lib.mjs';

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


const batch = new Batch({ number: 118, title: 'curved faces replaced: spheres by spheres, a cylinder by itself offset and by a sphere', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const tools = solidsOf('tool');
const probe = returns({ faces: `faceProbe(context, ${base})` });
const replace = (faces, offset = 0) =>
  call('replaceFace', 'replace1', { replaceFaces: faces, templateFace: `qOwnedByBody(${tools}, EntityType.FACE)`, oppositeSense: 'false', offset: mm(offset) });
const sphereFace = `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.SPHERE)`;
const cylinderFace = `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CYLINDER)`;
const cylinder = (name, r, h) => [
  sketch(name + 's', PLANES.xy), circle(name + 's', 'c1', [0, 0], r), solve(name + 's'),
  call('extrude', name, { entities: regions(name + 's'), endBound: 'BoundingType.BLIND', depth: mm(h) }),
];

batch.add('replace-face', "a sphere of radius 10's face replaced up to a concentric sphere of radius 12", [
  call('opSphere', 'base', { center: 'vector(0, 0, 0) * millimeter', radius: mm(10) }),
  call('opSphere', 'tool', { center: 'vector(0, 0, 0) * millimeter', radius: mm(12) }),
  replace(sphereFace), probe,
]);
batch.add('replace-face', "a sphere of radius 10's face replaced up to a concentric sphere of radius 8", [
  call('opSphere', 'base', { center: 'vector(0, 0, 0) * millimeter', radius: mm(10) }),
  call('opSphere', 'tool', { center: 'vector(0, 0, 0) * millimeter', radius: mm(8) }),
  replace(sphereFace), probe,
]);
batch.add('replace-face', "a cylinder (radius 10, 20 tall)'s side replaced up to a coaxial cylinder of radius 10, offset 2 mm", [
  cylinder('base', 10, 20), cylinder('tool', 10, 30), replace(cylinderFace, 2), probe,
]);
batch.add('replace-face', "a cylinder (radius 10, 20 tall)'s side replaced up to a sphere of radius 15 about its middle", [
  cylinder('base', 10, 20), call('opSphere', 'tool', { center: 'vector(0, 0, 10) * millimeter', radius: mm(15) }),
  replace(cylinderFace), probe,
]);

batch.write('tools/fixtures/gen-batch-118.mjs');

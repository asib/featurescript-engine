// Generates fixture batch 197: other curved faces replaced (replaceFace's untested curved faces): a cylinder's
// rounded top edge (a torus) up to a cone across it (a chamfer), a block's rounded vertical edge up to a plane across
// it, a D-shaped extrusion's half-cylindrical top up to a plane above it, a cylinder's rounded top edge up to the
// plane of its top (the round taken off), and a block's rounded top edge up to a larger coaxial cylinder.
//
// Usage: node tools/fixtures/gen-batch-197.mjs
//
// Writes fixtures/batch-197/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, planeBody, rect, regions, sketch, solidsOf, solve, vec2, vec3, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 197, title: 'other curved faces replaced', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const tools = solidsOf('tool');
const probe = returns({ faces: `faceProbe(context, ${base})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${base} }) / millimeter ^ 3, 6)` });
const faces = `qOwnedByBody(${base}, EntityType.FACE)`;
const ofType = (type) => `qGeometry(${faces}, GeometryType.${type})`;
const replaceWith = (replaced, template) =>
  call('replaceFace', 'replace1', { replaceFaces: replaced, templateFace: template, oppositeSense: 'false', offset: mm(0) });
const toolCylinderFace = `qGeometry(qOwnedByBody(${tools}, EntityType.FACE), GeometryType.CYLINDER)`;

const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetFace = (name) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`;

batch.add('replace-face', 'a cylinder of radius 10 and 10 tall, its top edge rounded to radius 3, the round replaced up to the cone through the round\'s ends (a chamfer)', [
  ...cylinder('base', [0, 0], 10, 0, 10),
  call('fillet', 'fillet1', { entities: edgeAt(base, [10, 0, 10]), radius: mm(3) }),
  // The cone: the line from (10, 0, 7) to (7, 0, 10), carried on 2 mm each way, revolved about z.
  sketch('c', PLANES.xz), line('c', 'l1', [11.414213562373096, 5.585786437626905], [5.585786437626905, 11.414213562373096]), solve('c'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'target', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('c'), axis: edgeOf('axis'), fullRevolve: 'true',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false',
  }),
  replaceWith(ofType('TORUS'), sheetFace('target')), probe,
]);
batch.add('replace-face', 'a 20 × 20 × 10 block, its vertical edge at (20, 20) rounded to radius 3, the round replaced up to the plane through its ends', [
  ...box('base', [0, 0, 0], [20, 20, 10]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [20, 20, 5]), radius: mm(3) }),
  planeBody('target', [18.5, 18.5, 5], [Math.SQRT1_2, Math.SQRT1_2, 0], [0, 0, 1]),
  replaceWith(ofType('CYLINDER'), `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);
batch.add('replace-face', 'a D-shaped extrusion 10 deep (a 20 wide rectangle 10 tall with a half circle on top), its half cylinder replaced up to the plane z = 14', [
  sketch('s', PLANES.xz), line('s', 'l1', [-10, 10], [-10, 0]), line('s', 'l2', [-10, 0], [10, 0]), line('s', 'l3', [10, 0], [10, 10]),
  arc('s', 'a1', [10, 10], [0, 20], [-10, 10]), solve('s'),
  call('extrude', 'base', { entities: regions('s'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
  planeBody('target', [0, 0, 14], [0, 0, 1], [1, 0, 0]),
  replaceWith(ofType('CYLINDER'), `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);
batch.add('replace-face', 'the cylinder with its top edge rounded, the round replaced up to the plane of its top', [
  ...cylinder('base', [0, 0], 10, 0, 10),
  call('fillet', 'fillet1', { entities: edgeAt(base, [10, 0, 10]), radius: mm(3) }),
  planeBody('target', [0, 0, 10], [0, 0, 1], [1, 0, 0]),
  replaceWith(ofType('TORUS'), `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);
batch.add('replace-face', 'a 20 × 20 × 10 block, its top edge along y = 20 rounded to radius 3, the round replaced up to a coaxial cylinder of radius 4', [
  ...box('base', [0, 0, 0], [20, 20, 10]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [10, 20, 10]), radius: mm(3) }),
  // The cylinder of radius 4 about the round's axis, the line x from 0 to 20 at y = 17, z = 7.
  sketch('t', planeAt([-5, 0, 0], [1, 0, 0], [0, 1, 0])), circle('t', 'c1', [17, 7], 4), solve('t'),
  call('extrude', 'tool', { entities: regions('t'), endBound: 'BoundingType.BLIND', depth: mm(30) }),
  replaceWith(ofType('CYLINDER'), toolCylinderFace), probe,
]);

batch.write('tools/fixtures/gen-batch-197.mjs');

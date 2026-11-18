// Generates fixture batch 189: more curved faces replaced: a fillet's face up to a plane across it (a chamfer), a
// hole's face up to a smaller coaxial cylinder, a boss's side up to a larger one, a square-to-circle loft's side up
// to a plane, and a dome on a cylinder up to a plane above it.
//
// Usage: node tools/fixtures/gen-batch-189.mjs
//
// Writes fixtures/batch-189/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, edgeAt, faceAt, mm, planeAt, planeBody, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 189, title: 'more curved faces replaced', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const tools = solidsOf('tool');
const probe = returns({ faces: `faceProbe(context, ${base})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${base} }) / millimeter ^ 3, 6)` });
const faces = `qOwnedByBody(${base}, EntityType.FACE)`;
const ofType = (type) => `qGeometry(${faces}, GeometryType.${type})`;
const replaceWith = (replaced, template) =>
  call('replaceFace', 'replace1', { replaceFaces: replaced, templateFace: template, oppositeSense: 'false', offset: mm(0) });
const toolCylinderFace = `qGeometry(qOwnedByBody(${tools}, EntityType.FACE), GeometryType.CYLINDER)`;

batch.add('replace-face', 'a 20 × 20 × 10 block, its top edge along y = 20 filleted to radius 3, the fillet replaced up to the plane through its ends (a chamfer)', [
  ...box('base', [0, 0, 0], [20, 20, 10]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [10, 20, 10]), radius: mm(3) }),
  planeBody('target', [10, 18.5, 8.5], [0, Math.SQRT1_2, Math.SQRT1_2], [1, 0, 0]),
  replaceWith(ofType('CYLINDER'), `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);
batch.add('replace-face', 'a 20 × 20 × 10 plate with a hole of radius 4 through its middle, the hole replaced up to a coaxial cylinder of radius 3', [
  sketch('s', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), rect('s', 'rect1', [0, 0], [20, 20]), circle('s', 'hole', [10, 10], 4), solve('s'),
  call('extrude', 'base', { entities: `qContainsPoint(qSketchRegion(id + "s"), ${vec3(2, 2, 0)})`, endBound: 'BoundingType.BLIND', depth: mm(10) }),
  ...cylinder('tool', [10, 10], 3, -5, 15),
  replaceWith(ofType('CYLINDER'), toolCylinderFace), probe,
]);
batch.add('replace-face', 'a 20 × 20 × 5 block with a boss of radius 4 and 5 tall on its top, the boss\'s side replaced up to a coaxial cylinder of radius 6', [
  ...box('base', [-10, -10, 0], [10, 10, 5]),
  sketch('b', planeAt([0, 0, 5], [0, 0, 1], [1, 0, 0])), circle('b', 'boss', [0, 0], 4), solve('b'),
  call('extrude', 'boss', { entities: regions('b'), endBound: 'BoundingType.BLIND', depth: mm(5), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
  ...cylinder('tool', [0, 0], 6, 0, 20),
  replaceWith(ofType('CYLINDER'), toolCylinderFace), probe,
]);
batch.add('replace-face', 'a 20 mm square at z = 0 lofted to a circle of radius 8 at z = 20, the side on the square\'s edge along x = 10 replaced up to the plane x = 8', [
  sketch('p0', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), rect('p0', 'rect1', [-10, -10], [10, 10]), solve('p0'),
  sketch('p1', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('p1', 'circle1', [0, 0], 8), solve('p1'),
  call('loft', 'base', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('p0')} }, { "sheetProfileEntities" : ${regions('p1')} }]`,
    bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
  }),
  planeBody('target', [8, 0, 10], [1, 0, 0], [0, 1, 0]),
  replaceWith(`qSubtraction(qContainsPoint(${faces}, ${vec3(10, 0, 0)}), ${ofType('PLANE')})`, `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);
batch.add('replace-face', 'a cylinder of radius 8 and 10 tall with a hemispherical dome on its top, the dome replaced up to the plane z = 14', [
  ...cylinder('base', [0, 0], 8, 0, 10),
  call('opSphere', 'ball', { center: 'vector(0, 0, 10) * millimeter', radius: mm(8) }),
  `opBoolean(context, id + "join", { "tools" : qUnion([${base}, ${solidsOf('ball')}]), "operationType" : BooleanOperationType.UNION });`,
  planeBody('target', [0, 0, 14], [0, 0, 1], [1, 0, 0]),
  replaceWith(ofType('SPHERE'), `qCreatedBy(id + "target", EntityType.FACE)`), probe,
]);

batch.write('tools/fixtures/gen-batch-189.mjs');

// Generates fixture batch 190: rounded rectangles lofted to circles, to find the slopes at the nodes of Onshape's
// correspondence (batch 188 c001's aren't the geometric means): a rounded square, rectangles of other sizes and
// corner radii, and one to a smaller circle.
//
// Usage: node tools/fixtures/gen-batch-190.mjs
//
// Writes fixtures/batch-190/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, planeAt, regions, sketch, solidsOf, solve } from './lib.mjs';

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

const batch = new Batch({ number: 190, title: 'probes: rounded rectangles lofted to circles', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const ring = (s, z, r) => [sketch(s, at(z)), circle(s, 'circle1', [0, 0], r), solve(s)];
const k = Math.SQRT1_2;
// A w × h rectangle centred on the origin, corners rounded to radius r, its first line along the top from the left.
const rounded = (s, z, w, h, r) => {
  const [a, b] = [w / 2, h / 2];
  return [sketch(s, at(z)),
    line(s, 'line1', [-a + r, b], [a - r, b]), arc(s, 'arc1', [a - r, b], [a - r + r * k, b - r + r * k], [a, b - r]),
    line(s, 'line2', [a, b - r], [a, -b + r]), arc(s, 'arc2', [a, -b + r], [a - r + r * k, -b + r - r * k], [a - r, -b]),
    line(s, 'line3', [a - r, -b], [-a + r, -b]), arc(s, 'arc3', [-a + r, -b], [-a + r - r * k, -b + r - r * k], [-a, -b + r]),
    line(s, 'line4', [-a, -b + r], [-a, b - r]), arc(s, 'arc4', [-a, b - r], [-a + r - r * k, b - r + r * k], [-a + r, b]), solve(s)];
};
const loft = () => call('loft', 'loft1', {
  sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('p0')} }, { "sheetProfileEntities" : ${regions('p1')} }]`,
  bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});
for (const [w, h, r, R] of [[20, 20, 2, 10], [20, 10, 2, 10], [30, 10, 2, 10], [20, 10, 4, 10], [20, 10, 2, 6], [20, 12, 2, 10]]) {
  batch.add('loft', `a ${w} × ${h} rectangle with corners rounded to radius ${r} at z = 0 to a circle of radius ${R} at z = 20`, [
    ...rounded('p0', 0, w, h, r), ...ring('p1', 20, R), loft(), probe]);
}

batch.write('tools/fixtures/gen-batch-190.mjs');

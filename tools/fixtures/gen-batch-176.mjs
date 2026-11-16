// Generates fixture batch 176: probes of plain lofts through three polygons, the first stretch a straight rise and
// the second changing the profile, so the middle profile's knot gives the second stretch's length in the loft's
// parameter against the first's rise: squares growing a quarter, a half, as much again and twice, shrinking a half,
// growing a quarter at twice the size, moved across, a rectangle growing, the order reversed, and a longer rise.
//
// Usage: node tools/fixtures/gen-batch-176.mjs
//
// Writes fixtures/batch-176/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, line, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

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

const batch = new Batch({ number: 176, title: 'probes: plain lofts through three polygons, a straight rise then a change of profile', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const box = (s, z, [x0, y0], [x1, y1]) => [sketch(s, at(z)), rect(s, 'rect1', [x0, y0], [x1, y1]), solve(s)];
const square = (s, z, h, [cx, cy] = [0, 0]) => box(s, z, [cx - h, cy - h], [cx + h, cy + h]);
const loft = (names) => call('loft', 'loft1', {
  sheetProfilesArray: `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`,
  bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});
const names = (n) => Array.from({ length: n }, (_, i) => `p${i}`);

batch.add('loft', 'squares of 20, 20 and 25 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 12.5), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 20 and 30 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 15), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 20 and 40 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 20), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 20 and 60 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 30), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 20 and 10 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 5), loft(names(3)), probe]);
batch.add('loft', 'squares of 40, 40 and 45 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 20), ...square('p1', 10, 20), ...square('p2', 20, 22.5), loft(names(3)), probe]);
batch.add('loft', 'squares of 20 mm at z = 0, 10 and 20, the last moved 5 along x', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 20, 10, [5, 0]), loft(names(3)), probe]);
batch.add('loft', 'a 20 x 10 rectangle at z = 0 and 10, and a 30 x 15 one at z = 20', [
  ...box('p0', 0, [-10, -5], [10, 5]), ...box('p1', 10, [-10, -5], [10, 5]), ...box('p2', 20, [-15, -7.5], [15, 7.5]), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 25 and 25 mm at z = 0, 10 and 20', [
  ...square('p0', 0, 10), ...square('p1', 10, 12.5), ...square('p2', 20, 12.5), loft(names(3)), probe]);
batch.add('loft', 'squares of 20, 20 and 25 mm at z = 0, 10 and 30', [
  ...square('p0', 0, 10), ...square('p1', 10, 10), ...square('p2', 30, 12.5), loft(names(3)), probe]);

batch.write('tools/fixtures/gen-batch-176.mjs');

// Generates fixture batch 196: constrained surfaces through points (constrainedSurface's only case, batch 011 c006,
// is a bicubic over the points' extent and 0.05 mm more, its knots refined unevenly): four corners of a 40 × 30
// rectangle, flat and with one raised; the 3 × 3 grid flat; the grid with its middle raised 4 mm for performance
// and for smoothness at tolerances of 0.1, 0.01 and 0.001 mm, raised 1 mm, and on a 20 × 20 grid; six points not on a
// grid. Each returns its face's area and its B-spline's degrees, knots and poles.
//
// Usage: node tools/fixtures/gen-batch-196.mjs
//
// Writes fixtures/batch-196/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch } from './lib.mjs';

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


const batch = new Batch({ number: 196, title: 'constrained surfaces through points', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const surface = (points, tolerance = 0.01, optimize = 'SMOOTH') => [
  ...points.map((p, i) => `opPoint(context, id + "p${i + 1}", { "point" : vector(${p[0]}, ${p[1]}, ${p[2]}) * millimeter });`),
  `constrainedSurface(context, id + "surface1", { "csType" : ConstrainedSurfaceType.POINTS, "vertices" : [${points
    .map((_, i) => `{ "vertex" : qCreatedBy(id + "p${i + 1}", EntityType.VERTEX), "normal" : qNothing(), "flipNormal" : false }`)
    .join(', ')}], "tolerance" : ${tolerance} * millimeter, "optimize" : OptimizationMethod.${optimize}, "computeDeviation" : false });`,
  returns({ faces: `faceProbe(context, qBodyType(qCreatedBy(id + "surface1", EntityType.BODY), BodyType.SHEET))` }),
];
const grid = (w, h, middle) => {
  const out = [];
  for (const x of [0, w / 2, w]) for (const y of [0, h / 2, h]) out.push([x, y, x === w / 2 && y === h / 2 ? middle : 0]);
  return out;
};

batch.add('constrained-surface', 'four corners of a 40 × 30 rectangle, flat', [surface([[0, 0, 0], [40, 0, 0], [0, 30, 0], [40, 30, 0]])]);
batch.add('constrained-surface', 'the four corners, the one at (40, 30) raised 5 mm', [surface([[0, 0, 0], [40, 0, 0], [0, 30, 0], [40, 30, 5]])]);
batch.add('constrained-surface', 'a 3 × 3 grid over 40 × 30, flat', [surface(grid(40, 30, 0))]);
batch.add('constrained-surface', 'the grid, its middle raised 4 mm, for performance', [surface(grid(40, 30, 4), 0.01, 'PERF')]);
batch.add('constrained-surface', 'the grid, its middle raised 4 mm, to 0.1 mm', [surface(grid(40, 30, 4), 0.1)]);
batch.add('constrained-surface', 'the grid, its middle raised 4 mm, to 0.001 mm', [surface(grid(40, 30, 4), 0.001)]);
batch.add('constrained-surface', 'the grid, its middle raised 1 mm, to 0.01 mm', [surface(grid(40, 30, 1))]);
batch.add('constrained-surface', 'a 3 × 3 grid over 20 × 20, its middle raised 4 mm, to 0.01 mm', [surface(grid(20, 20, 4))]);
batch.add('constrained-surface', 'six points not on a grid', [surface([[0, 0, 0], [25, 3, 1], [40, 0, 0], [5, 28, 2], [22, 18, 3], [38, 30, 0]])]);
batch.add('constrained-surface', 'the 3 × 3 grid over 40 × 30, its middle raised 4 mm, to 0.01 mm (batch 011 c006 again)', [surface(grid(40, 30, 4))]);

batch.write('tools/fixtures/gen-batch-196.mjs');

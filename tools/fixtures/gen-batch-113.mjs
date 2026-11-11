// Generates fixture batch 113: curvature fillets past magnitude 0.979, where Onshape's section follows no law found
// (batches 055, 063, 064 sampled it every 0.0025 from 0.975): the nine magnitudes halfway between those samples, on
// a block's upright edge as batch 064's, and 0.99 at a trapezoid prism's 60° and 120° upright edges (batch 053's),
// to see whether the section is the same in the corner's own frame at other angles.
//
// Usage: node tools/fixtures/gen-batch-113.mjs
//
// Writes fixtures/batch-113/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, box, call, edgeAt, mm, polygon, regions, sketch, solidsOf, solve } from './lib.mjs';

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


const batch = new Batch({ number: 113, title: 'curvature fillets between the sampled magnitudes past 0.979, and at 60° and 120° corners', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const curvature = (point, radius, magnitude) =>
  call('fillet', 'fillet1', { entities: edgeAt(base, point), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(radius), magnitude: String(magnitude) });
const probe = returns({ faces: `faceProbe(context, ${base})` });

for (const magnitude of [0.97875, 0.98125, 0.98375, 0.98625, 0.98875, 0.99125, 0.99375, 0.99625, 0.99875]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [
    box('base', [0, 0, 0], [30, 20, 10]), curvature([0, 0, 5], 4, magnitude), probe,
  ]);
}

// Batch 053's trapezoid prism: corners of 60° along the bottom (at (0, 0) and (40, 0)) and 120° along the top.
const h = 17.320508075688775;
const trapezoid = [
  sketch('s1', PLANES.xy), ...polygon('s1', [[0, 0], [40, 0], [30, h], [10, h]]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
];
batch.add('fillet', "a trapezoid prism's 60° upright edge, curvature, 4 mm, magnitude 0.99", [trapezoid, curvature([0, 0, 5], 4, 0.99), probe]);
batch.add('fillet', "a trapezoid prism's 120° upright edge, curvature, 4 mm, magnitude 0.99", [trapezoid, curvature([10, h, 5], 4, 0.99), probe]);

batch.write('tools/fixtures/gen-batch-113.mjs');

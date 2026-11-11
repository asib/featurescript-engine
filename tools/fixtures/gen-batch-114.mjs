// Generates fixture batch 114: curvature fillets past magnitude 0.975 at corners other than right angles, where
// Onshape's section differs from a right angle's (batch 113 c010, c011: 60° and 120° alike at 0.99). Batch 053's
// trapezoid prism's 60° upright edge at every 0.00125 from 0.975 to 0.99875 (the right angle's samples), and 0.99 at
// 45°, 75° and 135° corners, to see how the section goes with the angle (through cos², if 45° and 135° agree).
//
// Usage: node tools/fixtures/gen-batch-114.mjs
//
// Writes fixtures/batch-114/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, edgeAt, mm, polygon, regions, sketch, solidsOf, solve } from './lib.mjs';

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


const batch = new Batch({ number: 114, title: 'curvature fillets past 0.975 at 60°, and at 45°, 75° and 135° corners', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const curvature = (point, magnitude) =>
  call('fillet', 'fillet1', { entities: edgeAt(base, point), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(4), magnitude: String(magnitude) });
const probe = returns({ faces: `faceProbe(context, ${base})` });
const prism = (points) => [
  sketch('s1', PLANES.xy), ...polygon('s1', points), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
];

// Batch 053's trapezoid prism: corners of 60° along the bottom (at (0, 0) and (40, 0)).
const h = 17.320508075688775;
const trapezoid = prism([[0, 0], [40, 0], [30, h], [10, h]]);
for (let i = 0; i <= 19; i++) {
  if (i === 12) continue; // 0.99 is batch 113 c010
  const magnitude = Math.round((0.975 + 0.00125 * i) * 1e6) / 1e6;
  batch.add('fillet', `a trapezoid prism's 60° upright edge, curvature, 4 mm, magnitude ${magnitude}`, [trapezoid, curvature([0, 0, 5], magnitude), probe]);
}
// A 45° corner at (0, 0): a triangle.
batch.add('fillet', "a triangular prism's 45° upright edge at the origin, curvature, 4 mm, magnitude 0.99", [prism([[0, 0], [40, 0], [20, 20]]), curvature([0, 0, 5], 0.99), probe]);
// A 75° corner at (0, 0).
const c75 = [20 * Math.cos(75 * Math.PI / 180), 20 * Math.sin(75 * Math.PI / 180)];
batch.add('fillet', "a prism's 75° upright edge at the origin, curvature, 4 mm, magnitude 0.99", [prism([[0, 0], [40, 0], [30, c75[1]], c75]), curvature([0, 0, 5], 0.99), probe]);
// A 135° corner at (10, 0): a block with its corner at the origin cut off at 45°.
batch.add('fillet', "a prism's 135° upright edge at (10, 0), curvature, 4 mm, magnitude 0.99", [prism([[10, 0], [40, 0], [40, 30], [0, 30], [0, 10]]), curvature([10, 0, 5], 0.99), probe]);

batch.write('tools/fixtures/gen-batch-114.mjs');

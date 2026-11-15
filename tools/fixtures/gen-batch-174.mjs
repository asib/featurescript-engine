// Generates fixture batch 174: lofts with guides the engine refused, probed for their poles: between a 20 mm square
// and a 10 mm one 20 above, guides bulging out across two opposite sides, across two neighbouring ones, across a side
// off its middle (in an upright plane, and from a quarter along the bottom edge to a quarter along the top), along a
// corner, and across a side with the loft normal to its first profile; between circles, a guide bulging out; and
// through three squares, a guide across their sides.
//
// Usage: node tools/fixtures/gen-batch-174.mjs
//
// Writes fixtures/batch-174/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

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

const batch = new Batch({ number: 174, title: 'lofts with guides on several sides, off a side\'s middle, along a corner, with end conditions, between circles and through three profiles', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const profiles = (names) => `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`;
const guides = (names) => ({
  addGuides: 'true',
  guidesArray: `[${names.map((n) => `{ "guideEntities" : qUnion([qCreatedBy(id + "${n}", EntityType.EDGE)]), "guideDerivativeType" : LoftGuideDerivativeType.DEFAULT, "guideDerivativeMagnitude" : 1 }`).join(', ')}]`,
});
const loft = (names, gs, extra = {}) => call('loft', 'loft1', { sheetProfilesArray: profiles(names), ...guides(gs), ...extra });
const probe = returns({ faces: `faceProbe(context, ${loftBody})`, curves: `curveProbe(context, ${loftBody})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${loftBody} }) / millimeter ^ 3, 6)` });
const square = (s, z, h) => [sketch(s, at(z)), rect(s, 'rect1', [-h, -h], [h, h]), solve(s)];
const squares = [...square('s1', 0, 10), ...square('s2', 20, 5)];
// An arc guide in an upright plane through the origin, its x along `dir` (in xy): from (a, 0) through (b, 10) to (c, 20)
// in that plane's coordinates, offset `y` across it.
const upright = (name, dir, pts, y = 0) => {
  const n = [-dir[1], dir[0], 0];
  return [
    sketch(name, planeAt([n[0] * y, n[1] * y, 0], [n[0], n[1], 0].map((v) => -v), [dir[0], dir[1], 0])),
    arc(name, 'a1', pts[0], pts[1], pts[2]), solve(name),
  ];
};

batch.add('loft', 'the squares with guides bulging out across opposite sides, through (13, 0, 10) and (-13, 0, 10)', [
  ...squares, ...upright('g0', [1, 0], [[10, 0], [13, 10], [5, 20]]), ...upright('g1', [-1, 0], [[10, 0], [13, 10], [5, 20]]),
  loft(['s1', 's2'], ['g0', 'g1']), probe,
]);
batch.add('loft', 'the squares with guides bulging out across neighbouring sides, through (13, 0, 10) and (0, 13, 10)', [
  ...squares, ...upright('g0', [1, 0], [[10, 0], [13, 10], [5, 20]]), ...upright('g1', [0, 1], [[10, 0], [13, 10], [5, 20]]),
  loft(['s1', 's2'], ['g0', 'g1']), probe,
]);
batch.add('loft', 'the squares with a guide bulging out across a side off its middle, in the plane y = -2.5, through (13, -2.5, 10)', [
  ...squares, ...upright('g0', [1, 0], [[10, 0], [13, 10], [5, 20]], -2.5), loft(['s1', 's2'], ['g0']), probe,
]);
batch.add('loft', 'the squares with a guide from a quarter along a side\'s bottom edge to a quarter along its top, a fit spline bulging out through (13, -3.75, 10)', [
  ...squares, call('opFitSpline', 'g0', { points: `[${vec3(10, -5, 0)}, ${vec3(13, -3.75, 10)}, ${vec3(5, -2.5, 20)}]` }),
  loft(['s1', 's2'], ['g0']), probe,
]);
batch.add('loft', 'the squares with a guide along a corner, bulging out diagonally through (12, 12, 10)', [
  ...squares, ...upright('g0', [Math.SQRT1_2, Math.SQRT1_2], [[10 * Math.SQRT2, 0], [12 * Math.SQRT2, 10], [5 * Math.SQRT2, 20]]),
  loft(['s1', 's2'], ['g0']), probe,
]);
batch.add('loft', 'the squares with a guide bulging out across a side\'s middle (batch 075 c005), normal to the first profile', [
  ...squares, ...upright('g0', [1, 0], [[10, 0], [13, 10], [5, 20]]),
  loft(['s1', 's2'], ['g0'], { startCondition: 'LoftEndDerivativeType.NORMAL_TO_PROFILE', startMagnitude: '1', endCondition: 'LoftEndDerivativeType.DEFAULT' }), probe,
]);
batch.add('loft', 'circles of radius 10 at z = 0 and 5 at z = 20 with a guide bulging out through (12, 0, 10)', [
  sketch('s1', at(0)), circle('s1', 'c1', [0, 0], 10), solve('s1'), sketch('s2', at(20)), circle('s2', 'c1', [0, 0], 5), solve('s2'),
  ...upright('g0', [1, 0], [[10, 0], [12, 10], [5, 20]]), loft(['s1', 's2'], ['g0']), probe,
]);
batch.add('loft', 'squares of 20, 25 and 10 mm at z = 0, 10 and 20 with a guide across their sides at y = 0 through (12.5, 0, 10)', [
  ...square('s1', 0, 10), ...square('s3', 10, 12.5), ...square('s2', 20, 5),
  ...upright('g0', [1, 0], [[10, 0], [12.5, 10], [5, 20]]), loft(['s1', 's3', 's2'], ['g0']), probe,
]);

batch.write('tools/fixtures/gen-batch-174.mjs');

// Generates fixture batch 117: boundary surfaces between batch 062's arcs with end conditions beyond batch 063's one
// normal to the first profile: normal at both profiles, magnitude 2, at the second profile only, along a direction
// given and square to it, and both with different magnitudes; their poles probed, to find how Onshape builds them.
//
// Usage: node tools/fixtures/gen-batch-117.mjs
//
// Writes fixtures/batch-117/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, planeAt, sketch, solve } from './lib.mjs';

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


const batch = new Batch({ number: 117, title: 'boundary surfaces between arcs with end conditions at both profiles, the second, and along directions', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wires = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const arcs = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('s1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('s2', 'arc1', [0, 0], [15, 10], [30, 0]), solve('s2'),
];
const profile = (name, condition = 'DEFAULT', extra = {}) =>
  `{ "uProfileEntities" : ${wires(name)}, "uCondition" : BSurfEndDerivativeType.${condition}${Object.entries(extra).map(([k, v]) => `, "u${k}" : ${v}`).join('')} }`;
const normal = (magnitude) => ({ Magnitude: String(magnitude), Reference: 'qNothing()', Direction: 'qNothing()' });
const along = (condition, magnitude) => ({ Magnitude: String(magnitude), Reference: 'qNothing()', Direction: 'qCreatedBy(id + "dir", EntityType.EDGE)' });
const surface = (first, second, extra = []) => [
  arcs, ...extra,
  call('boundarySurface', 'surface1', { uProfilesArray: `[${first}, ${second}]`, vProfilesArray: '[]' }),
  returns({ faces: `faceProbe(context, ${sheetsOf('surface1')})` }),
];
const dir = axisLine('dir', [0, 0, 0], [0, 10, 5]);

batch.add('boundary-surface', "batch 062's arcs, both normal to their profiles' planes, magnitude 1", surface(profile('s1', 'NORMAL_TO_PROFILE', normal(1)), profile('s2', 'NORMAL_TO_PROFILE', normal(1))));
batch.add('boundary-surface', "batch 062's arcs, the first normal to its profile's plane, magnitude 2", surface(profile('s1', 'NORMAL_TO_PROFILE', normal(2)), profile('s2')));
batch.add('boundary-surface', "batch 062's arcs, the second normal to its profile's plane, magnitude 1", surface(profile('s1'), profile('s2', 'NORMAL_TO_PROFILE', normal(1))));
batch.add('boundary-surface', "batch 062's arcs, the first along the direction (0, 10, 5), magnitude 1", surface(profile('s1', 'TANGENT_DIRECTION', along('TANGENT_DIRECTION', 1)), profile('s2'), [dir]));
batch.add('boundary-surface', "batch 062's arcs, the first square to the direction (0, 10, 5), magnitude 1", surface(profile('s1', 'NORMAL_DIRECTION', along('NORMAL_DIRECTION', 1)), profile('s2'), [dir]));
batch.add('boundary-surface', "batch 062's arcs, the first normal to its profile's plane at magnitude 0.5, the second at 1.5", surface(profile('s1', 'NORMAL_TO_PROFILE', normal(0.5)), profile('s2', 'NORMAL_TO_PROFILE', normal(1.5))));

batch.write('tools/fixtures/gen-batch-117.mjs');

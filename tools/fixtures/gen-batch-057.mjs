// Generates fixture batch 057: lofts to points and matching the faces next to a profile, sweeps twisted and
// scaled along a path of a line and an arc, a torus sheet extended, and isoclines of a torus tilted to the
// direction. Cases return their faces' surfaces (a B-spline's degrees, knots and poles) or points along their
// edges.
//
// Usage: node tools/fixtures/gen-batch-057.mjs
//
// Writes fixtures/batch-057/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each face of a body: its area, its surface's type, and for a B-spline its degrees, knots and poles. (Batch 055's
// first build: taking a B-spline's knots with mapArray failed in Onshape with "Execution error"; indexed loops,
// each part caught on its own.)
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

// Knot arrays and control point matrices are typed: indexed, as std does, rather than iterated.
function knotsProbe(knots) returns array
{
    var out = [];
    for (var i = 0; i < size(knots); i += 1)
        out = append(out, roundToPrecision(knots[i], 9));
    return out;
}

function bsplineProbe(s) returns map
{
    var b = { "uDegree" : s.uDegree, "vDegree" : s.vDegree, "rational" : s.isRational };
    try
    {
        b.uKnots = knotsProbe(s.uKnots);
        b.vKnots = knotsProbe(s.vKnots);
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

// Nine points along each edge of a body, evenly spaced in its own parameter.
function edgeProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1], "arcLengthParameterization" : false });
        var points = [];
        for (var l in lines)
            points = append(points, round6(l.origin));
        out = append(out, points);
    }
    return out;
}
`;

const batch = new Batch({ number: 57, title: 'lofts to points and matching faces, sweeps twisted along a line and an arc, a torus sheet extended, tilted torus isoclines', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loftBody = solidsOf('loft1');
const probe = (body) => returns({ faces: `faceProbe(context, ${body})` });
// A point profile: a sketch point at (0, 0, z).
const point = (s, z) => [sketch(s, at(z)), `skPoint(${s}, "p1", { "position" : ${vec2(0, 0)} });`, solve(s)];
const pointOf = (s) => `qCreatedBy(id + "${s}", EntityType.VERTEX)`;
const loft = (profiles, fields) =>
  call('loft', 'loft1', { sheetProfilesArray: `[${profiles.map((q) => `{ "sheetProfileEntities" : ${q} }`).join(', ')}]`, ...fields });
const square = (s, z, h) => [sketch(s, at(z)), rect(s, 'rect1', [-h, -h], [h, h]), solve(s)];
const disc = (s, z, r) => [sketch(s, at(z)), circle(s, 'circle1', [0, 0], r), solve(s)];
const start = (condition, magnitude, extra = {}) => ({ startCondition: `LoftEndDerivativeType.${condition}`, startMagnitude: String(magnitude), ...extra });

// ----- lofts to points -----

batch.add('loft', 'a 20 mm square to a point 20 mm above its middle', [square('s1', 0, 10), point('s2', 20), loft([regions('s1'), pointOf('s2')], {}), probe(loftBody)]);
batch.add('loft', 'a circle of radius 10 to a point 20 mm above its middle', [disc('s1', 0, 10), point('s2', 20), loft([regions('s1'), pointOf('s2')], {}), probe(loftBody)]);
batch.add('loft', 'a point to a circle of radius 10 15 mm above it to a point 15 mm above that', [
  point('s0', 0), disc('s1', 15, 10), point('s2', 30), loft([pointOf('s0'), regions('s1'), pointOf('s2')], {}), probe(loftBody),
]);
batch.add('loft', 'a 20 mm square to a point 20 mm above, normal to the square, magnitude 1', [
  square('s1', 0, 10), point('s2', 20), loft([regions('s1'), pointOf('s2')], start('NORMAL_TO_PROFILE', 1)), probe(loftBody),
]);

// ----- lofts matching the faces next to the profile -----

const base = solidsOf('base');
const block = box('base', [-10, -10, -10], [10, 10, 0]);
const top = faceAt(base, [0, 0, 0]);
const sides = `qSubtraction(qOwnedByBody(${base}, EntityType.FACE), qUnion([${top}, ${faceAt(base, [0, 0, -10])}]))`;
batch.add('loft', 'a block\'s top face to a 10 mm square 20 mm above, matching the block\'s sides, magnitude 1', [
  block, square('s2', 20, 5), loft([top, regions('s2')], start('MATCH_TANGENT', 1, { adjacentFacesStart: sides })), probe(loftBody),
]);
batch.add('loft', 'the same, magnitude 2', [block, square('s2', 20, 5), loft([top, regions('s2')], start('MATCH_TANGENT', 2, { adjacentFacesStart: sides })), probe(loftBody)]);
batch.add('loft', 'the same, matching curvature, magnitude 1', [block, square('s2', 20, 5), loft([top, regions('s2')], start('MATCH_CURVATURE', 1, { adjacentFacesStart: sides })), probe(loftBody)]);
const can = cylinder('base', [0, 0], 10, -10, 0);
batch.add('loft', 'a cylinder\'s top face to a circle of radius 5 30 mm above, matching the cylinder\'s side, magnitude 1', [
  can, disc('s2', 30, 5), loft([top, regions('s2')], start('MATCH_TANGENT', 1, { adjacentFacesStart: faceAt(base, [10, 0, -5]) })), probe(loftBody),
]);

// ----- sweeps twisted and scaled along a line and an arc -----

// From the origin 20 mm along x, then a quarter circle of radius 20 about (20, 20) to (40, 20).
const path = [
  sketch('path', PLANES.xy), line('path', 'l1', [0, 0], [20, 0]), arc('path', 'a1', [20, 0], [34.14213562373095, 5.857864376269049], [40, 20]), solve('path'),
];
const rectProfile = [sketch('profile', PLANES.yz), rect('profile', 'rect1', [-3, -2], [3, 2]), solve('profile')];
const sweepBody = solidsOf('sweep1');
batch.add('sweep', 'a rectangle along a line and a quarter circle, twisted a quarter turn', [
  rectProfile, path, call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true' }),
  probe(sweepBody),
]);
batch.add('sweep', 'a rectangle along a line and a quarter circle, scaled to twice its size', [
  rectProfile, path, call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasScale: 'true', scaleFactor: '2' }), probe(sweepBody),
]);

// ----- a torus sheet extended -----

// A quarter of a tube: the arc of radius 3 about (10, 0) in the xz plane from (13, 0) to (10, 3), swept 90° about z.
const sheet = `qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SHEET)`;
const tube = [
  sketch('profile', PLANES.xz), arc('profile', 'arc1', [13, 0], [12.121320343559642, 2.1213203435596424], [10, 3]), solve('profile'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('profile'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: '90 * degree',
  }),
];
const extend = (edge, distance) =>
  call('extendSurface', 'extend1', { entities: edge, endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(distance), oppositeDirection: 'false', maintainCurvature: 'false', tangentPropagation: 'false' });
const sheetProbe = returns({ faces: `faceProbe(context, ${sheet})`, edges: `edgeProbe(context, ${sheet})` });
batch.add('extend-surface', 'a quarter of a tube extended 5 mm round the torus, at its edge in the xz plane', [tube, extend(edgeAt(sheet, [12.121320343559642, 0, 2.1213203435596424]), 5), sheetProbe]);
batch.add('extend-surface', 'a quarter of a tube extended 2 mm round the tube, past its top', [tube, extend(edgeAt(sheet, [7.0710678118654755, 7.0710678118654755, 3]), 2), sheetProbe]);

// ----- isoclines of a torus tilted to the direction -----

const torus = [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [10, 0], 3), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'true' }),
];
batch.add('isocline', 'the 20 degree isoclines of a torus, the direction tilted 30° from its axis, as curves', [
  torus, axisLine('up', [0, 0, 0], [0, 0.5, 0.8660254037844386]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'false' }),
]);

batch.write('tools/fixtures/gen-batch-057.mjs');

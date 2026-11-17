// Generates fixture batch 188: other profiles lofted: a rounded rectangle to a circle (with and without a
// connection), a D (a half circle and its diameter) to a circle, an ellipse to a smaller ellipse, a slot to an
// ellipse, a closed fit spline to a circle, the circle to the slot, and an ellipse and a slot connected to a circle
// twice.
//
// Usage: node tools/fixtures/gen-batch-188.mjs
//
// Writes fixtures/batch-188/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, line, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec2, mm } from './lib.mjs';

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

const batch = new Batch({ number: 188, title: 'probes: lofts of other profiles', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const square = (s, z, h) => [sketch(s, at(z)), rect(s, 'rect1', [-h, -h], [h, h]), solve(s)];
const ring = (s, z, r) => [sketch(s, at(z)), circle(s, 'circle1', [0, 0], r), solve(s)];
const round6 = (v) => Math.round(v * 1e6) / 1e6;
const ngon = (s, z, r, angles) => [sketch(s, at(z)),
  ...polygon(s, angles.map((a) => [round6(r * Math.cos(a * Math.PI / 180)), round6(r * Math.sin(a * Math.PI / 180))])), solve(s)];
const slot = (s, z) => [sketch(s, at(z)), line(s, 'line1', [-10, 5], [10, 5]), arc(s, 'arc1', [10, 5], [15, 0], [10, -5]),
  line(s, 'line2', [10, -5], [-10, -5]), arc(s, 'arc2', [-10, -5], [-15, 0], [-10, 5]), solve(s)];
const ellipse = (s, z, a = 15, b = 8) => [sketch(s, at(z)),
  `skEllipse(${s}, "ellipse1", { "center" : ${vec2(0, 0)}, "majorRadius" : ${mm(a)}, "minorRadius" : ${mm(b)}, "majorAxis" : vector(1, 0) });`, solve(s)];
const sheetOf = (s) => `qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.SHEET)`;
const vertex = (s, [x, y, z]) => `qContainsPoint(qOwnedByBody(${sheetOf(s)}, EntityType.VERTEX), vector(${x}, ${y}, ${z}) * millimeter)`;
const edges = (s) => `qOwnedByBody(${sheetOf(s)}, EntityType.EDGE)`;
const edgeAt = (s, [x, y, z]) => `qContainsPoint(qOwnedByBody(${sheetOf(s)}, EntityType.EDGE), vector(${x}, ${y}, ${z}) * millimeter)`;
// A connection: entities (vertices and edges), the edges among them with a fraction along each.
const connection = (entities, edgeList, params) =>
  `{ "connectionEntities" : qUnion([${entities.join(', ')}]), "connectionEdgeQueries" : qUnion([${edgeList.join(', ')}]), "connectionEdgeParameters" : [${params.join(', ')}] }`;
const loft = (names, connections) => call('loft', 'loft1', {
  sheetProfilesArray: `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`,
  bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
  ...(connections ? { matchConnections: 'true', connections: `[${connections.join(', ')}]` } : {}),
});
const circles = (pairs) => connection([edges('p0'), edges('p1')], [edges('p0'), edges('p1')], pairs);
const toCircle = (corner, t) => connection([vertex('p0', corner), edges('p1')], [edges('p1')], [t]);
const alongToCorner = (point, t, corner) => connection([edgeAt('p0', point), vertex('p1', corner)], [edgeAt('p0', point)], [t]);
const cornerToAlong = (corner, point, t) => connection([vertex('p0', corner), edgeAt('p1', point)], [edgeAt('p1', point)], [t]);
const alongToCircle = (point, t0, t) => connection([edgeAt('p0', point), edges('p1')], [edgeAt('p0', point), edges('p1')], [t0, t]);

const rounded = (s, z) => [sketch(s, at(z)),
  line(s, 'line1', [-8, 5], [8, 5]), arc(s, 'arc1', [8, 5], [8 + 2 * Math.SQRT1_2, 3 + 2 * Math.SQRT1_2], [10, 3]),
  line(s, 'line2', [10, 3], [10, -3]), arc(s, 'arc2', [10, -3], [8 + 2 * Math.SQRT1_2, -3 - 2 * Math.SQRT1_2], [8, -5]),
  line(s, 'line3', [8, -5], [-8, -5]), arc(s, 'arc3', [-8, -5], [-8 - 2 * Math.SQRT1_2, -3 - 2 * Math.SQRT1_2], [-10, -3]),
  line(s, 'line4', [-10, -3], [-10, 3]), arc(s, 'arc4', [-10, 3], [-8 - 2 * Math.SQRT1_2, 3 + 2 * Math.SQRT1_2], [-8, 5]), solve(s)];
const dee = (s, z) => [sketch(s, at(z)), arc(s, 'arc1', [0, -10], [10, 0], [0, 10]), line(s, 'line1', [0, 10], [0, -10]), solve(s)];
const spline = (s, z) => [sketch(s, at(z)),
  `skFitSpline(${s}, "spline1", { "points" : [${[[12, 0], [6, 9], [-8, 7], [-11, -2], [-2, -9], [9, -7], [12, 0]].map(([x, y]) => vec2(x, y)).join(', ')}] });`, solve(s)];
const connectAt = (s0, point, t0, t) => connection([edgeAt(s0, point), edges('p1')], [edgeAt(s0, point), edges('p1')], [t0, t]);
const ellipseTo = (t0, t) => connection([edges('p0'), edges('p1')], [edges('p0'), edges('p1')], [t0, t]);

batch.add('loft', 'a 20 × 10 rectangle with corners rounded to radius 2 at z = 0 to a circle of radius 10 at z = 20', [
  ...rounded('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'the rounded rectangle to the circle, the middle of its edge along x = 10 connected to the circle\'s start', [
  ...rounded('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], [connectAt('p0', [10, 0, 0], 0.5, 0)]), probe]);
batch.add('loft', 'a D (the half circle of radius 10 round the origin from (0, -10) through (10, 0), and its diameter) at z = 0 to a circle of radius 10 at z = 20', [
  ...dee('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'an ellipse (radii 15 along x and 8) at z = 0 to one of radii 10 and 6 at z = 20', [
  ...ellipse('p0', 0), ...ellipse('p1', 20, 10, 6), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'the slot (lines between (±10, ±5), half circles round (±15, 0)) at z = 0 to the ellipse of radii 10 and 6 at z = 20', [
  ...slot('p0', 0), ...ellipse('p1', 20, 10, 6), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'a closed fit spline through (12, 0), (6, 9), (-8, 7), (-11, -2), (-2, -9), (9, -7) at z = 0 to a circle of radius 10 at z = 20', [
  ...spline('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'a circle of radius 10 at z = 0 to the slot at z = 20', [
  ...ring('p0', 0, 10), ...slot('p1', 20), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'the ellipse of radii 15 and 8 to a circle of radius 10, connected twice: 0.1 round to the start, 0.6 round to 0.45', [
  ...ellipse('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], [ellipseTo(0.1, 0), ellipseTo(0.6, 0.45)]), probe]);
batch.add('loft', 'the slot to the circle, connected twice: its end at (15, 0) to the start, the middle of its edge along y = 5 to a third round', [
  ...slot('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], [connectAt('p0', [15, 0, 0], 0.5, 0), connectAt('p0', [0, 5, 0], 0.5, 0.333333)]), probe]);

batch.write('tools/fixtures/gen-batch-188.mjs');

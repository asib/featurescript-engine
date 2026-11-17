// Generates fixture batch 187: more lofts with connections: polygons connected along an edge to another's corner, a
// polygon connected at every corner (or all but one) of a circle in either order, circles connected four times and
// with their longest stretch first, and other profiles (a slot, an ellipse) lofted to a circle with and without a
// connection.
//
// Usage: node tools/fixtures/gen-batch-187.mjs
//
// Writes fixtures/batch-187/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 187, title: 'probes: more lofts with connections, other profiles', helpers });
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
const ellipse = (s, z) => [sketch(s, at(z)),
  `skEllipse(${s}, "ellipse1", { "center" : ${vec2(0, 0)}, "majorRadius" : ${mm(15)}, "minorRadius" : ${mm(8)}, "majorAxis" : vector(1, 0) });`, solve(s)];
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

batch.add('loft', 'a 20 mm square at z = 0 to one at z = 20, the middle of the first\'s edge along x = 10 connected to the second\'s corner at (10, 10)', [
  ...square('p0', 0, 10), ...square('p1', 20, 10), loft(['p0', 'p1'], [alongToCorner([10, 0, 0], 0.5, [10, 10, 20])]), probe]);
batch.add('loft', 'the squares, the middles of the first\'s edges along x = 10 and x = -10 connected to the second\'s corners at (10, 10) and (-10, -10)', [
  ...square('p0', 0, 10), ...square('p1', 20, 10),
  loft(['p0', 'p1'], [alongToCorner([10, 0, 0], 0.5, [10, 10, 20]), alongToCorner([-10, 0, 0], 0.5, [-10, -10, 20])]), probe]);
batch.add('loft', 'the square to a hexagon, corners 10 from the middle, at z = 20, the square\'s corner at (10, 10) connected to the middle of the hexagon\'s edge from (10, 0) to (5, 8.660254)', [
  ...square('p0', 0, 10), ...ngon('p1', 20, 10, [0, 60, 120, 180, 240, 300]),
  loft(['p0', 'p1'], [cornerToAlong([10, 10, 0], [7.5, 4.330127, 20], 0.5)]), probe]);
batch.add('loft', 'an equilateral triangle, corners 10 from the middle, one at (0, 10), to a circle of radius 10 at z = 20, every corner connected: to 0.25, 0.6 and 0.9 round', [
  ...ngon('p0', 0, 10, [90, 210, 330]), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [toCircle([0, 10, 0], 0.25), toCircle([-8.660254, -5, 0], 0.6), toCircle([8.660254, -5, 0], 0.9)]), probe]);
batch.add('loft', 'the square to the circle, every corner connected to the circle\'s point in its direction: 0.125, 0.375, 0.625 and 0.875 round', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [toCircle([10, 10, 0], 0.125), toCircle([-10, 10, 0], 0.375), toCircle([-10, -10, 0], 0.625), toCircle([10, -10, 0], 0.875)]), probe]);
batch.add('loft', 'the square to the circle, three corners connected: (10, 10) to 0.1 round, (-10, 10) to 0.3, (-10, -10) to 0.6', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [toCircle([10, 10, 0], 0.1), toCircle([-10, 10, 0], 0.3), toCircle([-10, -10, 0], 0.6)]), probe]);
batch.add('loft', 'the square to the circle, every corner connected as in batch 186 c004 but listed the other way round: (10, -10) to 0.85, (-10, -10) to 0.6, (-10, 10) to 0.3, (10, 10) to 0.1', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [toCircle([10, -10, 0], 0.85), toCircle([-10, -10, 0], 0.6), toCircle([-10, 10, 0], 0.3), toCircle([10, 10, 0], 0.1)]), probe]);
batch.add('loft', 'circles of radius 10 at z = 0 and 20 connected four times: from their starts, 0.2 round the first to 0.3 round the second, 0.5 to 0.45, 0.7 to 0.8', [
  ...ring('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [circles([0, 0]), circles([0.2, 0.3]), circles([0.5, 0.45]), circles([0.7, 0.8])]), probe]);
batch.add('loft', 'the circles connected twice, the longer stretch first: from their starts and from 0.6 round the first to 0.7 round the second', [
  ...ring('p0', 0, 10), ...ring('p1', 20, 10), loft(['p0', 'p1'], [circles([0, 0]), circles([0.6, 0.7])]), probe]);
batch.add('loft', 'a slot (lines from (-10, 5) to (10, 5) and (10, -5) to (-10, -5), half circles round (15, 0) and (-15, 0)) at z = 0 to a circle of radius 10 at z = 20, the end of the slot at (15, 0) connected to the circle\'s start', [
  ...slot('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], [alongToCircle([15, 0, 0], 0.5, 0)]), probe]);
batch.add('loft', 'an ellipse (radii 15 along x and 8) at z = 0 to the circle, the ellipse 0.1 round connected to the circle\'s start', [
  ...ellipse('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], [alongToCircle([15, 0, 0], 0.1, 0).replace(`qContainsPoint(qOwnedByBody(${sheetOf('p0')}, EntityType.EDGE), vector(15, 0, 0) * millimeter)`, edges('p0')).replace(`qContainsPoint(qOwnedByBody(${sheetOf('p0')}, EntityType.EDGE), vector(15, 0, 0) * millimeter)`, edges('p0'))]), probe]);
batch.add('loft', 'the slot to the circle, no connections', [...slot('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], null), probe]);
batch.add('loft', 'the ellipse to the circle, no connections', [...ellipse('p0', 0), ...ring('p1', 20, 10), loft(['p0', 'p1'], null), probe]);

batch.write('tools/fixtures/gen-batch-187.mjs');

// Generates fixture batch 182: probes of lofts with several connections: a square to a hexagon and a triangle to a
// square connected twice, two squares connected twice a quarter turn round, two circles connected twice unevenly, a
// pentagon connected to a circle three times, and a square connected to a circle at a corner and along an edge.
//
// Usage: node tools/fixtures/gen-batch-182.mjs
//
// Writes fixtures/batch-182/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, line, planeAt, polygon, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

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

const batch = new Batch({ number: 182, title: 'probes: lofts with several connections', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const box = (s, z, [x0, y0], [x1, y1]) => [sketch(s, at(z)), rect(s, 'rect1', [x0, y0], [x1, y1]), solve(s)];
const square = (s, z, h) => box(s, z, [-h, -h], [h, h]);
const ring = (s, z, r) => [sketch(s, at(z)), circle(s, 'circle1', [0, 0], r), solve(s)];
const round6 = (v) => Math.round(v * 1e6) / 1e6;
const ngon = (s, z, r, angles) => [sketch(s, at(z)),
  ...polygon(s, angles.map((a) => [round6(r * Math.cos(a * Math.PI / 180)), round6(r * Math.sin(a * Math.PI / 180))])), solve(s)];
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
  matchConnections: 'true', connections: `[${connections.join(', ')}]`,
});

const plainLoft = (names) => call('loft', 'loft1', {
  sheetProfilesArray: `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`,
  bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});
const triangle = (s, z) => ngon(s, z, 10, [90, 210, 330]);
const hexagon = (s, z) => ngon(s, z, 10, [0, 60, 120, 180, 240, 300]);
const pentagon = (s, z) => ngon(s, z, 10, [90, 162, 234, 306, 18]);
const isosceles = (s, z) => [sketch(s, at(z)), ...polygon(s, [[0, 12], [-10, -6], [10, -6]]), solve(s)];

batch.add('loft', 'a 20 mm square at z = 0 to a hexagon, corners 10 from the middle, at z = 20, its corners at (10, 10) and (-10, -10) connected to the hexagon\'s at (10, 0) and (-10, 0)', [
  ...square('p0', 0, 10), ...hexagon('p1', 20),
  loft(['p0', 'p1'], [connection([vertex('p0', [10, 10, 0]), vertex('p1', [10, 0, 20])], [], []),
    connection([vertex('p0', [-10, -10, 0]), vertex('p1', [-10, 0, 20])], [], [])]), probe]);
batch.add('loft', 'an equilateral triangle, corners 10 from the middle, at z = 0 to the square at z = 20, its corners at (0, 10) and (-8.660254, -5) connected to the corners at (10, 10) and (-10, -10)', [
  ...triangle('p0', 0), ...square('p1', 20, 10),
  loft(['p0', 'p1'], [connection([vertex('p0', [0, 10, 0]), vertex('p1', [10, 10, 20])], [], []),
    connection([vertex('p0', [-8.660254, -5, 0]), vertex('p1', [-10, -10, 20])], [], [])]), probe]);
batch.add('loft', 'the square at z = 0 to one at z = 20, its corners at (10, 10) and (-10, -10) connected to the other\'s at (-10, 10) and (10, -10)', [
  ...square('p0', 0, 10), ...square('p1', 20, 10),
  loft(['p0', 'p1'], [connection([vertex('p0', [10, 10, 0]), vertex('p1', [-10, 10, 20])], [], []),
    connection([vertex('p0', [-10, -10, 0]), vertex('p1', [10, -10, 20])], [], [])]), probe]);
batch.add('loft', 'circles of radius 10 at z = 0 and 20, connected from their starts and from half round the first to a third round the second', [
  ...ring('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [connection([edges('p0'), edges('p1')], [edges('p0'), edges('p1')], [0, 0]),
    connection([edges('p0'), edges('p1')], [edges('p0'), edges('p1')], [0.5, 0.333333])]), probe]);
batch.add('loft', 'a pentagon, corners 10 from the middle, one at (0, 10), at z = 0 to a circle of radius 10 at z = 20, its corners at 90°, 162° and 306° connected to the circle a quarter, two fifths and four fifths round', [
  ...pentagon('p0', 0), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [connection([vertex('p0', [0, 10, 0]), edges('p1')], [edges('p1')], [0.25]),
    connection([vertex('p0', [-9.510565, 3.090170, 0]), edges('p1')], [edges('p1')], [0.4]),
    connection([vertex('p0', [5.877853, -8.090170, 0]), edges('p1')], [edges('p1')], [0.8])]), probe]);
batch.add('loft', 'the square to the circle, its corner at (10, 10) connected to the circle an eighth round and the middle of its edge along y = -10 three quarters round', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [connection([vertex('p0', [10, 10, 0]), edges('p1')], [edges('p1')], [0.125]),
    connection([edgeAt('p0', [0, -10, 0]), edges('p1')], [edgeAt('p0', [0, -10, 0]), edges('p1')], [0.5, 0.75])]), probe]);

batch.write('tools/fixtures/gen-batch-182.mjs');

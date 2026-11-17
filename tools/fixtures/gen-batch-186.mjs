// Generates fixture batch 186: lofts with connections checking the rules batches 180-182 found on other cases
// (circles of different sizes connected twice, circles connected three times, a square connected once along an edge
// off its middle and at every corner unevenly, a hexagon to a larger circle twice, a triangle to a smaller circle at a
// corner and along an edge), and sketch text in Roboto Slab Bold and other letters in fonts batch 185 matched.
//
// Usage: node tools/fixtures/gen-batch-186.mjs
//
// Writes fixtures/batch-186/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec2, PLANES } from './lib.mjs';

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

// Each face of a sketch's regions: its area and box.
function regionList(context is Context, faces is Query) returns array
{
    return mapArray(evaluateQuery(context, faces), function(f)
        {
            const b = evBox3d(context, { "topology" : f, "tight" : true });
            return { "area" : roundToPrecision(evArea(context, { "entities" : f }) / millimeter ^ 2, 6), "min" : round6(b.minCorner), "max" : round6(b.maxCorner) };
        });
}
`;

const batch = new Batch({ number: 186, title: 'probes: lofts with connections, more sketch text', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const square = (s, z, h) => [sketch(s, at(z)), rect(s, 'rect1', [-h, -h], [h, h]), solve(s)];
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
const circles = (p0, p1, pairs) => [connection([edges('p0'), edges('p1')], [edges('p0'), edges('p1')], pairs)];
const toCircle = (corner, t) => connection([vertex('p0', corner), edges('p1')], [edges('p1')], [t]);
const alongToCircle = (point, t0, t) => connection([edgeAt('p0', point), edges('p1')], [edgeAt('p0', point), edges('p1')], [t0, t]);

batch.add('loft', 'circles of radius 10 at z = 0 and 6 at z = 20, connected from their starts and from a quarter round the first to half round the second', [
  ...ring('p0', 0, 10), ...ring('p1', 20, 6),
  loft(['p0', 'p1'], [...circles('p0', 'p1', [0, 0]), ...circles('p0', 'p1', [0.25, 0.5])]), probe]);
batch.add('loft', 'circles of radius 10 at z = 0 and 20, connected three times: from their starts, from 0.3 round the first to 0.25 round the second, and from 0.7 to 0.6', [
  ...ring('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [...circles('p0', 'p1', [0, 0]), ...circles('p0', 'p1', [0.3, 0.25]), ...circles('p0', 'p1', [0.7, 0.6])]), probe]);
batch.add('loft', 'a 20 mm square at z = 0 to a circle of radius 10 at z = 20, connected once: its edge along x = 10 a quarter along as it runs to the circle\'s start', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [alongToCircle([10, 0, 0], 0.25, 0)]), probe]);
batch.add('loft', 'the square to the circle, every corner connected: (10, 10) to 0.1 round, (-10, 10) to 0.3, (-10, -10) to 0.6, (10, -10) to 0.85', [
  ...square('p0', 0, 10), ...ring('p1', 20, 10),
  loft(['p0', 'p1'], [toCircle([10, 10, 0], 0.1), toCircle([-10, 10, 0], 0.3), toCircle([-10, -10, 0], 0.6), toCircle([10, -10, 0], 0.85)]), probe]);
batch.add('loft', 'a hexagon, corners 10 from the middle, at z = 0 to a circle of radius 15 at z = 20, its corners at (10, 0) and (-5, -8.660254) connected to the circle\'s start and 0.6 round', [
  ...ngon('p0', 0, 10, [0, 60, 120, 180, 240, 300]), ...ring('p1', 20, 15),
  loft(['p0', 'p1'], [toCircle([10, 0, 0], 0), toCircle([-5, -8.660254, 0], 0.6)]), probe]);
batch.add('loft', 'an equilateral triangle, corners 10 from the middle, one at (0, 10), at z = 0 to a circle of radius 5 at z = 20: its corner at (0, 10) to a quarter round, and its edge along y = -5 0.3 along as it runs to 0.7 round', [
  ...ngon('p0', 0, 10, [90, 210, 330]), ...ring('p1', 20, 5),
  loft(['p0', 'p1'], [toCircle([0, 10, 0], 0.25), alongToCircle([0, -5, 0], 0.3, 0.7)]), probe]);

const text = (words, font) => [
  `const s = newSketchOnPlane(context, id + "s", { "sketchPlane" : ${PLANES.xy} });`,
  `skText(s, "text1", { "text" : "${words}", "fontName" : "${font}", "firstCorner" : ${vec2(0, 0)}, "secondCorner" : ${vec2(40, 10)} });`,
  solve('s'),
  `return { "regions" : regionList(context, qSketchRegion(id + "s")) };`,
];
batch.add('sketch', '"Ag" in RobotoSlab-Bold.ttf in a 40 × 10 box', text('Ag', 'RobotoSlab-Bold.ttf'));
for (const font of ['RobotoSlab-Regular.ttf', 'RobotoSlab-Bold.ttf', 'NotoSans-Regular.ttf', 'NotoSans-BoldItalic.ttf', 'NotoSerif-Italic.ttf',
  'NotoSerif-Bold.ttf', 'Arimo-Bold.ttf', 'AllertaStencil-Regular.ttf', 'DroidSansMono.ttf']) {
  batch.add('sketch', `"Rf&9" in ${font} in a 40 × 10 box`, text('Rf&9', font));
}

batch.write('tools/fixtures/gen-batch-186.mjs');

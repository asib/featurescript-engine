// Generates fixture batch 178: probes of sheets lofted through three straight lines, the first stretch moving the line
// sideways and the second changing it, so the sheet is a single face whose middle knot gives the second stretch's
// length in the loft's parameter: lines growing about their middles (a quarter, a half), from one end, and growing
// while moving across, as a side of batch 176's squares does.
//
// Usage: node tools/fixtures/gen-batch-178.mjs
//
// Writes fixtures/batch-178/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

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

const batch = new Batch({ number: 178, title: 'probes: sheets lofted through three lines, moved across then changed', helpers });
const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const body = solidsOf('loft1');
const probe = returns({ faces: `faceProbe(context, ${body})`, volume: `roundToPrecision(evVolume(context, { "entities" : ${body} }) / millimeter ^ 3, 6)` });
const box = (s, z, [x0, y0], [x1, y1]) => [sketch(s, at(z)), rect(s, 'rect1', [x0, y0], [x1, y1]), solve(s)];
const square = (s, z, h, [cx, cy] = [0, 0]) => box(s, z, [cx - h, cy - h], [cx + h, cy + h]);
const segment = (s, z, x0, x1, y = 0) => [sketch(s, at(z)), line(s, 'line1', [x0, y], [x1, y]), solve(s)];
const ring = (s, z, r) => [sketch(s, at(z)), circle(s, 'circle1', [0, 0], r), solve(s)];
const triangle = (s, z, r) => [sketch(s, at(z)),
  ...polygon(s, [90, 210, 330].map((a) => [r * Math.cos(a * Math.PI / 180), r * Math.sin(a * Math.PI / 180)].map((v) => Math.round(v * 1e6) / 1e6))), solve(s)];
const loft = (names) => call('loft', 'loft1', {
  sheetProfilesArray: `[${names.map((n) => `{ "sheetProfileEntities" : ${regions(n)} }`).join(', ')}]`,
  bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetLoft = (names) => call('loft', 'loft1', {
  bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW',
  wireProfilesArray: `[${names.map((n) => `{ "wireProfileEntities" : qUnion([${wireEdges(n)}]) }`).join(', ')}]`,
  startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});
const sheets = `qCreatedBy(id + "loft1", EntityType.BODY)`;
const sheetProbe = returns({ faces: `faceProbe(context, ${sheets})`, area: `roundToPrecision(evArea(context, { "entities" : qOwnedByBody(${sheets}, EntityType.FACE) }) / millimeter ^ 2, 6)` });
const names = (n) => Array.from({ length: n }, (_, i) => `p${i}`);

batch.add('loft', 'a sheet through lines from x = -10 to 10 at y = 0, z = 0 and y = 5, z = 10, and from -12.5 to 12.5 at y = 5, z = 20', [
  ...segment('p0', 0, -10, 10, 0), ...segment('p1', 10, -10, 10, 5), ...segment('p2', 20, -12.5, 12.5, 5), sheetLoft(names(3)), sheetProbe]);
batch.add('loft', 'a sheet through lines from x = -10 to 10 at y = 0, z = 0 and y = 5, z = 10, and from -15 to 15 at y = 5, z = 20', [
  ...segment('p0', 0, -10, 10, 0), ...segment('p1', 10, -10, 10, 5), ...segment('p2', 20, -15, 15, 5), sheetLoft(names(3)), sheetProbe]);
batch.add('loft', 'a sheet through lines from x = 0 to 20 at y = 0, z = 0 and y = 5, z = 10, and from 0 to 25 at y = 5, z = 20', [
  ...segment('p0', 0, 0, 20, 0), ...segment('p1', 10, 0, 20, 5), ...segment('p2', 20, 0, 25, 5), sheetLoft(names(3)), sheetProbe]);
batch.add('loft', 'a sheet through lines from x = -10 to 10 at y = 0, z = 0 and y = 5, z = 10, and from -12.5 to 12.5 at y = 7.5, z = 20', [
  ...segment('p0', 0, -10, 10, 0), ...segment('p1', 10, -10, 10, 5), ...segment('p2', 20, -12.5, 12.5, 7.5), sheetLoft(names(3)), sheetProbe]);
batch.add('loft', 'a sheet through lines from x = -10 to 10 at y = 0, z = 0 and y = 5, z = 10, and from -15 to 15 at y = 10, z = 20', [
  ...segment('p0', 0, -10, 10, 0), ...segment('p1', 10, -10, 10, 5), ...segment('p2', 20, -15, 15, 10), sheetLoft(names(3)), sheetProbe]);
batch.add('loft', 'a sheet through lines from x = -10 to 10 at y = 0, z = 0 and 10, and from -12.5 to 12.5 at y = 2.5, z = 20', [
  ...segment('p0', 0, -10, 10, 0), ...segment('p1', 10, -10, 10, 0), ...segment('p2', 20, -12.5, 12.5, 2.5), sheetLoft(names(3)), sheetProbe]);

batch.write('tools/fixtures/gen-batch-178.mjs');

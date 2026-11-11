// Generates fixture batch 116: batch 037 again, its fourth case's aligned section returning its leftover sheets'
// curved edges' definitions. The sheet whose plane origin is 0.025 mm off the engine's (a piece of a default
// plane's rotated copy, bounded by a curve) is only left after the batch's earlier cases (batch 115 c001 alone left
// none), so they come first as they were.
//
// Usage: node tools/fixtures/gen-batch-116.mjs
//
// Writes fixtures/batch-116/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch } from './lib.mjs';

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

const batch = new Batch({ number: 116, title: "batch 037's sections again, the aligned section's leftover sheets' curves probed", helpers });

batch.add('section-part', "batch 037 c001: nothing", [
    'return { "slot" : 0 };',
  ]);
batch.add('section-part', "batch 037 c002", [
    'const baseSketch = newSketchOnPlane(context, id + "baseSketch", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skRectangle(baseSketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });',
    'skSolve(baseSketch);',
    'extrude(context, id + "base", { "entities" : qSketchRegion(id + "baseSketch"), "endBound" : BoundingType.BLIND, "depth" : 20 * millimeter });',
    'const pinSketch = newSketchOnPlane(context, id + "pinSketch", { "sketchPlane" : plane(vector(0, 0, 20) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skCircle(pinSketch, "circle1", { "center" : vector(8, 22) * millimeter, "radius" : 4 * millimeter });',
    'skSolve(pinSketch);',
    'extrude(context, id + "pin", { "entities" : qSketchRegion(id + "pinSketch"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter });',
    'jogSectionPart(context, id + "section1", { "target" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)), "isCropView" : true, "brokenOutPointNumbers" : [4], "jogPoints" : [vector(25, 8, 20) * millimeter, vector(35, 14, 20) * millimeter, vector(31, 25, 20) * millimeter, vector(20, 18, 20) * millimeter, vector(30, 15, 10) * millimeter] });',
  ]);
batch.add('section-part', "batch 037 c003", [
    'const baseSketch = newSketchOnPlane(context, id + "baseSketch", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skRectangle(baseSketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });',
    'skSolve(baseSketch);',
    'extrude(context, id + "base", { "entities" : qSketchRegion(id + "baseSketch"), "endBound" : BoundingType.BLIND, "depth" : 20 * millimeter });',
    'const pinSketch = newSketchOnPlane(context, id + "pinSketch", { "sketchPlane" : plane(vector(0, 0, 20) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skCircle(pinSketch, "circle1", { "center" : vector(8, 22) * millimeter, "radius" : 4 * millimeter });',
    'skSolve(pinSketch);',
    'extrude(context, id + "pin", { "entities" : qSketchRegion(id + "pinSketch"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter });',
    'jogSectionPart(context, id + "section1", { "target" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)), "jogPoints" : [vector(20, 35, 10) * millimeter, vector(20, -5, 10) * millimeter], "offsetPoints" : [vector(28, 15, 10) * millimeter] });',
  ]);
batch.add('section-part', "batch 037 c004's aligned section, its leftover sheets with their curved edges' definitions", [
  ...[
    'const baseSketch = newSketchOnPlane(context, id + "baseSketch", { "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skRectangle(baseSketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });',
    'skSolve(baseSketch);',
    'extrude(context, id + "base", { "entities" : qSketchRegion(id + "baseSketch"), "endBound" : BoundingType.BLIND, "depth" : 20 * millimeter });',
    'const pinSketch = newSketchOnPlane(context, id + "pinSketch", { "sketchPlane" : plane(vector(0, 0, 20) * millimeter, vector(0, 0, 1), vector(1, 0, 0)) });',
    'skCircle(pinSketch, "circle1", { "center" : vector(8, 22) * millimeter, "radius" : 4 * millimeter });',
    'skSolve(pinSketch);',
    'extrude(context, id + "pin", { "entities" : qSketchRegion(id + "pinSketch"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter });',
    'jogSectionPart(context, id + "section1", { "target" : qBodyType(qEverything(EntityType.BODY), BodyType.SOLID), "sketchPlane" : plane(vector(0, 0, 0) * millimeter, vector(0, 0, 1), vector(1, 0, 0)), "jogPoints" : [vector(20, 35, 10) * millimeter, vector(20, 15, 10) * millimeter, vector(35, -5, 10) * millimeter], "isAlignedSection" : true });',
  ],
  'return { "curves" : mapArray(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SHEET)), function(s) { return curveProbe(context, s); }) };',
]);

batch.write('tools/fixtures/gen-batch-116.mjs');

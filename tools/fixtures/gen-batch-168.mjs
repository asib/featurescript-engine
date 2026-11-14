// Generates fixture batch 168: options the engine refused: trimmed conic segments in a sketch (lowParameter and
// highParameter: a parabola, an elliptical and a hyperbolic one), the probe the curve's points and its B-spline; and a
// surface extruded from a line with its second direction up to a curved sheet.
//
// Usage: node tools/fixtures/gen-batch-168.mjs
//
// Writes fixtures/batch-168/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, line, mm, planeAt, sketch, solve } from './lib.mjs';

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
const batch = new Batch({ number: 168, title: 'trimmed conic segments; a surface extruded up to a curved sheet in its second direction', helpers });
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const conic = (rho, low, high) => [
  sketch('s', top),
  `skConicSegment(s, "c1", { "start" : vector(0, 0) * millimeter, "controlPoint" : vector(10, 10) * millimeter, "end" : vector(20, 0) * millimeter, "rho" : ${rho}, "lowParameter" : ${low}, "highParameter" : ${high} });`,
  solve('s'),
  `return { "curves" : curveProbe(context, qCreatedBy(id + "s", EntityType.BODY)) };`,
];
batch.add('sketch', 'a parabolic conic (rho 0.5, from (0, 0) by (10, 10) to (20, 0)) trimmed to parameters 0.2 to 0.7', conic(0.5, 0.2, 0.7));
batch.add('sketch', 'an elliptical conic (rho 0.3) trimmed to parameters 0 to 0.5', conic(0.3, 0, 0.5));
batch.add('sketch', 'a hyperbolic conic (rho 0.8) trimmed to parameters 0.25 to 1', conic(0.8, 0.25, 1));
// A curved sheet (an arc of radius 15 over x from -12 to 12, lowest at z = -15, extruded 30 along y) and a line on the
// top plane extruded as a surface 10 up and down to the sheet.
batch.add('extrude', 'a line extruded as a surface 10 up and down to a curved sheet in its second direction', [
  sketch('arcSketch', planeAt([0, -15, 0], [0, -1, 0], [1, 0, 0])), arc('arcSketch', 'a1', [-12, -6], [0, -15], [12, -6]), solve('arcSketch'),
  call('extrude', 'trough', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "arcSketch", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'true' }),
  sketch('prof', top), line('prof', 'l1', [-6, 2], [6, 2]), solve('prof'),
  call('extrude', 'ext1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "prof", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(10),
    hasSecondDirection: 'true', secondDirectionOppositeDirection: 'true', secondDirectionBound: 'BoundingType.UP_TO_BODY',
    secondDirectionBoundEntityBody: `qBodyType(qCreatedBy(id + "trough", EntityType.BODY), BodyType.SHEET)` }),
]);

batch.write('tools/fixtures/gen-batch-168.mjs');

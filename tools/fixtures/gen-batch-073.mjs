// Generates fixture batch 073: sketch fit splines through three and four points (their B-splines, to see how
// Onshape fits them), isoparametric curves both ways on a revolved torus (and its periodicity, to see which way its
// parameters run), and variable fillets beyond batch 072's conic one: curvature, asymmetric, a conic changing
// straight, and a conic on an edge at 60°.
//
// Usage: node tools/fixtures/gen-batch-073.mjs
//
// Writes fixtures/batch-073/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, circle, deg, edgeAt, edgeOf, line, mm, planeAt, regions, sketch, solidsOf, solve, vec2, vertexAt, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each edge of a body: its length, and a B-spline's degree, knots and poles (indexed, as std does).
function wireProbe(context is Context, wires is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(wires, EntityType.EDGE)))
    {
        var entry = { "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) };
        try
        {
            const c = evCurveDefinition(context, { "edge" : edge });
            if (c is BSplineCurve)
            {
                var points = [];
                for (var i = 0; i < size(c.controlPoints); i += 1)
                    points = append(points, round6(c.controlPoints[i]));
                var knots = [];
                for (var i = 0; i < size(c.knots); i += 1)
                    knots = append(knots, roundToPrecision(c.knots[i], 9));
                entry.bspline = { "degree" : c.degree, "rational" : c.isRational, "points" : points, "knots" : knots };
            }
            else if (c is Circle)
                entry.circle = { "radius" : roundToPrecision(c.radius / millimeter, 6) };
            else if (c is Line)
                entry.line = true;
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        out = append(out, entry);
    }
    return out;
}

// Three points along each edge of a body (its ends and middle, in its own parameter).
function edgePoints(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}

// Each face of a body: its area and its surface's type.
function faceProbe(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(body, EntityType.FACE)))
    {
        var entry = { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6) };
        try
        {
            entry["type"] = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true }).surfaceType;
        }
        catch (error)
        {
            entry["type"] = "failed";
        }
        out = append(out, entry);
    }
    return out;
}
`;

const batch = new Batch({
  number: 73,
  title: 'sketch fit splines, isoparametric curves on a torus, variable curvature, asymmetric and conic fillets',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const xy = PLANES.xy;
const sketchProbe = returns({ spline: `wireProbe(context, qCreatedBy(id + "s1", EntityType.BODY))` });
const fit = (points) => [
  sketch('s1', xy), `skFitSpline(s1, "spline1", { "points" : [${points.map(([x, y]) => vec2(x, y)).join(', ')}] });`, solve('s1'), sketchProbe,
];

// ----- sketch fit splines -----

batch.add('sketch-curve', 'a fit spline through (0, 0), (30, 10), (60, 0)', fit([[0, 0], [30, 10], [60, 0]]));
batch.add('sketch-curve', 'a fit spline through (0, 0), (20, 15), (60, 0)', fit([[0, 0], [20, 15], [60, 0]]));
batch.add('sketch-curve', 'a fit spline through (0, 0), (10, 10), (40, 5), (50, 20)', fit([[0, 0], [10, 10], [40, 5], [50, 20]]));

// ----- isoparametric curves on a torus -----

// A quarter torus: the circle of radius 5 about (20, 0) in the xz plane revolved 90° about z.
const torus = [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [20, 0], 5), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: deg(90),
  }),
];
const torusFace = `qGeometry(qOwnedByBody(${solidsOf('base')}, EntityType.FACE), GeometryType.TORUS)`;
const iso = (direction) => [
  torus,
  call('isoparametricCurve', 'curve1', { face: torusFace, directionType: `DirectionType.${direction}`, equalSpacing: 'true', nCurves: '3' }),
  returns({
    periodicity: `[evFacePeriodicity(context, { "face" : ${torusFace} }), evFacePeriodicity(context, { "face" : ${torusFace}, "trimmed" : false })]`,
    curves: `edgePoints(context, qCreatedBy(id + "curve1", EntityType.BODY))`,
  }),
];
batch.add('isoparametric-curve', 'three isoparametric curves in the U direction on a quarter torus, and its periodicity', iso('U_DIRECTION'));
batch.add('isoparametric-curve', 'three isoparametric curves in the V direction on a quarter torus, and its periodicity', iso('V_DIRECTION'));

// ----- variable fillets -----

const base = solidsOf('base');
const topEdge = edgeAt(base, [20, 0, 12]);
const variable = (settings, fields) =>
  call('fillet', 'fillet1', {
    entities: topEdge, radius: mm(3), isVariable: 'true',
    vertexSettings: `[${settings.map(([x, s]) => `{ "vertex" : ${vertexAt(base, [x, 0, 12])}, ${s} }`).join(', ')}]`, ...fields,
  });
const filletProbe = returns({ faces: `faceProbe(context, ${base})` });
batch.add('fillet', 'a variable fillet, 2 mm to 5 mm along a block\'s top edge, curvature (magnitude 0.5)', [
  box('base', [0, 0, 0], [40, 30, 12]),
  variable([[0, `"vertexRadius" : ${mm(2)}, "variableMagnitude" : 0.5`], [40, `"vertexRadius" : ${mm(5)}, "variableMagnitude" : 0.5`]],
    { crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(3), magnitude: '0.5', smoothTransition: 'true' }),
  filletProbe,
]);
batch.add('fillet', 'a variable asymmetric fillet along a block\'s top edge: 2 and 3 mm at one end, 4 and 5 mm at the other', [
  box('base', [0, 0, 0], [40, 30, 12]),
  variable([[0, `"vertexRadius" : ${mm(2)}, "vertexOtherRadius" : ${mm(3)}, "vertexFlipAsymmetric" : false`],
    [40, `"vertexRadius" : ${mm(4)}, "vertexOtherRadius" : ${mm(5)}, "vertexFlipAsymmetric" : false`]],
  { isAsymmetric: 'true', otherRadius: mm(3), flipAsymmetric: 'false', smoothTransition: 'true' }),
  filletProbe,
]);
batch.add('fillet', 'a variable conic fillet (rho 0.7), 2 mm to 5 mm along a block\'s top edge, changing straight', [
  box('base', [0, 0, 0], [40, 30, 12]),
  variable([[0, `"vertexRadius" : ${mm(2)}, "variableRho" : 0.7`], [40, `"vertexRadius" : ${mm(5)}, "variableRho" : 0.7`]],
    { crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(3), rho: '0.7', smoothTransition: 'false' }),
  filletProbe,
]);
// A prism whose top meets its front at 60° along the edge from (0, 0, 12) to (40, 0, 12): its front leans back.
batch.add('fillet', 'a variable conic fillet (rho 0.7), 2 mm to 4 mm, on an edge where the faces meet at 60 degrees', [
  sketch('s1', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])),
  line('s1', 'l1', [0, 0], [30, 0]), line('s1', 'l2', [30, 0], [30, 12]), line('s1', 'l3', [30, 12], [6.928203230275509, 12]),
  line('s1', 'l4', [6.928203230275509, 12], [0, 0]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(40) }),
  call('fillet', 'fillet1', {
    entities: edgeAt(base, [20, 6.928203230275509, 12]), radius: mm(3), nonCircularRadius: mm(3), isVariable: 'true', crossSection: 'FilletCrossSection.CONIC',
    rho: '0.7', smoothTransition: 'true',
    vertexSettings: `[{ "vertex" : ${vertexAt(base, [0, 6.928203230275509, 12])}, "vertexRadius" : ${mm(2)}, "variableRho" : 0.7 }, { "vertex" : ${vertexAt(base, [40, 6.928203230275509, 12])}, "vertexRadius" : ${mm(4)}, "variableRho" : 0.7 }]`,
  }),
  filletProbe,
]);

batch.write('tools/fixtures/gen-batch-073.mjs');

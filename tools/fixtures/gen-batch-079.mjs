// Generates fixture batch 079: face blends with a cap where one side stops short of it, a cliff edge, a help point
// choosing between two solutions and a tangent hold line; an elliptical arc (a rational B-spline) extended straight
// on and curving on; a revolved spline sheet extended past its top edge.
//
// Usage: node tools/fixtures/gen-batch-079.mjs
//
// Writes fixtures/batch-079/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, call, circle, deg, edgeOf, line, mm, planeAt, rect, sketch, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
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

// Each edge of a body: its length, and a B-spline's degree, knots, poles and weights.
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
                if (c.isRational)
                    entry.bspline.weights = mapArray(c.weights, function(w) { return roundToPrecision(w, 9); });
            }
            else if (c is Line)
                entry.line = true;
            else
                entry.other = true;
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

const batch = new Batch({
  number: 79,
  title: 'face blends with an end cap, a cliff edge, a help point and a hold line; an elliptical arc extended; a revolved sheet extended',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const createdBy = (name, type = 'BODY') => `qCreatedBy(id + "${name}", EntityType.${type})`;
const sheetProbe = (name) => returns({ faces: `faceProbe(context, ${sheetsOf(name)})` });

// ----- face blends -----

// Batch 054's corner: "floor" upright at y = 0 (x 0..length, z 0..height), "wall" level at z = 0 (x 0..length,
// y 0..20); the blend goes in the corner between them.
const upright = (length, height) => [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), line('s1', 'l1', [0, 0], [length, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(height), oppositeDirection: 'false' }),
];
const level = (length) => [
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [length, 0]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const blendFields = {
  propagation: 'false', crossSection: 'FaceBlendCrossSection.ROLLING_BALL', blendControlType: 'BlendControlType.RADIUS', asymmetric: 'false',
  crossSectionShape: 'FaceBlendCrossSectionShape.CIRCULAR', tangentHoldLines: 'false', conicHoldLines: 'false', hasCliffEdges: 'false',
  hasCaps: 'false', hasLimits: 'false', hasHelpPoint: 'false', trim: 'FaceBlendTrimType.WALLS', detach: 'false', showSelection: 'false',
  showIsocurves: 'false',
};
const cornerBlend = (fields) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('floor')}, EntityType.FACE)`, flipSide1: 'true', side2: `qOwnedByBody(${sheetsOf('wall')}, EntityType.FACE)`,
    flipSide2: 'false', radius: mm(5), ...blendFields, ...fields,
  });
// A 30 × 30 sheet in the plane x = 25 across the corner.
const capAt25 = [sketch('capSketch', planeAt([25, 0, 0], [1, 0, 0], [0, 1, 0])), rect('capSketch', 'r1', [-5, -5], [25, 25]), solve('capSketch')];
const cap = (flip) => ({ hasCaps: 'true', caps: `[{ "entity" : qOwnedByBody(${sheetsOf('capSketch')}, EntityType.FACE), "capFlip" : ${flip} }]` });

batch.add('face-blend', 'batch 054\'s corner, the level wall only 20 mm long, blended 5 mm and capped by a sheet across it at x = 25', [
  upright(30, 20), level(20), capAt25, cornerBlend(cap('false')), sheetProbe('floor'),
]);
batch.add('face-blend', 'the same, the cap flipped', [upright(30, 20), level(20), capAt25, cornerBlend(cap('true')), sheetProbe('floor')]);
batch.add('face-blend', 'batch 054\'s corner, the upright only 3 mm tall, blended 5 mm over its top edge as a cliff edge', [
  upright(30, 3), level(30),
  cornerBlend({ hasCliffEdges: 'true', cliffEdges: `qContainsPoint(qOwnedByBody(${sheetsOf('floor')}, EntityType.EDGE), ${vec3(15, 0, 3)})` }),
  sheetProbe('floor'),
]);
batch.add('face-blend', 'batch 054\'s corner, the upright only 4 mm tall, blended with its top edge as a tangent hold line', [
  upright(30, 4), level(30),
  cornerBlend({ tangentHoldLines: 'true', tangentEdges: `qContainsPoint(qOwnedByBody(${sheetsOf('floor')}, EntityType.EDGE), ${vec3(15, 0, 4)})`, inverseTangentEdges: 'qNothing()' }),
  sheetProbe('floor'),
]);
// A floor sheet (z = 0, 40 × 40 about the origin) through a cylinder sheet of radius 10 lying along x at z = 5: they
// meet along y = ±8.66; a help point picks the side y > 0.
batch.add('face-blend', 'a floor sheet through a cylinder sheet lying along it, blended 2 mm on the side a help point at (0, 14, 0) picks', [
  sketch('s1', PLANES.xy), rect('s1', 'r1', [-20, -20], [20, 20]), solve('s1'),
  sketch('s2', planeAt([-15, 0, 0], [1, 0, 0], [0, 1, 0])), circle('s2', 'c1', [0, 5], 10), solve('s2'),
  call('extrude', 'cyl', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'false' }),
  `opPoint(context, id + "help", { "point" : ${vec3(0, 14, 0)} });`,
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('s1')}, EntityType.FACE)`, flipSide1: 'false', side2: `qOwnedByBody(${sheetsOf('cyl')}, EntityType.FACE)`,
    flipSide2: 'false', radius: mm(2), ...blendFields, hasHelpPoint: 'true', helpPoint: createdBy('help', 'VERTEX'),
  }),
  sheetProbe('s1'),
]);

// ----- an elliptical arc extended -----

// A quarter of the ellipse with radii 20 (along x) and 10 about the origin, from (20, 0) to (0, 10).
const ellipse = [
  sketch('s1', PLANES.xy),
  `skEllipticalArc(s1, "e1", { "center" : ${vec2(0, 0)}, "majorAxis" : vector(1, 0), "majorRadius" : ${mm(20)}, "minorRadius" : ${mm(10)}, "startParameter" : 0, "endParameter" : PI / 2 });`,
  solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
];
const extend = (shape) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), flipHeuristics: 'false', helpPoint: vertexAt(createdBy('curve1'), [0, 10, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: `CurveExtensionShape.${shape}`,
  });
const curveProbe = returns({ edges: `wireProbe(context, ${createdBy('curve1')})` });
batch.add('trim-curve', 'a quarter ellipse (radii 20 and 10) extended 10 mm at its end (0, 10), straight on', [ellipse, extend('TANGENT'), curveProbe]);
batch.add('trim-curve', 'the same, curving on', [ellipse, extend('CURVATURE'), curveProbe]);

// ----- a revolved sheet extended -----

batch.add('extend-surface', 'a fit spline through (10, 0), (15, 10), (12, 20) in the xz plane revolved 90° about z as a sheet, its top edge extended 5 mm', [
  sketch('s1', PLANES.xz), `skFitSpline(s1, "spline1", { "points" : [${vec2(10, 0)}, ${vec2(15, 10)}, ${vec2(12, 20)}] });`, solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', angle: deg(90), oppositeDirection: 'false',
  }),
  call('extendSurface', 'extend1', {
    entities: `qContainsPoint(qOwnedByBody(${sheetsOf('base')}, EntityType.EDGE), ${vec3(12, 0, 20)})`, tangentPropagation: 'false',
    endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(5), maintainCurvature: 'false',
  }),
  sheetProbe('base'),
]);

batch.write('tools/fixtures/gen-batch-079.mjs');

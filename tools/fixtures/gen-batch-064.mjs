// Generates fixture batch 064: curvature fillets from magnitude 0.975 to 0.9975 (the fourth regime), and flat
// patterns of bent plates: a 60° flange, a flange on another edge, a flange bent down, a channel's second flange.
//
// Usage: node tools/fixtures/gen-batch-064.mjs
//
// Writes fixtures/batch-064/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, deg, edgeAt, mm, rect, regions, solidsOf, solve, vec3 } from './lib.mjs';

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

// Each edge of a body: its ends and its length.
function edgeEnds(context is Context, body is Query) returns array
{
    var out = [];
    for (var edge in evaluateQuery(context, qOwnedByBody(body, EntityType.EDGE)))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, { "start" : round6(lines[0].origin), "middle" : round6(lines[1].origin), "end" : round6(lines[2].origin),
                            "length" : roundToPrecision(evLength(context, { "entities" : edge }) / millimeter, 6) });
    }
    return out;
}

// A coordinate system's numbers, in mm, for comparing.
function frameProbe(cs) returns map
{
    return { "origin" : round6(cs.origin), "x" : [roundToPrecision(cs.xAxis[0], 6), roundToPrecision(cs.xAxis[1], 6), roundToPrecision(cs.xAxis[2], 6)],
             "z" : [roundToPrecision(cs.zAxis[0], 6), roundToPrecision(cs.zAxis[1], 6), roundToPrecision(cs.zAxis[2], 6)] };
}

// The coordinate systems of a body's cylindrical faces.
function cylinderFrames(context is Context, body is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qGeometry(qOwnedByBody(body, EntityType.FACE), GeometryType.CYLINDER)))
        out = append(out, frameProbe(evSurfaceDefinition(context, { "face" : face }).coordSystem));
    return out;
}

// A sheet metal plate 40 x 30, 2 mm thick, with a 10 mm flange up from its y = 30 edge.
function smBracket(context is Context, id is Id)
{
    const sketch = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, id + "sm1", { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "s1"),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
    const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, id + "flange1", {
            "edges" : qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 30, 2) * millimeter),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 10 * millimeter,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : false, "bendAngle" : 90 * degree,
            "autoMiter" : true, "useDefaultRadius" : true });
}

// A sheet metal plate 40 x 30, 2 mm thick, with 10 mm flanges up from both its y = 0 and y = 30 edges (a channel).
function smChannel(context is Context, id is Id)
{
    const sketch = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, id + "sm1", { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "s1"),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
    const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, id + "flange1", {
            "edges" : qUnion([qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 30, 2) * millimeter),
                              qClosestTo(qOwnedByBody(part, EntityType.EDGE), vector(20, 0, 2) * millimeter)]),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : 10 * millimeter,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : false, "bendAngle" : 90 * degree,
            "autoMiter" : true, "useDefaultRadius" : true });
}

// A sheet metal plate 40 x 30, 2 mm thick, 1 mm bends, with a flange of "distance" from the edge nearest "edgePoint",
// bent "angle" (up, or down if "down").
function smFlanged(context is Context, id is Id, edgePoint is Vector, distance is ValueWithUnits, angle is ValueWithUnits, down is boolean)
{
    const sketch = newSketchOnPlane(context, id + "s1", { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(sketch, "rect1", { "firstCorner" : vector(0, 0) * millimeter, "secondCorner" : vector(40, 30) * millimeter });
    skSolve(sketch);
    sheetMetalStart(context, id + "sm1", { "process" : SMProcessType.THICKEN, "regions" : qSketchRegion(id + "s1"),
                                   "thickness" : 2 * millimeter, "oppositeDirection" : false, "radius" : 1 * millimeter,
                                   "flipDirectionUp" : false, "tangentPropagation" : false, "bends" : qNothing() });
    const part = qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID);
    sheetMetalFlange(context, id + "flange1", {
            "edges" : qClosestTo(qOwnedByBody(part, EntityType.EDGE), edgePoint),
            "flangeAlignment" : SMFlangeAlignment.INNER, "limitType" : SMFlangeBoundingType.BLIND, "distance" : distance,
            "angleControlType" : SMFlangeAngleControlType.BEND_ANGLE, "oppositeDirection" : down, "bendAngle" : angle,
            "autoMiter" : true, "useDefaultRadius" : true });
}
`;

const batch = new Batch({ number: 64, title: 'curvature fillets 0.975 to 0.9975, flat patterns of more bent plates', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');

for (const magnitude of [0.975, 0.9775, 0.9825, 0.9875, 0.9925, 0.9975]) {
  batch.add('fillet', `a block's upright edge, curvature, 4 mm, magnitude ${magnitude}`, [
    box('base', [0, 0, 0], [30, 20, 10]),
    call('fillet', 'fillet1', { entities: edgeAt(base, [0, 0, 5]), crossSection: 'FilletCrossSection.CURVATURE', nonCircularRadius: mm(4), magnitude: String(magnitude) }),
    returns({ faces: `faceProbe(context, ${base})` }),
  ]);
}

const part = (name) => `qBodyType(qCreatedBy(id + "${name}" + "sm1", EntityType.BODY), BodyType.SOLID)`;
const inFlat = (name, point, shape, type) => {
  const flatFace = `qCorrespondingInFlat(qContainsPoint(qOwnedByBody(${part(name)}, EntityType.FACE), ${vec3(...point)}))`;
  return [
    `const flatPlane = evPlane(context, { "face" : ${flatFace} });`,
    `const f = newSketch(context, id + "f", { "sketchPlane" : ${flatFace} });`, shape, solve('f'),
    call('extrude', 'flat1', {
      bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: `FlatOperationType.${type}`,
      endBound: 'BoundingType.BLIND', depth: mm(2),
    }),
    returns({ flatPlane: '{ "origin" : round6(flatPlane.origin), "normal" : flatPlane.normal, "x" : flatPlane.x }', faces: `faceProbe(context, ${part(name)})` }),
  ];
};
batch.add('sheet-metal-flat', 'a plate with a 10 mm flange bent 60° up from its y = 30 edge, a 4 x 40 slot across the bend cut in its flat', [
  `smFlanged(context, id + "p", ${vec3(20, 30, 2)}, ${mm(10)}, ${deg(60)}, false);`, inFlat('p', [20, 15, 2], rect('f', 'r', [-2, -5], [2, 35]), 'REMOVE'),
]);
batch.add('sheet-metal-flat', 'a plate with a 15 mm flange up from its x = 40 edge, a 4 mm circle cut in the flat of the flange\'s inner face', [
  `smFlanged(context, id + "p", ${vec3(40, 15, 2)}, ${mm(15)}, ${deg(90)}, false);`, inFlat('p', [38, 15, 8], circle('f', 'circle1', [0, 0], 2), 'REMOVE'),
]);
batch.add('sheet-metal-flat', 'a plate with a 10 mm flange bent down from its y = 30 edge, a 4 x 40 slot across the bend cut in its flat', [
  `smFlanged(context, id + "p", ${vec3(20, 30, 2)}, ${mm(10)}, ${deg(90)}, true);`, inFlat('p', [20, 15, 2], rect('f', 'r', [-2, -5], [2, 35]), 'REMOVE'),
]);
batch.add('sheet-metal-flat', 'a channel, a 4 mm circle cut in the flat of its y = 0 flange\'s outer face', [
  'smChannel(context, id + "channel");', inFlat('channel', [20, -2, 6], circle('f', 'circle1', [0, 0], 2), 'REMOVE'),
]);

batch.write('tools/fixtures/gen-batch-064.mjs');

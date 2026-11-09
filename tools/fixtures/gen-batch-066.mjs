// Generates fixture batch 066: a second survey: a loft's B-spline sides split by isocline, a lofted sheet extended
// past a straight side, an arc extended up to a circle, an edge offset across a lofted B-spline face, a sweep
// twisted along a line and a spline, and a ruled surface normal to a loft's side from its bottom edge.
//
// Usage: node tools/fixtures/gen-batch-066.mjs
//
// Writes fixtures/batch-066/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, deg, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3, vertexAt, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 66, title: 'isocline splits and offsets on B-spline faces, sheet and curve extensions, sweeps along chains with splines, ruled surfaces off B-spline faces', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const wires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const loft = solidsOf('loft1');
const tangentLoft = [
  sketch('s1', at(0)), rect('s1', 'r', [-10, -10], [10, 10]), solve('s1'), sketch('s2', at(20)), rect('s2', 'r', [-5, -5], [5, 5]), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    startCondition: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', startMagnitude: '1',
  }),
];
const sides = `qGeometry(qOwnedByBody(${loft}, EntityType.FACE), GeometryType.OTHER_SURFACE)`;
const up = 'qContainsPoint(qCreatedBy(makeId("Top"), EntityType.FACE), vector(0, 0, 0) * meter)';

batch.add('isocline', 'the sides of batch 065 c003\'s loft split at 30° to z', [
  tangentLoft,
  call('isocline', 'iso1', { faces: sides, direction: up, oppositeDirection: 'false', angle: deg(30), split: 'true' }),
  returns({ faces: `faceProbe(context, ${loft})` }),
]);

const arcsSheet = [
  sketch('a1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), arc('a1', 'arc1', [0, 0], [15, 6], [30, 0]), solve('a1'),
  sketch('a2', planeAt([0, 20, 0], [0, -1, 0], [1, 0, 0])), arc('a2', 'arc1', [0, 0], [15, 10], [30, 0]), solve('a2'),
  call('loft', 'sheet', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('a1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('a2')}]) }]`,
  }),
];
batch.add('extend-surface', 'batch 065 c004\'s sheet extended 3 mm past its straight side at x = 30', [
  arcsSheet,
  call('extendSurface', 'extend1', {
    entities: `qContainsPoint(qOwnedByBody(${sheetsOf('sheet')}, EntityType.EDGE), ${vec3(30, 10, 0)})`, endCondition: 'ExtendBoundingType.BLIND',
    extendDistance: mm(3), oppositeDirection: 'false', maintainCurvature: 'false', tangentPropagation: 'false',
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('sheet')})`, edges: `edgeProbe(context, ${sheetsOf('sheet')})` }),
]);

batch.add('trim-curve', 'a quarter circle of radius 10 extended at its end up to a circle of radius 5 about (-5, 8)', [
  sketch('c', PLANES.xy), arc('c', 'a1', [10, 0], [7.0710678, 7.0710678], [0, 10]), solve('c'),
  call('compositeCurve', 'curve1', { edges: wireEdges('c') }),
  sketch('limit', PLANES.xy), circle('limit', 'c1', [-5, 8], 5), solve('limit'),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: wires('curve1'), endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY',
    extendTo: `qCreatedBy(id + "limit", EntityType.EDGE)`, extensionShape: 'CurveExtensionShape.TANGENT',
    helpPoint: vertexAt(wires('curve1'), [0, 10, 0]), flipHeuristics: 'false',
  }),
  returns({ edges: `edgeEnds(context, ${wires('curve1')})` }),
]);

batch.add('offset-curve-on-face', 'the bottom edge of a side of batch 065 c003\'s loft offset 4 mm across the side', [
  tangentLoft,
  call('offsetCurveOnFace', 'offset1', {
    edges: `qContainsPoint(qOwnedByBody(${loft}, EntityType.EDGE), ${vec3(-10, 0, 0)})`, distance: mm(4), offsetType: 'OffsetCurveType.GEODESIC',
    oppositeDirection: 'false', gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET',
    targets: `qContainsPoint(qOwnedByBody(${loft}, EntityType.FACE), ${vec3(-9.279898, 0, 0.648481)})`,
  }),
  returns({ edges: `edgeEnds(context, ${wires('offset1')})` }),
]);

batch.add('sweep', 'a 4 x 6 rectangle along a 10 mm line then a fit spline on through (25, 5), (40, 0), twisted a quarter turn', [
  sketch('profile', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), rect('profile', 'rect1', [-2, -3], [2, 3]), solve('profile'),
  sketch('path', PLANES.xy), line('path', 'l1', [0, 0], [10, 0]),
  'skFitSpline(path, "spline1", { "points" : [vector(10, 0) * millimeter, vector(25, 5) * millimeter, vector(40, 0) * millimeter], "startDerivative" : vector(15, 0) * millimeter });',
  solve('path'),
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true' }),
  returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);

batch.add('ruled-surface', 'a ruled surface 4 mm wide normal to a side of batch 065 c003\'s loft from its bottom edge', [
  tangentLoft,
  call('ruledSurface', 'ruled1', {
    edges: `qContainsPoint(qOwnedByBody(${loft}, EntityType.EDGE), ${vec3(-10, 0, 0)})`, ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(4),
    referenceFaces: `qContainsPoint(qOwnedByBody(${loft}, EntityType.FACE), ${vec3(-9.279898, 0, 0.648481)})`,
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('ruled1')})`, edges: `edgeProbe(context, ${sheetsOf('ruled1')})` }),
]);

batch.write('tools/fixtures/gen-batch-066.mjs');

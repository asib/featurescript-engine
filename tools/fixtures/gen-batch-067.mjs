// Generates fixture batch 067: a third survey: an edge offset down a cone and across a torus, an arc extended with
// its curvature up to a circle, a ruled surface normal to a cone from its top circle, and a block's top replaced
// up to a lofted sheet.
//
// Usage: node tools/fixtures/gen-batch-067.mjs
//
// Writes fixtures/batch-067/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, cylinder, edgeAt, faceAt, mm, planeAt, regions, sketch, solidsOf, solve, vec3, vertexAt, PLANES } from './lib.mjs';

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

const batch = new Batch({ number: 67, title: 'offsets on cones and tori, curving extensions up to curves, ruled surfaces normal to cones, faces replaced up to B-spline sheets', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const wires = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.WIRE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const offset = (edges, targets, distance) => call('offsetCurveOnFace', 'offset1', {
  edges, distance: mm(distance), offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false', gapFill: 'GapFill.LINEAR',
  scope: 'OffsetCurveScope.OFFSET', targets,
});

batch.add('offset-curve-on-face', 'the top circle of a cone (radius 12 at z = 0 to 8 at z = 30) offset 3 mm down its side', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 30), bottomRadius: mm(12), topRadius: mm(8) }),
  offset(edgeAt(base, [8, 0, 30]), `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`, 3),
  returns({ edges: `edgeEnds(context, ${wires('offset1')})` }),
]);
batch.add('offset-curve-on-face', 'the edge where a 4 mm round of a cylinder\'s top meets the top, offset 2 mm across the round', [
  cylinder('base', [0, 0], 10, 0, 20),
  call('fillet', 'round1', { entities: edgeAt(base, [10, 0, 20]), radius: mm(4) }),
  offset(edgeAt(base, [6, 0, 20]), `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.TORUS)`, 2),
  returns({ edges: `edgeEnds(context, ${wires('offset1')})` }),
]);
batch.add('trim-curve', 'a quarter circle of radius 10 extended at its end with its curvature up to a circle of radius 5 about (-5, 8)', [
  sketch('c', PLANES.xy), arc('c', 'a1', [10, 0], [7.0710678, 7.0710678], [0, 10]), solve('c'),
  call('compositeCurve', 'curve1', { edges: wireEdges('c') }),
  sketch('limit', PLANES.xy), circle('limit', 'c1', [-5, 8], 5), solve('limit'),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: wires('curve1'), endCondition: 'CurveExtensionEndCondition.UP_TO_ENTITY',
    extendTo: 'qCreatedBy(id + "limit", EntityType.EDGE)', extensionShape: 'CurveExtensionShape.CURVATURE',
    helpPoint: vertexAt(wires('curve1'), [0, 10, 0]), flipHeuristics: 'false',
  }),
  returns({ edges: `edgeEnds(context, ${wires('curve1')})` }),
]);
batch.add('ruled-surface', 'a ruled surface 4 mm wide normal to a cone from its top circle', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 0, 30), bottomRadius: mm(12), topRadius: mm(8) }),
  call('ruledSurface', 'ruled1', {
    edges: edgeAt(base, [8, 0, 30]), ruledType: 'RuledSurfaceInterfaceType.NORMAL', distance: mm(4),
    referenceFaces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`,
  }),
  returns({ faces: `faceProbe(context, ${sheetsOf('ruled1')})`, edges: `edgeProbe(context, ${sheetsOf('ruled1')})` }),
]);
batch.add('replace-face', 'a 30 x 20 x 10 block\'s top replaced up to a sheet lofted between arcs above it', [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('a1', planeAt([0, -1, 0], [0, -1, 0], [1, 0, 0])), arc('a1', 'arc1', [-2, 12], [15, 16], [32, 12]), solve('a1'),
  sketch('a2', planeAt([0, 21, 0], [0, -1, 0], [1, 0, 0])), arc('a2', 'arc1', [-2, 12], [15, 18], [32, 12]), solve('a2'),
  call('loft', 'sheet', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('a1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('a2')}]) }]`,
  }),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: `qOwnedByBody(${sheetsOf('sheet')}, EntityType.FACE)`, offset: mm(0) }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

batch.write('tools/fixtures/gen-batch-067.mjs');

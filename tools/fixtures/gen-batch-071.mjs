// Generates fixture batch 071: face blends limited by planes and capped by a face, between a floor and a cylinder
// and between a floor and a wall of several faces; a quadratic Bézier looked at and a quartic one extended; half a
// tilted torus split at its isoclines; a flex PCB's one bend unfolded; a hole cut in the flat of a rolled wall with
// a flange.
//
// Usage: node tools/fixtures/gen-batch-071.mjs
//
// Writes fixtures/batch-071/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, circle, deg, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

const helpers = `
function round6(v) returns array
{
    return [roundToPrecision(v[0] / millimeter, 6), roundToPrecision(v[1] / millimeter, 6), roundToPrecision(v[2] / millimeter, 6)];
}

// Each edge of a body: its curve type and length, and a B-spline's degree, knots and poles (indexed, as std does).
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
            else if (c is Line)
                entry.line = true;
            else
                entry.other = true;
        }
        catch (error)
        {
            entry.curve = "failed";
        }
        const ends = evaluateQuery(context, qAdjacent(edge, AdjacencyType.VERTEX, EntityType.VERTEX));
        var points = [];
        for (var v in ends)
            points = append(points, round6(evVertexPoint(context, { "vertex" : v })));
        entry.vertices = points;
        out = append(out, entry);
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

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// The bend lines a finished flex PCB keeps on each solid.
function pcbBendLines(context is Context) returns array
{
    var out = [];
    for (var part in evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)))
    {
        for (var info in getPcbBendLinesAttribute(context, part))
        {
            out = append(out, { "start" : round6(pcbBendLineStartPoint(info)), "end" : round6(pcbBendLineEndPoint(info)),
                                "angle" : roundToPrecision(info.angle / degree, 6), "radius" : roundToPrecision(info.radius / millimeter, 6),
                                "bendUp" : info.bendUp });
        }
    }
    return out;
}
`;

const batch = new Batch({
  number: 71,
  title: 'face blends limited, capped, on a cylinder and walls of several faces; Béziers; a tilted half torus split; a PCB bend unfolded; a cut in a rolled flat with a flange',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const createdBy = (name) => `qCreatedBy(id + "${name}", EntityType.BODY)`;
const xy = PLANES.xy;

// ----- face blends -----

// Batch 054's corner: "floor" upright at y = 0 (x 0..30, z 0..20), "wall" level at z = 0 (x 0..30, y 0..20); the
// blend goes in the corner between them.
const upright = [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const level = [
  sketch('s2', PLANES.xz), line('s2', 'l1', [0, 0], [30, 0]), solve('s2'),
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
const plane = (name, x) => call('opPlane', name, { plane: planeAt([x, 0, 0], [1, 0, 0], [0, 1, 0]), width: mm(60), height: mm(60) });
const limits = (first, second) => ({
  hasLimits: 'true', limitPlane1: first, limitPlane1Flip: 'false', limitPlane2: second, limitPlane2Flip: 'false', faceLimits: 'qNothing()', edgeLimits: '[]',
});
const sheetProbe = (name) => returns({ faces: `faceProbe(context, ${sheetsOf(name)})` });
batch.add('face-blend', 'batch 054\'s corner blended 5 mm, limited by the plane x = 10', [
  upright, level, plane('limit1', 10), cornerBlend(limits(`qCreatedBy(id + "limit1", EntityType.FACE)`, 'qNothing()')), sheetProbe('floor'),
]);
batch.add('face-blend', 'the same, limited by the planes x = 10 and x = 20', [
  upright, level, plane('limit1', 10), plane('limit2', 20),
  cornerBlend(limits(`qCreatedBy(id + "limit1", EntityType.FACE)`, `qCreatedBy(id + "limit2", EntityType.FACE)`)), sheetProbe('floor'),
]);
// A 30 × 30 sheet in the plane x = 15 across the corner, as a cap.
batch.add('face-blend', 'the same, capped by a sheet across it at x = 15', [
  upright, level,
  sketch('capSketch', planeAt([15, 0, 0], [1, 0, 0], [0, 1, 0])), rect('capSketch', 'r1', [-5, -5], [25, 25]), solve('capSketch'),
  cornerBlend({ hasCaps: 'true', caps: `[{ "entity" : qOwnedByBody(${sheetsOf('capSketch')}, EntityType.FACE), "capFlip" : false }]` }), sheetProbe('floor'),
]);

// A floor sheet (z = 0, 40 × 40 about the origin) and an upright cylinder sheet (radius 10 about z, 20 tall); the
// blend outside the cylinder, above the floor.
const floorAndCylinder = [
  sketch('s1', xy), rect('s1', 'r1', [-20, -20], [20, 20]), solve('s1'),
  sketch('s2', xy), circle('s2', 'c1', [0, 0], 10), solve('s2'),
  call('extrude', 'cyl', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
const floorBlend = (side2, flip2) =>
  call('faceBlend', 'blend1', {
    side1: `qOwnedByBody(${sheetsOf('s1')}, EntityType.FACE)`, flipSide1: 'false', side2: `qOwnedByBody(${sheetsOf(side2)}, EntityType.FACE)`,
    flipSide2: flip2, radius: mm(3), ...blendFields,
  });
batch.add('face-blend', 'a floor sheet and an upright cylinder sheet of radius 10 on it, blended 3 mm (the cylinder\'s side not flipped)', [
  floorAndCylinder, floorBlend('cyl', 'false'), sheetProbe('s1'),
]);
batch.add('face-blend', 'the same, the cylinder\'s side flipped', [floorAndCylinder, floorBlend('cyl', 'true'), sheetProbe('s1')]);

// The floor and a wall of three faces standing on it: lines from (20, -10) to (0, -10) and from (-10, 0) to
// (-10, 20) joined by an arc of radius 10 about (0, 0), 20 tall.
const floorAndWall = [
  sketch('s1', xy), rect('s1', 'r1', [-20, -20], [20, 20]), solve('s1'),
  sketch('s2', xy), line('s2', 'l1', [20, -10], [0, -10]), arc('s2', 'a1', [0, -10], [-7.0710678118654755, -7.0710678118654755], [-10, 0]),
  line('s2', 'l2', [-10, 0], [-10, 20]), solve('s2'),
  call('extrude', 'wall', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s2'), endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'false' }),
];
batch.add('face-blend', 'a floor sheet and a wall of a line, an arc and a line standing on it, blended 3 mm (the wall not flipped)', [
  floorAndWall, floorBlend('wall', 'false'), sheetProbe('s1'),
]);
batch.add('face-blend', 'the same, the wall flipped', [floorAndWall, floorBlend('wall', 'true'), sheetProbe('s1')]);

// ----- Béziers -----

batch.add('sketch-curve', 'a quadratic Bézier from (0, 0) through the control point (10, 20) to (30, 0), and a curve made of it', [
  sketch('s1', xy), `skBezier(s1, "bezier1", { "points" : [${vec2(0, 0)}, ${vec2(10, 20)}, ${vec2(30, 0)}] });`, solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  returns({ sketch: `wireProbe(context, qCreatedBy(id + "s1", EntityType.BODY))`, curve: `wireProbe(context, ${createdBy('curve1')})` }),
]);
batch.add('trim-curve', 'a quartic Bézier from (0, 0) to (30, 0) extended 10 mm at its end, straight on', [
  sketch('s1', xy), `skBezier(s1, "bezier1", { "points" : [${vec2(0, 0)}, ${vec2(5, 15)}, ${vec2(15, 20)}, ${vec2(25, 15)}, ${vec2(30, 0)}] });`, solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), flipHeuristics: 'false', helpPoint: vertexAt(createdBy('curve1'), [30, 0, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.TANGENT',
  }),
  returns({ before: `wireProbe(context, qCreatedBy(id + "s1", EntityType.BODY))`, edges: `wireProbe(context, ${createdBy('curve1')})` }),
]);

// ----- half a tilted torus split -----

const base = solidsOf('base');
batch.add('isocline', 'half a torus (revolved 180°) split at its 20 degree isoclines, the direction tilted 30° from its axis', [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [10, 0], 3), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: deg(180),
  }),
  axisLine('up', [0, 0, 0], [0, 0.5, 0.8660254037844386]),
  call('isocline', 'iso1', {
    faces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.TORUS)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'true',
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- a flex PCB's bend unfolded -----

const pcbPart = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
batch.add('pcb', 'a flex PCB plate (0.2 mm) with flanges on two sides, finished with one bend unfolded holding its top, and its bend lines', [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('pcbStart', 'sm1', {
    thickness: mm(0.2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', kFactor: '0.5', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qUnion([qClosestTo(qOwnedByBody(${pcbPart}, EntityType.EDGE), ${vec3(40, 15, 0.2)}), qClosestTo(qOwnedByBody(${pcbPart}, EntityType.EDGE), ${vec3(20, 30, 0.2)})])`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE',
    oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  call('pcbEnd', 'end1', {
    unfoldType: 'PcbUnfoldType.UNFOLD_SELECTED', pcbPart,
    entitiesToUnfold: `qClosestTo(qGeometry(qOwnedByBody(${pcbPart}, EntityType.FACE), GeometryType.CYLINDER), ${vec3(41, 15, 1)})`,
    holdEntity: `qContainsPoint(qOwnedByBody(${pcbPart}, EntityType.FACE), ${vec3(20, 15, 0.2)})`,
  }),
  returns({ counts: 'bodyCounts(context)', bendLines: 'pcbBendLines(context)' }),
]);

// ----- a hole cut in the flat of a rolled wall with a flange -----

const smParts = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
batch.add('sheet-metal-flat', 'batch 069\'s half tube with a flange, a 6 mm circle cut out of the middle of its flat pattern', [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qClosestTo(qOwnedByBody(${smParts}, EntityType.EDGE), vector(20, 0, 12.5) * millimeter)`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(10),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  `const f = newSketch(context, id + "f", { "sketchPlane" : qCorrespondingInFlat(qLargest(qGeometry(qOwnedByBody(${smParts}, EntityType.FACE), GeometryType.CYLINDER))) });`,
  circle('f', 'circle1', [0, 0], 3), solve('f'),
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: 'FlatOperationType.REMOVE',
    endBound: 'BoundingType.BLIND', depth: mm(2),
  }),
  returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${smParts})` }),
]);

batch.write('tools/fixtures/gen-batch-071.mjs');

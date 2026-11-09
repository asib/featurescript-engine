// Generates fixture batch 070: curves of other degrees and rational ones extended, isoclines of half a torus tilted
// to the direction, a draft on both sides of a cylindrical parting sheet, sweeps and a frame locked to curved faces,
// a flex PCB unfolded with its bend lines, a conical shell recognized as sheet metal, and a hole cut in a rolled
// wall's flat pattern.
//
// Usage: node tools/fixtures/gen-batch-070.mjs
//
// Writes fixtures/batch-070/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

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
                var weights = [];
                if (c.isRational)
                {
                    for (var i = 0; i < size(c.weights); i += 1)
                        weights = append(weights, roundToPrecision(c.weights[i], 9));
                }
                entry.bspline = { "degree" : c.degree, "rational" : c.isRational, "points" : points, "knots" : knots, "weights" : weights };
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

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
}

// The part's faces' areas and kinds.
function faceAreas(context is Context, part is Query) returns array
{
    var out = [];
    for (var face in evaluateQuery(context, qOwnedByBody(part, EntityType.FACE)))
    {
        const s = evSurfaceDefinition(context, { "face" : face, "returnBSplinesAsOther" : true });
        out = append(out, { "area" : roundToPrecision(evArea(context, { "entities" : face }) / millimeter ^ 2, 6), "type" : s.surfaceType });
    }
    return out;
}

// The bend lines a finished flex PCB keeps (pcbEnd's setPcbBendLinesAttribute) on each solid.
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

// A frame profile, the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;

const batch = new Batch({
  number: 70,
  title: 'quadratic and rational curves extended, half a tilted torus, a draft both sides of a curved parting sheet, sweeps locked to curved faces, a PCB unfolded, a conical shell recognized, a cut in a rolled flat',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const createdBy = (name) => `qCreatedBy(id + "${name}", EntityType.BODY)`;
const base = solidsOf('base');
const xy = PLANES.xy;

// ----- curves of other degrees and rational ones extended -----

const curve1 = createdBy('curve1');
const extend = (end, shape) =>
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: curve1, flipHeuristics: 'false', helpPoint: vertexAt(curve1, end),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: `CurveExtensionShape.${shape}`,
  });
// A quadratic Bézier from (0, 0) through the control point (10, 20) to (30, 0).
const quadratic = [
  sketch('s1', xy), `skBezier(s1, "bezier1", { "points" : [${vec2(0, 0)}, ${vec2(10, 20)}, ${vec2(30, 0)}] });`, solve('s1'),
  call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
];
// A conic from (20, 0) to (0, 20) about the corner at the origin, rho 0.7 (a rational quadratic).
const conic = [
  sketch('s1', xy), `skConicSegment(s1, "conic1", { "start" : ${vec2(20, 0)}, "controlPoint" : ${vec2(0, 0)}, "end" : ${vec2(0, 20)}, "rho" : 0.7 });`,
  solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
];
const curveProbe = returns({ before: `wireProbe(context, qCreatedBy(id + "s1", EntityType.BODY))`, edges: `wireProbe(context, ${curve1})` });
batch.add('trim-curve', 'a quadratic Bézier from (0, 0) to (30, 0) extended 10 mm at its end, straight on', [quadratic, extend([30, 0, 0], 'TANGENT'), curveProbe]);
batch.add('trim-curve', 'the same, curving on', [quadratic, extend([30, 0, 0], 'CURVATURE'), curveProbe]);
batch.add('trim-curve', 'a conic (rho 0.7) from (20, 0) to (0, 20) extended 10 mm at its end, straight on', [conic, extend([0, 20, 0], 'TANGENT'), curveProbe]);
batch.add('trim-curve', 'the same, curving on', [conic, extend([0, 20, 0], 'CURVATURE'), curveProbe]);

// ----- isoclines of half a torus tilted to the direction -----

batch.add('isocline', 'the 20 degree isoclines of half a torus (revolved 180°), the direction tilted 30° from its axis, as curves', [
  sketch('profile', PLANES.xz), circle('profile', 'c1', [10, 0], 3), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: deg(180),
  }),
  axisLine('up', [0, 0, 0], [0, 0.5, 0.8660254037844386]),
  call('isocline', 'iso1', {
    faces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.TORUS)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'false',
  }),
  returns({ edges: `edgeProbe(context, ${createdBy('iso1')})` }),
]);

// ----- a draft on both sides of a cylindrical parting sheet -----

batch.add('body-draft', 'a block drafted 3 degrees on both sides of a cylindrical sheet through it, matching at the parting sheet', [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]),
  sketch('arcSketch', PLANES.xz), `skArc(arcSketch, "arc1", { "start" : vector(-5, 6) * millimeter, "mid" : vector(20, 9) * millimeter, "end" : vector(45, 6) * millimeter });`, solve('arcSketch'),
  call('extrude', 'part', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('arcSketch'), endBound: 'BoundingType.BLIND', depth: mm(40), oppositeDirection: 'true' }),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()', draftOnSelf: 'false', pullDirection: edgeOf('pull'), flipPullDirection: 'false',
    partingObject: `qOwnedByBody(qBodyType(qCreatedBy(id + "part", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`, angle: deg(3), bothSides: 'true',
    matchFacesAtParting: 'true', matchFaceType: 'BodyDraftMatchFaceType.TANGENT_TO_FACE', concaveRepair: 'BodyDraftConcaveRepairType.NONE',
    cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false',
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

// ----- sweeps and a frame locked to curved faces -----

const locked = (path, lock) =>
  call('sweep', 'sweep1', {
    profiles: regions('prof'), path, bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: lock,
  });
// A frustum about z: radius 15 at z = 0 to 10 at z = 10.
batch.add('sweep', 'a 2 × 3 rectangle swept round a frustum\'s top circle, locked to its conical side', [
  sketch('profile', PLANES.xz), line('profile', 'l1', [0, 0], [15, 0]), line('profile', 'l2', [15, 0], [10, 10]), line('profile', 'l3', [10, 10], [0, 10]),
  line('profile', 'l4', [0, 10], [0, 0]), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', { entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'true' }),
  sketch('prof', planeAt([10, 0, 10], [0, 1, 0], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  locked(edgeAt(base, [10, 0, 10]), `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`),
  returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
// Half a cylinder of radius 10 about z, 30 tall: its straight edge at (10, 0) runs up its side.
batch.add('sweep', 'a 2 × 3 rectangle swept up the straight edge of half a cylinder, locked to its curved side', [
  sketch('s1', xy), arc('s1', 'a1', [10, 0], [0, 10], [-10, 0]), line('s1', 'l1', [-10, 0], [10, 0]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(30) }),
  sketch('prof', planeAt([10, 0, 0], [0, 0, 1], [1, 0, 0])), rect('prof', 'r', [0, 0], [2, 3]), solve('prof'),
  locked(edgeAt(base, [10, 0, 15]), faceAt(base, [0, 10, 15])),
  returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);
batch.add('frame', 'a square tube along a quarter of a cylinder\'s top rim, its profile locked to the cylinder\'s side', [
  cylinder('base', [0, 0], 20, 0, 10),
  sketch('path', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), arc('path', 'a1', [20, 0], [14.142135623730951, 14.142135623730951], [0, 20]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), lockProfile: 'true', profileLockFaces: faceAt(base, [20, 0, 5]) }),
  returns({ faces: `faceProbe(context, ${solidsOf('frame1')})` }),
]);

// ----- a flex PCB unfolded -----

const pcbPart = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
batch.add('pcb', 'a flex PCB plate (0.2 mm) with a flange, finished with every bend unfolded holding its top, and its bend lines', [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('pcbStart', 'sm1', {
    thickness: mm(0.2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', kFactor: '0.5', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qClosestTo(qOwnedByBody(${pcbPart}, EntityType.EDGE), ${vec3(40, 15, 0.2)})`, flangeAlignment: 'SMFlangeAlignment.INNER',
    limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false',
    bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  call('pcbEnd', 'end1', {
    unfoldType: 'PcbUnfoldType.UNFOLD_ALL', pcbPart, entitiesToUnfold: 'qNothing()',
    holdEntity: `qContainsPoint(qOwnedByBody(${pcbPart}, EntityType.FACE), ${vec3(20, 15, 0.2)})`,
  }),
  returns({ counts: 'bodyCounts(context)', bendLines: 'pcbBendLines(context)' }),
]);

// ----- a conical shell recognized as sheet metal -----

// The wall of a frustum 2 mm thick (its sides parallel, (−1, 2) / √5), revolved half round z.
batch.add('sheet-metal-recognize', 'half a conical shell 2 mm thick recognized as sheet metal', [
  sketch('profile', PLANES.xz), line('profile', 'l1', [10, 0], [12.236067977, 0]), line('profile', 'l2', [12.236067977, 0], [7.236067977, 10]),
  line('profile', 'l3', [7.236067977, 10], [5, 10]), line('profile', 'l4', [5, 10], [10, 0]), solve('profile'), axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    entities: regions('profile'), axis: edgeOf('axis'), fullRevolve: 'false', endBound: 'RevolveBoundingType.BLIND', oppositeDirection: 'false', angle: deg(180),
  }),
  call('sheetMetalRecognize', 'recognize1', { bodies: base, radius: mm(1), changeThickness: 'false', keepInputParts: 'false', kFactor: '0.45', minimalClearance: mm(0.02) }),
  returns({ counts: 'bodyCounts(context)', faces: `faceAreas(context, qBodyType(qEverything(EntityType.BODY), BodyType.SOLID))` }),
]);

// ----- a hole cut in a rolled wall's flat pattern -----

const smParts = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
// The rolled wall's outer face, its flat's sketch plane centred on its middle.
const flatFace = `qCorrespondingInFlat(qClosestTo(qGeometry(qOwnedByBody(${smParts}, EntityType.FACE), GeometryType.CYLINDER), ${vec3(0, 40, 12.5)}))`;
batch.add('sheet-metal-flat', 'half a tube of radius 20 with an 8 mm circle cut out of the middle of its flat pattern', [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  `const f = newSketch(context, id + "f", { "sketchPlane" : ${flatFace} });`, circle('f', 'circle1', [0, 0], 4), solve('f'),
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: 'FlatOperationType.REMOVE',
    endBound: 'BoundingType.BLIND', depth: mm(2),
  }),
  returns({ counts: 'bodyCounts(context)', faces: `faceAreas(context, ${smParts})` }),
]);

batch.write('tools/fixtures/gen-batch-070.mjs');

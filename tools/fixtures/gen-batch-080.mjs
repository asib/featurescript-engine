// Generates fixture batch 080: face blends with a conic hold line and an inverse tangent hold line; a loft from a
// circle to a square with a connection; batch 069's rolled half tube cut through its rolled wall and added to in
// its flat; a quintic extended curving on; a revolved spline sheet extended past its top edge alone and round past
// its side alone (batch 079 c008 did both at once).
//
// Usage: node tools/fixtures/gen-batch-080.mjs
//
// Writes fixtures/batch-080/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, call, circle, deg, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

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

// Each edge of a body that isn't straight: three points along it.
function curvedEdges(context is Context, body is Query) returns array
{
    var out = [];
    const edges = qOwnedByBody(body, EntityType.EDGE);
    for (var edge in evaluateQuery(context, qSubtraction(edges, qGeometry(edges, GeometryType.LINE))))
    {
        const lines = evEdgeTangentLines(context, { "edge" : edge, "parameters" : [0, 0.5, 1], "arcLengthParameterization" : false });
        out = append(out, mapArray(lines, function(l) { return round6(l.origin); }));
    }
    return out;
}

// Each edge of a body: its length, and a B-spline's degree, knots and poles.
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
`;

const batch = new Batch({
  number: 80,
  title: 'face blend conic and inverse hold lines, a circle lofted to a square with a connection, a rolled wall cut and added to, a quintic extended, a revolved sheet extended each way',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const createdBy = (name, type = 'BODY') => `qCreatedBy(id + "${name}", EntityType.${type})`;
const sheetProbe = (name) => returns({ faces: `faceProbe(context, ${sheetsOf(name)})` });
const xy = PLANES.xy;

// ----- face blend hold lines -----

// Batch 054's corner: "floor" upright at y = 0 (x 0..30, z 0..height), "wall" level at z = 0 (x 0..30, y 0..20).
const corner = (height) => [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  call('extrude', 'floor', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), endBound: 'BoundingType.BLIND', depth: mm(height), oppositeDirection: 'false' }),
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
const topEdge = `qContainsPoint(qOwnedByBody(${sheetsOf('floor')}, EntityType.EDGE), ${vec3(15, 0, 4)})`;
batch.add('face-blend', 'batch 054\'s corner, the upright 4 mm tall, blended 5 mm with its top edge as a conic hold line', [
  corner(4), cornerBlend({ conicHoldLines: 'true', conicEdges: topEdge, inverseConicEdges: 'qNothing()' }), sheetProbe('floor'),
]);
batch.add('face-blend', 'the same, its top edge an inverse tangent hold line', [
  corner(4), cornerBlend({ tangentHoldLines: 'true', tangentEdges: 'qNothing()', inverseTangentEdges: topEdge }), sheetProbe('floor'),
]);

// ----- a circle lofted to a square with a connection -----

const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const regionVertex = (s, p) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.SHEET), EntityType.VERTEX), ${vec3(...p)})`;
batch.add('loft', 'a circle of radius 10 to a 20 mm square 20 mm above, the circle\'s vertex at (10, 0) connected to the corner at (10, 10)', [
  sketch('s1', at(0)), circle('s1', 'c1', [0, 0], 10), solve('s1'), sketch('s2', at(20)), rect('s2', 'r1', [-10, -10], [10, 10]), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`, matchConnections: 'true',
    connections: `[{ "connectionEntities" : qUnion([${regionVertex('s1', [10, 0, 0])}, ${regionVertex('s2', [10, 10, 20])}]), "connectionEdgeQueries" : qNothing(), "connectionEdgeParameters" : [] }]`,
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('loft1')})`, curves: `curvedEdges(context, ${solidsOf('loft1')})` }),
]);

// ----- a rolled wall cut and added to -----

const smParts = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const halfTube = [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
];
const smProbe = returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${smParts})` });
batch.add('sheet-metal', 'batch 069\'s half tube (radius 20, 2 mm thick, 25 long) cut through its top by an 8 mm circle extruded down from y = 30', [
  halfTube,
  sketch('c1', planeAt([0, 30, 0], [0, 1, 0], [1, 0, 0])), circle('c1', 'circle1', [0, -12.5], 4), solve('c1'),
  call('extrude', 'cut1', {
    bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.REMOVE', entities: regions('c1'), endBound: 'BoundingType.BLIND',
    depth: mm(15), oppositeDirection: 'true', booleanScope: smParts,
  }),
  smProbe,
]);
batch.add('sheet-metal-flat', 'the half tube with a 10 × 6 tab added in its flat pattern at the end of its flat', [
  halfTube,
  `const f = newSketch(context, id + "f", { "sketchPlane" : qCorrespondingInFlat(qLargest(qGeometry(qOwnedByBody(${smParts}, EntityType.FACE), GeometryType.CYLINDER))) });`,
  rect('f', 'r1', [-5, 12.5], [5, 18.5]), solve('f'),
  call('extrude', 'flat1', {
    bodyType: 'ExtendedToolBodyType.SOLID', entities: regions('f'), domain: 'OperationDomain.FLAT', flatOperationType: 'FlatOperationType.ADD',
    endBound: 'BoundingType.BLIND', depth: mm(2),
  }),
  smProbe,
]);

// ----- a quintic extended -----

batch.add('trim-curve', 'a fit spline through (0, 0), (15, 10), (30, 0) raised to degree 5, extended 10 mm at its end, curving on', [
  sketch('s1', xy), `skFitSpline(s1, "spline1", { "points" : [${vec2(0, 0)}, ${vec2(15, 10)}, ${vec2(30, 0)}] });`, solve('s1'),
  call('editCurve', 'curve1', {
    wire: `sketchEntityQuery(id + "s1", EntityType.EDGE, "spline1")`, approximate: 'false', elevate: 'true', elevationDegree: '5', planarize: 'false',
    editControlPoints: 'false', showDetails: 'false', approximationShowDeviation: 'false',
  }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.EXTEND', wires: createdBy('curve1'), flipHeuristics: 'false', helpPoint: vertexAt(createdBy('curve1'), [30, 0, 0]),
    endCondition: 'CurveExtensionEndCondition.BLIND', extensionDistance: mm(10), extensionShape: 'CurveExtensionShape.CURVATURE',
  }),
  returns({ edges: `wireProbe(context, ${createdBy('curve1')})` }),
]);

// ----- a revolved sheet extended each way -----

const revolved = [
  sketch('s1', PLANES.xz), `skFitSpline(s1, "spline1", { "points" : [${vec2(10, 0)}, ${vec2(15, 10)}, ${vec2(12, 20)}] });`, solve('s1'),
  axisLine('axis', [0, 0, -50], [0, 0, 50]),
  call('revolve', 'base', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('s1'), axis: edgeOf('axis'), fullRevolve: 'false',
    endBound: 'RevolveBoundingType.BLIND', angle: deg(90), oppositeDirection: 'false',
  }),
];
const extendAt = (p) =>
  call('extendSurface', 'extend1', {
    entities: `qContainsPoint(qOwnedByBody(${sheetsOf('base')}, EntityType.EDGE), ${vec3(...p)})`, tangentPropagation: 'false',
    endCondition: 'ExtendBoundingType.BLIND', extendDistance: mm(5), maintainCurvature: 'false',
  });
const extendProbe = returns({ faces: `faceProbe(context, ${sheetsOf('base')})`, curves: `curvedEdges(context, ${sheetsOf('base')})` });
// The top edge, a quarter circle of radius 12 at z = 20, through (8.485, 8.485, 20).
batch.add('extend-surface', 'batch 079 c008\'s revolved sheet extended 5 mm past its top edge alone', [revolved, extendAt([8.485281374238571, 8.485281374238571, 20]), extendProbe]);
// The side edge at angle 0 is the spline itself, through (15, 0, 10).
batch.add('extend-surface', 'the same sheet extended 5 mm round past its side edge alone', [revolved, extendAt([15, 0, 10]), extendProbe]);

batch.write('tools/fixtures/gen-batch-080.mjs');

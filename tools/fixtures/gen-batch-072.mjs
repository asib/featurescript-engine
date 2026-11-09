// Generates fixture batch 072: std paths not yet exercised: cut lists of frames along an arc and a spline (their
// lengths from curves drawn on the swept faces), bodies cut from sheet metal that don't go square through it (a
// sphere and a slanted cylinder from a plate, a box from a rolled wall), a sweep twisted along a line and a spline,
// and an offset curve on a sphere.
//
// Usage: node tools/fixtures/gen-batch-072.mjs
//
// Writes fixtures/batch-072/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, deg, edgeAt, edgeOf, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec2, vec3, vertexAt, PLANES } from './lib.mjs';

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

function tableText(t is map) returns array
{
    var out = [mapArray(t.columnDefinitions, function(c) { return toString(c.name); })];
    for (var row in t.rows)
        out = append(out, mapArray(t.columnDefinitions, function(c) { return toString(row.columnIdToCell[c.id]); }));
    return out;
}

function caseTables(context is Context, caseId is Id, tables) returns array
{
    var out = [];
    for (var t in tables)
    {
        if (t.entities == undefined || !isQueryEmpty(context, qIntersection([t.entities, qCreatedBy(caseId, EntityType.BODY)])))
            out = append(out, tableText(t));
    }
    return out;
}

// Frame profiles, each the only sketch of its own context, as a profile from an element library would be.
const squareTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skSolve(s);
    return context;
};

const roundTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skCircle(s, "outer", { "center" : vector(0, 0) * millimeter, "radius" : 6 * millimeter });
    skCircle(s, "inner", { "center" : vector(0, 0) * millimeter, "radius" : 5 * millimeter });
    skSolve(s);
    return context;
};

function profileOf(build is function) returns PartStudioData
{
    return { "buildFunction" : build, "configuration" : {}, "partQuery" : qEverything(EntityType.BODY) } as PartStudioData;
}
`;

const batch = new Batch({
  number: 72,
  title: 'cut lists of frames along an arc and a spline, bodies cut through sheet metal at a slant, a sweep twisted along a line and a spline, an offset curve on a sphere',
  helpers,
});

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = PLANES.xy;

// ----- cut lists of frames along curves -----

const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const cutlist = [
  call('cutlist', 'cutlist1', { selectAllFrames: 'false', frames: frames('frame1') }),
  returns({ table: 'caseTables(context, id, cutlistTable(context, {}))', faces: `faceProbe(context, ${frames('frame1')})` }),
];
batch.add('cutlist', 'a round tube along a quarter circle of radius 40, and its cut list', [
  sketch('path', xy), arc('path', 'a1', [40, 0], [28.284271247461902, 28.284271247461902], [0, 40]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(roundTube)', selections: wireEdges('path') }),
  cutlist,
]);
batch.add('cutlist', 'a square tube along a fit spline through (0, 0), (30, 10), (60, 0), and its cut list', [
  sketch('path', xy), `skFitSpline(path, "spline1", { "points" : [${vec2(0, 0)}, ${vec2(30, 10)}, ${vec2(60, 0)}] });`, solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path') }),
  cutlist,
]);

// ----- bodies cut through sheet metal at a slant -----

const smParts = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
const subtract = [
  call('booleanBodies', 'cut1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: smParts, tools: solidsOf('tool'), offset: 'false', reFillet: 'false', keepTools: 'false',
  }),
];
const smProbe = returns({ counts: 'bodyCounts(context)', faces: `faceProbe(context, ${smParts})` });
batch.add('sheet-metal-boolean', 'a 40 x 30 plate, 2 mm, with a sphere of radius 6 about (20, 15, 2) taken away', [
  plate, call('opSphere', 'tool', { center: vec3(20, 15, 2), radius: mm(6) }), subtract, smProbe,
]);
// A cylinder of radius 4 along (0, sin 30°, cos 30°) through (20, 15, 1), 30 long both ways.
batch.add('sheet-metal-boolean', 'the plate with a cylinder of radius 4 through it, leaning 30° from upright, taken away', [
  plate,
  sketch('t1', planeAt([20, 15 - 15 * 0.5, 1 - 15 * 0.8660254037844386], [0, 0.5, 0.8660254037844386], [1, 0, 0])),
  `skCircle(t1, "c1", { "center" : ${vec2(0, 0)}, "radius" : ${mm(4)} });`, solve('t1'),
  call('extrude', 'tool', { entities: regions('t1'), endBound: 'BoundingType.BLIND', depth: mm(30) }),
  subtract, smProbe,
]);
batch.add('sheet-metal-boolean', 'a half tube of radius 20 (sheet metal extruded 25 mm) with a 10 mm cube across its top taken away', [
  sketch('s1', xy), arc('s1', 'a1', [20, 0], [0, 20], [-20, 0]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false',
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false',
  }),
  sketch('t1', planeAt([0, 0, 7.5], [0, 0, 1], [1, 0, 0])), rect('t1', 'r1', [-5, 15], [5, 25]), solve('t1'),
  call('extrude', 'tool', { entities: regions('t1'), endBound: 'BoundingType.BLIND', depth: mm(10) }),
  subtract, smProbe,
]);

// ----- a sweep twisted along a line and a spline -----

batch.add('sweep', 'a 4 x 6 rectangle along a line and a fit spline leaving it tangentially, twisted a quarter turn', [
  sketch('profile', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), rect('profile', 'rect1', [-2, -3], [2, 3]), solve('profile'),
  sketch('path', xy), line('path', 'l1', [0, 0], [10, 0]),
  `skFitSpline(path, "spline1", { "points" : [${vec2(10, 0)}, ${vec2(25, 5)}, ${vec2(40, 0)}], "startDerivative" : ${vec2(15, 0)} });`, solve('path'),
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true' }),
  returns({ faces: `faceProbe(context, ${solidsOf('sweep1')})` }),
]);

// ----- an offset curve on a sphere -----

// A hemisphere of radius 10 (a quarter disc revolved about z): its equator offset 3 mm up the sphere.
batch.add('offset-curve-on-face', 'a hemisphere\'s equator offset 3 mm up its sphere', [
  sketch('profile', PLANES.xz), line('profile', 'l1', [0, 0], [10, 0]), arc('profile', 'a1', [10, 0], [7.0710678118654755, 7.0710678118654755], [0, 10]),
  line('profile', 'l2', [0, 10], [0, 0]), solve('profile'),
  sketch('axisSketch', planeAt([0, 0, 0], [0, -1, 0], [0, 0, 1])), line('axisSketch', 'l1', [-50, 0], [50, 0]), solve('axisSketch'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axisSketch", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('offsetCurveOnFace', 'offset1', {
    edges: `qContainsPoint(qOwnedByBody(${solidsOf('base')}, EntityType.EDGE), ${vec3(10, 0, 0)})`, distance: mm(3),
    offsetType: 'OffsetCurveType.GEODESIC', oppositeDirection: 'false', gapFill: 'GapFill.LINEAR', scope: 'OffsetCurveScope.OFFSET',
    targets: `qGeometry(qOwnedByBody(${solidsOf('base')}, EntityType.FACE), GeometryType.SPHERE)`,
  }),
  returns({ edges: `edgeProbe(context, qCreatedBy(id + "offset1", EntityType.BODY))` }),
]);

// ----- a draft on both sides of a curved parting sheet, not matching -----

batch.add('body-draft', 'a block drafted 3 degrees on both sides of a cylindrical sheet through it, not matching at the sheet', [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]),
  sketch('arcSketch', PLANES.xz), `skArc(arcSketch, "arc1", { "start" : vector(-5, 6) * millimeter, "mid" : vector(20, 9) * millimeter, "end" : vector(45, 6) * millimeter });`, solve('arcSketch'),
  call('extrude', 'part', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('arcSketch'), endBound: 'BoundingType.BLIND', depth: mm(40), oppositeDirection: 'true' }),
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: solidsOf('base'), excludeFaces: 'qNothing()', draftOnSelf: 'false', pullDirection: edgeOf('pull'), flipPullDirection: 'false',
    partingObject: `qOwnedByBody(qBodyType(qCreatedBy(id + "part", EntityType.BODY), BodyType.SHEET), EntityType.FACE)`, angle: deg(3), bothSides: 'true',
    matchFacesAtParting: 'false', cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false',
  }),
  returns({ faces: `faceProbe(context, ${solidsOf('base')})` }),
]);

// ----- a variable conic fillet -----

const base = solidsOf('base');
batch.add('fillet', 'a variable fillet, 2 mm to 5 mm along a block\'s top edge, conic (rho 0.7)', [
  box('base', [0, 0, 0], [40, 30, 12]),
  call('fillet', 'fillet1', {
    entities: edgeAt(base, [20, 0, 12]), radius: mm(3), nonCircularRadius: mm(3), isVariable: 'true', crossSection: 'FilletCrossSection.CONIC', rho: '0.7',
    vertexSettings: `[{ "vertex" : ${vertexAt(base, [0, 0, 12])}, "vertexRadius" : ${mm(2)}, "variableRho" : 0.7 }, { "vertex" : ${vertexAt(base, [40, 0, 12])}, "vertexRadius" : ${mm(5)}, "variableRho" : 0.7 }]`,
    smoothTransition: 'true',
  }),
  returns({ faces: `faceProbe(context, ${base})` }),
]);

batch.write('tools/fixtures/gen-batch-072.mjs');

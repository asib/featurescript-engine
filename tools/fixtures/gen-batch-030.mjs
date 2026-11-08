// Generates fixture batch 030: more options of covered features. Frames trimmed to a part, with their profile
// locked to a face, and aligned at a corner of the profile; a cut list with a column overridden; a fit spline
// matching curvature at both ends; and the isoclines of a cylinder.
//
// Usage: node tools/fixtures/gen-batch-030.mjs
//
// Writes fixtures/batch-030/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, cylinder, deg, edgeOf, faceAt, line, mm, planeAt, sketch, solidsOf, solve, vertexAt } from './lib.mjs';

const helpers = `
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

const batch = new Batch({ number: 30, title: 'more feature options: frames trimmed to parts, locked and aligned; cut list overrides; curvature at both ends; cylinder isoclines', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);

// ----- frames -----

batch.add('frame', 'a square tube along a line, its end trimmed to a block it runs into', [
  box('base', [50, -20, -20], [70, 20, 20]),
  sketch('path', xy), line('path', 'l1', [0, 0], [60, 0]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), trim: 'true', trimPlanes: 'qNothing()', trimBodies: base }),
]);
batch.add('frame', 'a square tube L on a block\'s top, its profile locked to the top', [
  box('base', [-10, -10, -10], [110, 70, 0]),
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', {
    profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), lockProfile: 'true', profileLockFaces: faceAt(base, [50, 30, 0]),
  }),
]);
batch.add('frame', 'a square tube L aligned at a corner of its profile', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path'), index: '0' }),
]);

// ----- cut list -----

batch.add('cutlist', 'the two tubes of a mitred L, their description overridden', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path') }),
  call('cutlist', 'cutlist1', {
    selectAllFrames: 'false', frames: frames('frame1'),
    columnOverrides: '[{ "isFirst" : true, "overrideAllFrames" : true, "column" : "Description", "value" : "Rail" }]',
  }),
  returns({
    composites: `evaluateQueryCount(context, qBodyType(qCreatedBy(id + "cutlist1", EntityType.BODY), BodyType.COMPOSITE))`,
    table: 'caseTables(context, id, cutlistTable(context, {}))',
  }),
]);

// ----- curves -----

batch.add('fit-spline', 'through three vertices, along x at the start and y at the end, matching curvature at both', [
  box('base', [0, 0, 0], [30, 20, 10]), axisLine('dirX', [0, 0, 0], [1, 0, 0]), axisLine('dirY', [0, 0, 0], [0, 1, 0]),
  call('fitSpline', 'spline1', {
    vertices: `qUnion([${vertexAt(base, [0, 0, 0])}, ${vertexAt(base, [30, 0, 10])}, ${vertexAt(base, [30, 20, 0])}])`,
    hasStartDirection: 'true', startDirection: edgeOf('dirX'), startMagnitude: '1', matchStartCurvature: 'true',
    hasEndDirection: 'true', endDirection: edgeOf('dirY'), endMagnitude: '1', matchEndCurvature: 'true',
  }),
]);
batch.add('isocline', 'the 30 degree isoclines of a cylinder lying along y, as curves', [
  sketch('baseSketch', planeAt([0, 0, 0], [0, 1, 0], [1, 0, 0])), `skCircle(baseSketch, "circle1", { "center" : vector(0, 0) * millimeter, "radius" : ${mm(10)} });`,
  solve('baseSketch'),
  call('extrude', 'base', { entities: `qSketchRegion(id + "baseSketch")`, endBound: 'BoundingType.BLIND', depth: mm(30) }),
  axisLine('up', [0, 0, 0], [0, 0, 1]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(30), split: 'false' }),
]);

batch.write('tools/fixtures/gen-batch-030.mjs');

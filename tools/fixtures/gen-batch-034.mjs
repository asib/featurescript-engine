// Generates fixture batch 034: more options of covered features. A sweep with its profile locked to a planar face,
// a routing curve through vertices, a cut list column overridden for one frame, a frame on a tagged profile at its
// extra alignment point, the isoclines of a cone, and an ANSI thread on two shafts at once.
//
// Usage: node tools/fixtures/gen-batch-034.mjs
//
// Writes fixtures/batch-034/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, rect, sketch, solidsOf, solve, vec3, vertexAt } from './lib.mjs';

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

function routingPoint(index is number, position is Vector) returns map
{
    return { "index" : index, "referenceType" : ReferenceType.ORIGIN, "vertex" : qNothing(), "curve" : qNothing(),
             "curveParameter" : 0, "alignCSys" : false, "flipCSys" : false, "dx" : position[0], "dy" : position[1],
             "dz" : position[2], "rotationMatrix" : identityMatrix(3), "overrideBendRadius" : false,
             "pointBendRadius" : 0 * meter, "hasDerivative" : false, "derivativeAlignment" : DerivativeAlignment.X,
             "magnitude" : 1, "oppositeDirection" : false, "derivativeDirection" : qNothing(), "arrayAdded" : false };
}

function routingVertexPoint(index is number, vertex is Query) returns map
{
    var point = routingPoint(index, vector(0, 0, 0) * meter);
    point.referenceType = ReferenceType.VERTEX;
    point.vertex = vertex;
    return point;
}

// A square tube profile tagged as a frame profile, with an extra alignment point at its corner (5, 5).
const taggedTube = function(configuration is map) returns Context
{
    const context = newContextWithDefaults();
    const s = newSketchOnPlane(context, makeId("profile"), { "sketchPlane" : plane(vector(0, 0, 0) * meter, vector(0, 0, 1), vector(1, 0, 0)) });
    skRectangle(s, "outer", { "firstCorner" : vector(-5, -5) * millimeter, "secondCorner" : vector(5, 5) * millimeter });
    skRectangle(s, "inner", { "firstCorner" : vector(-4, -4) * millimeter, "secondCorner" : vector(4, 4) * millimeter });
    skPoint(s, "corner", { "position" : vector(5, 5) * millimeter });
    skSolve(s);
    tag(context, makeId("tag"), { "tagPurpose" : TagPurpose.FRAME, "sketch" : { (makeId("profile")) : function(id) {} } as FeatureList,
                                  "alignmentPoints" : sketchEntityQuery(makeId("profile"), EntityType.VERTEX, "corner"),
                                  "standard" : "Test", "description" : "Square tube 10", "additionalColumns" : [] });
    return context;
};

`;

const batch = new Batch({ number: 34, title: 'more feature options: locked sweeps, routing through vertices, frame overrides, tagged frame profiles, cone isoclines, ANSI threads', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const lPath = [sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path')];

batch.add('sweep', 'a square swept along an L on a block\'s top, locked to the top', [
  box('base', [-10, -10, -10], [110, 70, 0]), lPath,
  sketch('prof', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), rect('prof', 'r', [-3, 0], [3, 6]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: `qSketchRegion(id + "prof")`, path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: faceAt(base, [50, 30, 0]),
  }),
]);
batch.add('routing-curve', 'a polyline through three corners of a block, with 3 mm bends', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('routingCurve', 'route1', {
    preselectedEntities: 'qNothing()', curveType: 'RoutingCurveType.POLYLINE', step: 'CurveStep.POINTS', inputType: 'InputType.VERTEX',
    inputVertices: 'qNothing()', isPeriodic: 'false', curveBaseCSys: 'qNothing()', pointsEditType: 'PointsEditType.SINGLE', pointIndex: '0',
    addPointOnAxisDrag: 'false', showCSys: 'false', keepPoints: 'false', curveLength: mm(0), bendRadius: mm(3),
    points: `[${[[0, 0, 20], [40, 0, 20], [40, 30, 0]].map((p, i) => `routingVertexPoint(${i}, ${vertexAt(base, p)})`).join(', ')}]`,
  }),
]);
batch.add('cutlist', 'the tubes of a mitred L, the Description of the first overridden', [
  lPath, call('frame', 'frame1', { profileSketch: 'profileOf(squareTube)', selections: wireEdges('path') }),
  call('cutlist', 'cutlist1', {
    selectAllFrames: 'false', frames: frames('frame1'),
    columnOverrides: `[{ "isFirst" : true, "overrideAllFrames" : false, "overrideFrames" : qClosestTo(${frames('frame1')}, ${vec3(50, 0, 0)}), "column" : "Description", "value" : "Long rail" }]`,
  }),
  returns({ table: 'caseTables(context, id, cutlistTable(context, {}))' }),
]);
batch.add('frame', 'a tagged square tube profile along an L, at its extra alignment point', [
  lPath, call('frame', 'frame1', { profileSketch: 'profileOf(taggedTube)', selections: wireEdges('path'), index: '9' }),
]);
batch.add('isocline', 'the 20 degree isoclines of a cone lying along y, as curves', [
  call('fCone', 'base', { bottomCenter: vec3(0, 0, 0), topCenter: vec3(0, 30, 0), bottomRadius: mm(12), topRadius: mm(6) }),
  axisLine('up', [0, 0, 0], [0, 0, 1]),
  call('isocline', 'iso1', { faces: `qOwnedByBody(${base}, EntityType.FACE)`, direction: edgeOf('up'), oppositeDirection: 'false', angle: deg(20), split: 'false' }),
]);
batch.add('external-thread', 'a 3/8-16 thread 12 mm down two shafts at once', [
  cylinder('base', [0, 0], 4.7625, 0, 30), cylinder('pin', [20, 0], 4.7625, 0, 30),
  call('externalThread', 'thread1', {
    entities: `qUnion([${edgeAt(base, [4.7625, 0, 30])}, ${edgeAt(solidsOf('pin'), [24.7625, 0, 30])}])`,
    branchOfStandard: '{ "standard" : "ANSI", "size" : "3/8", "pitch" : "16 tpi" } as LookupTablePath', depthType: 'DepthType.Blind',
    threadDepth: mm(12), addChamfer: 'false', addUndercut: 'false', splitFace: 'true',
  }),
]);
// c001's sharp cornered path again, its profile left free: whether a sweep's path may turn a corner at all.
batch.add('sweep', 'a square swept along a sharp L', [
  lPath,
  sketch('prof', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), rect('prof', 'r', [-3, 0], [3, 6]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: `qSketchRegion(id + "prof")`, path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
  }),
]);
// The L's corner rounded 10 mm, so that the path is tangent throughout.
const roundPath = [
  sketch('path', xy), line('path', 'l1', [0, 0], [90, 0]), arc('path', 'a1', [90, 0], [90 + 10 * Math.SQRT1_2, 10 - 10 * Math.SQRT1_2], [100, 10]),
  line('path', 'l2', [100, 10], [100, 60]), solve('path'),
];
batch.add('sweep', 'a square swept along an L rounded 10 mm on a block\'s top, locked to the top', [
  box('base', [-10, -10, -10], [110, 70, 0]), roundPath,
  sketch('prof', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), rect('prof', 'r', [-3, 0], [3, 6]), solve('prof'),
  call('sweep', 'sweep1', {
    profiles: `qSketchRegion(id + "prof")`, path: wireEdges('path'), bodyType: 'ExtendedToolBodyType.SOLID', operationType: 'NewBodyOperationType.NEW',
    profileControl: 'ProfileControlMode.LOCK_FACES', lockFaces: faceAt(base, [50, 30, 0]),
  }),
]);

batch.write('tools/fixtures/gen-batch-034.mjs');

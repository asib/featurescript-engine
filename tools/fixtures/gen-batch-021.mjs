// Generates fixture batch 021: more options of covered features. Booleans with an offset, a hole from a plane,
// moving a face up to a vertex, extending a sheet up to a vertex, trimming a curve by another, sweeps keeping their
// orientation and locked to a direction, crossing ribs merged, a gusset off centre, a variable from a table, and
// sheet metal: a countersunk hole and hems meeting at a closed corner.
//
// Usage: node tools/fixtures/gen-batch-021.mjs
//
// Writes fixtures/batch-021/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm, planeAt, planeBody,
  rect, sketch, solidsOf, solve, vec2, vec3, vertexAt,
} from './lib.mjs';

const helpers = `
function pointAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Vector
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    return vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                  b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                  b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
}

function atFraction(context is Context, part is Query, entityType is EntityType, fx is number, fy is number, fz is number) returns Query
{
    return qContainsPoint(qOwnedByBody(part, entityType), pointAtFraction(context, part, fx, fy, fz));
}

function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "active" : evaluateQueryCount(context, qActiveSheetMetalFilter(all, ActiveSheetMetal.YES)) };
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

const batch = new Batch({ number: 21, title: 'more feature options: offset booleans, holes from planes, up-to entities, curve trims, sweep profile control, ribs, gussets, tables, sheet metal holes and hems', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const regions = (s) => `qSketchRegion(id + "${s}")`;
const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);

// ----- booleans, holes, up-to entities -----

batch.add('boolean', 'a cylinder taken out of a block, its faces offset 1 mm', [
  block, cylinder('tool', [15, 10], 4, -5, 15),
  call('booleanBodies', 'boolean1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: base, tools: solidsOf('tool'), offset: 'true', offsetAll: 'true',
    offsetDistance: mm(1), oppositeDirection: 'false', reFillet: 'false',
  }),
]);
const topPoints = (s, z, points) => [
  sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
  points.map(([x, y], i) => `skPoint(${s}, "p${i + 1}", { "position" : ${vec2(x, y)} });`),
  solve(s),
];
const pointsOf = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`;
batch.add('hole', 'a 6 mm deep hole started from a plane 3 mm below the top', [
  block, topPoints('pts1', 10, [[15, 10]]), planeBody('start', [0, 0, 7], [0, 0, 1], [1, 0, 0]),
  call('hole', 'hole1', {
    locations: pointsOf('pts1'), scope: base, style: 'HoleStyle.SIMPLE', holeDiameter: mm(5), holeDepth: mm(6), endStyle: 'HoleEndStyle.BLIND',
    startStyle: 'HoleStartStyle.PLANE', startBoundEntity: createdBy('start', 'FACE'),
  }),
]);
batch.add('move-face', 'a block\'s top translated up to a vertex of another block', [
  block, box('other', [40, 0, 0], [50, 10, 16]), axisLine('dirZ', [0, 0, 0], [0, 0, 1]),
  call('moveFace', 'move1', {
    moveFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.TRANSLATE', direction: edgeOf('dirZ'),
    limitType: 'MoveFaceBoundingType.UP_TO_ENTITY', limitEntity: vertexAt(solidsOf('other'), [50, 10, 16]), hasOffset: 'false',
  }),
]);
const sheetFrom = (name, curve) => [
  sketch(`${name}Sketch`, PLANES.xy), curve(`${name}Sketch`), solve(`${name}Sketch`),
  call('extrude', name, { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges(`${name}Sketch`), endBound: 'BoundingType.BLIND', depth: mm(20) }),
];
batch.add('extend-surface', 'a sheet extended up to a vertex', [
  sheetFrom('base', (s) => line(s, 'line1', [0, 0], [20, 0])), `opPoint(context, id + "p1", { "point" : ${vec3(26, 5, 3)} });`,
  call('extendSurface', 'extend1', {
    entities: edgeAt(sheetOf('base'), [20, 0, 10]), endCondition: 'ExtendBoundingType.UP_TO_VERTEX', targetVertex: createdBy('p1', 'VERTEX'),
    hasOffset: 'false',
  }),
]);
batch.add('trim-curve', 'a curve trimmed by another curve crossing it', [
  sketch('s1', PLANES.xy), line('s1', 'line1', [0, 0], [20, 10]), solve('s1'), call('compositeCurve', 'curve1', { edges: wireEdges('s1') }),
  sketch('s2', PLANES.xy), line('s2', 'line1', [10, -5], [10, 15]), solve('s2'), call('compositeCurve', 'curve2', { edges: wireEdges('s2') }),
  call('trimCurve', 'trim1', {
    moveBoundaryType: 'MoveCurveBoundaryType.TRIM', wires: createdBy('curve1'), trimTo: createdBy('curve2'),
    helpPoint: vertexAt(createdBy('curve1'), [20, 10, 0]), flipHeuristics: 'false',
  }),
]);

// ----- sweeps, ribs, gussets -----

const arcPath = [sketch('path', PLANES.xy), arc('path', 'arc1', [0, 0], [20, 8.28427], [28.28427, 28.28427]), solve('path')];
const rectProfile = [sketch('profile', PLANES.yz), rect('profile', 'rect1', [-3, -2], [3, 2]), solve('profile')];
batch.add('sweep', 'a rectangle along an arc, keeping its orientation', [
  rectProfile, arcPath,
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), profileControl: 'ProfileControlMode.KEEP_ORIENTATION' }),
]);
batch.add('sweep', 'a rectangle along an arc, locked square to z', [
  rectProfile, arcPath, axisLine('dirZ', [0, 0, 0], [0, 0, 1]),
  call('sweep', 'sweep1', {
    profiles: regions('profile'), path: wireEdges('path'), profileControl: 'ProfileControlMode.LOCK_DIRECTION', lockDirectionQuery: edgeOf('dirZ'),
  }),
]);
batch.add('rib', 'two crossing ribs on a plate, merged', [
  box('base', [0, 0, 0], [40, 30, 4]),
  sketch('profile', planeAt([0, 0, 12], [0, 0, 1], [1, 0, 0])), line('profile', 'l1', [5, 5], [35, 25]), line('profile', 'l2', [5, 25], [35, 5]),
  solve('profile'),
  call('rib', 'rib1', {
    profiles: wireEdges('profile'), parts: base, thickness: mm(2), ribExtrusionDirection: 'RibExtrusionDirection.NORMAL_TO_SKETCH_PLANE',
    oppositeDirection: 'true', hasDraft: 'false', extendProfilesUpToPart: 'false', mergeRibs: 'true',
  }),
]);
const frames = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
batch.add('gusset', 'a triangle in a mitred corner, 2 mm off centre', [
  sketch('path', xy), line('path', 'l1', [0, 0], [100, 0]), line('path', 'l2', [100, 0], [100, 60]), solve('path'),
  call('frame', 'frame1', { selections: wireEdges('path'), profileSketch: 'profileOf(squareTube)' }),
  call('gusset', 'gusset1', {
    edges: edgeAt(frames('frame1'), [95, 5, 0]), gussetType: 'GussetStyleType.TRIANGLE', length: mm(15), shouldFlipBaseSides: 'false',
    thickness: mm(2), gussetPosition: 'GussetPosition.CENTERED', offset: mm(2), shouldFlipOffset: 'false', baseSweptFaces: 'qNothing()',
  }),
]);
batch.add('variable', 'a value read from a table by row label and column index', [
  call('assignVariable', 'variable1', {
    name: '"w"', mode: 'VariableMode.TABLE', csv: '{ "csvData" : [["part", "width", "height"], ["a", "10", "4"], ["b", "25", "7"]] } as TableData',
    rowAccessType: 'AccessType.LABEL', rowValue: '"b"', rowRegexp: 'false', rowLabelIndex: '0', rowResult: 'ResultType.SINGLE',
    columnAccessType: 'AccessType.INDEX', columnIndex: '1', columnResult: 'ResultType.SINGLE', useConversionFactor: 'false',
  }),
  'return { "w" : getVariable(context, "w") };',
]);

// ----- sheet metal -----

const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const at = (type, fx, fy, fz) => `atFraction(context, ${part}, EntityType.${type}, ${fx}, ${fy}, ${fz})`;
const counts = returns({ counts: 'bodyCounts(context)' });
const plate = [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
    regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()',
  }),
];
batch.add('sheet-metal-hole', 'a countersunk hole through a plate', [
  plate, topPoints('pts1', 2, [[20, 15]]),
  call('hole', 'hole1', {
    locations: pointsOf('pts1'), scope: part, style: 'HoleStyle.C_SINK', holeDiameter: mm(4), cSinkDiameter: mm(7), cSinkAngle: deg(90),
    endStyle: 'HoleEndStyle.THROUGH',
  }),
  counts,
]);
batch.add('sheet-metal-hem', 'straight hems on two edges meeting at a closed corner', [
  plate,
  call('sheetMetalHem', 'hem1', {
    edges: `qUnion([${at('EDGE', 1, 0.5, 1)}, ${at('EDGE', 0.5, 1, 1)}])`, oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE',
    hemCornerType: 'SMHemCornerType.CLOSED', hemType: 'SMHemType.STRAIGHT', useMinimalGapForStraight: 'false', innerRadius: mm(1), length: mm(10),
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-021.mjs');

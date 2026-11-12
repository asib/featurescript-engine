// Generates fixture batch 141: everyday parts, a fourth round: a plate with a 3 × 2 grid of holes (a pattern in two
// directions), an L bracket with a gusset mirrored onto its other end, a square-to-round transition lofted and
// shelled, a sheet metal U channel with holes patterned along it and a hem, a block split by a plane and one piece
// filleted, two crossing extrusions intersected, a lofted sheet thickened, countersunk holes.
//
// Usage: node tools/fixtures/gen-batch-141.mjs
//
// Writes fixtures/batch-141/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, box, call, circle, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)) };
}
`;

const batch = new Batch({ number: 141, title: 'everyday parts: a hole grid, a mirrored gusset, a lofted transition, a sheet metal channel, a split block, an intersection, a thickened loft, countersinks', helpers });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const holeFn = (sketchName, extra = '') =>
  `extrude(context, instanceId, { "entities" : qSketchRegion(id + "${sketchName}"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base}${extra} });`;

batch.add('linear-pattern', 'a 60 × 40 × 5 plate with a hole (radius 3) at (10, 10) patterned 3 along x at 20 and 2 along y at 20', [
  box('base', [0, 0, 0], [60, 40, 5]),
  sketch('holeSketch', top(5)), circle('holeSketch', 'c1', [10, 10], 3), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  sketch('dirX', top(0)), line('dirX', 'line1', [0, 0], [10, 0]), solve('dirX'),
  sketch('dirY', top(0)), line('dirY', 'line1', [0, 0], [0, 10]), solve('dirY'),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { ${holeFn('holeSketch')} } } as FeatureList, "directionOne" : qCreatedBy(id + "dirX", EntityType.EDGE), "distance" : 20 * millimeter, "instanceCount" : 3, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : true, "directionTwo" : qCreatedBy(id + "dirY", EntityType.EDGE), "distanceTwo" : 20 * millimeter, "instanceCountTwo" : 2, "oppositeDirectionTwo" : false, "isCenteredTwo" : false, "fullFeaturePattern" : false });`,
]);
batch.add('mirror', 'an L bracket (a 40 × 30 × 4 plate, a wall 4 thick and 25 tall at x = 0) with a triangular gusset (4 thick) at y = 2, mirrored across y = 15', [
  box('base', [0, 0, 0], [40, 30, 4]), box('wall', [0, 0, 4], [4, 30, 25]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: `qUnion([${base}, ${solidsOf('wall')}])` }),
  sketch('gussetSketch', planeAt([0, 2, 0], [0, 1, 0], [1, 0, 0])), line('gussetSketch', 'l1', [4, 4], [20, 4]), line('gussetSketch', 'l2', [20, 4], [4, 20]), line('gussetSketch', 'l3', [4, 20], [4, 4]), solve('gussetSketch'),
  call('extrude', 'gusset', { entities: regions('gussetSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  call('opPlane', 'mid', { plane: `plane(${vec3(0, 15, 0)}, vector(0, 1, 0))`, width: mm(100), height: mm(100) }),
  `mirror(context, id + "mirror1", { "patternType" : MirrorType.FEATURE, "instanceFunction" : { (id + "gusset") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "gussetSketch"), "endBound" : BoundingType.BLIND, "depth" : 4 * millimeter, "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "mirrorPlane" : qCreatedBy(id + "mid", EntityType.FACE), "fullFeaturePattern" : false });`,
]);
batch.add('loft', 'a transition: a 30 × 30 square at z = 0 lofted to a circle of radius 10 at z = 25, shelled 1.5 open at both ends', [
  sketch('s1', top(0)), rect('s1', 'r1', [-15, -15], [15, 15]), solve('s1'),
  sketch('s2', top(25)), circle('s2', 'c1', [0, 0], 10), solve('s2'),
  call('loft', 'base', { sheetProfilesArray: `[{ "sheetProfileEntities" : qSketchRegion(id + "s1") }, { "sheetProfileEntities" : qSketchRegion(id + "s2") }]` }),
  call('shell', 'shell1', { entities: `qUnion([${faceAt(base, [0, 0, 0])}, ${faceAt(base, [0, 0, 25])}])`, thickness: mm(1.5) }),
]);
batch.add('sheet-metal-hem', 'a sheet metal U channel (a 20 base, 15 legs, 50 deep, 1.5 thick) with a hole (radius 2.5) in its base patterned 3 along it at 15, one leg\'s top edge hemmed 6', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 15], [0, 0]), line('s1', 'l2', [0, 0], [20, 0]), line('s1', 'l3', [20, 0], [20, 15]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()',
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'true', depth: mm(50), symmetric: 'false', hasSecondDirection: 'false',
  }),
  sketch('holeSketch', planeAt([0, 0, 5], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'c1', [10, 10], 2.5), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)` }),
  sketch('dir', top(0)), line('dir', 'line1', [0, 0], [0, 10]), solve('dir'),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "holeSketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID) }); } } as FeatureList, "directionOne" : qCreatedBy(id + "dir", EntityType.EDGE), "distance" : 15 * millimeter, "instanceCount" : 3, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : false, "fullFeaturePattern" : false });`,
  call('sheetMetalHem', 'hem1', {
    edges: `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID), EntityType.EDGE), ${vec3(21.5, 25, 15)})`,
    oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE', hemCornerType: 'SMHemCornerType.SIMPLE', hemType: 'SMHemType.STRAIGHT', useMinimalGapForStraight: 'true', innerRadius: mm(1), length: mm(6),
  }),
  `return { "counts" : bodyCounts(context) };`,
]);
batch.add('split-part', 'a 40 × 30 × 20 block split by a plane through it at 30° and the upper piece\'s top edges filleted 2', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('opPlane', 'cutter', { plane: `plane(${vec3(20, 15, 10)}, vector(${-Math.sin(Math.PI / 6)}, 0, ${Math.cos(Math.PI / 6)}))`, width: mm(100), height: mm(100) }),
  call('splitPart', 'split1', { targets: base, tool: `qCreatedBy(id + "cutter", EntityType.FACE)` }),
  call('fillet', 'fillet1', { entities: `qUnion([${edgeAt(base, [20, 0, 20])}, ${edgeAt(base, [20, 30, 20])}])`, radius: mm(2) }),
  `return { "counts" : bodyCounts(context) };`,
]);
batch.add('boolean', 'an L profile (30 × 20, 5 thick) extruded 30 along y intersected with a circle (radius 14 about (15, 15)) extruded 30 along z', [
  sketch('aSketch', PLANES.xz), line('aSketch', 'l1', [0, 0], [30, 0]), line('aSketch', 'l2', [30, 0], [30, 5]), line('aSketch', 'l3', [30, 5], [5, 5]),
  line('aSketch', 'l4', [5, 5], [5, 20]), line('aSketch', 'l5', [5, 20], [0, 20]), line('aSketch', 'l6', [0, 20], [0, 0]), solve('aSketch'),
  call('extrude', 'a', { entities: regions('aSketch'), endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'true' }),
  sketch('bSketch', top(0)), circle('bSketch', 'c1', [15, 15], 14), solve('bSketch'),
  call('extrude', 'b', { entities: regions('bSketch'), endBound: 'BoundingType.BLIND', depth: mm(30) }),
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.INTERSECTION', tools: `qUnion([${solidsOf('a')}, ${solidsOf('b')}])` }),
]);
batch.add('thicken', 'a sheet lofted between a line at z = 0 and an arc at z = 20 (rising 5 in its middle), thickened 2', [
  sketch('s1', top(0)), line('s1', 'l1', [0, 0], [30, 0]), solve('s1'),
  sketch('s2', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), `skArc(s2, "a1", { "start" : vector(0, 5) * millimeter, "mid" : vector(15, 10) * millimeter, "end" : vector(30, 5) * millimeter });`, solve('s2'),
  call('loft', 'sheet', { bodyType: 'ToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW', wireProfilesArray: `[{ "wireProfileEntities" : ${wireEdges('s1')} }, { "wireProfileEntities" : ${wireEdges('s2')} }]` }),
  call('thicken', 'base', { entities: `qCreatedBy(id + "sheet", EntityType.FACE)`, thickness1: mm(2), thickness2: mm(0) }),
]);
batch.add('hole', 'a 60 × 20 × 10 plate with countersunk holes (6, countersink 12 at 90°, 8 deep) at (15, 10) and (45, 10)', [
  box('base', [0, 0, 0], [60, 20, 10]),
  sketch('pts', top(10)), `skPoint(pts, "p1", { "position" : vector(15, 10) * millimeter });`, `skPoint(pts, "p2", { "position" : vector(45, 10) * millimeter });`, solve('pts'),
  call('hole', 'hole1', { locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`, scope: base, style: 'HoleStyle.C_SINK', holeDiameter: mm(6), cSinkDiameter: mm(12), cSinkAngle: deg(90), holeDepth: mm(8), endStyle: 'HoleEndStyle.BLIND' }),
]);

batch.write('tools/fixtures/gen-batch-141.mjs');

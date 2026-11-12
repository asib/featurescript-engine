// Generates fixture batch 140: everyday parts, a third round (after batches 124, 134 and 139): a hinge leaf, a gear
// blank with patterned teeth, a sheet metal box, a swept pipe elbow with flanges, a bottle, a swept handle,
// counterbored holes patterned along a plate, a drafted block with a rib. Each is a few features of different kinds,
// as parts are made.
//
// Usage: node tools/fixtures/gen-batch-140.mjs
//
// Writes fixtures/batch-140/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 140, title: 'everyday parts: a hinge, a gear blank, a sheet metal box, a pipe elbow, a bottle, a handle, counterbored holes, a drafted block', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const through = (name) => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped });

batch.add('extrude', 'a hinge leaf: a 40 × 20 × 2 plate with a knuckle (a tube of radius 3 round a pin hole of radius 1.5) along its far edge, two screw holes (radius 1.5), its free corners rounded 3', [
  box('base', [0, 0, 0], [40, 20, 2]),
  sketch('knuckleSketch', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), circle('knuckleSketch', 'c1', [20, 3], 3), solve('knuckleSketch'),
  call('extrude', 'knuckle', { entities: regions('knuckleSketch'), endBound: 'BoundingType.BLIND', depth: mm(40), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('pinSketch', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), circle('pinSketch', 'c1', [20, 3], 1.5), solve('pinSketch'),
  call('extrude', 'pin', { entities: regions('pinSketch'), endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  sketch('holesSketch', top(2)), circle('holesSketch', 'c1', [10, 8], 1.5), circle('holesSketch', 'c2', [30, 8], 1.5), solve('holesSketch'), through('holes'),
  call('fillet', 'corners', { entities: `qUnion([${edgeAt(base, [0, 0, 1])}, ${edgeAt(base, [40, 0, 1])}])`, radius: mm(3) }),
]);
batch.add('circular-pattern', 'a gear blank: a disc (radius 20, 6 thick) with a tooth (a 4 × 3 block) on its rim patterned 12 round it, a bore (radius 5) through, its faces\' bore edges chamfered 0.5', [
  cylinder('base', [0, 0], 20, 0, 6),
  sketch('toothSketch', top(0)), rect('toothSketch', 'r1', [19, -2], [23, 2]), solve('toothSketch'),
  call('extrude', 'tooth', { entities: regions('toothSketch'), endBound: 'BoundingType.BLIND', depth: mm(6), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "tooth") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "toothSketch"), "endBound" : BoundingType.BLIND, "depth" : 6 * millimeter, "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : 12, "equalSpace" : true, "fullFeaturePattern" : false });`,
  sketch('boreSketch', top(6)), circle('boreSketch', 'c1', [0, 0], 5), solve('boreSketch'), through('bore'),
  call('chamfer', 'chamfer1', { entities: `qUnion([${edgeAt(base, [5, 0, 6])}, ${edgeAt(base, [5, 0, 0])}])`, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(0.5) }),
]);
batch.add('sheet-metal-flange', 'a sheet metal box: a 40 × 30 plate (1.5 thick, bends of radius 1) with flanges 15 up on all four edges, its corners closed by default, a hole (radius 4) in its floor', [
  sketch('s1', top(0)), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', { thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qUnion([${[[20, 0], [40, 15], [20, 30], [0, 15]].map(([x, y]) => `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID), EntityType.EDGE), ${vec3(x, y, 1.5)})`).join(', ')}])`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE',
    oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  sketch('holeSketch', top(1.5)), circle('holeSketch', 'c1', [20, 15], 4), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)` }),
]);
batch.add('sweep', 'a pipe elbow: a circle (radius 5) swept round a quarter arc of radius 20, shelled 1 mm open at both ends, a flange (radius 9, 2 thick) on each end', [
  sketch('path', PLANES.xz), arc('path', 'a1', [0, 0], [20 - 20 * Math.SQRT1_2, 20 * Math.SQRT1_2], [20, 20]), solve('path'),
  sketch('profile', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('profile', 'c1', [0, 0], 5), solve('profile'),
  call('sweep', 'base', { profiles: regions('profile'), path: wireEdges('path') }),
  call('shell', 'shell1', { entities: `qUnion([${faceAt(base, [0, 0, 0])}, ${faceAt(base, [20, 0, 20])}])`, thickness: mm(1) }),
  sketch('f1Sketch', top(0)), circle('f1Sketch', 'outer', [0, 0], 9), circle('f1Sketch', 'inner', [0, 0], 4), solve('f1Sketch'),
  call('extrude', 'f1', { entities: `qContainsPoint(${regions('f1Sketch')}, ${vec3(6.5, 0, 0)})`, endBound: 'BoundingType.BLIND', depth: mm(2), oppositeDirection: 'true', operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('f2Sketch', planeAt([20, 0, 20], [1, 0, 0], [0, 1, 0])), circle('f2Sketch', 'outer', [0, 0], 9), circle('f2Sketch', 'inner', [0, 0], 4), solve('f2Sketch'),
  call('extrude', 'f2', { entities: `qContainsPoint(${regions('f2Sketch')}, ${vec3(20, 6.5, 20)})`, endBound: 'BoundingType.BLIND', depth: mm(2), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('revolve', 'a bottle: a profile (body radius 15, 40 tall, a shoulder up to a neck of radius 6 that is 10 tall) revolved, its foot filleted 3, shelled 1 open at its neck', [
  sketch('profile', PLANES.xz), line('profile', 'l1', [0, 0], [15, 0]), line('profile', 'l2', [15, 0], [15, 40]), line('profile', 'l3', [15, 40], [6, 50]),
  line('profile', 'l4', [6, 50], [6, 60]), line('profile', 'l5', [6, 60], [0, 60]), line('profile', 'l6', [0, 60], [0, 0]), solve('profile'),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [-100, 0], [100, 0]), solve('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('fillet', 'foot', { entities: edgeAt(base, [15, 0, 0]), radius: mm(3) }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 60]), thickness: mm(1) }),
]);
batch.add('sweep', 'a handle: a circle (radius 2.5) swept along a three-point arc from (0, 0, 0) through (15, 0, 12) to (30, 0, 0), standing on a 40 × 10 × 3 plate, united', [
  box('base', [-5, -5, -3], [35, 5, 0]),
  sketch('path', PLANES.xz), arc('path', 'a1', [0, 0], [15, 12], [30, 0]), solve('path'),
  `const pathStart = evEdgeTangentLine(context, { "edge" : qOwnedByBody(qBodyType(qCreatedBy(id + "path", EntityType.BODY), BodyType.WIRE), EntityType.EDGE), "parameter" : 0 });`,
  `const profile = newSketchOnPlane(context, id + "profile", { "sketchPlane" : plane(pathStart.origin, pathStart.direction, perpendicularVector(pathStart.direction)) });`,
  `skCircle(profile, "c1", { "center" : vector(0, 0) * millimeter, "radius" : 2.5 * millimeter });`, solve('profile'),
  call('sweep', 'handle', { profiles: regions('profile'), path: wireEdges('path'), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('hole', 'a 80 × 20 × 8 plate with a counterbored hole (6 through, counterbore 10 × 4) at (10, 10), patterned 4 along x at 20 mm', [
  box('base', [0, 0, 0], [80, 20, 8]),
  sketch('pts', top(8)), `skPoint(pts, "p1", { "position" : vector(10, 10) * millimeter });`, solve('pts'),
  call('hole', 'hole1', { locations: `qOwnedByBody(qBodyType(qCreatedBy(id + "pts", EntityType.BODY), BodyType.POINT), EntityType.VERTEX)`, scope: base, style: 'HoleStyle.C_BORE', holeDiameter: mm(6), cBoreDiameter: mm(10), cBoreDepth: mm(4), endStyle: 'HoleEndStyle.THROUGH' }),
  sketch('dir', top(0)), line('dir', 'line1', [0, 0], [10, 0]), solve('dir'),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole1") : function(instanceId) { hole(context, instanceId, { "locations" : qOwnedByBody(qBodyType(qCreatedBy(id + "pts", EntityType.BODY), BodyType.POINT), EntityType.VERTEX), "scope" : ${base}, "style" : HoleStyle.C_BORE, "holeDiameter" : 6 * millimeter, "cBoreDiameter" : 10 * millimeter, "cBoreDepth" : 4 * millimeter, "endStyle" : HoleEndStyle.THROUGH }); } } as FeatureList, "directionOne" : qCreatedBy(id + "dir", EntityType.EDGE), "distance" : 20 * millimeter, "instanceCount" : 4, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : false, "fullFeaturePattern" : false });`,
]);
batch.add('draft', 'a 40 × 30 × 20 block, its four sides drafted 3° from its bottom, a boss (radius 5, 8 tall) on its top, filleted 1 at its foot', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('draft', 'draft1', {
    neutralPlane: faceAt(base, [20, 15, 0]),
    draftFaces: `qUnion([${[[20, 0, 10], [40, 15, 10], [20, 30, 10], [0, 15, 10]].map((p) => faceAt(base, p)).join(', ')}])`,
    angle: deg(3),
  }),
  sketch('bossSketch', top(20)), circle('bossSketch', 'c1', [20, 15], 5), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(8), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [25, 15, 20]), radius: mm(1) }),
]);

batch.write('tools/fixtures/gen-batch-140.mjs');

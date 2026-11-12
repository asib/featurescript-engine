// Generates fixture batch 142: everyday parts, a fifth round: a pulley (a revolved profile with a V groove, a hub and
// a keyway), a shaft with a groove turned into it, a cross hole and chamfered ends, a sheet metal bracket with holes
// patterned round its flange, a plate with slots patterned round its middle, a part mirrored and united with itself,
// a box with a snap tab, a pocket whose floor and walls are filleted along tangent chains, a loft through three
// sections.
//
// Usage: node tools/fixtures/gen-batch-142.mjs
//
// Writes fixtures/batch-142/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 142, title: 'everyday parts: a pulley, a grooved shaft, a sheet metal bracket, a slotted plate, a mirrored part, a snap box, a filleted pocket, a three-section loft', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = [sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [-100, 0], [100, 0]), solve('axis')];
const xAxis = [sketch('xaxis', top(0)), line('xaxis', 'line1', [-100, 0], [100, 0]), solve('xaxis')];

batch.add('revolve', 'a pulley: a profile (radius 25, 12 wide, a V groove 6 deep and 8 across its rim, a hub of radius 8 standing 5 out each side) revolved, a bore (radius 4) with a 2 × 1.5 keyway', [
  sketch('profile', PLANES.xz),
  line('profile', 'l1', [4, -11], [8, -11]), line('profile', 'l2', [8, -11], [8, -6]), line('profile', 'l3', [8, -6], [25, -6]),
  line('profile', 'l4', [25, -6], [25, -4]), line('profile', 'l5', [25, -4], [19, 0]), line('profile', 'l6', [19, 0], [25, 4]),
  line('profile', 'l7', [25, 4], [25, 6]), line('profile', 'l8', [25, 6], [8, 6]), line('profile', 'l9', [8, 6], [8, 11]),
  line('profile', 'l10', [8, 11], [4, 11]), line('profile', 'l11', [4, 11], [4, -11]), solve('profile'),
  zAxis,
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  sketch('keySketch', top(11)), rect('keySketch', 'r1', [3.5, -1], [5.5, 1]), solve('keySketch'),
  call('extrude', 'key', { entities: regions('keySketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
]);
batch.add('revolve', 'a shaft (radius 8, 60 long along x) with a groove (2 wide, 1 deep) turned at x = 20, a cross hole (radius 2) through at x = 40, its ends chamfered 0.8', [
  sketch('baseSketch', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), circle('baseSketch', 'c1', [0, 0], 8), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(60) }),
  sketch('grooveSketch', PLANES.xz), rect('grooveSketch', 'r1', [19, 7], [21, 9]), solve('grooveSketch'),
  xAxis,
  call('revolve', 'groove', { entities: regions('grooveSketch'), axis: `qCreatedBy(id + "xaxis", EntityType.EDGE)`, fullRevolve: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  sketch('holeSketch', top(10)), circle('holeSketch', 'c1', [40, 0], 2), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('chamfer', 'chamfer1', { entities: `qUnion([${edgeAt(base, [0, 8, 0])}, ${edgeAt(base, [60, 8, 0])}])`, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(0.8) }),
]);
batch.add('sheet-metal-flange', 'a sheet metal bracket: a 40 × 30 plate (2 thick) with a flange 25 up on its far edge, a hole (radius 2.5) in the flange patterned 3 along it at 10', [
  sketch('s1', top(0)), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', { thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
  call('sheetMetalFlange', 'flange1', {
    edges: `qContainsPoint(qOwnedByBody(qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID), EntityType.EDGE), ${vec3(20, 30, 2)})`,
    flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(25), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE',
    oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
  }),
  sketch('holeSketch', planeAt([0, 35, 0], [0, -1, 0], [1, 0, 0])), circle('holeSketch', 'c1', [10, 15], 2.5), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'false', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)` }),
  sketch('dir', top(0)), line('dir', 'line1', [0, 0], [10, 0]), solve('dir'),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "holeSketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : false, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID) }); } } as FeatureList, "directionOne" : qCreatedBy(id + "dir", EntityType.EDGE), "distance" : 10 * millimeter, "instanceCount" : 3, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : false, "fullFeaturePattern" : false });`,
]);
batch.add('circular-pattern', 'a disc (radius 30, 4 thick) with an obround slot (12 long, 4 wide) at 18 from its centre patterned 6 round it', [
  cylinder('base', [0, 0], 30, 0, 4),
  sketch('slotSketch', top(4)), line('slotSketch', 'l1', [12, -2], [24, -2]), arc('slotSketch', 'a1', [24, -2], [26, 0], [24, 2]),
  line('slotSketch', 'l2', [24, 2], [12, 2]), arc('slotSketch', 'a2', [12, 2], [10, 0], [12, -2]), solve('slotSketch'),
  call('extrude', 'slot', { entities: regions('slotSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  zAxis,
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "slot") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "slotSketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : 6, "equalSpace" : true, "fullFeaturePattern" : false });`,
]);
batch.add('mirror', 'a 30 × 20 × 10 block with a 10 × 10 × 8 lug on its end at x = 30, mirrored across x = 0 and united', [
  box('base', [0, 0, 0], [30, 20, 10]), box('lug', [30, 5, 0], [40, 15, 8]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: `qUnion([${base}, ${solidsOf('lug')}])` }),
  call('mirror', 'mirror1', { entities: base, mirrorPlane: `qCreatedBy(makeId("Right"), EntityType.FACE)`, operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
]);
batch.add('shell', 'a snap box: a 40 × 30 × 15 box shelled 1.5 open at its top, a tab (6 wide, 1.5 thick, 5 tall) standing up from its rim at y = 0, a hole (radius 1) through the tab', [
  box('base', [0, 0, 0], [40, 30, 15]),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 15]), thickness: mm(1.5) }),
  sketch('tabSketch', top(15)), rect('tabSketch', 'r1', [17, 0], [23, 1.5]), solve('tabSketch'),
  call('extrude', 'tab', { entities: regions('tabSketch'), endBound: 'BoundingType.BLIND', depth: mm(5), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('holeSketch', planeAt([0, 0, 0], [0, 1, 0], [1, 0, 0])), circle('holeSketch', 'c1', [20, 18], 1), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', ...scoped, hasSecondDirection: 'true', secondDirectionBound: 'BoundingType.THROUGH_ALL' }),
]);
batch.add('fillet', 'a 50 × 40 × 12 block with an obround pocket (30 × 16, 8 deep), its walls\' top edge filleted 1 all round (a tangent chain) and its floor edge 2 all round', [
  box('base', [0, 0, 0], [50, 40, 12]),
  sketch('pocketSketch', top(12)), line('pocketSketch', 'l1', [18, 12], [32, 12]), arc('pocketSketch', 'a1', [32, 12], [40, 20], [32, 28]),
  line('pocketSketch', 'l2', [32, 28], [18, 28]), arc('pocketSketch', 'a2', [18, 28], [10, 20], [18, 12]), solve('pocketSketch'),
  call('extrude', 'pocket', { entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(8), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('fillet', 'floor', { entities: edgeAt(base, [25, 12, 4]), radius: mm(2), tangentPropagation: 'true' }),
  call('fillet', 'rim', { entities: edgeAt(base, [25, 12, 12]), radius: mm(1), tangentPropagation: 'true' }),
]);
batch.add('loft', 'a loft through a 20 × 20 square at z = 0, a circle of radius 8 at z = 15 and a 10 × 10 square at z = 30', [
  sketch('s1', top(0)), rect('s1', 'r1', [-10, -10], [10, 10]), solve('s1'),
  sketch('s2', top(15)), circle('s2', 'c1', [0, 0], 8), solve('s2'),
  sketch('s3', top(30)), rect('s3', 'r1', [-5, -5], [5, 5]), solve('s3'),
  call('loft', 'base', { sheetProfilesArray: `[{ "sheetProfileEntities" : qSketchRegion(id + "s1") }, { "sheetProfileEntities" : qSketchRegion(id + "s2") }, { "sheetProfileEntities" : qSketchRegion(id + "s3") }]` }),
]);

batch.write('tools/fixtures/gen-batch-142.mjs');

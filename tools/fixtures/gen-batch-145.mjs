// Generates fixture batch 145: everyday parts, a sixth round: a hex nut with its corners chamfered by a revolved cut,
// a bolt (a revolved shaft with a hexagonal head, its tip chamfered), a hinge leaf (a plate with two knuckles, one
// patterned from the other, and a pin hole), a spoked wheel (a disc with a window patterned five times round it), a
// bottle (a loft through three circles with a neck, shelled), an L bracket with two ribs and a hole, a shelled box
// with a lip round its top, a knob with notches patterned round it and its top's edges filleted.
//
// Usage: node tools/fixtures/gen-batch-145.mjs
//
// Writes fixtures/batch-145/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, cylinder, deg, edgeAt, faceAt, fsMap, line, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec2, vec3 } from './lib.mjs';

const batch = new Batch({ number: 145, title: 'everyday parts, a sixth round: a nut, a bolt, a hinge leaf, a spoked wheel, a bottle, a ribbed bracket, a box lip, a notched knob', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = (name) => [sketch(name, planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line(name, 'line1', [-100, 0], [100, 0]), solve(name)];
const hexagon = (s, r) => `skRegularPolygon(${s}, "polygon1", ${fsMap({ center: vec2(0, 0), firstVertex: vec2(r, 0), sides: '6' })});`;
const cut = (name, opposite = 'true') => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: opposite, operationType: 'NewBodyOperationType.REMOVE', ...scoped });
const cutFn = (name) =>
  `extrude(context, instanceId, { "entities" : qSketchRegion(id + "${name}Sketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} });`;
const circular = (feature, fn, count) =>
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "${feature}") : function(instanceId) { ${fn} } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : ${count}, "equalSpace" : true, "fullFeaturePattern" : false });`;

batch.add('revolve', 'a hex nut: a hexagon (corners 8 from its middle) extruded 6 with a hole (radius 3), the hole\'s edges chamfered 0.5 and the corners\' tops cut by a cone at 30° from radius 7.2', [
  sketch('baseSketch', top(0)), hexagon('baseSketch', 8), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(6) }),
  sketch('holeSketch', top(6)), circle('holeSketch', 'c1', [0, 0], 3), solve('holeSketch'), cut('hole'),
  call('chamfer', 'chamfer1', { entities: `qUnion([${edgeAt(base, [3, 0, 0])}, ${edgeAt(base, [3, 0, 6])}])`, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(0.5) }),
  sketch('coneSketch', PLANES.xz), polygon('coneSketch', [[7.2, 6], [9, 6], [9, 6 - 1.8 * Math.tan(Math.PI / 6)]]), solve('coneSketch'),
  zAxis('axis'),
  call('revolve', 'cone', { entities: regions('coneSketch'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
]);
batch.add('revolve', 'a bolt: a shaft (radius 3, 20 long) revolved, a hexagonal head (corners 6 from its middle) 4 tall added on its top, the tip\'s edge chamfered 0.5', [
  sketch('profile', PLANES.xz), polygon('profile', [[0, 0], [3, 0], [3, 20], [0, 20]]), solve('profile'),
  zAxis('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  sketch('headSketch', top(20)), hexagon('headSketch', 6), solve('headSketch'),
  call('extrude', 'head', { entities: regions('headSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [3, 0, 0]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(0.5) }),
]);
batch.add('linear-pattern', 'a hinge leaf: a 40 × 20 × 2 plate with a knuckle (radius 3, 10 long, its axis along x at y = 0, z = 1) patterned once at 20, a pin hole (radius 1.2) through both, the plate\'s far corners filleted 4', [
  box('base', [0, 0, 0], [40, 20, 2]),
  sketch('knuckleSketch', planeAt([0, 0, 0], [1, 0, 0], [0, 1, 0])), circle('knuckleSketch', 'c1', [0, 1], 3), solve('knuckleSketch'),
  call('extrude', 'knuckle', { entities: regions('knuckleSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('dir', top(0)), line('dir', 'line1', [0, 0], [10, 0]), solve('dir'),
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "knuckle") : function(instanceId) { extrude(context, instanceId, { "entities" : qSketchRegion(id + "knuckleSketch"), "endBound" : BoundingType.BLIND, "depth" : 10 * millimeter, "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} }); } } as FeatureList, "directionOne" : qCreatedBy(id + "dir", EntityType.EDGE), "distance" : 20 * millimeter, "instanceCount" : 2, "oppositeDirection" : false, "isCentered" : false, "hasSecondDir" : false, "fullFeaturePattern" : false });`,
  sketch('pinSketch', planeAt([-1, 0, 0], [1, 0, 0], [0, 1, 0])), circle('pinSketch', 'c1', [0, 1], 1.2), solve('pinSketch'), cut('pin', 'false'),
  call('fillet', 'fillet1', { entities: `qUnion([${edgeAt(base, [40, 20, 1])}, ${edgeAt(base, [0, 20, 1])}])`, radius: mm(4) }),
]);
const w = [[9.063, 4.226], [10, 0], [9.063, -4.226], [21.751, -10.143], [24, 0], [21.751, 10.143]];
batch.add('circular-pattern', 'a spoked wheel: a disc (radius 30, 6 thick) with a hub hole (radius 5), a window between radii 10 and 24 across 50° patterned five times round it, the rim\'s edges filleted 1', [
  cylinder('base', [0, 0], 30, 0, 6),
  sketch('hubSketch', top(6)), circle('hubSketch', 'c1', [0, 0], 5), solve('hubSketch'), cut('hub'),
  sketch('windowSketch', top(6)), arc('windowSketch', 'a1', w[0], w[1], w[2]), line('windowSketch', 'l1', w[2], w[3]), arc('windowSketch', 'a2', w[3], w[4], w[5]), line('windowSketch', 'l2', w[5], w[0]), solve('windowSketch'),
  cut('window'),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  circular('window', cutFn('window'), 5),
  call('fillet', 'fillet1', { entities: `qUnion([${edgeAt(base, [30, 0, 0])}, ${edgeAt(base, [30, 0, 6])}])`, radius: mm(1) }),
]);
batch.add('loft', 'a bottle: circles of radius 15 at z = 0 and z = 40 and of radius 6 at z = 55 lofted, a neck (radius 6) on to z = 65 added, shelled 1.2 open at the neck', [
  sketch('s1', top(0)), circle('s1', 'c1', [0, 0], 15), solve('s1'),
  sketch('s2', top(40)), circle('s2', 'c1', [0, 0], 15), solve('s2'),
  sketch('s3', top(55)), circle('s3', 'c1', [0, 0], 6), solve('s3'),
  call('loft', 'base', { sheetProfilesArray: `[{ "sheetProfileEntities" : qSketchRegion(id + "s1") }, { "sheetProfileEntities" : qSketchRegion(id + "s2") }, { "sheetProfileEntities" : qSketchRegion(id + "s3") }]` }),
  sketch('neckSketch', top(55)), circle('neckSketch', 'c1', [0, 0], 6), solve('neckSketch'),
  call('extrude', 'neck', { entities: regions('neckSketch'), endBound: 'BoundingType.BLIND', depth: mm(10), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 65]), thickness: mm(1.2) }),
]);
batch.add('rib', 'an L bracket (a 40 × 30 × 4 base, a 4 thick wall 20 up its x = 0 edge) with ribs (3 thick) in its corner at y = 6 and y = 24 and a hole (radius 3) in its base between them', [
  box('base', [0, 0, 0], [40, 30, 4]),
  box('wall', [0, 0, 4], [4, 30, 24]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: `qUnion([${base}, ${solidsOf('wall')}])` }),
  sketch('r1', planeAt([0, 6, 0], [0, -1, 0], [1, 0, 0])), line('r1', 'line1', [4, 20], [30, 4]), solve('r1'),
  sketch('r2', planeAt([0, 24, 0], [0, -1, 0], [1, 0, 0])), line('r2', 'line1', [4, 20], [30, 4]), solve('r2'),
  call('rib', 'rib1', { profiles: `qUnion([${wireEdges('r1')}, ${wireEdges('r2')}])`, parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.PARALLEL_TO_SKETCH_PLANE', oppositeDirection: 'true' }),
  sketch('holeSketch', top(4)), circle('holeSketch', 'c1', [28, 15], 3), solve('holeSketch'), cut('hole'),
]);
batch.add('shell', 'a 40 × 30 × 20 box shelled 2 open at its top, a lip 1 wide and 3 tall on the inner half of its walls\' top', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('shell', 'shell1', { entities: faceAt(base, [20, 15, 20]), thickness: mm(2) }),
  sketch('lipSketch', top(20)), rect('lipSketch', 'outer', [1, 1], [39, 29]), rect('lipSketch', 'inner', [2, 2], [38, 28]), solve('lipSketch'),
  call('extrude', 'lip', { entities: `qContainsPoint(qSketchRegion(id + "lipSketch"), ${vec3(1.5, 15, 20)})`, endBound: 'BoundingType.BLIND', depth: mm(3), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('circular-pattern', 'a knob: a cylinder (radius 10, 8 tall) with a notch (radius 1.5, centred on its side) patterned 12 times round it, its top\'s edges filleted 0.5', [
  cylinder('base', [0, 0], 10, 0, 8),
  sketch('notchSketch', top(8)), circle('notchSketch', 'c1', [10, 0], 1.5), solve('notchSketch'), cut('notch'),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  circular('notch', cutFn('notch'), 12),
  call('fillet', 'fillet1', { entities: `qAdjacent(${faceAt(base, [0, 0, 8])}, AdjacencyType.EDGE, EntityType.EDGE)`, radius: mm(0.5) }),
]);

batch.write('tools/fixtures/gen-batch-145.mjs');

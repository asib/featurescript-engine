// Generates fixture batch 146: everyday parts, a seventh round: an enclosure (a box with rounded corners shelled open
// at its top, four bosses with holes inside), a pipe elbow (a circle swept along a line, an arc and a line, shelled
// open at both ends), a flanged pipe (revolved, a bolt hole patterned round the flange, the flange's joint filleted),
// a box with its top edges filleted split into a lid by a plane, a box with its sides drafted, a block with a pocket
// mirrored as a feature, a gear-like disc (a tooth part patterned sixteen times and united with it, a hole with a keyway), a stepped shaft
// with a keyway and its edges chamfered.
//
// Usage: node tools/fixtures/gen-batch-146.mjs
//
// Writes fixtures/batch-146/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, createdBy, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, polygon, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 146, title: 'everyday parts, a seventh round: an enclosure, a pipe elbow, a flanged pipe, a split lid, a drafted box, a mirrored pocket, a gear-like disc, a keyed shaft', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = (name) => [sketch(name, planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line(name, 'line1', [-100, 0], [100, 0]), solve(name)];
const edges = (points) => `qUnion([${points.map((p) => edgeAt(base, p)).join(', ')}])`;
const faces = (points) => `qUnion([${points.map((p) => faceAt(base, p)).join(', ')}])`;
const featureFn = (name, fields) =>
  `extrude(context, instanceId, { ${Object.entries({ entities: `qSketchRegion(id + "${name}Sketch")`, ...fields, defaultScope: 'false', booleanScope: base }).map(([k, v]) => `"${k}" : ${v}`).join(', ')} });`;
const circular = (feature, fn, count) =>
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "${feature}") : function(instanceId) { ${fn} } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : ${count}, "equalSpace" : true, "fullFeaturePattern" : false });`;
const corners = [[8, 8], [52, 8], [52, 32], [8, 32]];

batch.add('shell', 'an enclosure: a 60 × 40 × 30 box, its upright edges filleted 5, shelled 2 open at its top, bosses (radius 3) from its floor to z = 25 near each corner with holes (radius 1.2) 15 deep', [
  box('base', [0, 0, 0], [60, 40, 30]),
  call('fillet', 'fillet1', { entities: edges([[0, 0, 15], [60, 0, 15], [60, 40, 15], [0, 40, 15]]), radius: mm(5) }),
  call('shell', 'shell1', { entities: faceAt(base, [30, 20, 30]), thickness: mm(2) }),
  sketch('bossSketch', top(2)), ...corners.map(([x, y], i) => circle('bossSketch', `c${i + 1}`, [x, y], 3)), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(23), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('holeSketch', top(25)), ...corners.map(([x, y], i) => circle('holeSketch', `c${i + 1}`, [x, y], 1.2)), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.BLIND', depth: mm(15), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
]);
batch.add('sweep', 'a pipe elbow: a circle (radius 5) swept up 20, round a quarter circle (radius 20) and on 20, shelled 1 open at both ends', [
  sketch('path', PLANES.xz),
  line('path', 'l1', [0, 0], [0, 20]), arc('path', 'a1', [0, 20], [20 - 20 * Math.SQRT1_2, 20 + 20 * Math.SQRT1_2], [20, 40]), line('path', 'l2', [20, 40], [40, 40]), solve('path'),
  sketch('profile', top(0)), circle('profile', 'c1', [0, 0], 5), solve('profile'),
  call('sweep', 'base', { profiles: regions('profile'), path: `qUnion([${wireEdges('path')}])` }),
  call('shell', 'shell1', { entities: faces([[0, 0, 0], [40, 0, 40]]), thickness: mm(1) }),
]);
batch.add('circular-pattern', 'a flanged pipe: a tube (radii 5 and 7, 30 tall) on a flange (radius 15, 3 thick) revolved, a bolt hole (radius 1.5) at 11 patterned six times round it, the tube\'s joint with the flange filleted 1', [
  sketch('profile', PLANES.xz), polygon('profile', [[5, 0], [15, 0], [15, 3], [7, 3], [7, 30], [5, 30]]), solve('profile'),
  zAxis('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  sketch('boltSketch', top(3)), circle('boltSketch', 'c1', [11, 0], 1.5), solve('boltSketch'),
  call('extrude', 'bolt', { entities: regions('boltSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  circular('bolt', featureFn('bolt', { endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE' }), 6),
  call('fillet', 'fillet1', { entities: edgeAt(base, [7, 0, 3]), radius: mm(1) }),
]);
batch.add('split', 'a 50 × 40 × 20 box with its top edges filleted 3, split into a lid by a plane at z = 15', [
  box('base', [0, 0, 0], [50, 40, 20]),
  call('fillet', 'fillet1', { entities: edges([[25, 0, 20], [50, 20, 20], [25, 40, 20], [0, 20, 20]]), radius: mm(3) }),
  call('opPlane', 'cut', { plane: `plane(${vec3(0, 0, 15)}, vector(0, 0, 1))`, width: mm(200), height: mm(200) }),
  call('splitPart', 'split1', { targets: base, tool: createdBy('cut', 'FACE') }),
]);
batch.add('draft', 'a 40 × 30 × 20 box with its four sides drafted 5° from its bottom', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('draft', 'draft1', { neutralPlane: faceAt(base, [20, 15, 0]), draftFaces: faces([[20, 0, 10], [40, 15, 10], [20, 30, 10], [0, 15, 10]]), angle: deg(5) }),
]);
batch.add('mirror', 'a 60 × 30 × 10 block with a 15 × 20 pocket 4 deep at one end, mirrored as a feature across x = 30', [
  box('base', [0, 0, 0], [60, 30, 10]),
  sketch('pocketSketch', top(10)), rect('pocketSketch', 'r1', [5, 5], [20, 25]), solve('pocketSketch'),
  call('extrude', 'pocket', { entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('opPlane', 'mid', { plane: `plane(${vec3(30, 0, 0)}, vector(1, 0, 0))`, width: mm(100), height: mm(100) }),
  `mirror(context, id + "mirror1", { "patternType" : MirrorType.FEATURE, "instanceFunction" : { (id + "pocket") : function(instanceId) { ${featureFn('pocket', { endBound: 'BoundingType.BLIND', depth: mm(4), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE' })} } } as FeatureList, "mirrorPlane" : qCreatedBy(id + "mid", EntityType.FACE), "fullFeaturePattern" : false });`,
]);
batch.add('circular-pattern', 'a gear-like disc: a disc (radius 20, 5 thick), a trapezoidal tooth out to 24 made as a part and patterned sixteen times, all united, a hole (radius 4) with a keyway to 5.5 through its middle', [
  cylinder('base', [0, 0], 20, 0, 5),
  sketch('toothSketch', top(0)), polygon('toothSketch', [[19, -2], [24, -1], [24, 1], [19, 2]]), solve('toothSketch'),
  call('extrude', 'tooth', { entities: regions('toothSketch'), endBound: 'BoundingType.BLIND', depth: mm(5) }),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  call('circularPattern', 'pattern1', { patternType: 'PatternType.PART', entities: solidsOf('tooth'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, angle: deg(360), instanceCount: '16', equalSpace: 'true' }),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: `qUnion([${base}, ${solidsOf('tooth')}, ${solidsOf('pattern1')}])` }),
  sketch('boreSketch', top(5)), circle('boreSketch', 'c1', [0, 0], 4), rect('boreSketch', 'key', [3, -1], [5.5, 1]), solve('boreSketch'),
  call('extrude', 'bore', { entities: regions('boreSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
]);
batch.add('chamfer', 'a stepped shaft (radius 10 for 20, then radius 7 to 45) revolved, a keyway (3 wide, 1.5 deep, z = 25 to 40) cut in its narrow end, its end and step edges chamfered 0.8 and 1', [
  sketch('profile', PLANES.xz), polygon('profile', [[0, 0], [10, 0], [10, 20], [7, 20], [7, 45], [0, 45]]), solve('profile'),
  zAxis('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  sketch('keySketch', planeAt([8, 0, 0], [1, 0, 0], [0, 1, 0])), rect('keySketch', 'r1', [-1.5, 25], [1.5, 40]), solve('keySketch'),
  call('extrude', 'key', { entities: regions('keySketch'), endBound: 'BoundingType.BLIND', depth: mm(2.5), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [-7, 0, 45]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(0.8) }),
  call('chamfer', 'chamfer2', { entities: edgeAt(base, [-10, 0, 20]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1) }),
]);

batch.write('tools/fixtures/gen-batch-146.mjs');

// Generates fixture batch 126: more everyday variants. Faces moved on curved faces (a hole's wall offset larger, a
// boss's side offset thinner); faces deleted and healed through curved faces (a cross hole through a lying
// cylinder, a spherical pocket); feature patterns of a boss round a cylinder's top and of a hole along a block; a
// chamfered block mirrored about its side face and merged; a rectangle extruded symmetrically with a draft; a
// profile revolved 270°; a cylinder subtracted from a block, kept.
//
// Usage: node tools/fixtures/gen-batch-126.mjs
//
// Writes fixtures/batch-126/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 126, title: 'faces moved and healed on curved faces, feature patterns, a merged mirror, drafts, a partial revolve', helpers: '' });
const base = solidsOf('base');

// A boss (circle extruded and added to `base`) or a hole (cut through it) from a sketch on the plane z.
const onBase = (op, name, [cx, cy], r, z, depth, flip = false, through = false) => {
  const s = `${name}Sketch`;
  return [
    sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
    circle(s, 'circle1', [cx, cy], r),
    solve(s),
    call('extrude', name, {
      entities: regions(s), ...(through ? { endBound: 'BoundingType.THROUGH_ALL' } : { endBound: 'BoundingType.BLIND', depth: mm(depth) }),
      oppositeDirection: flip ? 'true' : 'false',
      operationType: `NewBodyOperationType.${op}`, defaultScope: 'false', booleanScope: base,
    }),
  ];
};
// A cylinder lying along x from x = -l/2 to l/2, its axis at height z.
const lying = (name, r, l, z) => [
  sketch(name + 'Sketch', planeAt([-l / 2, 0, z], [1, 0, 0], [0, 1, 0])), circle(name + 'Sketch', 'circle1', [0, 0], r), solve(name + 'Sketch'),
  call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.BLIND', depth: mm(l) }),
];
const featurePattern = (fn, feature, body, fields) =>
  `${fn}(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "${feature}") : function(instanceId) { ${body} } } as FeatureList, ${Object.entries(fields).map(([k, v]) => `"${k}" : ${v}`).join(', ')} });`;

batch.add('move-face', "a 30 × 20 × 10 block with a hole (radius 4) through it: the hole's wall offset 1 mm (into the hole, larger)", [
  box('base', [0, 0, 0], [30, 20, 10]), onBase('REMOVE', 'hole', [15, 10], 4, 10, 0, true, true),
  call('moveFace', 'move1', { moveFaces: faceAt(base, [19, 10, 5]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(1) }),
]);
batch.add('move-face', "a 30 × 20 × 10 block with a boss (radius 5, 8 tall): the boss's side offset 1 mm inwards (thinner)", [
  box('base', [0, 0, 0], [30, 20, 10]), onBase('ADD', 'boss', [15, 10], 5, 10, 8),
  call('moveFace', 'move1', { moveFaces: faceAt(base, [20, 10, 14]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(1), oppositeDirection: 'true' }),
]);
batch.add('delete-face', 'a cylinder (radius 10, 40 long) lying along x with a hole of radius 2 cut down through it: the hole deleted and healed', [
  lying('base', 10, 40, 0),
  sketch('holeSketch', planeAt([0, 0, 20], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'circle1', [5, 0], 2), solve('holeSketch'),
  call('extrude', 'hole', {
    entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  call('deleteFace', 'delete1', { deleteFaces: `qCreatedBy(id + "hole", EntityType.FACE)` }),
]);
batch.add('delete-face', 'a 30 × 20 × 10 block with a pocket of a sphere (radius 6) centred 2 above its top: the pocket deleted and healed', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('opSphere', 'ball', { center: vec3(15, 10, 12), radius: mm(6) }),
  call('booleanBodies', 'pocket', { tools: solidsOf('ball'), targets: base, operationType: 'BooleanOperationType.SUBTRACTION' }),
  call('deleteFace', 'delete1', { deleteFaces: `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.SPHERE)` }),
]);
batch.add('circular-pattern', "a cylinder (radius 15, 10 tall) with a boss (radius 2, 3 tall) at (10, 0) on its top, patterned 6 times round its axis", [
  cylinder('base', [0, 0], 15, 0, 10),
  sketch('bossSketch', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('bossSketch', 'circle1', [10, 0], 2), solve('bossSketch'),
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [10, 0]), solve('axis1'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(3), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
  featurePattern('circularPattern', 'boss',
    `extrude(context, instanceId, { "entities" : ${regions('bossSketch')}, "endBound" : BoundingType.BLIND, "depth" : 3 * millimeter, "operationType" : NewBodyOperationType.ADD, "defaultScope" : false, "booleanScope" : ${base} });`,
    { axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, angle: deg(360), instanceCount: 6, equalSpace: 'true', fullFeaturePattern: 'false' }),
]);
batch.add('linear-pattern', 'a 40 × 20 × 10 block with a hole (radius 2) through it at (8, 10), patterned 4 times 8 mm apart along x', [
  box('base', [0, 0, 0], [40, 20, 10]),
  sketch('holeSketch', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('holeSketch', 'circle1', [8, 10], 2), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
  featurePattern('linearPattern', 'hole',
    `extrude(context, instanceId, { "entities" : ${regions('holeSketch')}, "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : true, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${base} });`,
    { directionOne: edgeAt(base, [20, 0, 0]), distance: mm(8), instanceCount: 4 }),
]);
batch.add('mirror', 'a 30 × 20 × 10 block with its top front edge chamfered 3 mm, mirrored about its right side and merged', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [15, 0, 10]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(3) }),
  call('mirror', 'mirror1', { entities: base, mirrorPlane: faceAt(base, [30, 10, 5]), operationType: 'NewBodyOperationType.ADD', defaultScope: 'false', booleanScope: base }),
]);
batch.add('extrude', 'a 20 × 10 rectangle extruded 12 symmetrically with a 5° draft', [
  sketch('s1', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), rect('s1', 'rect1', [0, 0], [20, 10]), solve('s1'),
  call('extrude', 'base', { entities: regions('s1'), endBound: 'BoundingType.BLIND', depth: mm(12), symmetric: 'true', hasDraft: 'true', draftAngle: deg(5), draftPullDirection: 'false' }),
]);
batch.add('revolve', 'a 10 × 6 rectangle 5 out from z revolved 270° about it', [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [5, 0], [15, 6]), solve('s1'),
  sketch('axis1', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis1', 'line1', [0, 0], [10, 0]), solve('axis1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `qCreatedBy(id + "axis1", EntityType.EDGE)`, endBound: 'RevolveBoundingType.BLIND', angle: deg(270) }),
]);
batch.add('boolean', 'a cylinder (radius 6, through) subtracted from a 30 × 20 × 10 block, the cylinder kept', [
  box('base', [0, 0, 0], [30, 20, 10]), cylinder('tool', [15, 10], 6, -5, 15),
  call('booleanBodies', 'cut1', { operationType: 'BooleanOperationType.SUBTRACTION', targets: base, tools: solidsOf('tool'), keepTools: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-126.mjs');

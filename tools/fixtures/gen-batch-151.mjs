// Generates fixture batch 151: everyday parts, a tenth round, through less travelled options: a profile revolved up
// to a plane, a disc's boss patterned round it as faces, a block's pocket mirrored as faces, a boss extruded up to
// another part with an offset, a block intersected with a cylinder, a cylinder's side thickened into a tube of its own,
// a filleted block split by a plane with the top piece deleted, a filleted block's side drafted with tangent
// propagation.
//
// Usage: node tools/fixtures/gen-batch-151.mjs
//
// Writes fixtures/batch-151/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, box, call, circle, createdBy, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, polygon, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 151, title: 'everyday parts, a tenth round: revolve up to a plane, face patterns and mirrors, up to a part with an offset, an intersection, a thickened side, a split and delete, a propagated draft', helpers: '' });
const base = solidsOf('base');
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = (name) => [sketch(name, planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line(name, 'line1', [-100, 0], [100, 0]), solve(name)];
const edges = (points) => `qUnion([${points.map((p) => edgeAt(base, p)).join(', ')}])`;
const faces = (points) => `qUnion([${points.map((p) => faceAt(base, p)).join(', ')}])`;

batch.add('revolve', 'a 10 × 8 rectangle (from x = 10) revolved about z up to a plane through the axis at 120°', [
  sketch('profile', PLANES.xz), polygon('profile', [[10, 0], [20, 0], [20, 8], [10, 8]]), solve('profile'),
  zAxis('axis'),
  call('opPlane', 'limit', { plane: `plane(${vec3(0, 0, 0)}, vector(${Math.sin(Math.PI * 2 / 3)}, ${-Math.cos(Math.PI * 2 / 3)}, 0))`, width: mm(100), height: mm(100) }),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'false', endBound: 'RevolveBoundingType.UP_TO_SURFACE', endBoundEntityFace: createdBy('limit', 'FACE'), oppositeDirection: 'false' }),
]);
batch.add('circular-pattern', 'a disc (radius 20, 5 thick) with a boss (radius 2, 4 tall) at (12, 0) on top, its faces patterned six times round the disc', [
  cylinder('base', [0, 0], 20, 0, 5),
  sketch('bossSketch', top(5)), circle('bossSketch', 'c1', [12, 0], 2), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  call('circularPattern', 'pattern1', { patternType: 'PatternType.FACE', faces: `qCreatedBy(id + "boss", EntityType.FACE)`, axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, angle: deg(360), instanceCount: '6', equalSpace: 'true' }),
]);
batch.add('mirror', 'a 60 × 30 × 10 block with a 15 × 20 pocket 4 deep at one end, the pocket\'s faces mirrored across x = 30', [
  box('base', [0, 0, 0], [60, 30, 10]),
  sketch('pocketSketch', top(10)), `skRectangle(pocketSketch, "r1", { "firstCorner" : vector(5, 5) * millimeter, "secondCorner" : vector(20, 25) * millimeter });`, solve('pocketSketch'),
  call('extrude', 'pocket', { entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
  call('opPlane', 'mid', { plane: `plane(${vec3(30, 0, 0)}, vector(1, 0, 0))`, width: mm(100), height: mm(100) }),
  call('mirror', 'mirror1', { patternType: 'MirrorType.FACE', faces: `qCreatedBy(id + "pocket", EntityType.FACE)`, mirrorPlane: createdBy('mid', 'FACE') }),
]);
batch.add('extrude', 'a 40 × 20 × 5 plate and, apart, a block above it (z = 20 to 30); a boss (radius 4) extruded from the plate up to the block, stopping 2 short', [
  box('base', [0, 0, 0], [40, 20, 5]), box('roof', [10, 0, 20], [30, 20, 30]),
  sketch('bossSketch', top(5)), circle('bossSketch', 'c1', [20, 10], 4), solve('bossSketch'),
  call('extrude', 'boss', {
    entities: regions('bossSketch'), endBound: 'BoundingType.UP_TO_BODY', endBoundEntityBody: solidsOf('roof'), hasOffset: 'true', offsetDistance: mm(2), offsetOppositeDirection: 'false',
    operationType: 'NewBodyOperationType.ADD', ...scoped,
  }),
]);
batch.add('boolean', 'a 30 × 30 × 20 block intersected with a cylinder (radius 18) up its middle', [
  box('base', [-15, -15, 0], [15, 15, 20]), cylinder('round', [0, 0], 18, 0, 20),
  call('booleanBodies', 'boolean1', { operationType: 'BooleanOperationType.INTERSECTION', tools: `qUnion([${base}, ${solidsOf('round')}])` }),
]);
batch.add('thicken', 'a cylinder (radius 10, 20 tall) with its side thickened 2 out into a tube of its own', [
  cylinder('base', [0, 0], 10, 0, 20),
  call('thicken', 'thick1', { entities: faceAt(base, [10, 0, 10]), thickness1: mm(2), thickness2: mm(0) }),
]);
batch.add('split', 'a 40 × 30 × 20 block with its upright edges filleted 4, split by a plane at z = 12 and the piece above deleted', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edges([[0, 0, 10], [40, 0, 10], [40, 30, 10], [0, 30, 10]]), radius: mm(4) }),
  call('opPlane', 'cut', { plane: `plane(${vec3(0, 0, 12)}, vector(0, 0, 1))`, width: mm(200), height: mm(200) }),
  call('splitPart', 'split1', { targets: base, tool: createdBy('cut', 'FACE') }),
  call('deleteBodies', 'delete1', { entities: `qContainsPoint(${base}, ${vec3(20, 15, 16)})` }),
]);
batch.add('draft', 'a 40 × 30 × 20 block with its upright edges filleted 4, its front side drafted 3° from its bottom with tangent propagation', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: edges([[0, 0, 10], [40, 0, 10], [40, 30, 10], [0, 30, 10]]), radius: mm(4) }),
  call('draft', 'draft1', { neutralPlane: faceAt(base, [20, 15, 0]), draftFaces: faceAt(base, [20, 0, 10]), angle: deg(3), tangentPropagation: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-151.mjs');

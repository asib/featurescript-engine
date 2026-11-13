// Generates fixture batch 148: everyday parts, a ninth round: a pillow block (a housing on a plate, a bore through it
// chamfered by distance and angle, mounting holes), a C frame with a column extruded up to the next face, a dome
// revolved from an arc and shelled with a hole in its top, a bracket mirrored and united with itself, a plate with a
// pin patterned as a part 3 × 2 and united, a block's top offset and the offset thickened, a channel with a rib
// normal to its sketch, a block with a chamfer of two distances and a fillet.
//
// Usage: node tools/fixtures/gen-batch-148.mjs
//
// Writes fixtures/batch-148/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, box, call, circle, createdBy, cylinder, deg, edgeAt, faceAt, line, mm, planeAt, polygon, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 148, title: 'everyday parts, a ninth round: a pillow block, an up-to-next column, a shelled dome, a mirrored bracket, a pin grid, a thickened offset, a rib, a two-distance chamfer', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const zAxis = (name) => [sketch(name, planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line(name, 'line1', [-100, 0], [100, 0]), solve(name)];
const cut = (name, opposite = 'true') => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: opposite, operationType: 'NewBodyOperationType.REMOVE', ...scoped });
const union = (name, parts) => call('booleanBodies', name, { operationType: 'BooleanOperationType.UNION', tools: `qUnion([${parts.join(', ')}])` });

batch.add('chamfer', 'a pillow block: a 30 × 30 × 22 housing on a 60 × 30 × 8 plate, a bore (radius 8) through it along y at z = 20, the bore\'s edges chamfered 1 at 30°, mounting holes (radius 3) at (7, 15) and (53, 15)', [
  box('base', [0, 0, 0], [60, 30, 8]), box('housing', [15, 0, 8], [45, 30, 30]), union('union1', [base, solidsOf('housing')]),
  sketch('boreSketch', planeAt([0, -5, 0], [0, 1, 0], [1, 0, 0])), circle('boreSketch', 'c1', [30, -20], 8), solve('boreSketch'), cut('bore', 'false'),
  sketch('mountSketch', top(8)), circle('mountSketch', 'c1', [7, 15], 3), circle('mountSketch', 'c2', [53, 15], 3), solve('mountSketch'), cut('mount'),
  call('chamfer', 'chamfer1', { entities: `qUnion([${edgeAt(base, [30, 0, 12])}, ${edgeAt(base, [30, 30, 12])}])`, chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(1), angle: deg(30), oppositeDirection: 'false' }),
]);
batch.add('extrude', 'a C frame (a 40 × 40 base 5 thick, a wall 5 thick up its x = 0 side to z = 25, a top 5 thick) with a column (radius 4) at (25, 20) extruded from the base up to the next face', [
  box('base', [0, 0, 0], [40, 40, 5]), box('wall', [0, 0, 5], [5, 40, 25]), box('roof', [0, 0, 25], [40, 40, 30]),
  union('union1', [base, solidsOf('wall'), solidsOf('roof')]),
  sketch('columnSketch', top(5)), circle('columnSketch', 'c1', [25, 20], 4), solve('columnSketch'),
  call('extrude', 'column', { entities: regions('columnSketch'), endBound: 'BoundingType.UP_TO_NEXT', operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('revolve', 'a dome: a quarter circle (radius 20) closed to the axis revolved, shelled 1.5 open at its bottom, a hole (radius 3) through its top', [
  sketch('profile', PLANES.xz), arc('profile', 'a1', [20, 0], [20 * Math.SQRT1_2, 20 * Math.SQRT1_2], [0, 20]), line('profile', 'l1', [0, 20], [0, 0]), line('profile', 'l2', [0, 0], [20, 0]), solve('profile'),
  zAxis('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 0]), thickness: mm(1.5) }),
  sketch('holeSketch', top(25)), circle('holeSketch', 'c1', [0, 0], 3), solve('holeSketch'), cut('hole'),
]);
batch.add('mirror', 'a bracket (a 30 × 20 × 5 foot, a 5 thick wall 20 up its x = 25 end, a hole of radius 3 through the wall) mirrored across x = 0 and united with its mirror', [
  box('base', [0, 0, 0], [30, 20, 5]), box('wall', [25, 0, 5], [30, 20, 25]), union('union1', [base, solidsOf('wall')]),
  sketch('holeSketch', planeAt([35, 0, 0], [1, 0, 0], [0, 1, 0])), circle('holeSketch', 'c1', [10, 15], 3), solve('holeSketch'), cut('hole'),
  call('opPlane', 'mirrorPlane', { plane: `plane(${vec3(0, 0, 0)}, vector(1, 0, 0))`, width: mm(100), height: mm(100) }),
  call('mirror', 'mirror1', { entities: base, mirrorPlane: createdBy('mirrorPlane', 'FACE') }),
  union('union2', [base, solidsOf('mirror1')]),
]);
batch.add('linear-pattern', 'a 40 × 30 × 5 plate with a pin (radius 2, 7 tall) at (8, 8) patterned as a part 3 along x at 12 and 2 along y at 14, all united', [
  box('base', [0, 0, 0], [40, 30, 5]), cylinder('pin', [8, 8], 2, 5, 12),
  sketch('dirX', top(0)), line('dirX', 'line1', [0, 0], [10, 0]), solve('dirX'),
  sketch('dirY', top(0)), line('dirY', 'line1', [0, 0], [0, 10]), solve('dirY'),
  call('linearPattern', 'pattern1', {
    patternType: 'PatternType.PART', entities: solidsOf('pin'), directionOne: `qCreatedBy(id + "dirX", EntityType.EDGE)`, distance: mm(12), instanceCount: '3', oppositeDirection: 'false',
    hasSecondDir: 'true', directionTwo: `qCreatedBy(id + "dirY", EntityType.EDGE)`, distanceTwo: mm(14), instanceCountTwo: '2', oppositeDirectionTwo: 'false',
  }),
  union('union1', [base, solidsOf('pin'), solidsOf('pattern1')]),
]);
batch.add('thicken', 'a 30 × 20 × 10 block with its top offset 5 up as a surface, the surface thickened 2 up into a plate of its own', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('offsetSurface', 'offset1', { surfacesAndFaces: faceAt(base, [15, 10, 10]), offset: mm(5), oppositeDirection: 'false' }),
  call('thicken', 'thick1', { entities: `qCreatedBy(id + "offset1", EntityType.FACE)`, thickness1: mm(2), thickness2: mm(0) }),
]);
batch.add('rib', 'a channel (a 40 × 30 × 4 base, walls 3 thick and 16 tall along y = 0 and y = 30) with a rib (3 thick) across it at x = 20, sketched on its top and extruded down to the base', [
  box('base', [0, 0, 0], [40, 30, 4]), box('w1', [0, 0, 4], [40, 3, 20]), box('w2', [0, 27, 4], [40, 30, 20]),
  union('union1', [base, solidsOf('w1'), solidsOf('w2')]),
  sketch('profile', top(20)), line('profile', 'line1', [20, 3], [20, 27]), solve('profile'),
  call('rib', 'rib1', { profiles: wireEdges('profile'), parts: base, thickness: mm(3), ribExtrusionDirection: 'RibExtrusionDirection.NORMAL_TO_SKETCH_PLANE', oppositeDirection: 'true' }),
]);
batch.add('chamfer', 'a 30 × 20 × 10 block with its top front edge chamfered 2 and 4 and its top back edge filleted 3', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('chamfer', 'chamfer1', { entities: edgeAt(base, [15, 0, 10]), chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(4), oppositeDirection: 'false' }),
  call('fillet', 'fillet1', { entities: edgeAt(base, [15, 20, 10]), radius: mm(3) }),
]);

batch.write('tools/fixtures/gen-batch-148.mjs');

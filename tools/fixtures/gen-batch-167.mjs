// Generates fixture batch 167: options the engine refused: face patterns of a boss's and a pocket's faces together,
// linear and circular; and surfaces extruded from a line with their second direction up to a lying cylinder.
//
// Usage: node tools/fixtures/gen-batch-167.mjs
//
// Writes fixtures/batch-167/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, deg, line, mm, planeAt, regions, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 167, title: 'face patterns of bosses and pockets together; surfaces extruded up to a curved face in their second direction', helpers: '' });
const base = solidsOf('base');
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
// A boss (radius 3, 4 tall) and a pocket (radius 2, 3 deep) on a block's top.
const bossAndPocket = (block, boss, pocket) => [
  ...block,
  sketch('bossSketch', top(10)), circle('bossSketch', 'c1', boss, 3), solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  sketch('pocketSketch', top(10)), circle('pocketSketch', 'c1', pocket, 2), solve('pocketSketch'),
  call('extrude', 'pocket', { entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(3), oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
];
const both = `qUnion([qCreatedBy(id + "boss", EntityType.FACE), qCreatedBy(id + "pocket", EntityType.FACE)])`;
batch.add('linear-pattern', 'a boss and a pocket on a 60 × 20 × 10 block, their faces patterned three times 15 along x', [
  ...bossAndPocket(box('base', [0, 0, 0], [60, 20, 10]), [8, 6], [8, 14]),
  sketch('dir', top(0)), line('dir', 'l1', [0, 0], [10, 0]), solve('dir'),
  call('linearPattern', 'pattern1', { patternType: 'PatternType.FACE', faces: both, directionOne: `qCreatedBy(id + "dir", EntityType.EDGE)`, distance: mm(15), instanceCount: '3', oppositeDirection: 'false', isCentered: 'false', hasSecondDir: 'false' }),
]);
batch.add('circular-pattern', 'a boss and a pocket on a disc (radius 25, 10 thick), their faces patterned four times round its axis', [
  ...bossAndPocket(cylinder('base', [0, 0], 25, 0, 10), [15, 0], [0, 15]),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'l1', [0, 0], [10, 0]), solve('axis'),
  call('circularPattern', 'pattern1', { patternType: 'PatternType.FACE', faces: both, axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, angle: deg(360), instanceCount: '4', equalSpace: 'true' }),
]);

// A cylinder lying along x under the top plane (radius 8, axis at z = -20); a line on the top plane extruded as a
// surface 10 up and down to it.
const lying = [
  sketch('cylSketch', planeAt([-30, 0, 0], [1, 0, 0], [0, 1, 0])), circle('cylSketch', 'c1', [0, -20], 8), solve('cylSketch'),
  call('extrude', 'cyl', { entities: regions('cylSketch'), endBound: 'BoundingType.BLIND', depth: mm(60) }),
];
batch.add('extrude', 'a line across the lying cylinder extruded as a surface 10 up and down to it', [
  ...lying, sketch('prof', top(0)), line('prof', 'l1', [5, -6], [5, 6]), solve('prof'),
  call('extrude', 'ext1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "prof", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(10),
    hasSecondDirection: 'true', secondDirectionOppositeDirection: 'true', secondDirectionBound: 'BoundingType.UP_TO_SURFACE',
    secondDirectionBoundEntityFace: `qContainsPoint(qOwnedByBody(${solidsOf('cyl')}, EntityType.FACE), vector(0, 0, -12) * millimeter)` }),
]);
batch.add('extrude', 'a line along the lying cylinder extruded as a surface 10 up and down to it', [
  ...lying, sketch('prof', top(0)), line('prof', 'l1', [-10, 3], [10, 3]), solve('prof'),
  call('extrude', 'ext1', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "prof", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(10),
    hasSecondDirection: 'true', secondDirectionOppositeDirection: 'true', secondDirectionBound: 'BoundingType.UP_TO_SURFACE',
    secondDirectionBoundEntityFace: `qContainsPoint(qOwnedByBody(${solidsOf('cyl')}, EntityType.FACE), vector(0, 0, -12) * millimeter)` }),
]);

batch.write('tools/fixtures/gen-batch-167.mjs');

// Generates fixture batch 143: everyday sheet metal: a box whose two long walls have holes patterned along them, a
// plate with a 3 × 2 grid of holes, a channel with holes patterned along both legs, a bracket with an obround slot in
// its flange and the flange's top edge hemmed, a plate with a jog and holes on both sides of it, a plate with its
// corners rounded and a hole patterned round its middle.
//
// Usage: node tools/fixtures/gen-batch-143.mjs
//
// Writes fixtures/batch-143/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, arc, call, circle, deg, line, mm, planeAt, rect, regions, sketch, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 143, title: 'everyday sheet metal: patterned holes on walls, a hole grid, a slotted hemmed flange, a jog with holes, rounded corners', helpers: '' });
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const edgeOfPart = (p) => `qContainsPoint(qOwnedByBody(${part}, EntityType.EDGE), ${vec3(...p)})`;
const plate = (w, h) => [
  sketch('s1', top(0)), rect('s1', 'rect1', [0, 0], [w, h]), solve('s1'),
  call('sheetMetalStart', 'sm1', { thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
];
const flange = (name, edges, distance) => call('sheetMetalFlange', name, {
  edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(distance), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE',
  oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
});
const cut = (name, opposite = 'true') => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: opposite, operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part });
const cutFn = (name, opposite = true) =>
  `extrude(context, instanceId, { "entities" : qSketchRegion(id + "${name}Sketch"), "endBound" : BoundingType.THROUGH_ALL, "oppositeDirection" : ${opposite}, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${part} });`;
const linear = (feature, fn, dir, distance, count, second = '') =>
  `linearPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "${feature}") : function(instanceId) { ${fn} } } as FeatureList, "directionOne" : qCreatedBy(id + "${dir}", EntityType.EDGE), "distance" : ${distance} * millimeter, "instanceCount" : ${count}, "oppositeDirection" : false, "isCentered" : false, ${second || '"hasSecondDir" : false'}, "fullFeaturePattern" : false });`;
const dir = (name, to) => [sketch(name, top(0)), line(name, 'line1', [0, 0], to), solve(name)];

batch.add('linear-pattern', 'a sheet metal box (a 60 × 40 plate, 1.5 thick, flanges 20 up on all four edges) with a hole (radius 2.5) in each long wall patterned 4 along it at 12', [
  plate(60, 40),
  flange('flange1', `qUnion([${[[30, 0], [60, 20], [30, 40], [0, 20]].map(([x, y]) => edgeOfPart([x, y, 1.5])).join(', ')}])`, 20),
  sketch('holesSketch', planeAt([0, -10, 0], [0, 1, 0], [1, 0, 0])), circle('holesSketch', 'c1', [12, -10], 2.5), solve('holesSketch'),
  call('extrude', 'holes', { entities: regions('holesSketch'), endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part }),
  dir('dir', [10, 0]),
  linear('holes', `extrude(context, instanceId, { "entities" : qSketchRegion(id + "holesSketch"), "endBound" : BoundingType.THROUGH_ALL, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${part} });`, 'dir', 12, 4),
]);
batch.add('linear-pattern', 'a sheet metal plate (60 × 40, 1.5 thick) with a hole (radius 3) at (10, 10) patterned 3 along x at 20 and 2 along y at 20', [
  plate(60, 40),
  sketch('holeSketch', top(1.5)), circle('holeSketch', 'c1', [10, 10], 3), solve('holeSketch'), cut('hole'),
  dir('dirX', [10, 0]), dir('dirY', [0, 10]),
  linear('hole', cutFn('hole'), 'dirX', 20, 3, `"hasSecondDir" : true, "directionTwo" : qCreatedBy(id + "dirY", EntityType.EDGE), "distanceTwo" : 20 * millimeter, "instanceCountTwo" : 2, "oppositeDirectionTwo" : false, "isCenteredTwo" : false`),
]);
batch.add('linear-pattern', 'a sheet metal channel (a 30 base, 20 legs, 60 deep, 1.5 thick) with a hole (radius 2) through both legs patterned 3 along it at 15', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 20], [0, 0]), line('s1', 'l2', [0, 0], [30, 0]), line('s1', 'l3', [30, 0], [30, 20]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()',
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'true', depth: mm(60), symmetric: 'false', hasSecondDirection: 'false',
  }),
  sketch('holeSketch', planeAt([-10, 0, 0], [1, 0, 0], [0, 1, 0])), circle('holeSketch', 'c1', [15, 12], 2), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part }),
  dir('dir', [0, 10]),
  linear('hole', `extrude(context, instanceId, { "entities" : qSketchRegion(id + "holeSketch"), "endBound" : BoundingType.THROUGH_ALL, "operationType" : NewBodyOperationType.REMOVE, "defaultScope" : false, "booleanScope" : ${part} });`, 'dir', 15, 3),
]);
batch.add('sheet-metal-hem', 'a sheet metal bracket (a 40 × 30 plate, 1.5 thick, a flange 25 up on its far edge) with an obround slot (16 × 6) in the flange, the flange\'s top edge hemmed 5', [
  plate(40, 30),
  flange('flange1', edgeOfPart([20, 30, 1.5]), 25),
  sketch('slotSketch', planeAt([0, 40, 0], [0, -1, 0], [1, 0, 0])),
  line('slotSketch', 'l1', [15, 12], [25, 12]), arc('slotSketch', 'a1', [25, 12], [28, 15], [25, 18]), line('slotSketch', 'l2', [25, 18], [15, 18]), arc('slotSketch', 'a2', [15, 18], [12, 15], [15, 12]), solve('slotSketch'),
  call('extrude', 'slot', { entities: regions('slotSketch'), endBound: 'BoundingType.THROUGH_ALL', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part }),
  call('sheetMetalHem', 'hem1', {
    edges: `qClosestTo(qOwnedByBody(${part}, EntityType.EDGE), ${vec3(20, 31, 25)})`,
    oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE', hemCornerType: 'SMHemCornerType.SIMPLE', hemType: 'SMHemType.STRAIGHT', useMinimalGapForStraight: 'true', innerRadius: mm(1), length: mm(5),
  }),
]);
batch.add('sheet-metal-jog', 'a sheet metal plate (60 × 30, 1.5 thick) with a 6 mm jog along x = 30, a hole (radius 2.5) on each side of it', [
  plate(60, 30),
  sketch('bl', top(1.5)), line('bl', 'l1', [30, -5], [30, 35]), solve('bl'),
  call('sheetMetalJog', 'jog1', {
    bendReference: wireEdges('bl'), holdOtherSide: 'false', face: `qContainsPoint(qOwnedByBody(${part}, EntityType.FACE), ${vec3(15, 15, 1.5)})`, bendAlignment: 'BendAlignment.BEND_LINE',
    angleControlType: 'BendAngleControlType.BEND_ANGLE', oppositeAngle: 'false', useDefaultRadius: 'true', useDefaultKFactor: 'true', bendAngleSetProgrammatically: 'false',
    bendAngle: deg(90), offsetType: 'JogOffsetBoundingType.BLIND', bendOffset: mm(6), bendOffsetAnchor: 'JogOffsetAnchor.OUTSIDE', preserveMaterial: 'false',
  }),
  sketch('holesSketch', top(30)), circle('holesSketch', 'c1', [12, 15], 2.5), circle('holesSketch', 'c2', [50, 15], 2.5), solve('holesSketch'), cut('holes'),
]);
batch.add('circular-pattern', 'a sheet metal plate (50 × 50, 1.5 thick) with its four corners rounded 6 and a hole (radius 2) at 15 from its middle patterned 5 round it', [
  plate(50, 50),
  call('sheetMetalCornerBreak', 'break1', {
    blendType: 'EdgeBlendType.FILLET', entities: `qUnion([${[[0, 0], [50, 0], [50, 50], [0, 50]].map(([x, y]) => `qClosestTo(qOwnedByBody(${part}, EntityType.VERTEX), ${vec3(x, y, 1.5)})`).join(', ')}])`, radius: mm(6),
  }),
  sketch('holeSketch', top(1.5)), circle('holeSketch', 'c1', [40, 25], 2), solve('holeSketch'), cut('hole'),
  sketch('axis', planeAt([25, 25, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [0, 0], [10, 0]), solve('axis'),
  `circularPattern(context, id + "pattern1", { "patternType" : PatternType.FEATURE, "instanceFunction" : { (id + "hole") : function(instanceId) { ${cutFn('hole')} } } as FeatureList, "axis" : qCreatedBy(id + "axis", EntityType.EDGE), "angle" : 360 * degree, "instanceCount" : 5, "equalSpace" : true, "fullFeaturePattern" : false });`,
]);

batch.write('tools/fixtures/gen-batch-143.mjs');

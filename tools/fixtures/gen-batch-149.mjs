// Generates fixture batch 149: everyday sheet metal, a second round: a tray (a plate with flanges up all four edges,
// the long walls' top edges flanged again into lips), a Z bracket extruded from a sketch with a hole in its web and in
// each leg, a plate with a tab added and bent up along a line across the tab, a plate with a flange whose top edge is
// rolled, a plate with a flange whose top edge is hemmed as a tear drop.
//
// Usage: node tools/fixtures/gen-batch-149.mjs
//
// Writes fixtures/batch-149/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, call, circle, deg, line, mm, planeAt, rect, regions, sketch, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 149, title: 'everyday sheet metal, a second round: a lipped tray, a Z bracket with holes, a bent tab, a rolled hem, a tear-drop hem', helpers: '' });
const part = `qBodyType(qCreatedBy(id + "sm1", EntityType.BODY), BodyType.SOLID)`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const edgeOfPart = (p) => `qContainsPoint(qOwnedByBody(${part}, EntityType.EDGE), ${vec3(...p)})`;
const closestEdge = (p) => `qClosestTo(qOwnedByBody(${part}, EntityType.EDGE), ${vec3(...p)})`;
const faceOfPart = (p) => `qContainsPoint(qOwnedByBody(${part}, EntityType.FACE), ${vec3(...p)})`;
const plate = (w, h) => [
  sketch('s1', top(0)), rect('s1', 'rect1', [0, 0], [w, h]), solve('s1'),
  call('sheetMetalStart', 'sm1', { thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()' }),
];
const flange = (name, edges, distance) => call('sheetMetalFlange', name, {
  edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(distance), angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE',
  oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true', useDefaultRadius: 'true',
});
const cut = (name, opposite = 'true') => call('extrude', name, { entities: regions(name + 'Sketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: opposite, operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: part });
const hem = (fields) => call('sheetMetalHem', 'hem1', {
  edges: closestEdge([25, 41, 25]), oppositeDirection: 'false', hemAlignment: 'SMHemAlignment.IN_PLACE', hemCornerType: 'SMHemCornerType.SIMPLE', innerRadius: mm(1), ...fields,
});

batch.add('sheet-metal-flange', 'a tray: a sheet metal plate (80 × 50, 1.5 thick) with flanges 25 up all four edges, the long walls\' outer top edges flanged again 8 into lips', [
  plate(80, 50),
  flange('flange1', `qUnion([${[[40, 0], [80, 25], [40, 50], [0, 25]].map(([x, y]) => edgeOfPart([x, y, 1.5])).join(', ')}])`, 25),
  flange('flange2', `qUnion([${closestEdge([40, -1, 40])}, ${closestEdge([40, 51, 40])}])`, 8),
]);
batch.add('sheet-metal-start', 'a Z bracket: a sketch up 15, along 25 and up 15 extruded 40 as sheet metal (1.5 thick), a hole (radius 3) in its web and one (radius 2.5) in each leg', [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [0, 15]), line('s1', 'l2', [0, 15], [25, 15]), line('s1', 'l3', [25, 15], [25, 30]), solve('s1'),
  call('sheetMetalStart', 'sm1', {
    thickness: mm(1.5), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()',
    endBound: 'SMExtrudeBoundingType.BLIND', oppositeExtrudeDirection: 'true', depth: mm(40), symmetric: 'false', hasSecondDirection: 'false',
  }),
  sketch('webSketch', top(40)), circle('webSketch', 'c1', [12.5, 20], 3), solve('webSketch'), cut('web'),
  sketch('legSketch', planeAt([-10, 0, 0], [1, 0, 0], [0, 1, 0])), circle('legSketch', 'c1', [20, 7], 2.5), circle('legSketch', 'c2', [20, 23], 2.5), solve('legSketch'), cut('leg', 'false'),
]);
batch.add('sheet-metal-bend', 'a sheet metal plate (60 × 40, 1.5 thick) with a 20 × 15 tab added on its far edge, bent 90° up along a line across the tab at y = 45', [
  plate(60, 40),
  sketch('t1', top(1.5)), rect('t1', 'rect1', [20, 40], [40, 55]), solve('t1'),
  call('sheetMetalTab', 'tab1', { tabFaces: `qSketchRegion(id + "t1")`, booleanUnionScope: faceOfPart([30, 20, 1.5]), booleanOffset: mm(0) }),
  sketch('bl', top(1.5)), line('bl', 'l1', [15, 45], [45, 45]), solve('bl'),
  call('sheetMetalBend', 'bend1', {
    bendReference: wireEdges('bl'), holdOtherSide: 'false', face: faceOfPart([30, 20, 1.5]), bendAlignment: 'BendAlignment.BEND_LINE',
    angleControlType: 'BendAngleControlType.BEND_ANGLE', oppositeAngle: 'false', useDefaultRadius: 'true', useDefaultKFactor: 'true', bendAngle: deg(90),
  }),
]);
batch.add('sheet-metal-hem', 'a sheet metal plate (50 × 40, 1.5 thick) with a flange 25 up its far edge, the flange\'s top edge rolled 240° (inner radius 1)', [
  plate(50, 40), flange('flange1', edgeOfPart([25, 40, 1.5]), 25),
  hem({ hemType: 'SMHemType.ROLLED', angle: deg(240) }),
]);
batch.add('sheet-metal-hem', 'a sheet metal plate (50 × 40, 1.5 thick) with a flange 25 up its far edge, the flange\'s top edge hemmed as a tear drop 6 long (inner radius 1)', [
  plate(50, 40), flange('flange1', edgeOfPart([25, 40, 1.5]), 25),
  hem({ hemType: 'SMHemType.TEAR_DROP', length: mm(6), useMinimalGapForTipGap: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-149.mjs');

// Generates fixture batch 164: options the engine refused: revolves bounded up to something at both ends, a revolve's
// start bounded up to something in the end's direction, extrudes whose second direction runs up to a curved face or
// sheet, and sheet metal thickened from a face with tangent propagation (and without, to compare).
//
// Usage: node tools/fixtures/gen-batch-164.mjs
//
// Writes fixtures/batch-164/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, axisLine, box, call, circle, deg, edgeOf, faceAt, line, mm, planeAt, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 164, title: 'revolves bounded at both ends, extrudes up to curved faces in their second direction, sheet metal thickened with tangent propagation', helpers: '' });

// A 10 × 10 square in the plane y = 0, 10 to 20 out from the z axis, revolved about it (from +x toward +y).
const revolveProfile = [
  sketch('s1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), rect('s1', 'rect1', [10, -5], [20, 5]), solve('s1'),
  axisLine('axis1', [0, 0, -50], [0, 0, 50]),
];
const revolve = (extra) => call('revolve', 'revolve1', {
  entities: `qSketchRegion(id + "s1")`, axis: edgeOf('axis1'), fullRevolve: 'false', oppositeDirection: 'false', symmetric: 'false',
  hasStartBound: 'true', startBoundHasOffset: 'false', endBoundHasOffset: 'false', ...extra,
});
// Blocks whose faces y = 5 (above the profile's plane) and y = -5 (below) stop it.
const blocks = [box('above', [-40, 5, -20], [-30, 15, 20]), box('below', [-40, -15, -20], [-30, -5, 20])];

batch.add('revolve', 'up to a face one way and up to a face the other', [
  ...blocks, ...revolveProfile,
  revolve({ endBound: 'RevolveBoundingType.UP_TO_SURFACE', endBoundEntityFace: faceAt(solidsOf('above'), [-35, 5, 0]),
    startBound: 'RevolveBoundingType.UP_TO_SURFACE', startOppositeDirection: 'false', startBoundEntityFace: faceAt(solidsOf('below'), [-35, -5, 0]) }),
]);
batch.add('revolve', 'up to a vertex one way and up to a face the other', [
  ...blocks, ...revolveProfile,
  revolve({ endBound: 'RevolveBoundingType.UP_TO_VERTEX', endBoundEntityVertex: `qContainsPoint(qOwnedByBody(${solidsOf('above')}, EntityType.VERTEX), vector(-30, 5, 20) * millimeter)`,
    startBound: 'RevolveBoundingType.UP_TO_SURFACE', startOppositeDirection: 'false', startBoundEntityFace: faceAt(solidsOf('below'), [-35, -5, 0]) }),
]);
// A point 20 out at 20° and the revolve 60° forward from the profile, its start up to the point the same way round.
batch.add('revolve', '60 degrees one way, its start up to a point 20 degrees round the same way', [
  ...revolveProfile,
  sketch('pt', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])),
  `skPoint(pt, "p1", { "position" : vector(${(15 * Math.cos(Math.PI / 9)).toFixed(6)}, ${(15 * Math.sin(Math.PI / 9)).toFixed(6)}) * millimeter });`, solve('pt'),
  revolve({ endBound: 'RevolveBoundingType.BLIND', angle: deg(60),
    startBound: 'RevolveBoundingType.UP_TO_VERTEX', startOppositeDirection: 'true', startBoundEntityVertex: `qCreatedBy(id + "pt", EntityType.VERTEX)` }),
]);

// A cylinder lying along x under the top plane: radius 8, axis at z = -20.
const lying = [
  sketch('cylSketch', planeAt([-30, 0, 0], [1, 0, 0], [0, 1, 0])), circle('cylSketch', 'c1', [0, -20], 8), solve('cylSketch'),
  call('extrude', 'cyl', { entities: regions('cylSketch'), endBound: 'BoundingType.BLIND', depth: mm(60) }),
];
const top = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const extrude = (extra) => call('extrude', 'ext1', {
  entities: regions('prof'), endBound: 'BoundingType.BLIND', depth: mm(10), hasSecondDirection: 'true',
  secondDirectionOppositeDirection: 'true', ...extra,
});
batch.add('extrude', 'a circle 10 up, and down to a lying cylinder in its second direction', [
  ...lying, sketch('prof', top), circle('prof', 'c1', [5, 2], 4), solve('prof'),
  extrude({ secondDirectionBound: 'BoundingType.UP_TO_SURFACE', secondDirectionBoundEntityFace: faceAt(solidsOf('cyl'), [0, 0, -12]) }),
]);
// A curved sheet: an arc (radius 15, over x from -12 to 12, lowest at z = -15) extruded 30 along y.
batch.add('extrude', 'a rectangle 10 up, and down to a curved sheet in its second direction', [
  sketch('arcSketch', planeAt([0, -15, 0], [0, -1, 0], [1, 0, 0])), arc('arcSketch', 'a1', [-12, -6], [0, -15], [12, -6]), solve('arcSketch'),
  call('extrude', 'trough', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "arcSketch", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(30), oppositeDirection: 'true' }),
  sketch('prof', top), rect('prof', 'r1', [-6, -4], [6, 4]), solve('prof'),
  extrude({ secondDirectionBound: 'BoundingType.UP_TO_BODY', secondDirectionBoundEntityBody: `qBodyType(qCreatedBy(id + "trough", EntityType.BODY), BodyType.SHEET)` }),
]);

// A sheet of a 20 mm flat running on into a quarter turn of radius 10 (tangent), 20 deep along y.
const sheet = [
  sketch('lSketch', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), line('lSketch', 'l1', [0, 0], [20, 0]),
  arc('lSketch', 'a1', [20, 0], [20 + 10 * Math.sin(Math.PI / 4), 10 - 10 * Math.cos(Math.PI / 4)], [30, 10]), solve('lSketch'),
  call('extrude', 'sheet', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qCreatedBy(id + "lSketch", EntityType.EDGE)`, endBound: 'BoundingType.BLIND', depth: mm(20), oppositeDirection: 'true' }),
];
const thicken = (propagate) => call('sheetMetalStart', 'sm1', {
  thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', process: 'SMProcessType.THICKEN',
  regions: faceAt(`qBodyType(qCreatedBy(id + "sheet", EntityType.BODY), BodyType.SHEET)`, [10, 10, 0]), tangentPropagation: String(propagate), bends: 'qNothing()',
});
batch.add('sheet-metal', 'a flat running on into a tangent quarter turn, thickened from the flat with tangent propagation', [...sheet, thicken(true)]);
batch.add('sheet-metal', 'the same thickened from the flat alone', [...sheet, thicken(false)]);

batch.write('tools/fixtures/gen-batch-164.mjs');

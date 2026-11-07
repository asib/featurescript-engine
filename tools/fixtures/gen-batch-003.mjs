// Generates fixture batch 003: fillet, chamfer, shell, draft, delete face, move face, replace face and thicken.
//
// Usage: node tools/fixtures/gen-batch-003.mjs
//
// Writes fixtures/batch-003/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import {
  Batch, PLANES, arc, axisLine, box, call, circle, createdBy, cylinder, deg, edgeAt, edgeOf, faceAt, line, mm,
  planeAt, planeBody, polygon, rect, regions, sketch, solidsOf, solve, vec3, vertexAt,
} from './lib.mjs';

const batch = new Batch({
  number: 3,
  title: 'fillet, chamfer, shell, draft, delete face, move face, replace face, thicken',
});

const base = solidsOf('base');
const block = box('base', [0, 0, 0], [30, 20, 10]);
const union = (...queries) => `qUnion([${queries.join(', ')}])`;
// The block's edges: four vertical, four on top, four on the bottom.
const vertical = [[0, 0], [30, 0], [30, 20], [0, 20]].map(([x, y]) => edgeAt(base, [x, y, 5]));
const topEdges = [[15, 0], [30, 10], [15, 20], [0, 10]].map(([x, y]) => edgeAt(base, [x, y, 10]));
const allEdges = `qOwnedByBody(${base}, EntityType.EDGE)`;

// An L-shaped solid: the block with a second block on top of half of it, unioned.
const lShape = [
  box('base', [0, 0, 0], [30, 20, 10]),
  box('upper', [0, 0, 10], [15, 20, 25]),
  call('booleanBodies', 'union1', { operationType: 'BooleanOperationType.UNION', tools: union(base, solidsOf('upper')) }),
];
// A block with a round hole through it.
const holed = [
  box('base', [0, 0, 0], [30, 20, 10]),
  cylinder('tool', [15, 10], 5, -5, 15),
  call('booleanBodies', 'cut1', {
    operationType: 'BooleanOperationType.SUBTRACTION', targets: base, tools: solidsOf('tool'),
  }),
];
// A slot: two straight sides joined by semicircles, extruded 8 mm. Its top edges are tangent-connected.
const slot = [
  sketch('baseSketch', PLANES.xy),
  line('baseSketch', 'line1', [0, 0], [30, 0]),
  arc('baseSketch', 'arc1', [30, 0], [40, 10], [30, 20]),
  line('baseSketch', 'line2', [30, 20], [0, 20]),
  arc('baseSketch', 'arc2', [0, 20], [-10, 10], [0, 0]),
  solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(8) }),
];
const can = cylinder('base', [0, 0], 12, 0, 20);

// ----- fillet -----

const fillet = (description, setup, fields) =>
  batch.add('fillet', description, [setup, call('fillet', 'fillet1', fields)]);
fillet('one edge, 3 mm', block, { entities: vertical[0], radius: mm(3) });
fillet('four vertical edges, 4 mm', block, { entities: union(...vertical), radius: mm(4) });
fillet('every edge, 2 mm', block, { entities: allEdges, radius: mm(2) });
fillet('three edges meeting at a corner, 3 mm', block, {
  entities: union(vertical[2], topEdges[1], topEdges[2]), radius: mm(3),
});
fillet('a top edge and a vertical edge that meet, 2.5 mm', block, { entities: union(vertical[1], topEdges[0]), radius: mm(2.5) });
fillet('top edges, different from the bottom, 5 mm', block, { entities: union(...topEdges), radius: mm(5) });
fillet('a cylinder top edge, 3 mm', can, { entities: edgeAt(base, [12, 0, 20]), radius: mm(3) });
fillet('both cylinder edges, 2 mm', can, { entities: union(edgeAt(base, [12, 0, 20]), edgeAt(base, [12, 0, 0])), radius: mm(2) });
fillet('a concave edge of an L, 4 mm', lShape, { entities: edgeAt(base, [15, 10, 10]), radius: mm(4) });
fillet('every edge of an L, 1.5 mm', lShape, { entities: allEdges, radius: mm(1.5) });
fillet('the edge of a hole, 1.5 mm', holed, { entities: edgeAt(base, [20, 10, 10]), radius: mm(1.5) });
fillet('a slot edge with tangent propagation, 2 mm', slot, {
  entities: edgeAt(base, [15, 0, 8]), radius: mm(2), tangentPropagation: 'true',
});
fillet('conic cross section, rho 0.7', block, {
  entities: vertical[0], crossSection: 'FilletCrossSection.CONIC', nonCircularRadius: mm(4), rho: '0.7',
});
fillet('asymmetric, 2 and 5 mm', block, {
  entities: vertical[0], radius: mm(2), isAsymmetric: 'true', otherRadius: mm(5), flipAsymmetric: 'false',
});
fillet('a radius too big for the block: fails', block, { entities: union(...vertical), radius: mm(15) });

// ----- chamfer -----

const chamfer = (description, setup, fields) =>
  batch.add('chamfer', description, [setup, call('chamfer', 'chamfer1', fields)]);
chamfer('one edge, equal offsets 3 mm', block, { entities: vertical[0], chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(3) });
chamfer('one edge, two offsets 2 and 5 mm', block, {
  entities: topEdges[0], chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(5),
});
chamfer('two offsets, opposite', block, {
  entities: topEdges[0], chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(5), oppositeDirection: 'true',
});
chamfer('offset and angle, 3 mm at 30 degrees', block, {
  entities: topEdges[1], chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(3), angle: deg(30),
});
chamfer('every edge, 1.5 mm', block, { entities: allEdges, chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1.5) });
chamfer('a cylinder top edge, 2 mm', can, { entities: edgeAt(base, [12, 0, 20]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(2) });
chamfer('a concave edge of an L, 3 mm', lShape, { entities: edgeAt(base, [15, 10, 10]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(3) });
chamfer('a slot edge with tangent propagation, 1 mm', slot, {
  entities: edgeAt(base, [15, 0, 8]), chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(1), tangentPropagation: 'true',
});

// ----- shell -----

const shell = (description, setup, fields) =>
  batch.add('shell', description, [setup, call('shell', 'shell1', fields)]);
shell('remove the top face, 2 mm inwards', block, { entities: faceAt(base, [15, 10, 10]), thickness: mm(2) });
shell('remove the top face, 2 mm outwards', block, {
  entities: faceAt(base, [15, 10, 10]), thickness: mm(2), oppositeDirection: 'true',
});
shell('remove two faces, 1.5 mm', block, { entities: union(faceAt(base, [15, 10, 10]), faceAt(base, [30, 10, 5])), thickness: mm(1.5) });
shell('hollow, 2 mm', block, { isHollow: 'true', parts: base, thickness: mm(2) });
shell('a cylinder open at the top, 1 mm', can, { entities: faceAt(base, [0, 0, 20]), thickness: mm(1) });
shell('an L open at the top, 2 mm', lShape, { entities: faceAt(base, [7, 10, 25]), thickness: mm(2) });

// ----- draft -----

const draft = (description, setup, fields) =>
  batch.add('draft', description, [setup, call('draft', 'draft1', fields)]);
const sides = [[15, 0, 5], [30, 10, 5], [15, 20, 5], [0, 10, 5]].map((p) => faceAt(base, p));
draft('four sides 5 degrees from the bottom', block, {
  neutralPlane: faceAt(base, [15, 10, 0]), draftFaces: union(...sides), angle: deg(5),
});
draft('one side 10 degrees', block, { neutralPlane: faceAt(base, [15, 10, 0]), draftFaces: sides[1], angle: deg(10) });
draft('one side 10 degrees, pulled the other way', block, {
  neutralPlane: faceAt(base, [15, 10, 0]), draftFaces: sides[1], angle: deg(10), pullDirection: 'true',
});
draft('from the top face', block, { neutralPlane: faceAt(base, [15, 10, 10]), draftFaces: union(sides[0], sides[2]), angle: deg(7) });
draft('a cylinder 3 degrees', can, { neutralPlane: faceAt(base, [0, 0, 0]), draftFaces: faceAt(base, [12, 0, 10]), angle: deg(3) });

// ----- delete face -----

const deleteFace = (description, setup, fields) =>
  batch.add('delete-face', description, [setup, call('deleteFace', 'delete1', fields)]);
deleteFace('heal a hole', holed, { deleteFaces: faceAt(base, [20, 10, 5]) });
deleteFace('heal a fillet', [block, call('fillet', 'fillet1', { entities: vertical[0], radius: mm(4) })], {
  deleteFaces: `qCreatedBy(id + "fillet1", EntityType.FACE)`,
});
deleteFace('heal a chamfer', [block, call('chamfer', 'chamfer1', { entities: topEdges[1], chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(3) })], {
  deleteFaces: `qCreatedBy(id + "chamfer1", EntityType.FACE)`,
});
deleteFace('leave the top open: a sheet', block, { deleteFaces: faceAt(base, [15, 10, 10]), healType: 'DeleteFaceType.VOID' });
deleteFace('cap a hole face', holed, { deleteFaces: faceAt(base, [20, 10, 5]), healType: 'DeleteFaceType.CAP' });

// ----- move face -----

const moveFace = (description, setup, fields) =>
  batch.add('move-face', description, [setup, call('moveFace', 'move1', fields)]);
moveFace('offset the top 4 mm', block, { moveFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(4) });
moveFace('offset the top 3 mm inwards', block, {
  moveFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(3), oppositeDirection: 'true',
});
moveFace('offset a hole 2 mm', holed, { moveFaces: faceAt(base, [20, 10, 5]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(2) });
moveFace('offset two sides 1.5 mm', block, {
  moveFaces: union(sides[1], sides[2]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(1.5),
});
moveFace('translate a side 6 mm along x', [block, axisLine('dirX', [0, 0, 0], [1, 0, 0])], {
  moveFaces: sides[1], moveFaceType: 'MoveFaceType.TRANSLATE', direction: edgeOf('dirX'), translationDistance: mm(6),
});
moveFace('translate a hole 4 mm along y', [holed, axisLine('dirY', [0, 0, 0], [0, 1, 0])], {
  moveFaces: faceAt(base, [20, 10, 5]), moveFaceType: 'MoveFaceType.TRANSLATE', direction: edgeOf('dirY'), translationDistance: mm(4),
});
moveFace('rotate the top 10 degrees about an edge', block, {
  moveFaces: faceAt(base, [15, 10, 10]), moveFaceType: 'MoveFaceType.ROTATE', axis: topEdges[3], angle: deg(10),
});

// ----- replace face -----

batch.add('replace-face', 'replace the top with a higher plane', [
  block, planeBody('target', [15, 10, 16], [0, 0, 1], [1, 0, 0]),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: createdBy('target', 'FACE'), offset: mm(0) }),
]);
batch.add('replace-face', 'replace the top with a slanted plane', [
  block, planeBody('target', [15, 10, 12], [0.2, 0, 1], [1, 0, -0.2]),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: createdBy('target', 'FACE'), offset: mm(0) }),
]);
batch.add('replace-face', 'replace the top with a plane, offset 2 mm', [
  block, planeBody('target', [15, 10, 14], [0, 0, 1], [1, 0, 0]),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: createdBy('target', 'FACE'), offset: mm(2) }),
]);

// ----- thicken -----

// thicken has no default for thickness2 (or replaceFace for offset).
const thicken = (description, setup, fields) =>
  batch.add('thicken', description, [setup, call('thicken', 'thicken1', { thickness2: mm(0), ...fields })]);
thicken('the top face of a block, 3 mm', block, { entities: faceAt(base, [15, 10, 10]), thickness1: mm(3) });
thicken('the top face, opposite', block, { entities: faceAt(base, [15, 10, 10]), thickness1: mm(3), oppositeDirection: 'true' });
thicken('the top face, both ways 2 and 1 mm', block, { entities: faceAt(base, [15, 10, 10]), thickness1: mm(2), thickness2: mm(1) });
thicken('the top face, midplane 4 mm', block, { entities: faceAt(base, [15, 10, 10]), midplane: 'true', thickness: mm(4) });
thicken('a cylinder side, 2 mm', can, { entities: faceAt(base, [12, 0, 10]), thickness1: mm(2) });
thicken('the top face, added to the block', block, {
  entities: faceAt(base, [15, 10, 10]), thickness1: mm(3), operationType: 'NewBodyOperationType.ADD',
  defaultScope: 'false', booleanScope: base,
});

batch.write('tools/fixtures/gen-batch-003.mjs');

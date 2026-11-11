// Generates fixture batch 122: everyday variants of features batch 003 and 005 covered only plainly. Chamfers of a
// boss's foot, a hole's mouth and a cylinder's top edge (offset and angle, two offsets both ways) and round a block's
// filleted top; shells of a block with a hole through it and of one with chamfered top edges; drafts of a boss's
// side and a hole's wall; faces deleted and healed (a boss, a pocket, a cylinder's chamfer, a block's top fillets);
// enclosures of six planes and of a tube cut by three planes; a curved sheet and a sphere thickened; a cylinder
// split through its axis.
//
// Usage: node tools/fixtures/gen-batch-122.mjs
//
// Writes fixtures/batch-122/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, circle, cylinder, deg, faceAt, edgeAt, mm, planeAt, planeBody, rect, regions, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 122, title: 'everyday variants: chamfers, shells, drafts, faces deleted, enclosures, thickened and split', perRow: 10 });

const base = solidsOf('base');
const allBodies = (...names) => `qUnion([${names.map((n) => `qCreatedBy(id + "${n}", EntityType.BODY)`).join(', ')}])`;

// A boss (circle extruded and added to `base`) or a hole (cut out of it) from a sketch on the plane z.
const onBase = (op, name, [cx, cy], r, z, depth, flip = false) => {
  const s = `${name}Sketch`;
  return [
    sketch(s, planeAt([0, 0, z], [0, 0, 1], [1, 0, 0])),
    circle(s, 'circle1', [cx, cy], r),
    solve(s),
    call('extrude', name, {
      entities: regions(s), endBound: 'BoundingType.BLIND', depth: mm(depth), oppositeDirection: flip ? 'true' : 'false',
      operationType: `NewBodyOperationType.${op}`, defaultScope: 'false', booleanScope: base,
    }),
  ];
};
const boss = (name, c, r, z, depth) => onBase('ADD', name, c, r, z, depth);
const hole = (name, c, r, z, depth) => onBase('REMOVE', name, c, r, z, depth, true);
const chamfer = (edges, fields) => call('chamfer', 'chamfer1', { entities: edges, ...fields });
const equal = (w) => ({ chamferType: 'ChamferType.EQUAL_OFFSETS', width: mm(w) });
// The top edges of the 30 × 20 × 10 block.
const topEdges = `qUnion([${[[15, 0], [15, 20], [0, 10], [30, 10]].map(([x, y]) => edgeAt(base, [x, y, 10])).join(', ')}])`;

// chamfers
batch.add('chamfer', 'a 30 × 20 × 10 block with a boss (radius 5, 8 tall) on its top: the boss\'s foot (a concave circle) chamfered 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]), boss('boss', [15, 10], 5, 10, 8), chamfer(edgeAt(base, [20, 10, 10]), equal(1)),
]);
batch.add('chamfer', 'a 30 × 20 × 10 block with a hole (radius 4) through it: the hole\'s mouth chamfered 1 mm', [
  box('base', [0, 0, 0], [30, 20, 10]), hole('hole', [15, 10], 4, 10, 10), chamfer(edgeAt(base, [19, 10, 10]), equal(1)),
]);
batch.add('chamfer', 'a cylinder (radius 10, 20 tall)\'s top edge chamfered 2 mm at 30°', [
  cylinder('base', [0, 0], 10, 0, 20),
  chamfer(edgeAt(base, [10, 0, 20]), { chamferType: 'ChamferType.OFFSET_ANGLE', width: mm(2), angle: deg(30) }),
]);
batch.add('chamfer', 'a cylinder (radius 10, 20 tall)\'s top edge chamfered with offsets 2 and 1 mm', [
  cylinder('base', [0, 0], 10, 0, 20),
  chamfer(edgeAt(base, [10, 0, 20]), { chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(1) }),
]);
batch.add('chamfer', 'a cylinder (radius 10, 20 tall)\'s top edge chamfered with offsets 2 and 1 mm, opposite', [
  cylinder('base', [0, 0], 10, 0, 20),
  chamfer(edgeAt(base, [10, 0, 20]), { chamferType: 'ChamferType.TWO_OFFSETS', width1: mm(2), width2: mm(1), oppositeDirection: 'true' }),
]);
batch.add('chamfer', 'a 40 × 30 × 20 block with its upright edges filleted 5 mm: the top\'s edges chamfered 1 mm all round, from one edge with tangent propagation', [
  box('base', [0, 0, 0], [40, 30, 20]),
  call('fillet', 'fillet1', { entities: `qUnion([${[[0, 0], [40, 0], [40, 30], [0, 30]].map(([x, y]) => edgeAt(base, [x, y, 10])).join(', ')}])`, radius: mm(5) }),
  chamfer(edgeAt(base, [20, 0, 20]), { ...equal(1), tangentPropagation: 'true' }),
]);

// shells
batch.add('shell', 'a 30 × 20 × 10 block with a hole (radius 4) through it: its top removed, shelled 2 mm inwards', [
  box('base', [0, 0, 0], [30, 20, 10]), hole('hole', [15, 10], 4, 10, 10),
  call('shell', 'shell1', { entities: faceAt(base, [5, 5, 10]), thickness: mm(2) }),
]);
batch.add('shell', 'a 30 × 20 × 10 block with its top edges chamfered 2 mm: its top removed, shelled 1 mm inwards', [
  box('base', [0, 0, 0], [30, 20, 10]),
  chamfer(topEdges, equal(2)),
  call('shell', 'shell1', { entities: faceAt(base, [15, 10, 10]), thickness: mm(1) }),
]);

// drafts
batch.add('draft', 'a 30 × 20 × 10 block with a boss (radius 5, 8 tall): the boss\'s side drafted 5° from the block\'s top', [
  box('base', [0, 0, 0], [30, 20, 10]), boss('boss', [15, 10], 5, 10, 8),
  call('draft', 'draft1', { neutralPlane: faceAt(base, [2, 2, 10]), draftFaces: faceAt(base, [20, 10, 14]), angle: deg(5) }),
]);
batch.add('draft', 'a 30 × 20 × 10 block with a hole (radius 4) through it: the hole\'s wall drafted 3° from the bottom', [
  box('base', [0, 0, 0], [30, 20, 10]), hole('hole', [15, 10], 4, 10, 10),
  call('draft', 'draft1', { neutralPlane: faceAt(base, [2, 2, 0]), draftFaces: faceAt(base, [19, 10, 5]), angle: deg(3) }),
]);

// faces deleted
batch.add('delete-face', 'a 30 × 20 × 10 block with a boss (radius 5, 8 tall): the boss\'s side and top deleted and healed', [
  box('base', [0, 0, 0], [30, 20, 10]), boss('boss', [15, 10], 5, 10, 8),
  call('deleteFace', 'delete1', { deleteFaces: `qUnion([${faceAt(base, [20, 10, 14])}, ${faceAt(base, [15, 10, 18])}])` }),
]);
batch.add('delete-face', 'a 30 × 20 × 10 block with a 10 × 8 pocket 4 deep in its top: the pocket\'s walls and floor deleted and healed', [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('pocketSketch', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), rect('pocketSketch', 'rect1', [10, 6], [20, 14]), solve('pocketSketch'),
  call('extrude', 'pocket', {
    entities: regions('pocketSketch'), endBound: 'BoundingType.BLIND', depth: mm(4), oppositeDirection: 'true',
    operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base,
  }),
  call('deleteFace', 'delete1', { deleteFaces: `qCreatedBy(id + "pocket", EntityType.FACE)` }),
]);
batch.add('delete-face', 'a cylinder (radius 10, 20 tall) with its top edge chamfered 2 mm: the chamfer deleted and healed', [
  cylinder('base', [0, 0], 10, 0, 20), chamfer(edgeAt(base, [10, 0, 20]), equal(2)),
  call('deleteFace', 'delete1', { deleteFaces: `qCreatedBy(id + "chamfer1", EntityType.FACE)` }),
]);
batch.add('delete-face', 'a 30 × 20 × 10 block with its top edges filleted 3 mm all round: the fillets deleted and healed', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fillet', 'fillet1', { entities: topEdges, radius: mm(3) }),
  call('deleteFace', 'delete1', { deleteFaces: `qCreatedBy(id + "fillet1", EntityType.FACE)` }),
]);

// enclosures
batch.add('enclose', 'six planes boxing the region 20 × 15 × 10 enclosed', [
  planeBody('p1', [0, 0, 0], [1, 0, 0], [0, 1, 0]), planeBody('p2', [20, 0, 0], [1, 0, 0], [0, 1, 0]),
  planeBody('p3', [0, 0, 0], [0, 1, 0], [0, 0, 1]), planeBody('p4', [0, 15, 0], [0, 1, 0], [0, 0, 1]),
  planeBody('p5', [0, 0, 0], [0, 0, 1], [1, 0, 0]), planeBody('p6', [0, 0, 10], [0, 0, 1], [1, 0, 0]),
  call('enclose', 'enclose1', { entities: allBodies('p1', 'p2', 'p3', 'p4', 'p5', 'p6') }),
]);
batch.add('enclose', 'an open tube (radius 10, 30 tall) cut by planes at heights 5 and 25 and through its axis: the two halves enclosed', [
  sketch('tubeSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), circle('tubeSketch', 'circle1', [0, 0], 10), solve('tubeSketch'),
  call('extrude', 'tube', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qOwnedByBody(qBodyType(qCreatedBy(id + "tubeSketch", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`,
    endBound: 'BoundingType.BLIND', depth: mm(30),
  }),
  planeBody('p1', [0, 0, 5], [0, 0, 1], [1, 0, 0]), planeBody('p2', [0, 0, 25], [0, 0, 1], [1, 0, 0]),
  planeBody('p3', [0, 0, 0], [1, 0, 0], [0, 1, 0]),
  call('enclose', 'enclose1', { entities: allBodies('tube', 'p1', 'p2', 'p3') }),
]);

// thickened
batch.add('thicken', 'a sheet extruded 20 mm from a quarter circle of radius 20, thickened 1 and 2 mm either side', [
  sketch('arcSketch', planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0])), arc('arcSketch', 'arc1', [20, 0], [14.142136, 14.142136], [0, 20]), solve('arcSketch'),
  call('extrude', 'sheet', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: `qOwnedByBody(qBodyType(qCreatedBy(id + "arcSketch", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`,
    endBound: 'BoundingType.BLIND', depth: mm(20),
  }),
  call('thicken', 'thicken1', { entities: `qCreatedBy(id + "sheet", EntityType.BODY)`, thickness1: mm(1), thickness2: mm(2) }),
]);
batch.add('thicken', 'a sphere of radius 10\'s face thickened 2 mm outwards', [
  call('opSphere', 'base', { center: 'vector(0, 0, 0) * millimeter', radius: mm(10) }),
  call('thicken', 'thicken1', { entities: `qOwnedByBody(${base}, EntityType.FACE)`, thickness1: mm(2), thickness2: mm(0) }),
]);

// split
batch.add('split-part', 'a cylinder (radius 10, 20 tall) split by a plane through its axis', [
  cylinder('base', [0, 0], 10, 0, 20), planeBody('cut', [0, 0, 0], [1, 1, 0], [0, 0, 1]),
  call('splitPart', 'split1', { targets: base, tool: `qCreatedBy(id + "cut", EntityType.FACE)` }),
]);
batch.add('split-part', 'a sphere of radius 10 split by a plane 4 mm off its centre, keeping the front', [
  call('opSphere', 'base', { center: 'vector(0, 0, 0) * millimeter', radius: mm(10) }), planeBody('cut', [0, 0, 4], [0, 0, 1], [1, 0, 0]),
  call('splitPart', 'split1', { targets: base, tool: `qCreatedBy(id + "cut", EntityType.FACE)`, keepBothSides: 'false', keepFront: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-122.mjs');

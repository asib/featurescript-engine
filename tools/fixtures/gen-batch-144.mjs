// Generates fixture batch 144: everyday surfacing and direct edits: a vase (a fit spline revolved, shelled open at its
// top), a cable (a circle swept along a 3D fit spline), an arched sheet lofted between arcs and thickened, a block
// lengthened by moving a face and its edges filleted, a filleted block with its fillet deleted, a block's top replaced
// by an offset plane, a block's face thickened onto it, a drafted extrude with its top edges filleted.
//
// Usage: node tools/fixtures/gen-batch-144.mjs
//
// Writes fixtures/batch-144/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, PLANES, box, call, circle, deg, edgeAt, faceAt, line, mm, planeAt, regions, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const batch = new Batch({ number: 144, title: 'everyday surfacing and direct edits: a vase, a cable, a thickened arch, moved, deleted and replaced faces, a thickened face, a drafted extrude', helpers: '' });
const base = solidsOf('base');
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const scoped = { defaultScope: 'false', booleanScope: base };
const top = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);

batch.add('revolve', 'a vase: a fit spline through (10, 0), (14, 10), (9, 25), (11, 35) closed by lines to the axis, revolved, shelled 1.2 open at its top', [
  sketch('profile', PLANES.xz),
  `skFitSpline(profile, "spline1", { "points" : [vector(10, 0) * millimeter, vector(14, 10) * millimeter, vector(9, 25) * millimeter, vector(11, 35) * millimeter] });`,
  line('profile', 'l1', [11, 35], [0, 35]), line('profile', 'l2', [0, 35], [0, 0]), line('profile', 'l3', [0, 0], [10, 0]), solve('profile'),
  sketch('axis', planeAt([0, 0, 0], [0, 1, 0], [0, 0, 1])), line('axis', 'line1', [-100, 0], [100, 0]), solve('axis'),
  call('revolve', 'base', { entities: regions('profile'), axis: `qCreatedBy(id + "axis", EntityType.EDGE)`, fullRevolve: 'true' }),
  call('shell', 'shell1', { entities: faceAt(base, [0, 0, 35]), thickness: mm(1.2) }),
]);
batch.add('sweep', 'a cable: a circle (radius 1.5) swept along a fit spline through (0, 0, 0), (15, 8, 4), (30, 0, 10), (45, -6, 6)', [
  call('opFitSpline', 'path', { points: `[${vec3(0, 0, 0)}, ${vec3(15, 8, 4)}, ${vec3(30, 0, 10)}, ${vec3(45, -6, 6)}]` }),
  `const pathStart = evEdgeTangentLine(context, { "edge" : qCreatedBy(id + "path", EntityType.EDGE), "parameter" : 0 });`,
  `const profile = newSketchOnPlane(context, id + "profile", { "sketchPlane" : plane(pathStart.origin, pathStart.direction, perpendicularVector(pathStart.direction)) });`,
  `skCircle(profile, "c1", { "center" : vector(0, 0) * millimeter, "radius" : 1.5 * millimeter });`, solve('profile'),
  call('sweep', 'base', { profiles: regions('profile'), path: `qCreatedBy(id + "path", EntityType.EDGE)` }),
]);
batch.add('thicken', 'an arched sheet lofted between an arc at y = 0 and one at y = 30 (both 40 across, rising 8), thickened 2', [
  sketch('a1', planeAt([0, 0, 0], [0, -1, 0], [1, 0, 0])), `skArc(a1, "arc1", { "start" : vector(0, 0) * millimeter, "mid" : vector(20, 8) * millimeter, "end" : vector(40, 0) * millimeter });`, solve('a1'),
  sketch('a2', planeAt([0, 30, 0], [0, -1, 0], [1, 0, 0])), `skArc(a2, "arc1", { "start" : vector(0, 0) * millimeter, "mid" : vector(20, 8) * millimeter, "end" : vector(40, 0) * millimeter });`, solve('a2'),
  call('loft', 'sheet', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW', wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('a1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('a2')}]) }]` }),
  call('thicken', 'base', { entities: `qCreatedBy(id + "sheet", EntityType.FACE)`, thickness1: mm(2), thickness2: mm(0) }),
]);
batch.add('move-face', 'a 30 × 20 × 10 block lengthened 8 by moving its end face, then its top edges filleted 2', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('moveFace', 'move1', { moveFaces: faceAt(base, [30, 10, 5]), moveFaceType: 'MoveFaceType.OFFSET', offsetDistance: mm(8) }),
  call('fillet', 'fillet1', { entities: `qUnion([${edgeAt(base, [19, 0, 10])}, ${edgeAt(base, [19, 20, 10])}])`, radius: mm(2) }),
]);
batch.add('delete-face', 'a 30 × 20 × 10 block with its top front edge filleted 4, the fillet then deleted', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('fillet', 'fillet1', { entities: edgeAt(base, [15, 0, 10]), radius: mm(4) }),
  call('deleteFace', 'delete1', { deleteFaces: `qCreatedBy(id + "fillet1", EntityType.FACE)` }),
]);
batch.add('replace-face', 'a 30 × 20 × 10 block with a 10 × 10 boss 5 tall on its top, the block\'s top replaced by a plane 3 above it', [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('bossSketch', top(10)), `skRectangle(bossSketch, "r1", { "firstCorner" : vector(10, 5) * millimeter, "secondCorner" : vector(20, 15) * millimeter });`, solve('bossSketch'),
  call('extrude', 'boss', { entities: regions('bossSketch'), endBound: 'BoundingType.BLIND', depth: mm(5), operationType: 'NewBodyOperationType.ADD', ...scoped }),
  call('opPlane', 'target', { plane: `plane(${vec3(0, 0, 13)}, vector(0, 0, 1))`, width: mm(100), height: mm(100) }),
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [2, 2, 10]), templateFace: `qCreatedBy(id + "target", EntityType.FACE)`, offset: mm(0) }),
]);
batch.add('thicken', 'a 30 × 20 × 10 block with its front face thickened 3 out and added to it', [
  box('base', [0, 0, 0], [30, 20, 10]),
  call('thicken', 'thick1', { entities: faceAt(base, [15, 0, 5]), thickness1: mm(3), thickness2: mm(0), operationType: 'NewBodyOperationType.ADD', ...scoped }),
]);
batch.add('extrude', 'a circle (radius 12) extruded 15 with a 5° draft inwards, its top edge filleted 2 and a hole (radius 4) through it', [
  sketch('baseSketch', top(0)), circle('baseSketch', 'c1', [0, 0], 12), solve('baseSketch'),
  call('extrude', 'base', { entities: regions('baseSketch'), endBound: 'BoundingType.BLIND', depth: mm(15), hasDraft: 'true', draftAngle: deg(5), draftPullDirection: 'false' }),
  call('fillet', 'fillet1', { entities: `qNthElement(qSubtraction(qOwnedByBody(${base}, EntityType.EDGE), qContainsPoint(qOwnedByBody(${base}, EntityType.EDGE), ${vec3(12, 0, 0)})), 0)`, radius: mm(2) }),
  sketch('holeSketch', top(15)), circle('holeSketch', 'c1', [0, 0], 4), solve('holeSketch'),
  call('extrude', 'hole', { entities: regions('holeSketch'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', ...scoped }),
]);

batch.write('tools/fixtures/gen-batch-144.mjs');

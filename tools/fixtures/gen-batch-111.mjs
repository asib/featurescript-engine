// Generates fixture batch 111: why Onshape won't replace a block's top up to a sheet lofted between arcs (batches 067
// c005, 108 c009, c012) though it will up to an extruded arc (batch 020) and a loft between lines (108 c010). Batch
// 006's STEP shows a loft between equal arcs is a cylinder in Onshape, as an extruded arc is: so the same arc
// extruded with the loft's extent, the loft's arcs drawn the other way, the loft wider than the block by 5 mm, and
// batch 020's extruded arc again.
//
// Usage: node tools/fixtures/gen-batch-111.mjs
//
// Writes fixtures/batch-111/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, faceAt, mm, planeAt, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 111, title: 'faces replaced up to cylinders lofted and extruded' });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const replace = [
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: `qOwnedByBody(${sheetsOf('sheet')}, EntityType.FACE)`, offset: mm(0) }),
  returns({
    volume: `roundToPrecision(evVolume(context, { "entities" : ${base} }) / millimeter ^ 3, 3)`,
    normal: `mapArray([0, 1, 2], function(i) { return roundToPrecision(evFaceTangentPlane(context, { "face" : qOwnedByBody(${sheetsOf('sheet')}, EntityType.FACE), "parameter" : vector(0.5, 0.5) }).normal[i], 6); })`,
  }),
];
const lofted = (y0, y1, a, b) => [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('a1', planeAt([0, y0, 0], [0, -1, 0], [1, 0, 0])), a('a1'), solve('a1'),
  sketch('a2', planeAt([0, y1, 0], [0, -1, 0], [1, 0, 0])), b('a2'), solve('a2'),
  call('loft', 'sheet', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('a1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('a2')}]) }]`,
  }),
  replace,
];
const extruded = (y0, depth, a) => [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('a1', planeAt([0, y0, 0], [0, -1, 0], [1, 0, 0])), a('a1'), solve('a1'),
  call('extrude', 'sheet', { bodyType: 'ExtendedToolBodyType.SURFACE', surfaceEntities: wireEdges('a1'), endBound: 'BoundingType.BLIND', depth: mm(depth) }),
  replace,
];
const forward = (s) => arc(s, 'arc1', [-2, 12], [15, 16], [32, 12]);
const backward = (s) => arc(s, 'arc1', [32, 12], [15, 16], [-2, 12]);
const wide = (s) => arc(s, 'arc1', [-5, 12], [15, 16], [35, 12]);

batch.add('replace-face', "a block's top replaced up to the arc of batch 108 c009 extruded from y = -1 to 21 (a cylinder as the loft is)", extruded(-1, 22, forward));
batch.add('replace-face', "up to batch 108 c009's loft between equal arcs, the arcs drawn the other way", lofted(-1, 21, backward, backward));
batch.add('replace-face', "up to a loft between equal arcs 5 mm wider than the block every way (x -5 to 35, y -5 to 25)", lofted(-5, 25, wide, wide));
batch.add('replace-face', "up to batch 108 c009's loft between equal arcs, again", lofted(-1, 21, forward, forward));
batch.add('replace-face', "up to batch 020's curved sheet: an arc extruded 30 mm from y = -5", extruded(-5, 30, (s) => arc(s, 'arc1', [-5, 11], [15, 14], [35, 11])));

batch.write('tools/fixtures/gen-batch-111.mjs');

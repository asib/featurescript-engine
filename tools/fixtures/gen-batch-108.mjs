// Generates fixture batch 108: when Onshape refuses what the engine builds: batch 061 c011's loft between triangles
// turned to each other, tangent to the start at magnitude 0.5 (`LOFT_FAILED`), varied (no condition, other
// magnitudes, tangent to the end, triangles not turned, normal to the profile, squares turned); and batch 067
// c005's top replaced up to a sheet lofted between arcs (`DIRECT_EDIT_REPLACE_FACE_FAILED`), varied (between equal
// arcs, between lines at different heights, and the arcs' sheet extended past the block first).
//
// Usage: node tools/fixtures/gen-batch-108.mjs
//
// Writes fixtures/batch-108/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, box, call, faceAt, line, mm, planeAt, polygon, regions, sketch, solidsOf, solve } from './lib.mjs';

const batch = new Batch({ number: 108, title: 'lofts and face replacements Onshape refuses, varied' });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
const triangle = (s) => polygon(s, [[10, 0], [-5, 8.660254], [-5, -8.660254]]);
const turned = (s) => polygon(s, [[4, 3], [-5, 4], [0, -5]]);
const smaller = (s) => polygon(s, [[5, 0], [-2.5, 4.330127], [-2.5, -4.330127]]);
const square = (s) => polygon(s, [[10, 10], [-10, 10], [-10, -10], [10, -10]]);
const turnedSquare = (s) => polygon(s, [[6, 2], [-2, 6], [-6, -2], [2, -6]]);
const loftBody = 'qCreatedBy(id + "loft1", EntityType.BODY)';
const loft = (bottom, top, conditions) => [
  sketch('s1', at(0)), bottom('s1'), solve('s1'), sketch('s2', at(20)), top('s2'), solve('s2'),
  call('loft', 'loft1', {
    sheetProfilesArray: `[{ "sheetProfileEntities" : ${regions('s1')} }, { "sheetProfileEntities" : ${regions('s2')} }]`,
    ...conditions,
  }),
  returns({ faces: `size(evaluateQuery(context, qOwnedByBody(${loftBody}, EntityType.FACE)))` }),
];
const tangent = (end, magnitude) => ({ [`${end}Condition`]: 'LoftEndDerivativeType.TANGENT_TO_PROFILE', [`${end}Magnitude`]: magnitude });

batch.add('loft', 'triangles turned to each other, no end condition', loft(triangle, turned, {}));
batch.add('loft', 'triangles turned to each other, tangent to the start, magnitude 1', loft(triangle, turned, tangent('start', '1')));
batch.add('loft', 'triangles turned to each other, tangent to the start, magnitude 0.25', loft(triangle, turned, tangent('start', '0.25')));
batch.add('loft', 'triangles turned to each other, tangent to the start, magnitude 2', loft(triangle, turned, tangent('start', '2')));
batch.add('loft', 'triangles turned to each other, tangent to the end, magnitude 0.5', loft(triangle, turned, tangent('end', '0.5')));
batch.add('loft', 'a triangle to a smaller one not turned, tangent to the start, magnitude 0.5', loft(triangle, smaller, tangent('start', '0.5')));
batch.add('loft', 'triangles turned to each other, normal to the start, magnitude 0.5', loft(triangle, turned, { startCondition: 'LoftEndDerivativeType.NORMAL_TO_PROFILE', startMagnitude: '0.5' }));
batch.add('loft', 'a square to a smaller turned one, tangent to the start, magnitude 0.5', loft(square, turnedSquare, tangent('start', '0.5')));

const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const sheetsOf = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SHEET)`;
const base = solidsOf('base');
const replaced = (profile1, profile2, extra = []) => [
  box('base', [0, 0, 0], [30, 20, 10]),
  sketch('a1', planeAt([0, -1, 0], [0, -1, 0], [1, 0, 0])), profile1('a1'), solve('a1'),
  sketch('a2', planeAt([0, 21, 0], [0, -1, 0], [1, 0, 0])), profile2('a2'), solve('a2'),
  call('loft', 'sheet', {
    bodyType: 'ExtendedToolBodyType.SURFACE', surfaceOperationType: 'NewSurfaceOperationType.NEW',
    wireProfilesArray: `[{ "wireProfileEntities" : qUnion([${wireEdges('a1')}]) }, { "wireProfileEntities" : qUnion([${wireEdges('a2')}]) }]`,
  }),
  extra,
  call('replaceFace', 'replace1', { replaceFaces: faceAt(base, [15, 10, 10]), templateFace: `qOwnedByBody(${sheetsOf('sheet')}, EntityType.FACE)`, offset: mm(0) }),
  returns({ volume: `roundToPrecision(evVolume(context, { "entities" : ${base} }) / millimeter ^ 3, 3)` }),
];
batch.add('replace-face', "a block's top replaced up to a sheet lofted between equal arcs above it", replaced(
  (s) => arc(s, 'arc1', [-2, 12], [15, 16], [32, 12]), (s) => arc(s, 'arc1', [-2, 12], [15, 16], [32, 12])));
batch.add('replace-face', "a block's top replaced up to a sheet lofted between lines at different heights above it", replaced(
  (s) => line(s, 'l1', [-2, 12], [32, 12]), (s) => line(s, 'l1', [-2, 14], [32, 16])));
batch.add('replace-face', "a block's top replaced up to batch 067 c005's sheet lofted between arcs (16 and 18 high), extended 5 mm first", replaced(
  (s) => arc(s, 'arc1', [-2, 12], [15, 16], [32, 12]), (s) => arc(s, 'arc1', [-2, 12], [15, 18], [32, 12]),
  [call('extendSurface', 'extend1', { entities: `qOwnedByBody(${sheetsOf('sheet')}, EntityType.EDGE)`, extendDistance: mm(5) })]));
batch.add('replace-face', "a block's top replaced up to batch 067 c005's sheet, as it was", replaced(
  (s) => arc(s, 'arc1', [-2, 12], [15, 16], [32, 12]), (s) => arc(s, 'arc1', [-2, 12], [15, 18], [32, 12])));

batch.write('tools/fixtures/gen-batch-108.mjs');

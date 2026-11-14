// Generates fixture batch 169: lofts whose profiles are more than one face (which the engine refused): a 20 × 10
// rectangle split in two by a line across it, both regions taken as one profile, lofted to a circle, to a rectangle,
// and to another rectangle split the same way.
//
// Usage: node tools/fixtures/gen-batch-169.mjs
//
// Writes fixtures/batch-169/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, call, circle, line, planeAt, rect, regions, sketch, solve } from './lib.mjs';

const batch = new Batch({ number: 169, title: 'lofts whose profiles are more than one face', helpers: '' });
const at = (z) => planeAt([0, 0, z], [0, 0, 1], [1, 0, 0]);
// A 20 × 10 rectangle centred on the axis, split at x = 0 (or x = 3) by a line across it.
const split = (name, z, x = 0) => [sketch(name, at(z)), rect(name, 'r1', [-10, -5], [10, 5]), line(name, 'l1', [x, -5], [x, 5]), solve(name)];
const loft = (profiles) => call('loft', 'loft1', {
  sheetProfilesArray: `[${profiles.map((p) => `{ "sheetProfileEntities" : ${p} }`).join(', ')}]`, bodyType: 'ExtendedToolBodyType.SOLID',
  operationType: 'NewBodyOperationType.NEW', startCondition: 'LoftEndDerivativeType.DEFAULT', endCondition: 'LoftEndDerivativeType.DEFAULT',
});

batch.add('loft', 'a rectangle split in two, both regions one profile, lofted to a circle (radius 6) 20 up', [
  ...split('a', 0), sketch('b', at(20)), circle('b', 'c1', [0, 0], 6), solve('b'), loft([regions('a'), regions('b')]),
]);
batch.add('loft', 'the split rectangle lofted to a 20 × 10 rectangle 15 up', [
  ...split('a', 0), sketch('b', at(15)), rect('b', 'r1', [-10, -5], [10, 5]), solve('b'), loft([regions('a'), regions('b')]),
]);
batch.add('loft', 'the split rectangle lofted to one split at x = 3, 15 up', [
  ...split('a', 0), ...split('b', 15, 3), loft([regions('a'), regions('b')]),
]);

batch.write('tools/fixtures/gen-batch-169.mjs');

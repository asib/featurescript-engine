// Generates fixture batch 048: sweeps twisted and scaled along curved paths (batch 019 covered straight ones): a
// rectangle along a quarter circle twisted a quarter turn, a circle along it scaled to twice its size, and a
// rectangle along it both twisted and scaled.
//
// Usage: node tools/fixtures/gen-batch-048.mjs
//
// Writes fixtures/batch-048/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, arc, call, circle, deg, rect, sketch, solve, PLANES } from './lib.mjs';

const batch = new Batch({ number: 48, title: 'sweeps twisted and scaled along curved paths' });

const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const regions = (s) => `qSketchRegion(id + "${s}")`;
// A quarter circle of radius 30 about (0, 30) in the xy plane, from the origin (leaving along x) to (30, 30).
const arcPath = [sketch('path', PLANES.xy), arc('path', 'a1', [0, 0], [21.213203435596427, 8.786796564403573], [30, 30]), solve('path')];
const rectProfile = [sketch('profile', PLANES.yz), rect('profile', 'rect1', [-3, -2], [3, 2]), solve('profile')];

batch.add('sweep', 'a rectangle along a quarter circle, twisted a quarter turn', [
  rectProfile, arcPath,
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true' }),
]);
batch.add('sweep', 'a circle along a quarter circle, scaled to twice its size', [
  sketch('profile', PLANES.yz), circle('profile', 'circle1', [0, 0], 3), solve('profile'), arcPath,
  call('sweep', 'sweep1', { profiles: regions('profile'), path: wireEdges('path'), hasScale: 'true', scaleFactor: '2' }),
]);
batch.add('sweep', 'a rectangle along a quarter circle, twisted a quarter turn and scaled to half its size', [
  rectProfile, arcPath,
  call('sweep', 'sweep1', {
    profiles: regions('profile'), path: wireEdges('path'), hasTwist: 'true', twistType: 'SweepTwistType.ANGLE', angle: deg(90), ccw: 'true',
    hasScale: 'true', scaleFactor: '0.5',
  }),
]);

batch.write('tools/fixtures/gen-batch-048.mjs');

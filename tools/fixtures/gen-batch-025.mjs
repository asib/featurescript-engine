// Generates fixture batch 025: section parts as drawings make them, with a jogged section line and several planes.
//
// A section part deletes every body it isn't given, and the cases share one Part Studio, so each case sections every
// solid there is, cutting so that the earlier cases (moved to their slots along +x by then) lie wholly behind its
// cut. The first case only holds slot 0, where each case is built.
//
// Usage: node tools/fixtures/gen-batch-025.mjs
//
// Writes fixtures/batch-025/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, cylinder, vec3 } from './lib.mjs';

const batch = new Batch({ number: 25, title: 'section parts: jogged section lines and several planes' });

const solids = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';
const block = [box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [8, 22], 4, 20, 30)];
const plane = (origin, normal, x) => `plane(${vec3(...origin)}, vector(${normal.join(', ')}), vector(${x.join(', ')}))`;
// The side of the jog line that's removed lies towards −x with the sketch's x along +x.
const jogPlane = plane([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const jog = `[${[[25, 35, 10], [25, 15, 10], [15, 15, 10], [15, -5, 10]].map((p) => vec3(...p)).join(', ')}]`;

batch.add('section', 'nothing: holds slot 0, where the other cases are built', ['return { "slot" : 0 };']);
batch.add('section', 'a block and pin cut along a jogged line', [
  block,
  call('jogSectionPart', 'section1', { target: solids, sketchPlane: jogPlane, jogPoints: jog }),
]);
batch.add('section', 'a block and pin cut by two planes: x < 15, and y − x > −15', [
  block,
  call('multiplePlaneSectionPart', 'section1', {
    target: solids, sketchPlanes: `[${plane([15, 0, 0], [-1, 0, 0], [0, 1, 0])}, ${plane([30, 15, 0], [-0.7071067811865476, 0.7071067811865476, 0], [0, 0, 1])}]`,
  }),
]);
batch.add('section', 'a block and pin cut along the jogged line as a partial section', [
  block,
  call('jogSectionPart', 'section1', { target: solids, sketchPlane: jogPlane, jogPoints: jog, isPartialSection: 'true' }),
]);

// A broken-out section: material inside a closed spline through four points on the top, from the depth point's
// height (z = 10) up, is removed.
batch.add('section', 'a block and pin broken out inside a closed spline down to z = 10', [
  block,
  call('jogSectionPart', 'section1', {
    target: solids, sketchPlane: jogPlane, isBrokenOut: 'true', brokenOutPointNumbers: '[4]',
    jogPoints: `[${[[25, 8, 20], [35, 14, 20], [31, 25, 20], [20, 18, 20], [30, 15, 10]].map((p) => vec3(...p)).join(', ')}]`,
  }),
]);

batch.write('tools/fixtures/gen-batch-025.mjs');

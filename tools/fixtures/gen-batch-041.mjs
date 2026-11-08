// Generates fixture batch 041: the assembly forms of the section parts drawings make (`sectionTransformedParts`,
// `multiplePlanesSectionTransformedParts`), which section copies of parts at instance transforms and move the
// results back.
//
// As in batches 025 and 037, the cases share one Part Studio and each cut removes the −x side of slot 0, where each
// case is built, behind which the earlier cases (moved to their slots along +x by then) lie. The first case only
// holds slot 0.
//
// Usage: node tools/fixtures/gen-batch-041.mjs
//
// Writes fixtures/batch-041/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, cylinder, vec3 } from './lib.mjs';

const batch = new Batch({ number: 41, title: 'section parts for assemblies: transformed parts, several planes' });

const solids = 'qBodyType(qCreatedBy(id + "base", EntityType.BODY), BodyType.SOLID)';
const pin = 'qBodyType(qCreatedBy(id + "pin", EntityType.BODY), BodyType.SOLID)';
const block = [box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [8, 22], 4, 20, 30)];
const plane = (origin, normal, x) => `plane(${vec3(...origin)}, vector(${normal.join(', ')}), vector(${x.join(', ')}))`;
const top = plane([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const points = (list) => `[${list.map((p) => vec3(...p)).join(', ')}]`;
const target = (part, instances) =>
  `sectionTarget([${instances.map((_, i) => `"i${i + 1}"`).join(', ')}], ${part}, [${instances.join(', ')}])`;
const identity = 'identityTransform()';
const moved = (x, y, z) => `transform(${vec3(x, y, z)})`;

batch.add('section', 'nothing: holds slot 0, where the other cases are built', ['return { "slot" : 0 };']);
batch.add('section', 'a block at two instances, the second 40 mm along y, cut along x = 20', [
  block,
  `sectionTransformedParts(context, id + "section1", { "targets" : [${target(solids, [identity, moved(0, 40, 0)])}], "sketchPlane" : ${top}, "jogPoints" : ${points([[20, 80, 10], [20, -5, 10]])} });`,
]);
batch.add('section', 'a block and its pin as two targets, the pin at two instances, cut along a jogged line', [
  block,
  `sectionTransformedParts(context, id + "section1", { "targets" : [${target(solids, [identity])}, ${target(pin, [identity, moved(0, -15, 0)])}], "sketchPlane" : ${top}, "jogPoints" : ${points([[25, 35, 10], [25, 15, 10], [15, 15, 10], [15, -15, 10]])} });`,
]);
// Each plane of a multiple planes section cuts square to its x axis, through its origin (its normal is the way it's
// viewed).
batch.add('section', 'a block at two instances cut by two planes: square to x at x = 15, and square to (−1, 1) through (30, 15)', [
  block,
  `multiplePlanesSectionTransformedParts(context, id + "section1", { "targets" : [${target(solids, [identity, moved(0, 40, 0)])}], "sketchPlanes" : [${plane([15, 0, 0], [0, 0, 1], [-1, 0, 0])}, ${plane([30, 15, 0], [0, 0, 1], [-0.7071067811865476, 0.7071067811865476, 0])}], "bbox" : box3d(${vec3(-10, -10, -10)}, ${vec3(60, 90, 40)}) });`,
]);

batch.write('tools/fixtures/gen-batch-041.mjs');

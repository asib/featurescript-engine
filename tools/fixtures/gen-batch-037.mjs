// Generates fixture batch 037: the section parts drawings make for crop views, offset sections and aligned sections.
//
// As in batch 025, a section part deletes every body it isn't given and the cases share one Part Studio, so each case
// sections every solid there is: the crop view comes first, when only its own block is there, and the later cuts
// remove the −x side, behind which the earlier cases (moved to their slots along +x by then) lie. The first case
// only holds slot 0, where each case is built.
//
// Usage: node tools/fixtures/gen-batch-037.mjs
//
// Writes fixtures/batch-037/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, cylinder, vec3 } from './lib.mjs';

const batch = new Batch({ number: 37, title: 'section parts: crop views, offset sections and aligned sections' });

const solids = 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)';
const block = [box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [8, 22], 4, 20, 30)];
const plane = (origin, normal, x) => `plane(${vec3(...origin)}, vector(${normal.join(', ')}), vector(${x.join(', ')}))`;
const top = plane([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const points = (list) => `[${list.map((p) => vec3(...p)).join(', ')}]`;

batch.add('section', 'nothing: holds slot 0, where the other cases are built', ['return { "slot" : 0 };']);
batch.add('section', 'a block and pin cropped to a closed spline through four points on the top', [
  block,
  call('jogSectionPart', 'section1', {
    target: solids, sketchPlane: top, isCropView: 'true', brokenOutPointNumbers: '[4]',
    jogPoints: points([[25, 8, 20], [35, 14, 20], [31, 25, 20], [20, 18, 20], [30, 15, 10]]),
  }),
]);
batch.add('section', 'a block and pin cut along x = 20, offset 8 mm to a point at x = 28', [
  block,
  call('jogSectionPart', 'section1', {
    target: solids, sketchPlane: top, jogPoints: points([[20, 35, 10], [20, -5, 10]]), offsetPoints: points([[28, 15, 10]]),
  }),
]);
batch.add('section', 'a block and pin cut along a line bent at (20, 15), as an aligned section', [
  block,
  call('jogSectionPart', 'section1', {
    target: solids, sketchPlane: top, jogPoints: points([[20, 35, 10], [20, 15, 10], [35, -5, 10]]), isAlignedSection: 'true',
  }),
  // Every sheet left in the Part Studio: its area, whether it's construction or a sketch's, and whether this case
  // made it.
  `return { "sheets" : mapArray(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.SHEET)), function(s) { return [roundToPrecision(evArea(context, { "entities" : s }) / millimeter ^ 2, 3), !isQueryEmpty(context, qConstructionFilter(s, ConstructionObject.YES)), !isQueryEmpty(context, qSketchFilter(s, SketchObject.YES)), !isQueryEmpty(context, qIntersection([s, qCreatedBy(id, EntityType.BODY)]))]; }) };`,
]);

batch.write('tools/fixtures/gen-batch-037.mjs');

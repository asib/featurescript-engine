// Generates fixture batch 050: body drafts beyond batch 007's one side: whole parts and chosen faces drafted on both
// sides of a plane through their middle (matching at the parting plane, and not), and corners made planes.
//
// Usage: node tools/fixtures/gen-batch-050.mjs
//
// Writes fixtures/batch-050/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, createdBy, deg, edgeOf, faceAt, planeBody, solidsOf } from './lib.mjs';

const batch = new Batch({ number: 50, title: 'body drafts: both sides, matching at the parting plane, plane corners' });

const base = solidsOf('base');
const setup = [
  box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1]), planeBody('mid', [20, 15, 6], [0, 0, 1], [1, 0, 0]),
];
const parts = (fields) =>
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()', draftOnSelf: 'false',
    pullDirection: edgeOf('pull'), flipPullDirection: 'false', partingObject: createdBy('mid', 'FACE'), angle: deg(3),
    cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false', ...fields,
  });

batch.add('body-draft', 'a whole part drafted 3 degrees on both sides of a plane through its middle', [
  setup, parts({ bothSides: 'true', matchFacesAtParting: 'true', matchFaceType: 'BodyDraftMatchFaceType.TANGENT_TO_FACE', concaveRepair: 'BodyDraftConcaveRepairType.NONE' }),
]);
batch.add('body-draft', 'the same, not matching faces at the parting plane', [setup, parts({ bothSides: 'true', matchFacesAtParting: 'false' })]);
batch.add('body-draft', 'two faces drafted 3 degrees on both sides of the plane', [
  setup,
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.FACES', faces: `qUnion([${faceAt(base, [20, 0, 6])}, ${faceAt(base, [40, 15, 6])}])`, draftOnSelf: 'false',
    pullDirection: edgeOf('pull'), flipPullDirection: 'false', partingObject: createdBy('mid', 'FACE'), angle: deg(3), bothSides: 'true',
    matchFacesAtParting: 'true', matchFaceType: 'BodyDraftMatchFaceType.TANGENT_TO_FACE', concaveRepair: 'BodyDraftConcaveRepairType.NONE',
    cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false',
  }),
]);
batch.add('body-draft', 'a whole part drafted 3 degrees from a plane through its middle, its corners made planes', [
  setup, parts({ bothSides: 'false', cornerType: 'BodyDraftCornerType.PLANE' }),
]);

batch.write('tools/fixtures/gen-batch-050.mjs');

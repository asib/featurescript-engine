// Generates fixture batch 051: more body draft options: by edges on both sides, a whole part drafted on itself (no
// parting object, refused), and an L-shaped part on both sides with its concave edge repaired.
//
// Usage: node tools/fixtures/gen-batch-051.mjs
//
// Writes fixtures/batch-051/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, axisLine, box, call, createdBy, deg, edgeAt, edgeOf, mm, planeBody, solidsOf } from './lib.mjs';

const batch = new Batch({ number: 51, title: 'body drafts: by edges on both sides, on itself, concave repair' });

const base = solidsOf('base');
const union = (...q) => `qUnion([${q.join(', ')}])`;
const block = [box('base', [0, 0, 0], [40, 30, 12]), axisLine('pull', [0, 0, 0], [0, 0, 1])];
const mid = planeBody('mid', [20, 15, 6], [0, 0, 1], [1, 0, 0]);
const common = { pullDirection: edgeOf('pull'), flipPullDirection: 'false', angle: deg(3), cornerType: 'BodyDraftCornerType.EXTEND', keepMaterial: 'false' };

batch.add('body-draft', 'the front and right faces by their top and bottom edges, both sides of a plane through the middle', [
  block, mid,
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.EDGES', topEdges: union(edgeAt(base, [20, 0, 12]), edgeAt(base, [40, 15, 12])),
    bottomEdges: union(edgeAt(base, [20, 0, 0]), edgeAt(base, [40, 15, 0])), draftOnSelf: 'false', partingObject: createdBy('mid', 'FACE'),
    bothSides: 'true', matchFacesAtParting: 'true', matchFaceType: 'BodyDraftMatchFaceType.TANGENT_TO_FACE',
    concaveRepair: 'BodyDraftConcaveRepairType.NONE', ...common,
  }),
]);
batch.add('body-draft', 'a whole part drafted on itself', [
  block,
  call('bodyDraft', 'draft1', { selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()', draftOnSelf: 'true', bothSides: 'false', ...common }),
]);
// (A cylindrical sheet as the parting object fails in Onshape with "Execution error", so it's left out.)
// An L: a 40 × 30 × 12 block with a 10 × 10 × 12 block beside it at x 40..50, y 0..10 — its inside upright edge at
// (40, 10) is concave.
batch.add('body-draft', 'an L-shaped part drafted on both sides of a plane through its middle, its concave edge repaired with a 1 mm radius', [
  block, box('wing', [40, 0, 0], [50, 10, 12]),
  call('booleanBodies', 'join', { tools: union(base, solidsOf('wing')), operationType: 'BooleanOperationType.UNION' }),
  mid,
  call('bodyDraft', 'draft1', {
    selectionType: 'BodyDraftSelectionType.PARTS', bodies: base, excludeFaces: 'qNothing()', draftOnSelf: 'false',
    partingObject: createdBy('mid', 'FACE'), bothSides: 'true', matchFacesAtParting: 'true',
    matchFaceType: 'BodyDraftMatchFaceType.TANGENT_TO_FACE', concaveRepair: 'BodyDraftConcaveRepairType.RADIUS', concaveRepairRadius: mm(1),
    ...common,
  }),
]);

batch.write('tools/fixtures/gen-batch-051.mjs');

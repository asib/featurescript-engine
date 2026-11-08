// Generates fixture batch 049: parting line drafts. A 40 × 30 × 20 block whose four sides are split at z = 10 (the
// parting line), drafted one sided (the faces above, and those below), symmetric, and two sided.
//
// Usage: node tools/fixtures/gen-batch-049.mjs
//
// Writes fixtures/batch-049/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, createdBy, deg, edgeOf, planeBody, solidsOf, vec3, axisLine } from './lib.mjs';

const batch = new Batch({ number: 49, title: 'parting line drafts' });

const base = solidsOf('base');
const at = (type, p) => `qContainsPoint(qOwnedByBody(${base}, EntityType.${type}), ${vec3(...p)})`;
// The block, its sides split by the plane z = 10, and the pull direction up z.
const block = [
  box('base', [0, 0, 0], [40, 30, 20]), planeBody('cut', [20, 15, 10], [0, 0, 1], [1, 0, 0]),
  call('splitPart', 'split1', {
    splitType: 'SplitType.FACE', faceTargets: `qUnion([${[[20, 0, 5], [40, 15, 5], [20, 30, 5], [0, 15, 5]].map((p) => at('FACE', p)).join(', ')}])`,
    faceTools: createdBy('cut', 'FACE'),
  }),
  axisLine('up', [0, 0, 0], [0, 0, 1]),
];
const parting = `qUnion([${[[20, 0, 10], [40, 15, 10], [20, 30, 10], [0, 15, 10]].map((p) => at('EDGE', p)).join(', ')}])`;
const draft = (fields) =>
  call('draft', 'draft1', {
    draftFeatureType: 'DraftFeatureType.PARTING_LINE', pullDirectionEntity: edgeOf('up'), partingEdges: parting, hintFaces: 'qNothing()',
    angle: deg(5), pullDirection: 'false', tangentPropagation: 'false', ...fields,
  });

batch.add('draft', 'a block\'s sides drafted 5 degrees one sided from a parting line at mid height, the faces along the pull', [
  block, draft({ partingLineSides: 'PartingLineSides.ONE_SIDED', alongPull: 'true' }),
]);
batch.add('draft', 'the same, the faces away from the pull', [block, draft({ partingLineSides: 'PartingLineSides.ONE_SIDED', alongPull: 'false' })]);
batch.add('draft', 'the same, symmetric', [block, draft({ partingLineSides: 'PartingLineSides.SYMMETRIC' })]);
batch.add('draft', 'the same, two sided, 5 and 10 degrees', [
  block, draft({ partingLineSides: 'PartingLineSides.TWO_SIDED', secondAngle: deg(10), secondPullDirection: 'true' }),
]);

batch.write('tools/fixtures/gen-batch-049.mjs');

// Generates fixture batch 092: whether a construction plane stays construction when an operation cuts it (batch
// 090 c010's rotated copies of default planes stayed construction in Onshape; batch 037 c004's piece didn't): a
// plane cut by an extrude, a pattern copy of one cut so, and the default planes after a crop view (batch 037 c002's
// section), which comes last as it deletes every body it isn't given.
//
// Usage: node tools/fixtures/gen-batch-092.mjs
//
// Writes fixtures/batch-092/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, cylinder, mm, rect, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
// The bodies a query finds: how many, how many construction, and their areas.
function constructionProbe(context is Context, bodies is Query) returns map
{
    return { "all" : evaluateQueryCount(context, bodies),
             "construction" : evaluateQueryCount(context, qConstructionFilter(bodies, ConstructionObject.YES)),
             "areas" : mapArray(evaluateQuery(context, bodies), function(b) { return roundToPrecision(evArea(context, { "entities" : b }) / millimeter ^ 2, 3); }) };
}
`;

const batch = new Batch({ number: 92, title: 'construction planes cut by operations', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const plane = (name) => `opPlane(context, id + "${name}", { "plane" : plane(${vec3(0, 0, 0)}, vector(0, 0, 1), vector(1, 0, 0)), "width" : ${mm(40)}, "height" : ${mm(40)} });`;
const cutter = (scope) => [
  sketch('c1', PLANES.xy), rect('c1', 'r1', [0, 0], [30, 30]), solve('c1'),
  call('extrude', 'cut', { entities: regions('c1'), endBound: 'BoundingType.BLIND', depth: mm(5), symmetric: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: scope }),
];
const created = (name) => `qCreatedBy(id + "${name}", EntityType.BODY)`;

batch.add('extrude', 'a 40 × 40 construction plane cut by a 30 × 30 extrude through it', [
  plane('p1'), cutter(created('p1')), returns({ plane: `constructionProbe(context, ${created('p1')})` }),
]);
batch.add('extrude', 'a pattern copy of a 40 × 40 construction plane, cut by a 30 × 30 extrude through it', [
  plane('p1'),
  `opPattern(context, id + "copy", { "entities" : ${created('p1')}, "transforms" : [identityTransform()], "instanceNames" : ["1"] });`,
  cutter(created('copy')),
  returns({ copy: `constructionProbe(context, ${created('copy')})`, original: `constructionProbe(context, ${created('p1')})` }),
]);
batch.add('section', 'a block and pin cropped to a closed spline (batch 037 c002): the default planes after', [
  box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [8, 22], 4, 20, 30),
  call('jogSectionPart', 'section1', {
    target: 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)', sketchPlane: `plane(${vec3(0, 0, 0)}, vector(0, 0, 1), vector(1, 0, 0))`, isCropView: 'true',
    brokenOutPointNumbers: '[4]', jogPoints: `[${[[25, 8, 20], [35, 14, 20], [31, 25, 20], [20, 18, 20], [30, 15, 10]].map((p) => vec3(...p)).join(', ')}]`,
  }),
  returns({
    top: `constructionProbe(context, qCreatedBy(makeId("Top"), EntityType.BODY))`,
    front: `constructionProbe(context, qCreatedBy(makeId("Front"), EntityType.BODY))`,
    right: `constructionProbe(context, qCreatedBy(makeId("Right"), EntityType.BODY))`,
  }),
]);

batch.write('tools/fixtures/gen-batch-092.mjs');

// Generates fixture batch 012: sheet metal basics. Sheet metal models started by thickening sketch regions,
// extruding sketch lines and converting a part, and flanges on them.
//
// Usage: node tools/fixtures/gen-batch-012.mjs
//
// Writes fixtures/batch-012/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, deg, faceAt, line, mm, planeAt, rect, sketch, solidsOf, solve, vec3 } from './lib.mjs';

const helpers = `
// An edge of a part through the point at fractions (fx, fy, fz) of its bounding box: where sheet metal puts its
// material depends on the model, so edges are found from the part itself.
function edgeAtFraction(context is Context, part is Query, fx is number, fy is number, fz is number) returns Query
{
    const b = evBox3d(context, { "topology" : part, "tight" : true });
    const p = vector(b.minCorner[0] + fx * (b.maxCorner[0] - b.minCorner[0]),
                     b.minCorner[1] + fy * (b.maxCorner[1] - b.minCorner[1]),
                     b.minCorner[2] + fz * (b.maxCorner[2] - b.minCorner[2]));
    return qContainsPoint(qOwnedByBody(part, EntityType.EDGE), p);
}

// How many of each kind of body there are, all told: sheet metal keeps its definition bodies out of sight.
function bodyCounts(context is Context) returns map
{
    const all = qEverything(EntityType.BODY);
    return { "solids" : evaluateQueryCount(context, qBodyType(all, BodyType.SOLID)),
             "sheets" : evaluateQueryCount(context, qBodyType(all, BodyType.SHEET)),
             "flat" : evaluateQueryCount(context, qSheetMetalFlatFilter(all, SMFlatType.YES)) };
}
`;

const batch = new Batch({ number: 12, title: 'sheet metal: starting by thickening, extruding and converting; flanges', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const wireEdges = (s) => `qOwnedByBody(qBodyType(qCreatedBy(id + "${s}", EntityType.BODY), BodyType.WIRE), EntityType.EDGE)`;
const xy = planeAt([0, 0, 0], [0, 0, 1], [1, 0, 0]);
const smParts = (name) => `qBodyType(qCreatedBy(id + "${name}", EntityType.BODY), BodyType.SOLID)`;
const counts = returns({ counts: 'bodyCounts(context)' });

const start = (name, fields) =>
  call('sheetMetalStart', name, {
    thickness: mm(2), oppositeDirection: 'false', radius: mm(1), flipDirectionUp: 'false', ...fields,
  });
const plate = (name = 'sm1', fields = {}) => [
  sketch('s1', xy), rect('s1', 'rect1', [0, 0], [40, 30]), solve('s1'),
  start(name, { process: 'SMProcessType.THICKEN', regions: `qSketchRegion(id + "s1")`, tangentPropagation: 'false', bends: 'qNothing()', ...fields }),
];
const extruded = (name, lines, fields = {}) => [
  sketch('s1', xy), lines.map(([a, b], i) => line('s1', `l${i + 1}`, a, b)), solve('s1'),
  start(name, {
    process: 'SMProcessType.EXTRUDE', sketchCurves: wireEdges('s1'), bendArcs: 'qNothing()', endBound: 'SMExtrudeBoundingType.BLIND',
    oppositeExtrudeDirection: 'false', depth: mm(25), symmetric: 'false', hasSecondDirection: 'false', ...fields,
  }),
];
const flange = (name, edges, fields = {}) =>
  call('sheetMetalFlange', name, {
    edges, flangeAlignment: 'SMFlangeAlignment.INNER', limitType: 'SMFlangeBoundingType.BLIND', distance: mm(15),
    angleControlType: 'SMFlangeAngleControlType.BEND_ANGLE', oppositeDirection: 'false', bendAngle: deg(90), autoMiter: 'true',
    useDefaultRadius: 'true', ...fields,
  });

// ----- starting a model -----

batch.add('sheet-metal-start', 'a 40 × 30 mm plate, 2 mm thick, from a sketch rectangle', [plate(), counts]);
batch.add('sheet-metal-start', 'the same plate, thickened the other way', [plate('sm1', { oppositeDirection: 'true' }), counts]);
batch.add('sheet-metal-start', 'an L section extruded 25 mm, 1.5 mm thick with 2 mm bends', [
  extruded('sm1', [[[0, 0], [30, 0]], [[30, 0], [30, 20]]], { thickness: mm(1.5), radius: mm(2) }), counts,
]);
batch.add('sheet-metal-start', 'a U section extruded 20 mm both ways', [
  extruded('sm1', [[[0, 20], [0, 0]], [[0, 0], [30, 0]], [[30, 0], [30, 20]]], { depth: mm(10), symmetric: 'true' }), counts,
]);
batch.add('sheet-metal-start', 'an open box converted from a block, bent along the bottom', [
  box('base', [0, 0, 0], [40, 30, 20]),
  start('sm1', {
    process: 'SMProcessType.CONVERT', partToConvert: solidsOf('base'), facesToExclude: faceAt(solidsOf('base'), [20, 15, 20]),
    bends: `qUnion([${[[20, 0, 0], [40, 15, 0], [20, 30, 0], [0, 15, 0]].map(p => `qContainsPoint(qOwnedByBody(${solidsOf('base')}, EntityType.EDGE), ${vec3(...p)})`).join(', ')}])`,
    clearance: mm(0), keepInputParts: 'false', thickness: mm(1),
  }),
  counts,
]);

// ----- flanges -----

batch.add('sheet-metal-flange', 'a 15 mm flange at 90 degrees on one edge of the plate', [
  plate(), flange('flange1', `edgeAtFraction(context, ${smParts('sm1')}, 1, 0.5, 1)`), counts,
]);
batch.add('sheet-metal-flange', 'flanges on two edges of the plate that meet at a corner', [
  plate(),
  flange('flange1', `qUnion([edgeAtFraction(context, ${smParts('sm1')}, 1, 0.5, 1), edgeAtFraction(context, ${smParts('sm1')}, 0.5, 1, 1)])`),
  counts,
]);
batch.add('sheet-metal-flange', 'a 45 degree flange the other way, with a 3 mm bend', [
  plate(),
  flange('flange1', `edgeAtFraction(context, ${smParts('sm1')}, 1, 0.5, 0)`, {
    bendAngle: deg(45), oppositeDirection: 'true', useDefaultRadius: 'false', bendRadius: mm(3),
  }),
  counts,
]);

batch.write('tools/fixtures/gen-batch-012.mjs');

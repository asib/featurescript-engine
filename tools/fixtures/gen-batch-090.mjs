// Generates fixture batch 090: mate connectors inferred the ways no batch has tried (the top, middle and bottom of a
// cylinder's and a cone's axis, the centre of an L-shaped face's loop and of a hole's, the virtual sharp of a rounded
// corner, a cone's tip, a sphere's centre); an aligned section of a closed composite part (std adds its rotated
// copies to the composite, `opModifyCompositePart`).
//
// Usage: node tools/fixtures/gen-batch-090.mjs
//
// Writes fixtures/batch-090/{cases.fs,generator.fs,cases.json}. The cases are written out by hand.

import { Batch, box, call, circle, cylinder, edgeAt, faceAt, line, mm, planeAt, regions, sketch, solidsOf, solve, vec3, PLANES } from './lib.mjs';

const helpers = `
function roundVector(v is Vector) returns Vector
{
    return vector(round(v[0] * 1e6) / 1e6, round(v[1] * 1e6) / 1e6, round(v[2] * 1e6) / 1e6);
}

// A mate connector's coordinate system, rounded so it compares as text.
function connectorFrame(context is Context, connector is Query) returns map
{
    const cs = evMateConnector(context, { "mateConnector" : connector });
    return { "origin" : roundVector(cs.origin / millimeter), "x" : roundVector(cs.xAxis), "z" : roundVector(cs.zAxis) };
}
`;

const batch = new Batch({ number: 90, title: 'mate connectors by axis points, loop centres, a virtual sharp, a cone tip and a sphere centre; an aligned section of a composite', helpers });

const returns = (entries) => `return { ${Object.entries(entries).map(([k, v]) => `"${k}" : ${v}`).join(', ')} };`;
const base = solidsOf('base');
const probe = returns({ frame: `connectorFrame(context, qCreatedBy(id + "mc1", EntityType.BODY))` });
const mc = (description, setup, query, inference) =>
  batch.add('mate-connector', description, [setup, call('mateConnector', 'mc1', { originQuery: query, entityInferenceType: `EntityInferenceType.${inference}`, ownerPart: base }), probe]);
const can = cylinder('base', [0, 0], 10, 0, 30);
const frustum = [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [6, 20]), line('s1', 'l3', [6, 20], [0, 20]), line('s1', 'l4', [0, 20], [0, 0]), solve('s1'),
  `opPoint(context, id + "a1", { "point" : ${vec3(0, 0, -50)} }); opPoint(context, id + "a2", { "point" : ${vec3(0, 0, 50)} });`,
  call('revolve', 'base', { entities: regions('s1'), axis: `sketchEntityQuery(id + "s1", EntityType.EDGE, "l4")`, fullRevolve: 'true' }),
];
const cone = `qGeometry(qOwnedByBody(${base}, EntityType.FACE), GeometryType.CONE)`;

mc('the top of a cylinder\'s axis (radius 10, z 0 to 30)', can, faceAt(base, [10, 0, 15]), 'TOP_AXIS_POINT');
mc('the middle of the same cylinder\'s axis', can, faceAt(base, [10, 0, 15]), 'MID_AXIS_POINT');
mc('the bottom of a cone frustum\'s axis (radius 10 to 6, 20 tall)', frustum, cone, 'BOTTOM_AXIS_POINT');
mc('the top of the same cone\'s axis', frustum, cone, 'TOP_AXIS_POINT');
// An L-shaped top: a 40 × 30 block with a 20 × 15 notch cut from its corner at (40, 30).
const notched = [
  box('base', [0, 0, 0], [40, 30, 10]),
  sketch('n1', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), line('n1', 'l1', [20, 15], [40, 15]), line('n1', 'l2', [40, 15], [40, 30]), line('n1', 'l3', [40, 30], [20, 30]), line('n1', 'l4', [20, 30], [20, 15]), solve('n1'),
  call('extrude', 'notch', { entities: regions('n1'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
];
mc('the centre of an L-shaped top face\'s loop (a 40 × 30 block notched 20 × 15 at a corner), from its front edge', notched, edgeAt(base, [20, 0, 10]), 'LOOP_CENTER');
const holed = [
  box('base', [0, 0, 0], [40, 30, 10]),
  sketch('h1', planeAt([0, 0, 10], [0, 0, 1], [1, 0, 0])), circle('h1', 'c1', [25, 12], 4), solve('h1'),
  call('extrude', 'hole', { entities: regions('h1'), endBound: 'BoundingType.THROUGH_ALL', oppositeDirection: 'true', operationType: 'NewBodyOperationType.REMOVE', defaultScope: 'false', booleanScope: base }),
];
mc('the centre of a hole\'s loop (radius 4 at (25, 12)) on a block\'s top', holed, edgeAt(base, [29, 12, 10]), 'LOOP_CENTER');
const rounded = [box('base', [0, 0, 0], [40, 30, 10]), call('fillet', 'fillet1', { entities: edgeAt(base, [40, 30, 5]), radius: mm(6) })];
mc('the virtual sharp of a block\'s upright corner rounded 6 mm, from the arc on its top', rounded, edgeAt(base, [40 - 6 + 6 * Math.SQRT1_2, 30 - 6 + 6 * Math.SQRT1_2, 10]), 'VIRTUAL_SHARP');
// A cone with its tip: a right triangle (radius 10 at z = 0 to the tip at z = 20) revolved.
const pointed = [
  sketch('s1', PLANES.xz), line('s1', 'l1', [0, 0], [10, 0]), line('s1', 'l2', [10, 0], [0, 20]), line('s1', 'l3', [0, 20], [0, 0]), solve('s1'),
  call('revolve', 'base', { entities: regions('s1'), axis: `sketchEntityQuery(id + "s1", EntityType.EDGE, "l3")`, fullRevolve: 'true' }),
];
mc('the tip of a cone (radius 10, 20 tall), from its face', pointed, cone, 'POINT');
mc('the centre of a sphere of radius 10 at the origin, from its face', call('fSphere', 'base', { radius: mm(10) }), `qOwnedByBody(${base}, EntityType.FACE)`, 'CENTER');

// ----- an aligned section of a composite part -----

// As in batch 037, a section part deletes every body it isn't given and the cases share one Part Studio: this case
// comes last and sections every solid there is, cutting away the -x side behind which the others lie.
batch.add('section', 'a block and pin made one closed composite part, cut along a line bent at (20, 15) as an aligned section', [
  box('base', [0, 0, 0], [40, 30, 20]), cylinder('pin', [8, 22], 4, 20, 30),
  call('compositePart', 'composite1', { bodies: `qUnion([${base}, ${solidsOf('pin')}])`, closed: 'true' }),
  call('jogSectionPart', 'section1', {
    target: 'qBodyType(qEverything(EntityType.BODY), BodyType.SOLID)', sketchPlane: `plane(${vec3(0, 0, 0)}, vector(0, 0, 1), vector(1, 0, 0))`,
    jogPoints: `[${[[20, 35, 10], [20, 15, 10], [35, -5, 10]].map((p) => vec3(...p)).join(', ')}]`, isAlignedSection: 'true',
  }),
  `return { "composites" : mapArray(evaluateQuery(context, qBodyType(qEverything(EntityType.BODY), BodyType.COMPOSITE)), function(c) { return evaluateQueryCount(context, qContainedInCompositeParts(c)); }) };`,
]);

batch.write('tools/fixtures/gen-batch-090.mjs');
